# Multi-agent review, 2026-09-07

Review of the workspace at commit `a1be429` along six questions: architecture,
test coverage, test-suite stability and speed, coverage of the Excalidraw
0.18.1 format and exporter behavior, comparability of rendered assets (with
emphasis on font embedding), and opportunities to test in more isolation.

Five read-only review agents produced the reports in this directory. The
timing data and the spot-checks of the highest-impact claims (the
`strokeSharpness` gap, the namespaced `clip-path` in official evidence, the
absence of PDF stream filters, the dead `ttf2woff2` test tree, the unused
dependencies) were verified directly.

| Report | Question |
|---|---|
| [architecture.md](architecture.md) | Crate and module boundaries, pipeline shape, generated code, diagnostics model, dependency hygiene, determinism architecture |
| [test-suite.md](test-suite.md) | Inventory of every test target, fragility patterns, approval-pinned files, CI grouping, slowness suspects, recommendations |
| [spec-coverage.md](spec-coverage.md) | Requirement checklist against `docs/renderer-spec-v1.md` and against Excalidraw 0.18.1 restore/export behavior; prioritized gap list |
| [asset-comparability.md](asset-comparability.md) | SVG structure, font embedding (SVG/PDF/PNG), visual parity evidence, PDF comparability, determinism-vs-comparability tension |
| [test-isolation.md](test-isolation.md) | Per-file mapping of integration tests to the pipeline stage they exercise, internal seams, proposed test pyramid, conversion risks |

The work items derived from these reports are tracked as GitHub issues under
the parent tracking issue named in the commit that adds this directory.

## Measured test timings

Debug profile (what `conformance.yml` runs on every push), 4 cores, from a
clean worktree of `a1be429` with a warm dependency cache.

| Target | Tests | Wall time | Share |
|---|---|---|---|
| `-p excalidraw-render --lib` | 108 | 336.8 s | 54% |
| of which `font_subset_qualification::complete_font_subset_qualification_corpus_matches_generated_bytes_and_traces` and `frame_batch_property::frame_batch_rendering_matches_standalone_around_resource_limits` | 2 | ~326 s | |
| of which everything else | 102 | 10.7 s | |
| `--test registered_case_png_parity` | 7 | 96.9 s | 16% |
| `--test registered_case_svg_raster_parity` | 4 | 91.9 s | 15% |
| `-p excalidraw-render-cli` (35 process spawns) | 18 | 17.5 s | 3% |
| `-p xtask` (spawns `catalog verify`, `conformance verify`) | 62 | ~10 s | 2% |
| `--test frame_conformance` | 1 | 5.5 s | |
| `--test registered_case_svg_path_parity` | 1 | 4.5 s | |
| `-p excalidraw-render-assets-base` (230 WOFF2 decodes) | 12 | 3.8 s | |
| `--test scene_admission` (64 MiB / 1M-entry scenes) | 8 | 3.7 s | |
| `-p excalidraw-render-compare` | 27 | ~5 s | |
| remaining 41 renderer integration binaries | ~400 | ~15 s | |
| **Total (serial by binary)** | ~656 | **~10 min** | |

Four tests account for roughly 85% of the wall time. Every text-bearing
integration binary — the ones that exercise WOFF2 subsetting at brotli quality
11 per render — finishes in 0.1–3 s (`font_catalog_render`: 19 tests, 2.9 s),
so per-render font subsetting is not the bottleneck the static analysis in
[test-suite.md](test-suite.md) and [test-isolation.md](test-isolation.md)
predicted. The corpus-driven qualification test, the frame-batch property
test, and the two raster parity harnesses are.

`conformance.yml` runs `--lib` and the three parity binaries as named steps
and then again inside `cargo test --workspace`, so a push pays this twice.
`catalog verify` runs four times per push across the two workflows and
`xtask/tests/catalog_verify.rs`.

## Headline findings

1. **Architecture is sound at the crate level and accreted at the file
   level.** Stage IRs (`SceneDocument` → `NormalizationOutcome` →
   `RelationshipOutcome` → `ResolvedScene` → per-format plans → bytes) exist
   and are the right ones; backends never see the scene document; determinism
   is designed in. `render/mod.rs` (4,787 lines), `normalization.rs`,
   `pdf.rs`, `png.rs`, `asset.rs` and `scene.rs` each hold several modules'
   worth of responsibilities behind glob imports. Resource accounting is
   woven through every stage rather than being one.
2. **Spec coverage of the project's own contract is strong; coverage of
   Excalidraw 0.18.1's restore path has undeclared gaps.** The legacy
   `strokeSharpness` migration the scene-surface manifest promises is not
   implemented; image `roundness` clipping, dark mode on frame outlines and
   image pixels, and the pinned image viewport integer rounding are missing.
   None carries a diagnostic or a test.
3. **Geometry and text placement are comparable to the official exporter;
   size and font-embedding policy are not.** Path data is byte-identical
   across all registered cases. One emoji embeds the complete 862 KB Noto
   Emoji TTF; Helvetica text embeds a Liberation subset the official never
   embeds; PDFs embed complete uncompressed font programs and use no stream
   filter at all; `<metadata>` repeats license bodies per font.
4. **The suite is stable but churns.** No flakiness sources were found. The
   fragility is maintenance: 31 of 184 commits repin a digest; 16 of 21 parity
   variants carry no-margin tolerance ceilings that fail on improvement;
   approval manifests pin test files; approval metadata lives in a Rust test.
5. **Isolation is the lever for stability and legibility, not for speed.**
   `color.rs`, every codec decoder, the SVG sanitizer, Rough geometry, bound
   text placement and the CLI's pure functions have zero direct tests; the
   in-crate `#[cfg(test)]` mechanism to test them already exists and is used
   elsewhere.
