# Library replacement review

Date: **2026-09-26**. Base HEAD: [`adb6f8e211e3a331e56495b393811a3a1c95e372`](https://github.com/sagikazarmark/excalidraw-render.orig/commit/adb6f8e211e3a331e56495b393811a3a1c95e372).
Scope: the **actual dirty worktree**, including the user's font-packaging migration
to `excalidraw-fonts` and optional CJK shards. Four parallel research agents reviewed
history, raster/geometry, font/PDF/JSON, and CLI/tooling; this report consolidates
their findings and follow-up first-party API/source checks.
No candidate prototypes or tests were executed in this review. Historical experiments
below are prior evidence, not new measurements. No issues were created.

LOC estimates are **net physical production lines, including comments**, after
replacement adapters; tests and generated data are excluded unless explicitly stated.
They are estimates, not measured patches. Paths/ranges refer to the inspected worktree.
Abbreviated renderer paths (`render/…`, `scene/…`) are relative to
`crates/excalidraw-render/src/`; other abbreviated paths inherit the explicitly named directory.

## Executive decision

**Yes: meaningful maintenance gains exist in CLI serialization/staging and in a
strategic refresh onto maintained shaping, SVG, and PDF libraries.** There is no
established broad drop-in renderer replacement. Prioritize actual ownership transfer,
not adding libraries around compatibility code that must remain.

| Priority | Candidate | Plausible net production removal | Decision |
| --- | --- | ---: | --- |
| Immediate | Serde for CLI JSON | 240–270 lines | Strongest straightforward removal |
| Immediate | tempfile for staging/lifetimes | 45–65 lines | Worth adopting with delivery semantics retained |
| Next | clap derive for xtask | Near neutral | Centralize grammar/help; maintenance benefit, not LOC savings |
| Strategic | HarfRust + refreshed usvg/resvg + Krilla/krilla-svg | Hundreds of specialized PDF lines potentially replaced | Highest-value research investment; bounded experiment first |
| Optional batch | Fontations checksums; base64 algorithms | 35–50; 40–55 lines | Small, credible existing-dependency reuse |
| Conditional | weezl LZW; moxcms ICC execution | 35–70; 350–500 lines | Allocation/numerical contracts unresolved |

[ADR-0016](../../adr/0016-release-0.1.0-with-a-version-scoped-contract.md) permits reviewed
output/diagnostic changes between 0.x releases. [ADR-0019](../../adr/0019-review-expectations-in-git-without-approval-hashes.md)
retires executable approval hashes in favor of Git review and reproducible tests.
Old byte identity is a differential control, **not an immutable adoption gate**.
Visual/textual correctness, bounded rendering, diagnostics, metadata recovery, and
repeatability for a pinned release remain requirements; changes need characterization.

## 1. Existing improvements and historical decisions

Do not count these completed changes as new replacement opportunities:

| Already adopted | Issue/upstream record | Local implementation history |
| --- | --- | --- |
| crc32fast | [#179](https://github.com/sagikazarmark/excalidraw-render.orig/issues/179) | [768fb90](https://github.com/sagikazarmark/excalidraw-render.orig/commit/768fb90) |
| ryu-js ECMAScript formatting | [#180](https://github.com/sagikazarmark/excalidraw-render.orig/issues/180) | [e08fc52](https://github.com/sagikazarmark/excalidraw-render.orig/commit/e08fc52) |
| cssparser tokenization, retaining compatibility scans | [#181](https://github.com/sagikazarmark/excalidraw-render.orig/issues/181) | [43c41b6](https://github.com/sagikazarmark/excalidraw-render.orig/commit/43c41b6) |
| Fontations name readers | [#182](https://github.com/sagikazarmark/excalidraw-render.orig/issues/182) | [777c33a](https://github.com/sagikazarmark/excalidraw-render.orig/commit/777c33a), [e8a716d](https://github.com/sagikazarmark/excalidraw-render.orig/commit/e8a716d) |
| miniz PNG/PDF streams | [#113](https://github.com/sagikazarmark/excalidraw-render.orig/issues/113), [#133](https://github.com/sagikazarmark/excalidraw-render.orig/issues/133) | [6137b6b](https://github.com/sagikazarmark/excalidraw-render.orig/commit/6137b6b), [6220458](https://github.com/sagikazarmark/excalidraw-render.orig/commit/6220458) |
| libm portable math | [#148](https://github.com/sagikazarmark/excalidraw-render.orig/issues/148) | [6f1af6b](https://github.com/sagikazarmark/excalidraw-render.orig/commit/6f1af6b) |
| Upstream brotli 9 / ttf2woff2 0.13.3 / skera 0.7 | [brotli #259](https://github.com/dropbox/rust-brotli/pull/259), [ttf2woff2 #28](https://github.com/0x6b/ttf2woff2/pull/28), [Fontations #1996](https://github.com/googlefonts/fontations/pull/1996) | [39ed6ea](https://github.com/sagikazarmark/excalidraw-render.orig/commit/39ed6ea), [ee95007](https://github.com/sagikazarmark/excalidraw-render.orig/commit/ee95007), [1a7aaa7](https://github.com/sagikazarmark/excalidraw-render.orig/commit/1a7aaa7) |

The coordinated identity refresh completed in [e6f1344](https://github.com/sagikazarmark/excalidraw-render.orig/commit/e6f1344).
`crates/excalidraw-fonts/src/lib.rs:47–48` already names the upstream versions;
stale Cargo comments are not the current profile. `pdfium-render 0.9.4` is already
used by `tools/pdf-harness/pdfium-rust`; CLI SHA-256 already uses sha2.

Closed experiments remain useful evidence, with sharply bounded conclusions:

- [#183 BMP](https://github.com/sagikazarmark/excalidraw-render.orig/issues/183), [6a42d61](https://github.com/sagikazarmark/excalidraw-render.orig/commit/6a42d61): the old 487-line baseline predates [b858d30](https://github.com/sagikazarmark/excalidraw-render.orig/commit/b858d30) and today's 903-line BMP implementation. Old mismatch counts do not characterize current code.
- [#184 kurbo](https://github.com/sagikazarmark/excalidraw-render.orig/issues/184), [cbc39ae](https://github.com/sagikazarmark/excalidraw-render.orig/commit/cbc39ae): kurbo 0.11.3 produced 17,147/32,768 bitwise bounds differences and changed 17 sampled PDFs for roughly 89 lines. This is not a verdict on all kurbo versions or reviewed visual changes.
- [#185 color](https://github.com/sagikazarmark/excalidraw-render.orig/issues/185), [931ee81](https://github.com/sagikazarmark/excalidraw-render.orig/commit/931ee81): Palette 0.7.7/adaptation and matrix choices deserve narrow reassessment if a concrete gain appears; no established remedy follows.
- [#186 pdf-writer](https://github.com/sagikazarmark/excalidraw-render.orig/issues/186), [7a1b4a1](https://github.com/sagikazarmark/excalidraw-render.orig/commit/7a1b4a1): the 0.12 exact-f64 probe is not applicable unchanged to 0.15, which seals `Primitive`.
- [#198 resampling](https://github.com/sagikazarmark/excalidraw-render.orig/issues/198), [bee5165](https://github.com/sagikazarmark/excalidraw-render.orig/commit/bee5165)/[c5fec02](https://github.com/sagikazarmark/excalidraw-render.orig/commit/c5fec02): the 73-row oracle is reference-specific evidence, not a generic resizing contract.

See [prior rendering review](rendering-library-review.md), [Fontations name evaluation](../fontations-name-tables.md),
[PDF-writer study](pdf-writer-integration.md), and [CSS compatibility](../../css-tokenization.md).

## 2. Immediate: replace CLI JSON assembly with Serde

**Boundary:** `crates/excalidraw-render-cli/src/main.rs:1218–1237,1615–1842` and
`crates/excalidraw-render-cli/src/frames.rs:268–307,1119–1171`: 341 lines of manual
JSON output. Models already exist at `main.rs:121–194,265–295` and `frames.rs:129–151`.

Use derived structs, `kind`-tagged snake_case enums, an untagged `ReportError`, and
borrowed frame envelopes. Add a direct workspace serde edge; serde_json and serde
are already in the graph. Approximately 70–100 adapter lines leave **240–270 net**.
Keep struct field order, explicit nulls and the final LF. Serialize typed data
directly; a `serde_json::Value` roundtrip adds no value. Failure/delivery coordination stays.

This is output serialization, not preservation-sensitive scene admission. Existing
exact-byte tests at `main.rs:2155+` and CLI `tests/render_cli.rs:454–524,716–1120`
provide useful characterization. Validate every success/error variant, absent values,
Unicode/control escaping, stdout/stderr routing and broken-output behavior.

Pinned serde 1.0.219 works: [derive source](https://github.com/serde-rs/serde/blob/v1.0.219/serde_derive/src/ser.rs)
and [enum representations](https://serde.rs/enum-representations.html) establish the APIs.
Both serde/serde_json are MIT OR Apache-2.0 and actively maintained; releases
[1.0.229](https://crates.io/api/v1/crates/serde/1.0.229) and
[1.0.151](https://crates.io/api/v1/crates/serde_json/1.0.151) were published July 18/20.
Safe public APIs do not mean unsafe-free internals. This CLI-only change adds no browser-WASM dependency.

## 3. Immediate: tempfile for staged files and capture directories

**Boundary:** `crates/excalidraw-render-cli/src/main.rs:1513–1590` (78 lines) and
`xtask/src/representative/capture.rs:9–42` (33 lines). `NamedTempFile`, `TempPath`,
`TempDir`, `persist` and `persist_noclobber` can own exclusive creation and cleanup.
Keep roughly 40–50 staging and 8–15 capture adapter lines: **45–65 net production**.
Another roughly 98 repeated test-helper lines exist across six files; combined
production/test savings might be 110–145, but that is not production removal.

Retain coordinators at `main.rs:1096–1216` and `frames.rs:865–1034`: stage everything,
then commit each path atomically. A commit-phase failure can still partially publish
a batch; tempfile does not provide a multi-file transaction. Preserve flush and
`sync_all`, same-filesystem staging, error stages and hidden `.tmp` naming. Close
descriptors with `into_temp_path` for large batches. Review default 0600 permissions
versus current ordinary umask behavior, and preserve race-free no-clobber semantics.

Validate collisions, interrupted writes, cleanup, existing destinations, permissions,
descriptor pressure and partial commit failures. `atomic-write-file` does not supply
the equivalent create-new behavior. tempfile is host-only; its OS operations and
random temporary names must not enter renderer bytes or semantic accounting.

[tempfile 3.27.0 APIs](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html)
and [Unix implementation](https://github.com/Stebalien/tempfile/blob/v3.27.0/src/file/imp/unix.rs)
were checked: MIT OR Apache-2.0, released March 11, upstream activity September 5.

## 4. Next: clap derive for xtask grammar

`xtask/src/main.rs:33–561,577–590` has 72 `required_arg` and 67 `no_more_args` calls;
related grammar lives in `representative/case.rs:14–68`, `comparison.rs:384–412`,
and `pdf_harness.rs:16–30` under `xtask/src/`. Existing clap 4.5.41 can replace
hand-dispatched grammar with derive subcommands and centralized help/options.

Expect **near-neutral LOC**: the benefit is coherent grammar and fewer manually
synchronized checks. Preserve invocation grammar, retired-command diagnostics, and
xtask's exit 1 using `try_parse` rather than clap's default terminating parser.
Characterize missing/extra arguments and help before migration.
[Pinned tutorial](https://docs.rs/clap/4.5.41/clap/_derive/_tutorial/index.html);
MIT OR Apache-2.0, maintained [4.6.7 release](https://crates.io/api/v1/crates/clap/4.6.7), September 14.

## 5. Strategic: retire archived shaping/PDF dependencies together

[rustybuzz](https://github.com/harfbuzz/rustybuzz/blob/main/README.md) and
[svg2pdf](https://github.com/typst/svg2pdf/blob/main/README.md) explicitly declare
themselves unmaintained/archived, recommending HarfRust and Krilla respectively.
The opportunity is continuing upstream maintenance plus removal of PDF mechanisms,
not claiming that a direct shaper rename deletes the renderer's text policy.

### Minimum credible dependency migration

| Edge | Released coupling and required action |
| --- | --- |
| Renderer direct text | Replace rustybuzz shaping and its `ttf_parser` re-export; direct ttf-parser can be transitional, skrifa/read-fonts consolidation is separate |
| svg2pdf 0.13 | Requires usvg/resvg ^0.45, pdf-writer ^0.12, fontdb ^0.23, tiny-skia ^0.11.4; `to_chunk` cannot consume a usvg 0.48 tree |
| usvg/resvg 0.48.1 | HarfRust ^0.12, skrifa ^0.44, tiny-skia/path 0.12, svgtypes 0.16.1, roxmltree 0.21.1, fontdb 0.24 |
| Krilla 0.8.2 | Default `simple-text` activates rustybuzz; direct positioned-glyph use can disable it |
| krilla-svg 0.8.1 | Enables Krilla defaults and usvg/resvg ^0.47 defaults; both paths reintroduce rustybuzz |

Sources: [svg2pdf manifest](https://github.com/typst/svg2pdf/blob/v0.13.0/Cargo.toml),
[usvg 0.48.1](https://github.com/linebender/resvg/blob/v0.48.1/crates/usvg/Cargo.toml),
[Krilla release workspace](https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/Cargo.toml),
[published krilla-svg edges](https://crates.io/api/v1/crates/krilla-svg/0.8.1/dependencies).
HarfRust ^0.12 does not admit 0.13.3. Taking newer fixes requires an upstream bump
or temporarily two HarfRust minors. Downstream `default-features = false` cannot
undo defaults enabled by krilla-svg. Its dependency/feature update needs an upstream
release or a narrowly scoped, pinned integration patch with an upstream exit.

The endpoint replaces svg2pdf and updates shared tree/pixmap/fontdb types together.
A serialization bridge to an old usvg tree is experimental scaffolding, not retirement.
Inspected Krilla main uses HarfRust but still usvg/resvg 0.47; main alone is no solution.
Do not expand this work merely to unify every Fontations minor: read-fonts 0.42.1
still supplies the bounded layout validator's removed traversal API.

### Essential behavior versus revisitable prescriptions

| Preserve as essential | Prescription that may change explicitly |
| --- | --- |
| Selectable ordinary text, logical extraction, no reflow | Named shaper, CID numbering, complete versus qualified subset programs |
| Vector-first scene geometry; diagnosed, isolated raster closures | PDF writer, object order, classic xref, compression spelling |
| Accurate placement across admitted coordinates/scales | Full-f64 serialization may become a measured precision/error budget, not an unchecked cast |
| Pinned-release repeatability without clock/random input | Through-xref SHA-256/16 uppercase-hex ID formula |
| Notices, recoverable scene/manifest, metadata atomicity | Exact XMP whitespace/carrier spelling, with consumer compatibility reviewed |
| Bounded rendering, no hidden resources, typed failures and occurrence diagnostics | Internal allocation/layout/cache strategy, never uncharged semantic work |

Normative locations: [spec §8.6](../../renderer-spec-v1.md#86-vector-first-pdf), especially
lines 1704–1719, 1721–1805 and 1834–1874; [§12.2–12.3](../../renderer-spec-v1.md#122-format-carriers);
[§13.2](../../renderer-spec-v1.md#132-semantic-accounting) and [§13.3](../../renderer-spec-v1.md#133-safety-and-recovery).
These are proposed revisions, not permission to silently disregard today's contract.
PDF subsetting also needs [ADR-0001](../../adr/0001-treat-font-subsets-as-modified-versions.md)
legal-record, Reserved Font Name and notice treatment; selectability does not require whole programs.

### Focused Krilla experiment: recommended

Use existing resolved scene/text/closure plans with `Surface::draw_path`,
`draw_glyphs`, clips/masks/images, and krilla-svg for already-lowered nested content.
`Glyph::text_range` and `SpanTag::with_actual_text` provide credible extraction seams;
Krilla has content-derived deterministic IDs. [Surface](https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/surface.rs),
[ActualText](https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/interchange/tagging/mod.rs),
[IDs](https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/chunk_container.rs).

Real blockers beyond changing bytes:

1. Internal allocations are not made fallible by `Document::finish() -> Result`.
   Semantic preflight and physical allocation refusal need separate, concrete solutions.
2. krilla-svg's [filter renderer](https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/filter.rs)
   chooses its own grid and approximately 5,000-pixel cap. Pass filter-free trees
   with pre-admitted replacements; no residual filter may silently invoke this path.
3. Arbitrary manifest XMP and existing metadata failure stages lack a demonstrated
   matching public hook. Standard metadata and attachments alone do not finish integration.
4. f32 errors at large coordinates/scales, source-line ActualText versus automatic
   cluster spans, fallback replacements and bidi require focused qualification.
5. Dependency types and feature propagation must actually remove archived crates.

Probe one precision-stressing vector/frame/clip case, multilingual positioned text
with terminators, native nested SVG, one isolated closure, attachment and notices.
Use both PDF viewers, structural/text extraction, fresh-process replay, and selected
limit boundaries. Accept explained output changes; measure size, resource outcomes,
and actual net code removed before committing to a backend rewrite.

Potentially replaced production boundaries: `crates/excalidraw-render/src/render/`
`pdf/writer.rs:13–302`, `pdf.rs:673–960`, `pdf/fonts.rs:82–188,339–389`, and
`pdf/nested_svg.rs:629–884`. These contain hundreds of specialized PDF lines,
but new lowering remains; there is no verified net total. Preserve scene semantics,
closure planning, diagnostics and accounting. The normalized SVG bridge is not
automatically removable; [ADR-0009](../../adr/0009-share-pdf-raster-closure-planning-and-encoding.md)
and implemented [ADR-0013 dark lowering](../../adr/0013-lower-dark-mode-into-nested-svg-content-in-pdf.md) still matter.

Krilla 0.8.2 / krilla-svg 0.8.1 were released June 4, MIT OR Apache-2.0;
upstream was active September 24. Its [released testing description](https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/README.md)
reports 90+ snapshots, 210+ multi-viewer tests and 1,500+ SVG integration cases;
[CI](https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/.github/workflows/ci.yml) exercises visual,
Arlington, veraPDF and strict PDF checks. These are upstream evidence, not reproduced results.
Krilla forbids unsafe; krilla-svg has an unsafe fontdb sharing call related to mmap.
Use memory-only fonts, disabled host-loading features and explicit browser-WASM qualification.

### HarfRust maintenance/testing evidence

HarfRust 0.13.3 (August 25, MIT, Rust 1.85) tracks HarfBuzz 14.3.1. Its
[released changelog](https://github.com/harfbuzz/harfrust/blob/0.13.3/CHANGELOG.md) records recursive-lookup
rejection, bounded ligature caches, long-line/positioning overflow fixes, improved
mark/ligature handling, and Arabic fallback synthesis. [PR #411](https://github.com/harfbuzz/harfrust/pull/411)
includes a red/green public-shaping bounds regression; [#454](https://github.com/harfbuzz/harfrust/pull/454)
enables 13 shared Arabic cases and [#456](https://github.com/harfbuzz/harfrust/pull/456) adds legacy fallback coverage.

[HARFBUZZ.md](https://github.com/harfbuzz/harfrust/blob/0.13.3/HARFBUZZ.md) reports August 24 results
of 800/800, 437/437 and 5,012/5,021 upstream shaping tests. Its nine-failure explanation
predates the next day's Arabic fixes: do not claim a verified current zero-failure total.
[Released CI](https://github.com/harfbuzz/harfrust/blob/0.13.3/.github/workflows/main.yml) runs Linux/macOS,
ICU and no_std builds; a [libFuzzer target](https://github.com/harfbuzz/harfrust/blob/0.13.3/fuzz/fuzz_targets/fuzz_shape.rs)
exercises font/variation inputs. This supports “actively maintained, extensively tested,”
not unqualified equivalence or browser qualification. The library forbids unsafe code.

Direct call sites: `render/text.rs:1740–1761` and buffer consumers at
`450–495,509–589,1635–1737,1788–1802`; subset validation at
`render/font_subset.rs:551–651,857–924` (all under `crates/excalidraw-render/src/`).
Keep source lines, family metrics, CJK/chunk fallback and per-occurrence probe accounting.
The [resvg 0.48 release](https://github.com/linebender/resvg/blob/v0.48.1/CHANGELOG.md) independently brings
glyph-advance, transform, nonfinite-bound and filter-panic fixes; changed rendering is expected.

## 6. Optional existing-dependency cleanups

- **Fontations checksum/directory:** `render/font_subset.rs:1228–1305`, 35–50 net.
  Use typed `TableDirectory`/`TableRecord` and [compute_checksum](https://github.com/googlefonts/fontations/blob/59fcc3b4833c3e5032309bf1b75e50c5cff8fda2/read-fonts/src/tables.rs).
  Check declared count against returned records: truncated arrays default empty.
  Keep record-relative checked slices, whole-file alignment and zeroed head adjustment.
  Latest read-fonts 0.44/write-fonts 0.53: September 8, MIT OR Apache-2.0, active upstream.
- **base64:** `render/svg/write.rs:1344–1368` plus `render/asset/payload.rs:102–143`,
  40–55 net. [Base64Display](https://docs.rs/base64/0.22.1/src/base64/display.rs.html) is allocation-free;
  decode each four-byte admission quantum into `[u8; 3]`, retaining valid-prefix and
  padded-terminal semantics. Whole-input error offsets do not define admission charges.
  Pinned 0.22.1 suffices, forbids unsafe; latest 0.23.1 August 4, MIT OR Apache-2.0.
- Both are integer-only, browser-friendly mechanisms. Differential malformed-input,
  byte/count and resource-boundary checks should establish unchanged behavior.
  JSON string-escaping consolidation offers only 10–30 net after adapters; lower priority.

## 7. Where broad replacement is not established

**JSON admission:** `scene/json.rs:197–771` intertwines grammar with first-crossing
limits and reliable pointers; duplicate decoded keys fail before their values at
`283–309`, strings are metered during decoding at `362–576`, numbers at `578–660`.
`serde_json::Value` overwrites duplicates; preserve-order/arbitrary-precision features
do not solve this (the pinned integer fast path also normalizes `-0`).
[json-syntax](https://github.com/timothee-haudebourg/json-syntax/blob/v0.12.5/src/lib.rs) preserves entries/order/tokens,
and [json-event-parser](https://github.com/oxigraph/json-event-parser/blob/v0.2.3/src/read.rs) exposes ordered raw-number
events, but neither establishes incremental scratch admission and current error precedence.
Retain the parser; do not replace preservation with RFC 8785 canonicalization.

**Font names/XML:** `font_subset.rs:371–501` intentionally preserves malformed legal
bytes and first-match semantics; typed name writers decode/pool strings differently.
`render/asset/svg/entities.rs:106–191,259–362,410–526` and `asset/svg.rs:558–793`
own entity expansion limits and source-attributed removals. xmlparser has DTD grammar
gaps; quick-xml 0.42 has growing open-name buffers even with name checks disabled.
svg-hush changes animation/reference, namespace and entity policies without equivalent
occurrence diagnostics. Generic sanitation would retain most difficult code.

**Raster/geometry:** no substantial immediately compatible replacement was established.
weezl 0.1.12 is already transitive: `render/asset/gif.rs:210–315` could save 35–70
lines using `decode_bytes` and `with_yield_on_full_buffer(true)`. Its internal Box/Vec
allocations and dictionary/EOI semantics need qualification. [Pinned source](https://docs.rs/crate/weezl/0.1.12/source/src/decode.rs);
latest 0.2.1 May 15, MIT OR Apache-2.0. moxcms could replace 350–500 net of
`render/asset/icc_lut.rs:1–471` and `icc_matrix.rs:1–185`, but needs bounded/fallible
parsing/execution and numerical evidence. [0.9.0→0.9.1](https://github.com/awxkee/moxcms/compare/0.9.0...0.9.1)
does not fix the private reader's allocating collection; 0.9.1 September 15, BSD OR
Apache-2.0. This was already explored in [stage-4 review](png-stage4-review.md).

roughr 0.14 is active but uses different RNG/f32/options/hachure and lacks an operation
budget; a theoretical 1,300–1,800-line removal from `render/rough.rs:301–2411` requires
major compatibility work. perfect_freehand 0.1.1 offers a theoretical 200–260 lines
around `render/freedraw.rs:221–465,623+`, but pressure/trig/float-order/dot/allocation
differences and thin maintenance evidence do not establish a battle-tested replacement.
Generic resizers do not own the reference's mip/cache/draw-order behavior. PNG 0.18.1
still needs fdAT-interruption/strict-container qualification. These are not byte-only objections.

Opportunistic follow-up: [zune-image #448](https://github.com/etemesi254/zune-image/issues/448)
closed September 23 with [90b6bbca](https://github.com/etemesi254/zune-image/commit/90b6bbca2d88e840149988cd2ab0b15c0e737766).
Stable zune-jpeg 0.5.15 predates it; qualify a containing release and separately
validate [local #193 strict entropy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/193).
This is not completed deletion. [#178 tiny-skia math](https://github.com/sagikazarmark/excalidraw-render.orig/issues/178) remains open.

Viewer CSS tokenization may save 50–120 lines across `tools/svg-viewer-rust/src/main.rs:691–742,821–918,940–997,1025–1039`
and `tools/reference-oracle/scripts/svg-viewer.mjs:396–450,463–480,536–541`; restricted
grammar/comment joining/wildcard admission need differential checks. Walkdir's 25–65
lines are low priority. Comparator replacements are not established: see current
[comparison-library research](rust-comparison-libraries.md); metric arithmetic is specified.
lopdf's f32 numeric representation is unsuitable for raw-token preservation; qpdf
already supplies structural parsing. pdf-writer 0.15's [sealed Primitive](https://github.com/typst/pdf-writer/releases/tag/v0.15.0)
removes the old exact-number adapter; its limits are not a fallible resource sink.

## Recommended sequence and success criteria

1. Implement CLI Serde, then tempfile, preserving public report/delivery behavior.
2. Migrate xtask grammar to clap for maintainability, without a LOC-savings claim.
3. Separately fund the focused Krilla experiment and coordinated font/SVG qualification;
   explicitly revisit precision, whole-program embedding and ID prescriptions.
4. Resolve upstream dependency/feature, allocation and metadata gaps; optionally batch
   checksum/base64 cleanups. Reassess codecs only against concrete upstream changes.

Success means measurable net implementation removal, essential rendering/text/limits
and diagnostics intact, reproducible pinned-release output, and reviewed/documented
changes allowed. After the full strategic migration, **neither archived crate remains
in the production dependency closure**. A new dependency or a passing sample alone is not success.
