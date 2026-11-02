# Fast tier

Scope: everything `profile.ci-fast` selects
(`not binary(/^(harness|parity|limits)_/)`, `.config/nextest.toml`). ADR-0024
records 1632 fast-tier tests; the static counts below reproduce that within a
few.

## 1. Inventory

Static counts of `#[test]` / `#[wasm_bindgen_test]` attributes per directory:

| Location | `#[test]` | `#[wasm_bindgen_test]` | `#[ignore]` |
|---|---:|---:|---:|
| `crates/excalidraw-render/src/**` (unit) | 499 | 18 (all in `src/lib.rs`) | 5 (+2 env-gated) |
| `crates/excalidraw-render/tests/fast/` (one binary, 100 modules) | 712 | 0 | 0 |
| `tests/asset_fuzz/` | 10 | 1 | 2 |
| `tests/render_fuzz/` | 7 | 1 | 2 |
| `tests/format_features.rs` | 3 (cfg'd out with default features) | | 0 |
| `tests/jpeg_matrix_inputs.rs`, `jpeg_release_limitation.rs`, `pdf_image_colorspace.rs` | 1 each | | 0 |
| `tests/stage4_typography.rs` | 6 | | 0 |
| `crates/excalidraw-render-cli` (30 integration + 25 unit) | 55 | | 0 |
| `crates/excalidraw-render-compare/tests/` | 27 | | 1 |
| `crates/excalidraw-fonts` | 20 | | 0 |
| `xtask/src/**` (unit) | 188 | | 10 |
| `xtask/tests/` | 104 | | 19 |
| **Fast tier total** | **≈1634** | | |

Outside the fast tier: `parity_*` 19, `limits_*` 5, `harness_*` 10 (all
`#[ignore]`).

Largest modules by test count: `fast/resource_diagnostic_contract.rs` 44
(192 KB), `fast/diagnostic_contract.rs` 40 (141 KB),
`fast/nested_svg_asset_render.rs` 38, `fast/static_png_asset_render.rs` 29,
`fast/png_stage4_contract.rs` 27; in `src/`: `render/asset/tests.rs` 101,
`render/tests.rs` 76, `normalization/tests.rs` 48.

Thematic split of the 712 `fast/` tests: codec and image-asset contracts ≈332
(47%), diagnostic, normalization and field contracts ≈250 (35%), geometry,
rough, frame and scene rendering ≈105 (15%), text and font ≈57, PDF backend
≈22.

### Ignored tests

| Test | Reason |
|---|---|
| `src/scene_fuzz/mod.rs:252`, `src/render/font_subset_fuzz.rs:234`, `src/render/frame_batch_property.rs:680` | nightly expansions |
| `src/render/font_subset_qualification.rs:1706,1760` `materialize_*` | regenerate committed evidence |
| `src/render/font_subset_qualification.rs:1692-1695` corpus test, `src/render/frame_batch_property.rs:671-674` | `cfg_attr(not(release_qualification), ignore)`; `build.rs:10-18` |
| `tests/asset_fuzz/mod.rs:669,737`, `tests/render_fuzz/mod.rs:472,485` | nightly expansion or replay a reproducer |
| `compare/tests/profile.rs:130` | rewrites the committed dependency closure |
| `xtask/src/coverage/layers.rs:355-706` (9) | "optional historical five-layer audit" |
| `xtask/tests/representative_*.rs` (≈20) | need `REPRESENTATIVE_RENDERER` or Docker |

## 2. Pin-based versus behavioral

### Raw counts

64-hex SHA-256 literals in fast-tier `.rs`: 952 in total, of which 710 are
in `crates/excalidraw-fonts/src/generated/revision_1.rs` and `fonts/src/lib.rs`
(the upstream font-chunk catalog, provenance data rather than assertions).
About 242 remain in test code:

| File | Literals | Category |
|---|---:|---|
| `src/render/tests.rs` | 93 | (b) Rough.js 4.6.4 / perfect-freehand 1.2.0 path-vector digests derived by `tools/rough-filler-vectors/derive.mjs` |
| `xtask/src/catalog.rs` | 24 | machinery fixtures |
| `fast/pdf_backend_qualification.rs` | 8 | (a) 3 whole-PDF self-pins + 5 digests of committed inputs |
| `compare/tests/profile.rs` | 8 | tautology (comparator profile sidecar) |
| `xtask/src/conformance_pdf_requalification.rs` | 7 | machinery |
| `fast/webp_sample_contract.rs` | 6 | evidence-file digests + encoder binary digests |
| `fast/rough_style_render.rs` | 6 | 5 Rough.js-derived, 1 unattributed self-pin (`:45-48`) |
| `xtask/src/svg_font_subset_targets.rs`, `conformance_candidate_native.rs`, `conformance.rs` | 5 each | machinery |
| `fonts/tests/revision_catalog.rs` | 4 | upstream font provenance |
| `fast/jpeg_icc_contract.rs`, `jpeg_gain_map_contract.rs`, `jpeg_exif_contract.rs` | 3 each | evidence-file digests |
| `fast/excalifont_text_render.rs` | 3 | (a) whole-artifact self-pins SVG/PNG/PDF (`:85-88, :100-103, :129-132`) |
| `fast/arrow_render.rs` | 3 | (a) PNG/PDF whole-artifact (`:56-63`), SVG path digest (`:84-87`) |
| `common/png_color_reference.rs` | 3 | evidence-file digests |

Exact byte-length pins: 3. `assert_eq!` against four-or-more-digit integer
literals in `fast/`: 12 (mostly row-count cardinalities). Work-unit pins are
formulaic rather than literal, e.g. `element_charge("large", "/elements/0",
16 * 82 * 62)` (`fast/rectangle_png_render.rs:118-121`). The bisection
helpers `total_work_boundary`/`trailing_work_charges`
(`common/render.rs:66-200`) are called from six sites and perform about 32
renders each.

### Classification

**(a) Self-referential snapshots of the renderer's own output**, re-pinned on
every intentional change (the "Moved with #…" comment trail appears 22 times;
`fast/pdf_backend_qualification.rs` alone has 10):
`fast/pdf_backend_qualification.rs:63-66, :108-111, :219-222`;
`fast/excalifont_text_render.rs:85-132`; `fast/arrow_render.rs:56-63`;
`fast/outline_shape_render.rs:46-55`; `fast/cluster_text_pdf.rs:55-60` and
`fast/png_paint_mask_work.rs:143-148` (pinned to a prior implementation's
bytes); `fast/webp_container_contract.rs:37-43`; `fast/rough_style_render.rs:45-48`;
known-defect characterizations `jpeg_release_limitation.rs:21`,
`jpeg_gain_map_contract.rs:733`, `webp_sample_contract.rs:1768`. About 5–7%
of fast-tier assertions, around 20 test functions, but they guard the most
visible whole-artifact outputs.

**(b) Against official or upstream reference evidence**, the strongest part of
the tier: `tests/oracle/*/reference.json` (Chromium 149 exports of official
0.18.1: png-color 984 + stage4 1968 + 1968 + 1968 + 840 rows, jpeg-color 76,
jpeg-icc 160, webp-container 195, webp-sample, gif, bmp-ico 104,
image-resample 73, nested-svg 54, png-precision 30), consumed by the codec
contracts through `excalidraw_render_compare::compare`; official SVG/PNG of
registered cases in `fast/rectangle_svg_render.rs:22-36`,
`fast/rectangle_png_render.rs:42`, `fast/linear_rotation_render.rs:108`,
`fast/excalifont_text_render.rs:159`, `stage4_typography.rs:35`;
ported-algorithm vectors in `src/render/tests.rs:3082-4030` and
`src/render/number_format.rs:115-160`. About 45–50% of fast-tier test
functions.

**(c) Behavioral, property and A/B equivalence**: render twice and compare
bytes (nearly every contract loop); "scene with invalid field X renders
byte-identical to a clean scene with the default" (the field-contract
matrices); authored diagnostic rows; metamorphic checks
(`fast/geometry_style_contract.rs:282-316`). About 45%.

The render-twice determinism check is the single most repeated assertion in
the tier; it protects against nondeterminism and says nothing about
correctness.

## 3. Self-referential tautologies

1. **Digests of committed files compared with literals**, 13 sites:
   `common/png_color_reference.rs:12-15, :158-161, :289`;
   `common/png_stage4_composition.rs:7,:31`; `common/png_stage4_reference.rs:7`;
   `fast/jpeg_color_contract.rs:43`; `fast/jpeg_icc_contract.rs:49,:88`;
   `fast/jpeg_exif_contract.rs:75,:93,:99`; `fast/jpeg_gain_map_contract.rs:46,:52,:73`;
   `fast/webp_sample_contract.rs:24-25,:1211`; `src/lib.rs:459`;
   `src/png_projection_contract.rs:56`; `src/stage4_diagnostic_contract.rs:19`.
   Git already guarantees this; a change requires two edits in lockstep.
2. `fast/pdf_backend_qualification.rs:33-40, :95-98, :195-202` hash
   `PROJECT_STRUCTURE`, `NESTED_NATIVE_STRUCTURE`, `FALLBACK_STRUCTURE`, which
   are never used for anything but their own digest.
3. `fast/webp_sample_contract.rs:1895-1916` tests the test file's own
   `within_pdf_exception` predicate against synthetic JSON.
4. `fast/webp_sample_contract.rs:552-574` asserts the evidence record names
   `libwebp 1.3.2`, `python 3.12.3`, a `/lib/x86_64-linux-gnu/libwebp.so.7`
   sha256 and the Playwright OCI digest.
5. `fast/pdf_renderer_expectations.rs:19-131` checks key-set coherence of two
   JSON files; renders nothing.
6. `compare/tests/profile.rs:28-58`: the `.sha256` sidecar equals the profile
   digest and every `implementation.sources[*].sha256` matches disk, so a
   comment edit in `compare/src` fails until the profile is regenerated.
7. `fonts/tests/revision_catalog.rs:495-545`: artifact digests equal
   constants.
8. `tests/jpeg_matrix_inputs.rs:95` regenerates `inputs.json` with
   `jpeg-encoder 0.7.1` and compares bytes; it tests the dev-dependency.
9. `fast/license_bundle.rs:27-30`: SHA-256 of the license text.
10. `fast/readme_examples.rs`, `fast/scratch_dir.rs`, `fast/outcome_contract.rs:20-38`:
    repository hygiene and helper tests in the renderer's fast binary.
11. `xtask/tests/coverage.rs:176-189` asserts that `verify-closure` fails.
12. `fast/cluster_text_pdf.rs:55-60`, `fast/png_paint_mask_work.rs:143-148`:
    the oracle is "what the writer produced before the optimization".

**Umbrella tests re-run other `#[test]` bodies**: 230 re-invocations (141
same-file, 89 cross-module) across 23 files, e.g.
`resource_diagnostic_contract.rs` 34, `diagnostic_contract.rs` 22,
`jpeg_container_contract.rs:209-279` 22, `final_surface_contract.rs` 13,
`png_stage4_contract.rs:274-284, :468-479` (the latter re-runs 1968 rows × 3
formats × 2 renders). 112 of the 712 `fast/` tests are `pub(super)` so they can
be re-invoked. About a third of the tier's wall clock is duplicated work.

## 4. Contract matrices

All five `fast::*` matrices are behavioral A/B matrices, not pins. Pattern
(`fast/geometry_style_contract.rs:144-178`): build an original scene, derive a
clean one with the field at its default, and for each kind × field × format ×
wrong-type (nine values) assert that the mutated render equals the clean
baseline (memoized per thread in `common/clean_baselines.rs`), that the
diagnostic report equals an authored row, that two renders agree, that
embedded metadata round-trips, that the same holds through `render_frame`, and
that `isDeleted` silences it.

| Matrix | Iterates | Oracle |
|---|---|---|
| `geometry_style_contract` | 14 kinds × 5 fields × 3 formats × 9 wrong types, plus style sets and a metamorphic participation check | clean-scene A/B and authored diagnostics |
| `surface_metadata_contract` | about 60 `appState` fields, element and file metadata, fractional indices | bytes equal the clean render |
| `text_field_contract` | text fields × 12 values × 3 formats | A/B plus authored repair rows |
| `relationship_field_contract` | bindings, groups, frames × 11 values | A/B plus authored rows |
| `png_stage4_contract` | authored PNG chunk vectors plus 1968 + 840 official composition rows | mixed: authored pixels and official Chromium evidence |
| `png_projection_contract` (`src/`) | 504 official rows plus resource boundaries | official evidence and authored accounting |

They are slow because of thousands of full renders and the umbrella
duplication.

## 5. Upstream compatibility

Fast-tier tests that genuinely exercise upstream behavior: the Rough.js port
(`src/render/tests.rs:3082-3653, :3667-3840, :3980-4030`,
`fast/rough_fillers_dots_scene.rs`, `fast/rough_style_render.rs`), derived by
`tools/rough-filler-vectors/derive.mjs` against the oracle's `node_modules`;
perfect-freehand (`src/render/tests.rs:3886-3930`, `fast/freedraw_render.rs`,
`fast/point_stroke_contract.rs`); number formatting
(`src/render/number_format.rs` against `tests/fixtures/number-format-vectors.json`);
restore semantics (`src/normalization/tests.rs`, `fast/normalization.rs`,
`fast/compatibility_*`); text metrics (`src/render/tests.rs` Helvetica and
baseline tests, `common/svg_baselines.rs`); official export evidence
(`fast/rectangle_svg_render.rs`, `rectangle_png_render.rs`,
`linear_rotation_render.rs`, `excalifont_text_render.rs:159`,
`stage4_typography.rs`, and the codec oracles); font provenance
(`fonts/tests/revision_catalog.rs`).

Internal-invariant only: resource accounting, `diagnostic_contract`,
`duplicate_identity_contract`, `admission_identity_contract`,
`scene_admission`, `render_request`, `outcome_contract`,
`artifact_metadata`, the PDF string checks, the fuzz smokes,
`registered_case_determinism`.

## 6. Platform dependence

Mitigations: transcendental math through `libm` (`src/portable_math.rs`,
enforced by `clippy.toml` `disallowed-methods`); `tiny-skia` without SIMD
features. ADR-0024 reports the full fast tier matched Linux bytes on
`aarch64-apple-darwin`.

Platform-sensitive items: `crates/excalidraw-render-cli/tests/render_cli.rs:1302-1304`
(`/dev/full`, Linux only), `:1089, :1114` (`cfg(unix)`), `:1144` (`mkfifo`);
xtask tests writing `#!/bin/sh` fake `qpdf` scripts (unix only);
`xtask/src/conformance_candidate_native.rs:205-208` refuses any `rustc` other
than 1.97.1; qpdf 12.3.2 exactly (`xtask/src/pdf_harness.rs:185-192`);
`xtask/tests/webp_boundary.rs:7` runs node;
`xtask/src/coverage/ownership/representative.rs:259` runs node; Docker only in
ignored tests. No `HOME`, line-ending or path-separator assumptions in renderer
tests. `x86_64-unknown-linux-gnu` is asserted only as a recorded evidence
string.

## 7. xtask tests

292 tests: 188 unit, 104 integration, 29 ignored. Essentially all (about
98.6%) test the verifier and evidence machinery. The only xtask tests that run
the renderer are the four `release-cli` group members
(`.config/nextest.toml:55-66`): `xtask/tests/coverage.rs` (via
`verify-closure` → representative owner → native replay → `build()`),
`xtask/tests/representative_candidate.rs`,
`coverage::ownership::representative::tests::representative_owner_release_positive_path_uses_real_gate_in_isolated_fixture`
(also runs node), and
`conformance_candidate_native::tests::fresh_native_replay_rejects_rehashed_artifacts_reports_sources_and_builds`.
`build()` runs `cargo build --release --locked` with LTO and one codegen unit.

## 8. Assessment

**Real regression protection (keep)**: the Rough.js, perfect-freehand and
number-format vectors; the codec oracle contracts; the official-evidence
comparisons; `normalization/tests.rs` and the field-contract matrices;
`registered_case_determinism` and the render-twice checks; the fuzz smokes and
regression corpora.

**Mostly machinery testing itself**: the 13 file-digest guards and the unused
`*_STRUCTURE` constants; the webp provenance assertions and predicate
self-test; `pdf_renderer_expectations.rs`, `license_bundle.rs`,
`readme_examples.rs`, `scratch_dir.rs`, `outcome_contract.rs`,
`jpeg_matrix_inputs.rs`, `compare/tests/profile.rs:28`; about 288 of 292 xtask
tests.

**Recommendations**:

1. Delete the 13 file-digest guards and the unused `*_STRUCTURE` includes;
   move the predicate self-test and encoder assertions out of the fast binary.
2. Demote the whole-artifact self-pins to a snapshot module or the render tier;
   keep the Rough.js-derived path digests.
3. De-duplicate the umbrella tests: either the sub-tests are plain helpers or
   the umbrellas go, not both. Highest payoff in `resource_diagnostic_contract`,
   `diagnostic_contract`, `jpeg_container_contract`, `nested_svg_asset_render`,
   `pdf_raster_fallback_contract`, `png_stage4_contract`, `final_surface_contract`,
   `png_color_contract`.
4. Move the host-tool-dependent xtask tests (node, qpdf, release CLI) to an
   opt-in profile.
5. Fold `jpeg_matrix_inputs.rs`, `pdf_image_colorspace.rs`,
   `jpeg_release_limitation.rs` and `stage4_typography.rs` into the `fast`
   binary.
6. Mark known-defect pins as characterizations so a fix is recognized as
   progress.
