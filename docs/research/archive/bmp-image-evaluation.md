# BMP decoding and EXIF orientation: retain the current readers

**Issue [#183](https://github.com/sagikazarmark/excalidraw-render.orig/issues/183),
2026-09-18.** Evaluated production source at
`e8a716df0873c605be460ad44716d8680e11803c` against **image 0.25.10**.

**Recommendation: retain both current readers.** The experiment confirms useful
common-subset equivalence, but not an exact replacement. Keeping the complete
BMP admission/RLE validator while adding another decoder offers little net code
reduction, leaves multiple pixel incompatibilities, and introduces private
allocations without uniformly fallible failure paths. EXIF has a smaller but
real first-match incompatibility. This documented no-go completes the evaluation;
production migration would be a separate task.

## Reproducible evidence

The probe was retired on 2026-09-30 with no live consumer; it is in Git
history at `20ab732`. Restore it with `git checkout 20ab732 -- tools/bmp-evaluation`.

- Standalone probe and reproduction commands: `tools/bmp-evaluation/README.md`.
- Exact, readable matrix, `tools/bmp-evaluation/matrix.md`: **84 BMP vectors
  and 70 EXIF vectors**, generated from deterministic byte builders.
- Machine-readable evidence, `tools/bmp-evaluation/expected.json`: input
  bytes, exact output bytes or rejection category, the raw library and the
  library behind current validation, mask sweeps, limits probes, and artifact
  hashes. BMP pixels are straight RGBA before `finish_rgba` zeroes transparent
  RGB and derives premultiplied pixels. RGB library output gains alpha 255.
- **131,072 additional source-word comparisons:** all 65,536 possible words for
  each of RGB555 and bitfields565, with zero differing pixels in both sweeps.
- **20 render observations:** four fixtures, each through original BMP SVG/PNG/PDF
  and candidate-derived PNG/PDF. The latter uses a test-only lossless PNG bridge
  into the public renderer. It exercises the candidate's actual pixels without
  introducing a production injection seam or changing semantic asset identity.

The test compiles the actual private production BMP file and extracts the actual
EXIF function at build time. It does not compare against a hand-maintained copy.
The generated matrix is a comparison with current behavior, not a claim that
either decoder is the browser reference authority or that all codec profile
obligations are closed. No conformance coverage or release evidence is relabeled.

### Observed target results

| Target | Execution | Result |
| --- | --- | --- |
| Linux x86-64 | Rust 1.97.1, optimized dev/test profile | Complete native report generated; exact-report test passes. |
| Browser `wasm32-unknown-unknown` | Rust 1.97.1, wasm-bindgen-test-runner 0.2.126, Chrome for Testing and ChromeDriver 149.0.7827.55 | Exact equality with the entire native report, including all 20 artifact hashes. |

The browser archives were SHA-256 checked against
[`tests/package-consumer/tool-manifest.json`](../../tests/package-consumer/tool-manifest.json).
The successful browser run used `CARGO_PROFILE_TEST_DEBUG=0`; the initial
debuginfo-enabled run timed out before browser startup. The environment's default
Rust 1.98.1 lacked the WASM standard library, so both final legs explicitly select
the installed 1.97.1 toolchain. These are local experimental executions, not a
new Dagger release aggregate.

Evidence SHA-256: `18c7011c015a986497e36099a6262c28d0a87a95b87b019f384f5e268db58bad`.
Experiment lock SHA-256: `f5f53dbc2b46e4bfe6d963aacda1ef10262675b9170a0e2b750c3a78cf60268a`.

Final checks: the isolated workspace's complete native test suite (including
doctests), browser exact-report assertion, all-target Clippy with warnings denied,
and formatting pass. The production-lock `--test fast static_bmp_asset_render`
suite passes all **six** tests, covering SVG passthrough/no decoded-pixel charge,
PNG row order, PDF alpha embedding, typed malformed-input recovery and the RLE
validation-work equality/first-crossing boundary. The two-axis internal review
reported **zero Standards findings and zero Spec findings**. This is the full
evaluation suite plus focused production regressions, not a full renderer release
qualification run.

The isolated lock resolves the experiment's entire graph independently from the
production lock. Reproduce with this lock; the results are not a claim of
production-lock release qualification. The direct dependency is
`image = { version = "=0.25.10", default-features = false, features = ["bmp"] }`.
Resolved image features are **bmp, gif, jpeg, png**: svg2pdf already enables the
last three. `cargo tree --locked --offline -i image -e features` in each workspace
confirms the feature delta. BMP itself enables no additional package dependency
in image's manifest; it does compile additional decoder/encoder code. Neither ICO,
Rayon, default-formats nor native AVIF is enabled by this edge. No binary-size or
performance improvement is claimed or measured.

## Behavior that decides the recommendation

| Surface | Measured result | Consequence |
| --- | --- | --- |
| CORE/INFO/V2/V3/V4/V5 RGB24; top-down rows; 1/4/8-bit palettes | Exact agreement for authored samples. | Ordinary row/palette unpacking is delegable. |
| RGB555 / bitfields565 | Exhaustive agreement for all 16-bit source words in each layout. | Do not generalize the wide-bitfield mismatch to these common layouts. |
| RLE4/RLE8 early EOB, short EOL, X/XY deltas and absolute runs leaving holes | Current fills skipped regions with palette zero `(10,20,30,255)`; image writes opaque black after RGB expansion. | Prefilling the output cannot fix it: image explicitly overwrites skips with zero. Repair needs the RLE walk or an upstream policy option. |
| RLE8 encoded overrun | Current rejects; image clamps and succeeds. | Current strict validator must precede decoding. |
| RLE4 encoded and both absolute overruns; out-of-bounds deltas | Both reject, at different stages/categories. | Keep public failure mapping independent of library categories. |
| RLE end-of-image | image accepts completed rows ending in EOL with no EOB; current requires EOB. | Successful library decoding cannot replace current validation. |
| Palette holes | Current rejects indexed and RLE out-of-range indices; image's padded palette yields black. | Retain palette-index validation. |
| BI_RGB32, ordinary header | Both ignore unused alpha and emit opaque RGB. | Common path agrees. |
| BI_RGB32, V3/V4/V5 with explicit alpha masks | Current preserves alpha 128; image emits alpha 255. | Another accepted-input incompatibility, beyond compression code 6. |
| BI_BITFIELDS (3), V3/V4/V5 alpha | Exact agreement for authored RGBA8888 samples, including zero alpha. | Keep final transparent RGB zeroing/premultiplication outside the decoder. |
| BI_ALPHABITFIELDS (6) | Current accepts external and V3/V4/V5 masks; image constructor rejects. | Header rewriting or a fallback would be required. |
| 10-bit channels | Opaque source word `0xc0000003`: current `(0,0,1,255)`, image `(0,0,0,255)`. | Full-range rounded scaling differs from high-bit truncation; lost low bits cannot be recovered from decoded output. |
| Overlapping masks | Current rejects; image accepts the authored overlap. Both reject authored zero/noncontiguous/out-of-word RGB masks. | Keep cross-channel mask validation. |
| Header/encoding policy | image accepts 2-bit palettes, nonzero reserved fields, declared-size mismatch, and the authored pixel offset inside the header; current rejects. | Library parsing is not project admission. |
| Truncation | Both reject the header and final-pixel truncations, but constructor versus decode failure differs. | Exact stages are recorded in the matrix; map errors at the adapter boundary. |
| V5 embedded-profile fields | Both ignore an explicitly marked invalid embedded profile and decode the authored RGB unchanged. | Header support is not ICC interpretation. Current BMP preparation uses no color normalization; adoption must not silently add one. |

There are **20 differing gated BMP rows**: twelve RLE-hole cases, three BI_RGB
alpha cases, four code-6 cases and one wide-bitfield case. Keeping validation
fixes acceptance widening, not these accepted-input differences.

The render smoke confirms the practical impact: RGB24, explicit code-3 alpha,
and complete RLE8 produce equal original/candidate PNG and PDF bytes. The RLE8
XY-delta fixture changes PNG **514 → 528 bytes** and PDF **2163 → 2161 bytes**.
Both versions are individually native/WASM stable. Thus output-byte charges can
change even when the semantic pixel/work formulas remain unchanged. This simple
image-only experiment does not exercise or resolve the separate geometry defect
tracked in #178.

### Codec profile obligation mapping

The BMP obligations in `tests/conformance/coverage/ownership-policy.json` remain
the contract vocabulary:

- **alpha:** explicit/unused/zero alpha samples, raw RGBA comparison and an
  alpha-bearing PDF render; final zeroing remains `color::finish_rgba`.
- **color-profile:** source audit plus the ignored V5 profile vector; this is
  not a complete BMP color-profile qualification.
- **default-frame/orientation:** static BMP, top-down/bottom-up rows; neither
  BMP path discovers EXIF. The separate EXIF experiment below covers the shared
  reader used by other formats.
- **malformed:** palette holes, RLE overrun/delta/EOB, masks, headers and
  truncation matrix, with existing typed BMP recovery tests.
- **limits:** independent constructor/decode audit below, exact `Limits` probes,
  existing admission and work-boundary tests. A production adapter's complete
  cold/warm/batch transcript equivalence has not been implemented or claimed.
- **passthrough:** all four direct SVG renders assert that the original BMP
  payload's entire canonical data URL is preserved, including RLE bytes. No
  candidate decode/re-encode enters the direct SVG path.

## Proposed adapter and physical boundedness

The viable boundary is **only `asset::decode_bmp` pixel derivation**, after the
existing validator and semantic transactions have admitted it:

```text
payload admission + identity + current BMP metadata/profile/RLE validation
  ├─ direct SVG: original validated source bytes, no pixel decode
  └─ PNG/PDF: existing dimensions/pixels/work/memory admission
       → BmpDecoder::new(Cursor<&[u8]>)
       → verify dimensions, RGB8/RGBA8 type and checked exact output length
       → fallibly reserve 4 × pixels once
       → read_image into the 3 × pixels or 4 × pixels prefix
       → expand RGB backwards in that allocation
       → existing finish_rgba (zero transparent RGB, premultiply)
```

The probe executes both a raw-library path on fixed tiny inputs and a separate
current-validation-first path. It caps decoded experiments at 65,536 pixels and
uses the caller-owned buffer approach above. Production must retain its own
limits rather than inherit that experimental cap. ICO entry selection remains
area first, then depth, earliest tie; no ICO facade or DIB replacement is proposed.

### Constructor, output, scratch and work are separate questions

Source audit of image's [BMP decoder][bmp], [decoder trait][decoder] and
[read helper][read-helper], not a claim derived from `Limits`:

| Phase | Bound / failure observation |
| --- | --- |
| Before construction | Current parse validates signed dimensions, row/palette/mask offsets and lengths using checked arithmetic; validation scans indexed/RLE data. For pixel derivation keep renderer dimension/pixel/work checks and semantic workspace acquisition ahead of constructor work. |
| Constructor | Parses bounded header fields and at most 256 palette entries. Temporary palette capacity is at most 1024 bytes (CORE 768); persistent palette is 256 RGB entries (768 bytes). Temporary capacity uses `try_reserve`, but the RGB palette `collect` is infallible. Seeking a `Cursor` does not allocate missing bytes. No full decoded surface is constructed here. |
| Dimensions | image checks signed height against 65,535 before negation. Constructor-only probes reject `1 × +65536` and `65536 × 1`, but accept `1 × -65536`, reporting 196,608 output bytes without reading pixels. Do not rely on that soft cap or introduce it as a new project rejection. |
| Output | Use checked `4P` capacity and a fallible reservation; verify `total_bytes == channels × P` before `read_image`, which asserts exact buffer length. RGB expansion needs no second full surface. Existing premultiplied storage still costs a separate physical `4P`. |
| Indexed decode | Private `vec![0; row_byte_length]`: `4 × ceil(bits × width / 32)` bytes, infallibly allocated. For image's admitted width ≤65,535 and at most 8 bits, this is at most 65,536 bytes. |
| RLE decode | Absolute-run scratch has at most 256 encoded bytes for RLE8, 128 for RLE4 (including padding). `read_exact_vec` uses `try_reserve`; an allocation failure becomes an I/O error. No private full-image buffer. Retained validation makes legal pixel traversals bounded by `P`; commands by input length `B`; fills and pixel writes are O(B + P). |
| Truecolor decode | Fixed small row-padding buffers and direct writes into caller output. Per-pixel integer reads/scaling; no row-sized heap scratch. |
| Limits | `max_alloc=0` is accepted and an indexed decode still succeeds. Strict width limit 1 rejects width 2 with `ImageError::Limits`. BMP inherits the default `set_limits`, which checks dimensions only. It cannot retroactively constrain constructor allocations. |

These are requested buffer-size/source-level bounds, not allocator/RSS
measurements or proof that all allocation failures are recoverable. Infallible
palette/row allocations are concrete remaining blockers to claiming an entirely
fallible adapter. Catching panics would not fix allocation-abort behavior.

### Typed failures and semantic resource accounting

Keep current admission errors and their ordering. Constructor/decode format,
unsupported and in-memory I/O failures must become `ResolveError::BmpDecodeFailed`,
so session recovery still emits `IMAGE_DECODE_FAILED` with sniffed `image/bmp`.
The current BMP decoder also maps its output reservation failure to that variant;
changing it to a memory diagnostic would be a separate behavior change. Failures
in `finish_rgba` and earlier admission retain their existing mappings. The probe's
stage/category strings are internal evidence, not proposed public errors.

The current `asset.rs` ledger, with payload bytes `B` and source pixels `P`, is:

- Source validation work: `B`, plus `4P` for RLE; no decoded-pixel charge.
- Validation persistent logical storage: `512 + aligned(B)`; temporary decoder
  workspace: `1,048,576 + 32 × max(width,height)`.
- Pixel-path local-work precheck: `5B + 4P`.
- Color-preparation event: `B` work with the same temporary workspace.
- Decoded event: `3B + 4P` work, `P` decoded pixels, `256 + 4P` persistent logical
  bytes, with temporary workspace released afterward.

These are canonical semantic amounts, not counters for the library's extra
passes, palette bytes or RGB expansion. Preserve identity, event order, abort and
release handling, per-target replay and deduplication. The proposed decode-only
boundary lets those transactions stay in place, but pixel mismatches would still
change artifacts and exact output-size accounting. A PNG bridge is not evidence
of BMP transcript equivalence: it has a different payload and media identity.

## Owned-code economics

Counts include blank lines and comments in the evaluated source, making the
estimate reproducible rather than an implied implementation benchmark:

| Current BMP component | Lines | Disposition with current validator retained |
| --- | ---: | --- |
| `Info`, validation/dimension/work entry points (1–38) | 38 | Keep admission/semantic API. |
| `decode` (40–98) | 59 | Replace only if every supported encoding's pixels agree. |
| `parse` (100–251) | 152 | Keep format policy, offsets, lengths and mask setup. |
| `validate_pixels`, `walk_rle`, `put_index` (253–382) | 130 | Keep strict palette/RLE rules. Small write-only branches could be removed. |
| packed indices/palette color (384–412) | 29 | Still used by validation/RLE. |
| masked color/scaling (414–435) | 22 | Removable only when wide-channel policy is delegated exactly. |
| mask validation and byte readers (437–487) | 51 | Keep. |

The file is **487 lines**. Simply deleting `decode` and masked-color/scaling
removes **81**, leaving **406**, before adding an adapter. The experiment's
candidate plus admitted wrapper is roughly **70 lines**; production mapping and
compatibility repairs would add more. A straight substitution therefore saves
only about ten lines even in the hypothetical case where those pixel blockers
are solved, before minor validator simplification. This is a scoped estimate,
not a claim that 406 is an irreducible minimum.

With image 0.25.10, exact fallback code for code 6, BI_RGB alpha and wide masks
keeps much of the supposedly deleted pixel loop. RLE repair retains its walk
and writes in addition to the library's walk. **Keeping the full RLE validator
plus another decoder is not worthwhile here.** Delegating only common RGB24/555/
565 paths adds dispatch and a second implementation while leaving the hard
owned mechanisms. ICO contributes **zero** deletions to this estimate.

## Separate EXIF result

The 70-vector matrix compares raw TIFF chunks, not JPEG's `Exif\0\0` prefix.
All orientations 1–8 agree in both endiannesses. Both readers return no result
for the tested invalid values (0, 9, 255, 256, 65535), malformed/truncated headers,
outside-chunk IFD offsets and absent tags. Both require a complete 12-byte entry,
return the first valid duplicate, stop on an invalid value in a well-typed first
tag, and ignore subsequent IFDs. A first successful tag needs no next-IFD pointer.

**Six counterexamples:** wrong type, wrong count or zero count in the first
orientation tag, followed by a valid orientation-8 tag, in each byte order.
Current returns `None`; [image's reader][exif] skips the malformed tag and returns
`Some(8)`. Current ownership is **44 lines** (`color.rs:107–150`). A compatibility
pre-scan would reproduce most of the function just to call the library afterward.
Retain it. Keep format-specific chunk discovery/precedence and pixel transforms
outside this evaluation; no new full EXIF dependency is justified.

## Bounded follow-up scope

1. **No production adoption ticket is justified against 0.25.10.** Retention has
   zero production deletions and zero new production dependency/features.
2. If pursuing upstream reuse, start with these tiny reproductions: configurable
   RLE skipped-color policy and strict overrun/EOB/palette checking; full-range
   rounded bitfield scaling; code-6/explicit BI_RGB-alpha support; consistently
   fallible bounded palette/row allocation. An API that owns strict validation
   would be needed for a material reduction, not just another decoder flag.
3. Separately ask for an EXIF first-orientation-tag policy if reuse remains
   desirable. Re-run the 70 cases against a pinned release before proposing the
   small reader substitution; preserve chunk precedence and existing transforms.
4. Only after such changes, prototype the decode-only adapter against this matrix
   and the production lock, and qualify complete diagnostics, exact limits,
   cold/warm/frame-batch transcripts, direct SVG bytes and PNG/PDF artifacts on
   both targets. Expand vectors for any newly delegated surface. Do not broaden
   ICO selection, other codecs, BMP profile interpretation or compatibility
   revision behavior as incidental effects.

[bmp]: https://github.com/image-rs/image/blob/v0.25.10/src/codecs/bmp/decoder.rs
[decoder]: https://github.com/image-rs/image/blob/v0.25.10/src/io/decoder.rs
[read-helper]: https://github.com/image-rs/image/blob/v0.25.10/src/io.rs
[exif]: https://github.com/image-rs/image/blob/v0.25.10/src/metadata.rs
