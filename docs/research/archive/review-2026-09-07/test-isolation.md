# Test-isolation review

Read-only review at `a1be429`. Numbers are static counts; the measured
timings in [README.md](README.md) qualify the cost analysis below.

## 0. Findings that frame everything else

**Actual render volume is ~2.3× the call-site count.**
`crates/excalidraw-render/tests/` has 423 `#[test]`s and 292 `Renderer::new()`
sites, but 50 sites sit inside `for format in [Svg, Png, Pdf]` loops and 16
files use a "render twice, compare bytes" determinism pattern. A crude
call-graph estimate yields **~690 render invocations** per full run.

**Font subsetting is paid by tests that don't care about it — but is cheap at
test-scene scale.** Every `RenderSession::new` creates a fresh
`svg_fonts::Session` (`render/mod.rs:1009`, `:1036`), so there is no
cross-render subset cache for scene renders. Every SVG render containing text,
a frame name, an embeddable label, or a fallback placeholder label
(`font_demand.rs:133-167`) goes through `font_subset::derive` → WOFF2 encode at
brotli quality 11 (`font_subset.rs:206`). Measurement shows this costs well
under 100 ms per render for test-sized scenes; isolation therefore buys
stability and legibility, not wall time.

**In-crate unit testing is already the established mechanism** — no
`test-support` feature or `#[doc(hidden)] pub mod internals` is needed (which
would also cut against spec §9). Precedent exists at every layer:
- `normalization.rs:3493-4474` — 27 tests calling `normalize_scene(&scene, REVISION_1, Svg, |_| Ok(()))` directly
- `relationships.rs:850-949` — `relationships::resolve` with no-op observers
- `render/tests.rs:1057-1180` — `text::OfficialFont::load(5,"…").layout(...)` and `text::layout_with_fallbacks(...)` asserting baselines/runs/glyph ids with **zero rendering**
- `render/tests.rs:976` — `text::FallbackFont::load().resolve_frame_name(...)`
- `render/png.rs:3222-3446` — encoder-level tests
- `render/metadata.rs:745-860` — `ArtifactMetadata::build/estimate`
- `render/mod.rs:612-788` — `render_with_trace`, `render_with_trace_and_font_subset_cache`, `new_font_subset_cache`, and `svg_fonts::Payload::CompleteChunks` (`svg_fonts.rs:67`) that **skips subsetting entirely**

---

## 1. Test file → stage → cost driver → proposal

"Sub" = triggers WOFF2 subsetting; "Rast" = PNG rasterization; "3F" = 3-format loop.

| File (tests) | What it really tests | Owning stage | Cost driver | Proposed level | Renders now → after |
|---|---|---|---|---|---|
| `normalization.rs` (24) | Default/negative-dim/deleted-element equivalence; repair diagnostics; diagnostic **ordering** across phases; limit crossings | `normalize_scene` (`normalization.rs:558`); ordering: `RenderSession::take_report` sort (`mod.rs:4348-4363`); limits: session meter | Full SVG render ×2 per equivalence check | Equivalence/content → in-crate `normalize_scene` observer tests (capture `NormalizationEvent::Diagnostic` at `normalization.rs:228`). Extract the sort into a pure `fn`. Keep the 4 limit-crossing tests E2E. | ~32 → ~8 |
| `scene_surface_policy.rs` (21) | Which unknown/malformed members produce which diagnostics | `scene_surface.rs` policy inside `normalize_scene_with_assets` | Full SVG render per case | Same observer pattern. Keep the preflight-limit tests E2E. | ~24 → ~3 |
| `scene_admission.rs` (8) | `SceneDocument::parse` limits | Layer 1 | **No render** | Keep | 0 |
| `relationships.rs` (2) | Cycle-closing edge removed; depth limit | `relationships::resolve` | 3F + 3 | Cycle → in-crate; depth → keep E2E | 7 → 3 |
| `whole_scene_controls.rs` (12) | CSS color lowering (15 renders in one test, 475-510), alpha/opacity propagation, dark mode, sizing | `color::parse_css` (`color.rs:102`), `CanonicalColor::for_mode/darkened` (`:46,:86`), `write_hex` (`:78`) — **939 lines, zero unit tests** | Full render, 2×3F | Color → `color.rs` `#[cfg(test)]`. Keep 1 test per format asserting alpha/opacity reach the encoder. | ~35 → ~8 |
| `outline_shape_render.rs` (11) | Pinned Rough.js path vectors as SHA-256 of `<path d>` (79-112, 163-236); 3F golden; geometry-limit accounting; local recovery | `geometry::rough_drawing` (`geometry.rs:157`); path text via `svg::push_rough_path` (`svg.rs:1517`, private) | 5×3F | Pinned vectors → in-crate: normalize → `rough_drawing` → `push_rough_path` → same SHA-256 (digests unchanged). Keep one 3F golden + accounting. | ~24 → ~7 |
| `rough_style_render.rs` (7) | Fill families / zoom threshold / polygon pinned vectors; per-backend fill rule | `rough::*_drawing`; `DrawableShape::svg_uses_even_odd_fill` (`rough.rs:304`) | 1×3F + ~9 SVG | Drawing-level vectors; fill-rule → keep E2E | ~12 → ~4 |
| `arrow_render.rs` (2) | Arrowheads/elbows/bindings 3F golden + determinism | `geometry::add_arrowheads/elbow_drawing` (`geometry.rs:438-722`) | 2×3F, double render | Keep 1×3F; drop double render | 8 → 4 |
| `freedraw_render.rs` (10) | Pinned freedraw path digests; legacy `draw` migration 3F; edge cases ×3F | `freedraw::generate/project` (`freedraw.rs:76,476`), `svg::freedraw_path_data` (`svg.rs:1561`, private) | 6×3F | Vectors + edge cases → `freedraw` unit tests; migration → `normalize_scene`; keep 1×3F | ~26 → ~5 |
| `helvetica_text_render.rs` (4) | tspan **baselines** | `text::OfficialFont::layout` (`text.rs:588`) | SVG render → Sub | Port to the pattern at `render/tests.rs:1078-1096`. | 4 → 0 |
| `legacy_line_height_render.rs` (2) | Missing `lineHeight` → baselines | **Already unit tested** at `normalization.rs:4346-4450` | SVG ×2 | Delete after adding a baseline assertion to the existing tests | 2 → 0 |
| `text_line_endings_render.rs` (3) | CR/CRLF splitting; raster identical | **Already unit tested** at `render/tests.rs:1078-1180` | 1 SVG + 3 PNG + 1 PDF | Keep the PNG raster-equality test; delete the rest | 5 → 2 |
| `empty_text_line_render.rs` (4) | Empty lines paint nothing, keep line step | `layout` | 4 SVG + 5 PNG + 1 PDF | Layout-level; keep 1 PNG inked-row test | 12 → 3 |
| `bound_text_render.rs` (6) | Bound-text placement transforms | `bound_text::resolve_bound_text_placements` (`bound_text.rs:20`) | 6 SVG (Sub) | Resolve-level (needs `resolve_for_test`, §2) | 6 → 0 |
| `excalifont_text_render.rs` (8) | 3F sha pins for approved case (**true golden**); PNG typography budget; limits; CJK; PDF cmap | Mixed | 1×3F + ~15 | Keep #1-#3 as gates and the limit tests. CJK → layout/resolve-level. | ~18 → ~12 |
| `font_catalog_render.rs` (19) | Face selection per family, Helvetica substitution count, CJK trigger propagation, bidi runs, unicode-range chunk selection, embedded-subset cmap, RFN rewrite | `layout_with_fallbacks` (`text.rs:762`), `CjkSubsetTrigger::evaluate` (`:128`), `font_subset::derive`, `svg_fonts::build` | 21 SVG + 12 PNG + 16 PDF | Bidi/run/face → `layout_with_fallbacks` (pattern at `tests.rs:1272-1490`). CJK trigger → `resolve_for_test`. Subset cmap/RFN → `font_subset::derive` tests. Keep 2-3 E2E. | ~36 → ~8 |
| `missing_glyph_render.rs` (3), `emoji_fallback_render.rs` (3) | Registered-case report parity 3F + embedding details | Registered case | 2×(1×3F + 5) | Keep the 3F report tests (they compare `expected-report` files). Details → 1 render each. | 16 → 10 |
| `static_{png,jpeg,gif,webp,bmp,ico}_asset_render.rs` (94) | Codec decode correctness (EXIF, ICC/cICP/gAMA, palette/RLE8/bitfields, animation validation, ICO selection, sniffing), typed recovery, SVG passthrough, PDF `/SMask`, accounting | `asset.rs` private fns (`decode_static_png` 2420, `decode_static_jpeg` 2585, `decode_webp_default_frame` 1466, `decode_gif_default_frame` 1587, `decode_bmp` 1084, `decode_ico` 1160, `exif_orientation` 2667, `orient_rgba` 2720, `png_color_profile` 2857), `asset/bmp.rs:17-40`, `asset/ico.rs:25-264`. **Zero unit tests.** | `rendered_center()` = PNG render + decode + pixel pick; ~125 renders | `asset::Cache::resolve(...)` (`asset.rs:360`) returns `Resolution.asset.rgba` → assert pixel directly. Keep per codec: 1 SVG passthrough, 1 PNG projection, 1 PDF embed. | ~125 → ~25 |
| `nested_svg_asset_render.rs` (45) | Sanitizer removals (39-345); recursion & accounting (145-533); **PDF effect-closure isolation** (612-999); native-vs-raster matrix (1061-1150) | `asset::svg::sanitize` (`asset/svg.rs:84`) → `SanitizedSvg{bytes,removals,failures,effects}`; `PdfPlan::raster_closures()` (`pdf.rs:350`) | 19 SVG + 15 PNG + 26 PDF | Sanitizer → unit. PDF closures → plan-level via `resolve_for_test` + `PdfPlan::new`. Keep ~8 E2E. | ~108 → ~35 |
| `resource_accounting.rs` (16) | Exact aggregate charge order at ±1 boundaries | Session meter — the ADR-0002 surface | ~30 cheap renders | **Keep E2E.** | 30 → 30 |
| `frame_clip_accounting.rs` (4) | Clip-mask charge per frame | Overlaps `tests.rs:898` | PNG w/ limits | Keep | 6 → 4 |
| `artifact_metadata.rs` (10) | Carrier round-trip per format (**keep**); selection under output exhaustion; notices | `ArtifactMetadata::build` (already unit tested) | 5×3F | Keep round-trip + 2 exhaustion tests | 17 → 12 |
| `compatibility_fallback_render.rs` (23) | Placeholder markup; fit/ellipsis; link sanitizer; embeddable shell; accounting | `text::FallbackFont::resolve` (`text.rs:235`) — pure; `scene_surface::fallback_label` (`scene_surface.rs:1519`) | 65 renders; 4×3F | Fit/ellipsis → `FallbackFont::resolve` tests. Link sanitizer → `normalize_scene`. Keep 1 SVG golden, 1×3F, 3 accounting. | ~65 → ~22 |
| `frame_render.rs` (12) | Name box metrics, colour, truncation (dup of `tests.rs:976`), clip-path defs, nested clipping 3F | `text::FallbackFont::resolve_frame_name`, `frame_target::resolve_frame_clips` (`frame_target.rs:174`) | 24 renders | Resolve-level; keep the 3F nested-clip test and PNG grey-name test | 24 → 8 |
| `frame_export.rs` (10), `frame_conformance.rs` (2) | Public frame API, pinned official frame evidence | API + gates | | **Keep** | keep |
| `rectangle_{svg,png,pdf}_render.rs` (17) | Approved-case goldens + visual gates + limit boundaries + PDF number formatting | Gates; `format_pdf_number` (`pdf.rs:3042`) | ~33 | Keep goldens/gates/limits. `format_pdf_number` and `rough::push_number` → unit | 33 → 28 |
| `empty_scene_render.rs` (3) | One golden per format | Gate | 7 | **Keep** | 7 |
| `registered_case_*_parity.rs` (12) | 17 registered scenes vs official | **The** E2E gates | corpus ×3 binaries | **Keep** (merge binaries) | keep |
| `pdf_backend_qualification.rs` (8), `pdf_viewer_adapters.rs` (2) | PDF matrix + harness gates | Gate | 14 | Keep | keep |
| `compatibility_identity.rs`, `outcome_contract.rs`, `render_request.rs`, `license_bundle.rs` | Public API surface | — | **No render** | Keep | 0 |
| **CLI** `render_cli.rs` (18 tests, 35 spawns) | 12 CLI-only (arg parsing, exit codes, staging, emergency JSON); 6 render | `frame_output_basenames` (`frames.rs:741`), `portable_frame_slug` (790), `truncate_basename` (807), `is_reserved_device_name` (815); `select_format` (`main.rs:722`), `parse_asset_declarations` (759), `same_path` (887), `preparse_json_mode` (1579) — **0 unit tests in the CLI crate** | Process spawn | `frame_filenames_use_unicode_case_folding…` (924-991) → unit test of `frame_output_basenames`. Add unit tests for the pure fns; keep 1 render smoke + staging tests | 35 spawns → ~25 |

**Aggregate:** ~690 → roughly **250–300** render invocations.

---

## 2. Internal seams to expose (or just use) and mechanism

Mechanism: **in-crate `#[cfg(test)]` modules** — no visibility change for
`pub(super)`/`pub(crate)` items; private items need a one-word `pub(super)`.
Suggested layout: split `render/tests.rs` (2,058 lines) into
`render/tests/{mod,trace,text,geometry,assets,fallback,plans}.rs`.

| Seam | Current visibility · location | Unit-tested today? | Change needed |
|---|---|---|---|
| `normalize_scene` / `normalize_scene_with_assets` | `pub(crate)` · `normalization.rs:558,582` | Yes (27) | Add observer collecting `NormalizationEvent::Diagnostic` |
| `NormalizationOutcome` | `pub(crate)`, `Clone, Debug` · `normalization.rs:204` | — | Add `PartialEq` |
| `relationships::resolve` | `pub(crate)` · `relationships.rs:138` | Yes (3) | None |
| `RenderSession::take_report` sort | private · `mod.rs:4348-4363` | No | Extract `fn sort_diagnostics(&mut Vec<CompatibilityDiagnostic>)` |
| `color::parse_css`, `CanonicalColor::{for_mode, write_hex, rgba8}` | `pub(crate)` · `color.rs:102,46,78,61` | **No (0 tests in 939 lines)** | None |
| `geometry::rough_drawing` | `pub(super)` · `geometry.rs:157` | No | None |
| `rough::{Drawing, Path, Point, push_number}` | `pub(super)` · `rough.rs:275,135,120,336` | 2 tests | Add `Debug` (and `PartialEq` on `Path`) |
| `svg::push_rough_path`, `svg::freedraw_path_data` | **private** · `svg.rs:1517,1561` | No | `pub(super)` |
| `freedraw::{generate, project, background_fill}` | `pub(super)` · `freedraw.rs:76,476,111` | No | None |
| `text::OfficialFont::{load, layout}`, `layout_with_fallbacks`, `CjkSubsetTrigger::evaluate` | `pub(super)` · `text.rs:533,588,762,128` | Yes (~9) | None — extend |
| `text::FallbackFont::{load, resolve, resolve_frame_name}` | `pub(super)` · `text.rs:220,235,267` | `resolve_frame_name` only | None |
| `bound_text::resolve_bound_text_placements` | `pub(super)` · `bound_text.rs:20` | No | None |
| `frame_target::{FrameSelection::compute, resolve_frame_clips}` | `pub(super)` · `frame_target.rs:246,174` | No | None |
| `asset::Cache::resolve` | `pub(super)` · `asset.rs:360` | No | None; pass `|_| Ok(())` |
| `asset::{decode_static_png, decode_static_jpeg, decode_webp_default_frame, decode_gif_default_frame, decode_bmp, decode_ico, exif_orientation, orient_rgba, sniff_media_kind, validate_*}` | **private** · `asset.rs` | **No** | Test from `asset.rs`'s own `#[cfg(test)] mod tests` |
| `asset::bmp::{validate, decode}`, `asset::ico::{entries, selected_entry, validate_dib, decode_dib}` | `pub(super)` | **No** | None |
| `asset::svg::sanitize` | `pub(super)` · `asset/svg.rs:84` | **No** | None |
| `ResolvedScene`, `ResolvedElement`, `ResolvedOutline`, `ResolvedTextElement`, … | `pub(super)`, **no derives** · `resolve.rs:5,1153,1256,1238` | No direct tests | Add `#[derive(Debug)]`; add `#[cfg(test)] fn resolve_for_test(scene, request) -> Result<ResolvedScene, RenderError>` in `mod.rs` next to `render_with_trace` that calls `admit_session` → `resolve_scene` → `account_fallback_semantics` and stops before `encode` |
| `SvgPlan::new`, `PngPlan::new`, `PdfPlan::new` | `pub(super)` · `svg.rs:433`, `png.rs:486`, `pdf.rs:200` | No | None (SVG: build with `Payload::CompleteChunks`) |
| `PngPlan::{width,height,frame_clip_masks,…}`, `PdfPlan::raster_closures`, `PdfPlan::page_dimensions` | `pub(super)` | No | None — natural plan-level assertion points |
| `pdf::format_pdf_number`, `rough::push_number` | private / `pub(super)` · `pdf.rs:3042`, `rough.rs:336` | No | `pub(super)` for the former |
| `metadata::ArtifactMetadata::{build, estimate}` | `pub(super)` · `metadata.rs:71,154` | Yes (3) | None |
| `svg_fonts::Payload::CompleteChunks`, `new_font_subset_cache`, `render_with_trace_and_font_subset_cache` | `#[cfg(test)]` | — | Use them |
| CLI: `frame_output_basenames`, `portable_frame_slug`, `truncate_basename`, `is_reserved_device_name` | private · `frames.rs:741,790,807,815` | **No** | `#[cfg(test)] mod tests` in `frames.rs` |
| CLI: `select_format`, `parse_asset_declarations`, `split_asset_option`, `same_path`, `lexical_absolute`, `preparse_json_mode`, `render_json_line` | private · `main.rs` | **No** | Same |

**Plan-level seam verdict.** The right shared seam is `ResolvedScene`, not the
three plans: spec §9 defines it as the format-neutral semantic graph, and it is
the boundary where a single `resolve` replaces a 3-format loop for every
pre-encode behavior. Plans are thin per-format projections; test them only for
projection *policy* via their existing accessors. None of the plan or resolved
types implement `Debug` — a mechanical `#[derive(Debug)]` pass is the
prerequisite. Tests at this level are immune to PNG filter, zlib, brotli/WOFF2
and PDF object-numbering changes, which is what ADR-0002 says the contract
*does* pin — so byte-level checks must stay concentrated in registered parity,
goldens, and encoder unit tests.

---

## 3. Proposed test pyramid

**Tier 1 — End-to-end gates to KEEP (~45 tests):**
- `registered_case_png_parity`, `registered_case_svg_path_parity`, `registered_case_svg_raster_parity`.
- `frame_conformance.rs`, `pdf_backend_qualification.rs`, `pdf_viewer_adapters.rs`, `rectangle_{svg,png,pdf}_render.rs` goldens + visual gates.
- `empty_scene_render.rs` — one golden per format.
- `excalifont_text_render.rs` #1-#3; `missing_glyph`/`emoji_fallback` 3F report tests.
- One `*_in_every_format` per element family (outline, rough fill, arrow, freedraw, bound text, image, nested SVG, fallback, frame).
- `resource_accounting.rs`, `frame_clip_accounting.rs`, the limit-boundary tests — the ADR-0002 charge-order surface.
- `artifact_metadata.rs` round trip + 2 exhaustion tests; `frame_export.rs`; `compatibility_identity`, `outcome_contract`, `render_request`, `license_bundle`, `scene_admission`.
- `tests/package-consumer/tests/public_facade.rs`.
- **New:** one determinism property test rendering each registered case twice per format — replaces the 16-file double-render pattern.

**Tier 2 — Plan/resolve-level (in-crate, ~40 tests):** bound-text placement,
frame name/clip geometry, fallback fit/omission, CJK trigger propagation,
image crop/scale mapping, PDF raster-closure selection, PNG surface
dimensions.

**Tier 3 — Stage unit tests (in-crate, ~250 tests, zero renders):**
`color.rs` (~20), normalization observer/diagnostic content (~35),
relationships (~3), `rough_drawing`/`freedraw` pinned vectors (~20), text
layout/runs/faces (~30), `asset::Cache::resolve` + decoders (~80),
`svg::sanitize` (~15), `FallbackFont::resolve` (~6),
`format_pdf_number`/`push_number` (~4), `sort_diagnostics` (~3), CLI pure fns
(~15).

**DELETE as redundant (after the above exist):** `legacy_line_height_render.rs`,
`frame_render::frame_name_is_truncated…`, `text_line_endings_render.rs` SVG
tests, `normalization_repairs_are_format_stable`,
`static_{bmp,ico,webp}_artifacts_and_reports_are_deterministic`,
`retained_latin_families_render_deterministically_in_png_and_pdf`, and every
"render twice" branch.

---

## 4. Risks of converting

1. **Losing charge-order coverage (ADR-0002).** Stage tests bypass the meter.
   **Do not convert any test whose name contains `charges`, `limit`,
   `crossing`, `boundary`, `admitted`, `preflight`, or that sets
   `ResourceLimits`** — ~45 tests, all cheap.
2. **Stage-interaction behaviors that only exist in the orchestrator.** CJK
   subset trigger evaluated *once per target* (`font_demand.rs:38-55`);
   frame-name synthesis before bounds measurement (`resolve.rs:317-319`);
   embeddable color substitution in the element loop (`mod.rs:1367-1390`).
   Test these at `resolve_for_test`, not at `text.rs`.
3. **Journal replay vs live path.** `resolve_for_test` always takes the live
   path. Frame-target behaviors must keep `frame_export.rs` +
   `frame_batch_property` as-is.
4. **Diagnostic ordering is session-owned.** Tests asserting final order must
   apply the extracted sort or stay E2E.
5. **Pinned digests.** Recomputing Rough.js vectors via `push_rough_path`
   keeps digests unchanged; pinning `Drawing` structs directly would require
   re-deriving pins — avoid.
6. **`--lib` binary growth.** Moving ~300 tests in-crate makes every `src/`
   edit recompile a larger test binary and the wasm `--lib` build. File-reading
   tests must stay integration or be `#[cfg(not(target_arch = "wasm32"))]`.
7. **Derive additions** touch files pinned by approvals
   (`font_subset_qualification.rs`) — keep derives out of pinned files.
8. **Encoder regressions no longer caught by feature tests** — acceptable only
   because encoder-level unit tests exist plus Tier 1 goldens. Don't delete the
   3F smoke tests.

---

## 5. Quick wins vs structural

**Quick wins (< 1 day each)**
- `color.rs` unit tests; drop the 15-render loop at `whole_scene_controls.rs:475-510`.
- Port `helvetica_text_render.rs`, `legacy_line_height_render.rs`, `text_line_endings_render.rs` (SVG parts) to layout-level asserts.
- `frames.rs` unit tests for `frame_output_basenames` replacing the CLI spawn at `render_cli.rs:924`.
- Delete double-render determinism branches; add one property test over registered cases.
- Make `svg::push_rough_path` `pub(super)` and move the pinned Rough.js/freedraw digests in-crate.
- Consolidate the 45 integration binaries into a few `[[test]]` targets — pure link-time win.
- `[profile.dev.package."*"] opt-level = 2` (or `[profile.test]`) for the debug `--workspace` run.

**Structural**
- `#[cfg(test)] fn resolve_for_test` + `Debug` derives → resolve-level tests.
- `asset::Cache::resolve`-level codec suite replacing `rendered_center()`.
- `svg::sanitize`-level suite + `PdfPlan::raster_closures` plan-level suite.
- Normalization observer suite + extracted `sort_diagnostics`.
- Split `render/tests.rs` into submodules.
- Optional: a request-scoped-but-shareable subset cache on `Renderer` — contract-adjacent (trace equality across cold/warm paths must hold); its own decision.
