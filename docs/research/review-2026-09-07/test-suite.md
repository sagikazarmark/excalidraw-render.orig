# Test-suite review

Read-only review at `a1be429`. Counts were taken with `rg -c`; nothing was
built or run for this report. Measured timings are in [README.md](README.md).

Legend for *layer*: U = pure unit; C = component (parse/normalize/API without
rasterizing); R = full `Renderer` pipeline with structural assertions on
output; G = golden/byte-pin (SHA-256 / exact byte count / exact pixel count);
P = parity against reference evidence via image comparison; H = harness-only
(`#[ignore]`, needs external tools).

---

## 1. Inventory

Totals: **~671 compiled tests** (423 integration in the renderer crate, 108
`--lib`, 18 CLI, 27 compare, 12 assets-base, 11 ttf2woff2 unit, 62 xtask, 10
package-consumer), **15 `#[ignore]`**, plus 6 wasm-only tests and **29 tests
that are never compiled** (see ttf2woff2 row).

### 1a. `crates/excalidraw-render/tests/*.rs` (45 binaries, 18,543 lines, 423 tests, 10 ignored)

| target | tests | ign | lines | exercises | layer | heavy fixtures / full render |
|---|---|---|---|---|---|---|
| arrow_render | 2 | 0 | 106 | arrows/bindings/elbows in SVG+PNG+PDF | R+G | 3 artifact SHA pins (`:45-52`) |
| artifact_metadata | 10 | 0 | 703 | embedded scene/manifest metadata per format | R | 26 render calls |
| bound_text_render | 6 | 0 | 169 | bound text rotation/placement | R | text → font subsetting |
| compatibility_fallback_render | 23 | 0 | 939 | unknown-element placeholders, fallback raster, limit crossings | R | 57 render calls |
| compatibility_identity | 4 | 0 | 110 | revision selection API, `Send`/`Sync` | U/C | – |
| emoji_fallback_render | 3 | 0 | 343 | Noto Emoji substitution; PDF font inspection | R | loads conformance scene |
| empty_scene_render | 3 | 0 | 140 | empty scene artifacts per format | R+G | byte-exact empty report `:116-119` |
| empty_text_line_render | 4 | 0 | 132 | glyph-less text lines | R | – |
| excalifont_text_render | 8 | 1 | 648 | Excalifont text all formats; work-unit boundaries | R+G+P+H | 7 SHA pins, 3 exact byte-lengths, exact pixel count; ignored PDFium/PDF.js/qpdf gate |
| font_catalog_render | 19 | 0 | 955 | every bundled family shapes/embeds | R | 52 render calls |
| frame_clip_accounting | 4 | 0 | 203 | PNG clip-mask charging | R | trace inspection |
| frame_conformance | 2 | 1 | 418 | frame-export case vs official evidence | P+H | 7 rgba evidence files; SHA pin of exceptions file `:25-26` |
| frame_export | 10 | 0 | 448 | frame discovery/listing/export API | C/R | – |
| frame_render | 12 | 0 | 507 | frame name box / outline / clip rendering | R | – |
| freedraw_render | 10 | 0 | 272 | freedraw strokes | R+G | 4 SHA pins |
| helvetica_text_render | 4 | 0 | 115 | Liberation Sans baselines | R | uses `common/svg_baselines.rs` |
| legacy_line_height_render | 2 | 0 | 125 | lineHeight restore rule | R | – |
| license_bundle | 1 | 0 | 28 | license bundle | G | SHA pin `:5` |
| missing_glyph_render | 3 | 0 | 273 | replacement glyph; PDF ToUnicode | R | loads conformance scene |
| nested_svg_asset_render | 45 | 0 | 1352 | nested-SVG sanitization/embedding in 3 formats | R | 45 render calls incl. PDF Form XObjects |
| normalization | 24 | 0 | 609 | default repair/normalization | C/R | 32 inline scenes |
| outcome_contract | 2 | 0 | 38 | report schema byte form, trait bounds | U | – |
| outline_shape_render | 11 | 0 | 381 | rect/ellipse/diamond/line families | R+G | 3 artifact SHA + 4 path-vector SHA pins |
| pdf_backend_qualification | 8 | 4 | 423 | PDF op matrix, fallback confinement | R+G+H | SHA of inputs and outputs; ignored qpdf/PDFium/PDF.js gates |
| pdf_viewer_adapters | 2 | 2 | 235 | viewer adapter placement contract | H | all ignored |
| rectangle_pdf_render | 6 | 1 | 469 | approved rectangle PDF | R+G+H | ICC length pin `:82` |
| rectangle_png_render | 4 | 0 | 223 | approved rectangle PNG vs evidence | R+P | – |
| rectangle_svg_render | 7 | 1 | 333 | approved rectangle SVG | R+H | ignored test spawns `cargo run` (`:101-118`) |
| registered_case_png_parity | 7 | 0 | 274 | registry-driven direct-PNG parity | P (+4 `should_panic` U) | 21 variants × render + 1-1.5 MB rgba each |
| registered_case_svg_path_parity | 1 | 0 | 194 | `<path d>` + root extent text parity | P | 21 SVG renders |
| registered_case_svg_raster_parity | 4 | 0 | 518 | resvg-rasterize both SVGs, compare | P | 21 × 2 rasterizations + WOFF2 decodes |
| relationships | 2 | 0 | 88 | cycles / depth | C/R | – |
| render_request | 9 | 0 | 419 | builder validation, default limit constants | U | – |
| resource_accounting | 16 | 0 | 705 | limit crossings, exact byte accounting | R | 17 render calls |
| rough_style_render | 7 | 0 | 265 | rough fills/strokes | R+G | 4 SHA pins |
| scene_admission | 8 | 0 | 412 | parser admission & fixed ceilings | C | builds 64 MiB / 1M-entry / ~70 MB scenes (`:290-351`) |
| scene_surface_policy | 21 | 0 | 816 | unknown-key suppression policy | C/R | 44 render calls |
| static_bmp/gif/ico/jpeg/png/webp_asset_render | 9/12/8/13/40/12 | 0 | 272–1515 | image codec decode/embed in 3 formats | R | inline base64 fixtures; PNG pixel probes |
| text_line_endings_render | 3 | 0 | 115 | `\r\n` handling | R | – |
| whole_scene_controls | 12 | 0 | 563 | colours, dark mode, sizing, background | R | 36 format renders |

### 1b. `crates/excalidraw-render` `--lib` (108 tests, 4 ignored, 6 wasm-only)

| module | tests | ign | lines | exercises | layer |
|---|---|---|---|---|---|
| `render/tests.rs` | 38 | 0 | 2058 | subset-vs-complete-chunk size wins, trace digests, diagnostics precedence, text layout/bidi, PNG stroke styles, approval-manifest binding | R+G |
| `render/font_subset_qualification.rs` | 4 | 2 | 1469 | 230 fonts × 8 classes: derive twice, compare WOFF2 SHA per cell | G (corpus-driven) |
| `render/font_subset_fuzz.rs` | 3 | 1 | 676 | fixed-seed fuzz (64 cases PR / 4096 nightly), regression corpus replay | property |
| `render/frame_batch_property.rs` | 2 | 1 | 321 | batch == standalone around resource-limit ladders (16 cases PR / 512 nightly) | property |
| `render/resource_trace_qualification.rs` | 1 | 0 | 229 | cold==warm==cached trace equality + absolute envelope spot-checks | R |
| `normalization.rs` | 27 | 0 | – | pure normalization | U |
| `render/png.rs` 7, `render/font_subset.rs` 6, `scene.rs` 3, `render/metadata.rs` 3, `relationships.rs` 3, others 1-2 | 30 | 0 | – | unit | U |
| `lib.rs::wasm_tests` | 6 | – | – | re-exports of the above for `wasm32` in Chromium | CI-only |

### 1c. Other crates

| target | tests | ign | lines | exercises | layer |
|---|---|---|---|---|---|
| `excalidraw-render-cli/tests/render_cli.rs` | 18 | 0 | 1256 | spawns the CLI binary ~40×: exit codes, staging, frames, stdout failure | end-to-end process |
| `excalidraw-render-compare/tests/{canonical_raster,comparison_protocol,pixelmatch_vectors,ssim_vectors}` | 2/7/6/6 | 0 | 46–247 | comparator numerics against normative vectors | U |
| `…/pixelmatch_upstream.rs` | 1 | 0 | 209 | imported Pixelmatch 7.2.0 fixtures | G |
| `…/profile.rs` | 3 | 1 | 471 | comparator profile digest, source-file digests, Cargo.lock closure | G (source pins) |
| `…/{svg,pdf}_viewer_qualification.rs` | 1/1 | 0 | 69/66 | manifests + digests present | G |
| `excalidraw-render-assets-base/tests/revision_catalog.rs` | 12 | 0 | 415 | reassemble/verify all 230 shards; **decode all 230 WOFF2** (`:72-78`) | G |
| `ttf2woff2-deterministic/tests/encode_test.rs` + `fonttools_compat/` | 3 + 26 | – | 61 + ~ | **never compiled**: `autotests = false`, no `[[test]]` (Cargo.toml), and the fixtures dir they read does not exist | dead |
| `ttf2woff2-deterministic/src/woff2/*.rs` | 11 | 0 | – | varint/triplet/header unit | U |
| `xtask/tests/*.rs` | 13 | 0 | 15–294 | spawn `xtask` for `catalog verify`, `conformance verify`, viewer/font-subset verify | end-to-end process (re-runs CI steps) |
| `xtask/src/{catalog,conformance,svg_font_subset_targets}.rs` | 17/15/17 | 0 | – | canonical JSON, digest helpers | U |
| `tests/package-consumer/tests/public_facade.rs` | 10 | 0 | 458 | packaged crate: 34 artifact/report SHA-256 pins across targets | G (cross-target canary) |

---

## 2. What kind of tests dominate

| kind | share | notes |
|---|---|---|
| **Full-pipeline render with structural assertions** | **~55 %** | 305 `.render(` calls in integration tests; 218 inline `"type":"excalidraw"` literals. |
| **Pure unit** | ~22 % | `render_request.rs`, `normalization.rs` (lib), compare vectors, xtask src. |
| **Golden / byte-pin** | ~12 % | ~50 SHA literals in renderer integration tests, 34 in package-consumer, 3 trace digests in `render/tests.rs`, corpus-driven 1,840-cell WOFF2 pin. |
| **Parity vs reference evidence** | ~4 % | 3 registry harnesses + `rectangle_png_render`, `frame_conformance`, `excalifont_text_render:170`. |
| **Property / fuzz** | ~1 % | `font_subset_fuzz.rs`, `frame_batch_property.rs`, both fixed-seed xorshift. |
| **Harness-dependent (`#[ignore]`)** | ~2 % | 15 tests. |
| **Process-spawning end-to-end** | ~5 % | CLI (18) and xtask (13). |

---

## 3. Fragility analysis

### 3a. Byte-exact pins

**Whole-artifact SHA-256 pins** (break on any byte change):

- `outline_shape_render.rs:35-37`, `arrow_render.rs:45-52`,
  `rough_style_render.rs:48,101,128,133`, `freedraw_render.rs:41,67,71,210`,
  `excalifont_text_render.rs:107,135,157,391,487`,
  `pdf_backend_qualification.rs:61-62,100-101`,
  `tests/package-consumer/tests/public_facade.rs:20-60` (34 hashes, by design
  a cross-target canary).
- **Exact byte lengths**: `excalifont_text_render.rs:104,132,154` — redundant
  with the SHA on the following line.
- **Exact pixel counts / bbox**: `excalifont_text_render.rs:127-128`.
- **Exact accounting measurements**: `excalifont_text_render.rs:384-412,
  475-514`, `resource_trace_qualification.rs:57-58`,
  `rectangle_pdf_render.rs:82` (ICC profile length `820`), `:180`,
  `normalization.rs:445`.
- **Trace digests still pinned** in `render/tests.rs:545-557, 620-624` despite
  ADR-0002 moving trace stability to cross-target equality.
- **Input-fixture SHA pins**: `pdf_backend_qualification.rs:42-49, 90-93`.

**Churn evidence**: 31 of 184 commits changed a 64-hex literal in a test file.

**Tolerance exception / known defect / compatibility-exception pin machinery**
(`tests/common/registered_cases.rs:66-204`): every ceiling is the *exact
observed* `changed_pixels` and the *binary64 bit pattern* of SSIM, and an
excepted variant must *still fail* the canonical budget. Consequences:

- 16 of 21 scene variants carry a `png/whole` tolerance exception; a
  one-pixel improvement in any of them fails `registered_case_png_parity` with
  "tolerance exception is stale" and requires a re-approval round.
- `registered_case_svg_raster_parity.rs:60-84` hard-codes two
  `COMPATIBILITY_EXCEPTION_PINS` with SSIM bit patterns that depend on the
  *unpinned* workspace `resvg`/`fontdb`/`tiny-skia` versions.
- `KNOWN_DEFECTS` lists are currently empty in both harnesses.

**Approval metadata hard-coded in a unit test**: `render/tests.rs:273-375`
asserts `approved_by`, `approved_at`, an issue-comment URL, and a subject
digest. Every re-approval round edits this Rust file.

**Byte-exact serialized report comparisons**: 13 sites use
`serde_json::to_vec(outcome.report())` compared against raw bytes. The registry
harnesses compare as `Value` (`registered_case_png_parity.rs:105-109`). Three
sites pin the literal `{"schema_version":1,"diagnostics":[]}`.

### 3b. Environment dependence

- `env!("CARGO_MANIFEST_DIR")` → `../..` in 21 files.
- Runtime env vars: only `EXCALIDRAW_FONT_SUBSET_FUZZ_CASES`,
  `EXCALIDRAW_FRAME_BATCH_CASES`, compile-time
  `EXCALIDRAW_FONT_SUBSET_TRACE_DIGESTS`. All have defaults.
- External tools only behind `#[ignore = "requires the exact-pinned …"]`.
  Gating is by `#[ignore]` + CI filename selection only — no cfg/feature.
- **Writes into `<repo>/target/` by literal path** with no cleanup:
  `rectangle_svg_render.rs:79-86`, `excalifont_text_render.rs:240-241`,
  `rectangle_pdf_render.rs:319-330`, `pdf_backend_qualification.rs:339-340,
  380-382`, `frame_conformance.rs:259-268`, `pdf_viewer_adapters.rs:45-46,
  106-107`. The `materialize_*` tests write into the source tree
  (`font_subset_qualification.rs:1454-1457`, `compare/tests/profile.rs:125`).
- Temp dirs: CLI and xtask use `temp_dir()/<prefix>-<pid>-<name>`; cleanup by
  explicit `remove_dir_all` — skipped on panic.
- Network: none.

### 3c. Shared mutable state / order dependence

None found. Tests are order-independent.

### 3d. Timing

No `Instant`, `Duration`, `sleep`, or timeout assertions anywhere in tests.

### 3e. Fixtures

- **218 inline scene JSON literals** across 25 integration files; no shared
  scene-builder. Image tests each re-implement `image_scene(...)`.
- `.excalidraw` files are used only for the approved conformance cases and two
  CLI fixtures. The rectangle-outline scene is loaded by 6 different binaries.
- Large binary evidence (`official-*.rgba`, 28 MB total) is read per test run
  by the three parity harnesses and `frame_conformance`.

### 3f. `should_panic` / unwrap

- 4 `#[should_panic]` in `registered_case_png_parity.rs:245-274` — legitimate
  unit tests of `variant_request` that live in the parity binary.
- 280 `.unwrap()` vs 1,343 `.expect(…)` in integration tests.

---

## 4. Approval-pinned source files

**Mechanism.** Manifests under `tests/conformance/` carry an `implementation`
array of `{path, sha256}` descriptors verified on every `xtask conformance
verify-*` run. The manifest minus its `approval` object is the approval
subject. Editing a listed file → verify fails → regenerate manifest → new
subject digest → new human approval comment → re-record.

**Pinned `.rs` files:**

| pinning manifest | pinned source files |
|---|---|
| `svg-font-subset-viewer-qualification/revision-1.json` (#99) | `crates/excalidraw-render/src/render/font_subset.rs`, `…/render/font_subset_qualification.rs` (a `#[cfg(test)]` module), `xtask/src/svg_font_subset_runner.rs`, `xtask/src/svg_font_subsets.rs`, `tools/svg-viewer-rust/src/main.rs` |
| `svg-font-subset-release-evidence/revision-1.json` | `xtask/src/svg_font_subset_targets.rs` |
| `pdf-viewer-qualification/rectangle-outline/manifest.json` (#35) | `xtask/src/pdf_viewers.rs`, `xtask/src/main.rs`, `xtask/tests/pdf_viewer_verify.rs`, `crates/excalidraw-render-compare/tests/pdf_viewer_qualification.rs`, `…/examples/compare_pdf_rgba.rs` |
| `svg-viewer-qualification/rectangle-outline/manifest.json` (#34) | `crates/excalidraw-render-compare/tests/svg_viewer_qualification.rs`, `…/examples/compare_rgba.rs`, `tools/svg-viewer-rust/tests/viewer_cli.rs` |
| `comparator-profiles/canonical-visual-v1.json` | `crates/excalidraw-render-compare/{Cargo.toml, src/lib.rs, src/comparison.rs, src/pixelmatch.rs, src/ssim.rs}` **and its five test files**, plus the Cargo.lock closure |
| `viewer-profiles/canonical-svg-pure-rust-v1.json` | `tools/svg-viewer-rust/src/main.rs`, `tools/svg-viewer-rust/tests/viewer_cli.rs` |

**Maintainability impact** — already visible:
- `cargo fmt --check` excludes `xtask` because `pdf_viewers.rs` carries an
  un-fixable rustfmt diff (`conformance.yml:22-26`).
- `clippy::manual_is_multiple_of` is allowed *workspace-wide* because one
  occurrence in `font_subset.rs` cannot be fixed (`conformance.yml:36-40`).
- `docs/cleanup-plan.md:178-196` keeps duplicated helpers in pinned xtask
  modules "deliberately".
- **Test files are pinned**: adding an assertion to `pixelmatch_vectors.rs`,
  `pdf_viewer_qualification.rs` or `xtask/tests/pdf_viewer_verify.rs` requires
  a re-approval round. Same for `xtask/src/main.rs`.

---

## 5. CI grouping today

| trigger | what runs | profile |
|---|---|---|
| **every push / PR** (`conformance.yml`) | fmt (subset); clippy (3 crates); `xtask conformance verify-all`; `cargo test -p excalidraw-render --lib`; the three `registered_case_*` binaries; then **`cargo test --workspace --no-fail-fast`** (re-runs all four) | **debug** |
| **every push / PR** (`packaged-public-facade.yml`) | `verify-svg-font-subset-release`, `verify-svg-font-subsets`, `verify-svg-viewers`, `verify-pdf-viewers`; `catalog verify`; `cargo test --release --lib font_subset` (native + wasm32 in Chromium); `cargo package`; `check-packaged-cli.sh`; `--test public_facade` (native + wasm) | **release** |
| **PR + push to main** (`dagger.yaml`) | Dagger checks mirroring the above; `dagger.dang:289-291` compiles once and reuses the layer | debug |
| **push to main + nightly + dispatch** (`pdf-harness-gates.yml`) | `--test pdf_backend_qualification -- --ignored`; four other binaries `-- --ignored` | release |
| **nightly + dispatch** (`svg-font-subset-fuzz.yml`) | nightly fuzz (4096 cases), nightly frame-batch property (512 cases), wasm smoke | release |
| **never in CI** | `approved_rectangle_passes_both_canonical_svg_visual_gates`, the three `materialize_*` tests, all `fonttools_compat` tests | – |

Observations:
- **No `cargo nextest`**, no `[profile.test]`/`[profile.dev]` overrides, no
  `.config/nextest.toml`, no `.cargo/config.toml`.
- **No build caching** in GitHub workflows.
- **Push gates run in debug.**
- **Test-group tagging exists only as a naming convention**: `cargo test --lib
  font_subset` selects ~20 lib tests whose *path or name* contains
  `font_subset`; renaming a test silently drops it from qualification evidence.
- **Redundancy**: `--lib` and the three parity binaries run twice per push;
  `xtask/tests/*.rs` spawn `xtask catalog verify` inside `--workspace`, which
  the same job already ran; `catalog verify` runs in both workflows.

---

## 6. Slowness suspects (static estimate; see README.md for measurements)

1. `font_subset_qualification.rs:172-239` — 230 sources × 8 classes = 1,840
   cells; each success cell runs `Session::derive` **twice**, each a WOFF2
   decode → `skera` subset → brotli quality-11 encode → decode → validation.
   **Measured: dominant.**
2. `frame_batch_property.rs:49-71` (PR variant, 16 cases): per case, per frame
   (≤4), per format (3), a probe render + a `limit_ladder` of ~7 limit sets ×
   2 renders. **Measured: dominant.**
3. Parity harnesses: `registered_case_png_parity` renders 21 variants and runs
   Pixelmatch + Gaussian SSIM on 1.0–1.5 MB rasters each;
   `registered_case_svg_raster_parity` additionally rasterizes 42 SVGs with
   resvg and decodes every embedded WOFF2 on both sides. **Measured: ~95 s
   each.**
4. `revision_catalog.rs:72-78`: decodes all 230 WOFF2 chunks. Also re-done
   inside `xtask catalog verify`, which runs multiple times per push.
5. Every text-bearing SVG render re-derives a font subset because the cache is
   per-session (`render/mod.rs:728`). **Measured: not significant** — the
   scenes are tiny.
6. `scene_admission.rs:290-351`: parses ~400 MB of JSON in debug. **Measured:
   3.7 s.**
7. `render_cli.rs`: ~40 process spawns of a debug binary. **Measured: 17.5 s.**

---

## 7. Test-code quality

**Strengths**: behaviour-descriptive names; module docs on harnesses explain
*why*; informative `.expect` messages; registry-driven harnesses make new cases
free; no shared mutable state; `common/registered_cases.rs` is a clean seam.

**Duplication** (`rg -o '^fn (\w+)' | sort | uniq -c`):
- `fn repository_root` — **9 copies** in the renderer crate plus 3 in compare,
  1 in xtask tests, while `common/registered_cases.rs:218` already exports one.
- `fn decode_png` — **8 copies** with two different return shapes; `fn pixel`
  7; `fn render`/`render_outcome` 16 near-identical wrappers; `image_scene` 6;
  `rendered_center` 4; `text_scene` 4.
- `common/` is used inconsistently: 5 binaries via `mod common;`, 6 via
  `#[path = "common/…"]`.

**Dead / unreachable test code**: the entire
`crates/ttf2woff2-deterministic/tests/` tree (29 tests) is not compiled.

**Readability**: several files exceed 900 lines with one test doing many
unrelated assertions (`excalifont_text_render.rs:17-163`); 218 inline JSON
scenes make intent hard to see versus a builder.

**Placement**: `render/tests.rs:273-375` (approval-manifest binding) and the
four `should_panic` tests in the parity binary belong elsewhere.

---

## 8. Recommendations

### (a) Reduce fragility

1. Replace redundant exact pins with the single strongest one.
2. Compare reports as `serde_json::Value`, not bytes; keep exactly one
   byte-form contract test (`outcome_contract.rs`).
3. Delete or reduce `focused_pdf_expectations_are_approval_bound`
   (`render/tests.rs:273-375`).
4. Finish ADR-0002 for `render/tests.rs:545-624`.
5. Move absolute measurement pins behind a derived value.
6. Pin the rasterizer the SVG-raster harness uses, or make
   `COMPATIBILITY_EXCEPTION_PINS` tolerant.
7. Un-pin test files from approval manifests on the next ride-along round.
8. Use `CARGO_TARGET_DIR`/`temp_dir()` and RAII cleanup for harness candidates.
9. Compile the dead ttf2woff2 tests or delete them.
10. One `repository_root`, one `decode_png`, one `render(scene, format)`, one
    `image_scene` builder in `tests/common/`.

### (b) Tier the suite

Adopt **`cargo-nextest` with filtersets + per-binary naming**:

```toml
# .config/nextest.toml
[profile.default]                       # what a developer runs
default-filter = 'not (test(/^(nightly_|materialize_)/) | binary(parity) | binary(harness))'
slow-timeout = { period = "60s", terminate-after = 3 }

[profile.ci-fast]                       # PR gate, debug OK
default-filter = 'kind(lib) | (kind(test) & not (binary(/_(parity|harness|qualification)$/) | binary(scene_admission)))'

[profile.ci-render]                     # PR gate, release
default-filter = 'binary(/_(parity|qualification)$/) | binary(scene_admission) | test(/font_subset|frame_batch/)'

[profile.harness]                       # main + nightly, release
run-ignored = "ignored-only"
default-filter = 'binary(/_harness$/)'
```

Supporting conventions:
- Rename binaries by tier; move the ignored viewer tests into
  `harness_pdf_viewers.rs` / `harness_svg_viewers.rs` so tier == binary.
- Tag the qualification set with a Cargo feature rather than a name substring.
  Note: `font_subset_qualification.rs` is approval-pinned, so the attribute
  can only be added on a ride-along round; until then exclude by runner.
- Collapse the duplicated CI steps.
- Drop or refocus `xtask/tests/{catalog,conformance,svg_viewer,pdf_viewer}_verify.rs`.

### (c) Speed

1. `[profile.test] opt-level = 2` (and `[profile.dev.package."*"] opt-level =
   2` for brotli/skera/resvg/tiny-skia). Do **not** reuse `profile.release`
   for tests.
2. Render each registered variant once per push: merge
   `registered_case_svg_path_parity` and `registered_case_svg_raster_parity`;
   share rendered outputs via `OnceLock`.
3. Pre-decode evidence once per process.
4. Bound `frame_batch_property` PR cost: restrict the PR ladder to `[default,
   output_bytes±1]`.
5. Add `Swatinem/rust-cache`; share one `nextest archive` between jobs.
6. Run the debug `xtask catalog verify` once per push.
