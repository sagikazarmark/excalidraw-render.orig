# Fontations name-table handling (#182)

Base: `43c41b6a8b7e7d2bc0c2f0c8c4f0d8e48fd5003d`. Checked 2026-09-18.

## Reader and policy

`render/font_subset.rs` uses the existing `write_fonts::read` re-export
(read-fonts **0.44.0**) for `Name`, `NameRecord`, record layout and field
reading. The separate read-fonts **0.42.1** dependency remains necessary for
the bounded layout-closure validator's traversal API.

The policy remains explicit: select the **first** platform-3, language-0x0409
record for the requested name ID, irrespective of its encoding ID. Rewriting
emits IDs 0–6, 13 and 14 in that order, omitting absent optional records;
IDs 1, 2, 3, 4 and 6 receive the deterministic generated names, while
0, 5, 13 and 14 retain their exact selected source bytes. Output records
remain platform 3 / encoding 1 / language 0x0409. The profile, semantic subset
identity, aliases and ADR-0001/ADR-0003 policy are unchanged.

Two small raw-byte adapters preserve the old malformed-input behavior:

- `Name::name_record()` returns an empty array if the declared record array
  is truncated. Instead, iterate its typed byte range and read each packed
  `NameRecord` through `FontData::read_ref_at`. A complete earlier match still
  wins; encountering a truncated record before a match returns absence.
- `Name::string_data()` substitutes empty data for an invalid storage offset,
  and `NameString` exposes decoded characters rather than the original bytes.
  Slice the original table using checked addition of the typed storage offset,
  record offset and length. An unreadable first match returns absence rather
  than trying a later duplicate. A valid empty record stays distinct from an
  out-of-bounds empty record. Table version and encoding do not add filters.

The u16 count bounds record traversal; packed record size/layout comes from
Fontations. Its parser checks the six-byte header before its infallible
header accessors are used. All raw string slicing remains fallible and bounded.
Read failure still maps to optional-record absence or the caller's existing
`Sfnt`/`Validate` error. Independent checksum, table-policy, layout-closure,
glyph/metric and naming validation remain active.

## Writer evaluation: retain raw serialization

Evaluated the locally pinned **write-fonts 0.53.0**, not a newer release:

| Concern | Upstream behavior and decision |
| --- | --- |
| Raw legal bytes | `NameRecord.string` is a string, not bytes. `FromObjRef<NameString> for String` collects decoded characters. An unpaired UTF-16 surrogate becomes U+FFFD; an odd trailing byte disappears. These are concrete incompatibilities with preservation. The derivation regression retains `d8 00`, `dc 00` and `00 41 ff` through sfnt → subset → WOFF2 → sfnt. |
| Encoding IDs | `Encoding::new` supports Windows IDs 0, 1 and 10, not every Windows encoding. Unknown encodings iterate no characters; the writer rejects unknown platform/encoding pairs. Our reader deliberately selects raw records without that restriction. Output is still encoding 1. |
| Record ordering | `Name` validation requires sorted, unique platform/encoding/language/name-ID tuples. Our nine-ID output order satisfies this, but converting the whole source table would not implement first-match selection. |
| String pooling | `NameStringAndLenWriter` uses `TableWriter::write_offset`; that writer deduplicates equal objects. Our raw writer appends every selected string independently in record order. Even valid strings can therefore change storage offsets and sfnt/WOFF2 bytes under typed serialization. |
| Length bounds | `NameStringWriter::compute_length` sums UTF-16 byte lengths in **u16**; storage-header calculation uses `try_into().unwrap()`. Validation of encoding/sortedness is not a replacement for explicit size checks. |
| Offset bounds | Typed offsets use the generic table writer's offset resolution. Our existing writer checks every record byte length, string start offset and storage-header offset with `u16::try_from`, and uses checked total-size arithmetic plus fallible table reservation. It allows the last string to extend beyond 65535 if both its length and start offset fit, as the format permits. |

Keep the existing raw name-table writer and `FontBuilder::add_raw`,
`copy_missing_tables`, and `build`. No lossy source decoding or conditional
writer/fallback path is introduced. New derivation tests exercise individual
UTF-16 length overflow (BMP and supplementary characters) and cumulative
string-offset overflow, all returning `SvgFontSubsetStage::Sfnt`.

Primary sources (also inspected in the pinned registry source):

- [read-fonts 0.44.0 name table](https://docs.rs/read-fonts/0.44.0/src/read_fonts/tables/name.rs.html)
- [read-fonts 0.44.0 generated records](https://docs.rs/read-fonts/0.44.0/src/read_fonts/generated/generated_name.rs.html)
- [write-fonts 0.53.0 name table](https://docs.rs/write-fonts/0.53.0/src/write_fonts/tables/name.rs.html)
- [write-fonts table writer](https://docs.rs/write-fonts/0.53.0/src/write_fonts/write.rs.html)
- [FontBuilder](https://docs.rs/write-fonts/0.53.0/src/write_fonts/font_builder.rs.html)

## Qualification

Tests use the agreed derivation/validation seam. The truncated-array regression
failed with the naive `Name::name_record()` adapter, then passed with incremental
typed reading. Four further characterization tests cover first-match selection,
platform/language/encoding behavior, malformed and empty records, complete
derivation/legal-byte preservation, and size failure mapping.

Rust **1.97.1**, wasm-bindgen-test-runner **0.2.126**, and checksum-verified
Chrome for Testing / ChromeDriver **149.0.7827.55**:

- Workspace/all-target and browser-WASM typechecks pass.
- Focused `render::font_subset::tests`: **14 passed**.
- Native `--features release-qualification --lib font_subset`: **25 passed**,
  three explicitly ignored materialization/nightly tests.
- Browser-WASM `--lib font_subset`: **9 passed**, including the five new tests,
  full corpus, fuzz regression/smoke, and resource trace matrix.
- `catalog verify` passes.

Qualification used the optimized test profile. On WASM, `CARGO_PROFILE_TEST_DEBUG=0`
kept wasm-bindgen processing bounded; the first debuginfo-enabled attempt timed
out before browser startup. Optimization, debug assertions and overflow checks
were otherwise the test-profile defaults.

Both targets reproduce the committed measurement exactly:

| Measurement | Result |
| --- | --- |
| Successful font-byte cells | 984 |
| Generated bytes SHA-256 | `0a8b686fc3c4a8740b0cd0ce985df9e61f206809da27d4248d44f1192ac35d8d` |
| Semantic resource trace cells | 1211 |
| Resource traces SHA-256 | `cb3e7b25f127a6cb10b09fa5a8efcb3ee281ab662ccc616aefdec4cf87636dde` |

The resource matrix covers cold/warm/frame-batch reuse and failure boundaries.
Regenerating `materialize_complete_font_subset_viewer_inputs` reproduces the
approved input-manifest digest
`f697e227c2002af4bd4caf3f72ef12e114b95bbdfad5b18d5a4f49f78ca27901`.
That binds all 864 executions' source/subset sfnt, WOFF2, SVG and asset bytes.
The canonical viewer reports can therefore be reused: no sfnt serialization,
WOFF2 serialization or dependent viewer artifact bytes changed. The changed
`font_subset.rs` implementation descriptor requires a renewed #99 approval
binding under the existing evidence process.

The maintainer explicitly approved the renewed subject
`sha256:15ef4a33035bd4083d9c18704e39045b2b8bef3662c80f8d8480181eabf41d27`
on 2026-09-18, recorded in
[#99](https://github.com/sagikazarmark/excalidraw-render.orig/issues/99#issuecomment-5732225713).
Only the `font_subset.rs` implementation descriptor changes in the subject.
The release manifest binds the renewed approval. Its old supported-target
aggregate was retired before fresh executions, rather than relabeling a report
produced for the earlier approval.

The parallel `/code-review` axes found no standards violations or implementation
defects; the spec axis identified this evidence renewal as the remaining gate.
Workspace/all-target Clippy with warnings denied, formatting and diff checks pass.

## Committed-revision evidence

Both supported targets executed again on
`777c33a10d14f3a5f9e3b97e7e3cca8b216bee46`, which contains the implementation
and renewed approval. Native renderer library qualification passed **392 tests**
(five intentionally ignored); browser font qualification passed **9 tests**.
The optimized test-profile settings above apply to these runs too.
The existing xtask materializer and aggregator generated and compared their
reports, again agreeing with the unchanged committed 984/1211 measurement.

| Evidence | SHA-256 |
| --- | --- |
| Supported-target aggregate | `cc1bc9dfe1fc01672e4bcf248d05d85ccca4a0ecd418a384b5acf38ebf181d9f` |
| Native report | `ac85fa59bdb55fb2a1ba7f392d7f71c7ed84ca5965aaf6ac28f3148fe5a2b1f9` |
| Browser-WASM report | `5ec9d9e3238639efc0960709313d6bbc74f47a8239279130fdfb1a94013443fe` |

[The #101 attestation](https://github.com/sagikazarmark/excalidraw-render.orig/issues/101#issuecomment-5732400822)
records exact execution scope and tool identities. The aggregate is copied
verbatim into the repository through `record-svg-font-subset-target-aggregate`;
release evidence is **aggregated / pass**. Both `verify-svg-font-subsets` and
`verify-svg-font-subset-release` pass with the renewed binding.

Final full-workspace verification on Rust 1.97.1:

```sh
cargo test --locked --workspace --features excalidraw-render/release-qualification \
  --no-fail-fast -- --nocapture
```

**1119 passed, 0 failed, 16 explicitly ignored**, including the complete
qualification corpus, frame-batch property test, 535 renderer integration tests,
all parity/limit tests, CLI and xtask tests, and doc-test targets. The first
attempt reached the command harness's ten-minute timeout during integration
tests; the completed rerun used background execution and finished successfully.
Both review axes also checked the final approval/aggregate delta and found no
remaining findings. Local execution logs are under `/tmp/opencode/182-*.log`.
