# Spec-coverage audit

Read-only audit at `a1be429`. `R/` abbreviates `crates/excalidraw-render/src/`,
`T/` abbreviates `crates/excalidraw-render/tests/`.

## 0. Frame of reference

| Item | Value | Source |
|---|---|---|
| Pinned reference | `@excalidraw/excalidraw` **0.18.1**, commit `a2ec288…`, Rough.js 4.6.4, perfect-freehand 1.2.0, points-on-curve 1.0.1, fractional-indexing 3.2.0, sanitize-url 6.0.2, pako 2.0.3 | `tools/reference-oracle/package.json:14`, `tests/oracle/reference-profiles/revision-1.json`, spec §3 |
| Spec | `docs/renderer-spec-v1.md` v1.2.1 — delegates every field/value disposition to the scene-surface manifest `tests/oracle/scene-surfaces/revision-1.json` (202 dispositions, 13 element kinds, 42 diagnostic codes, 7 normalization rules, 5 official fallback recipes) | spec §1, §3.1 |
| Official-evidence corpus | 18 registered cases under `tests/conformance/cases/focused/` | `tests/conformance/cases/registry.json` |

Correction to a common assumption: **the official 0.18.1 exporter does render
frame names in whole-scene export** (`prepareElementsForRender` →
`addFrameLabelsAsTextElements` when `frameRendering.enabled &&
frameRendering.name`, default both `true`). It suppresses names/outlines only
for `exportingFrame`. Spec §8.1 and the implementation
(`R/render/resolve.rs:317-365`) are correct on this point.

Status legend: **I+T** implemented + tested · **I+E** implemented + tested
against official evidence · **I−T** implemented, untested · **P** partial ·
**M** missing · **D** roadmap-deferred.

---

## Part 1 — Coverage of `docs/renderer-spec-v1.md`

### §4 Scene admission & preservation

| Requirement | Implementation | Tests | Status |
|---|---|---|---|
| Envelope validation; duplicate keys rejected | `R/scene.rs:100`, `:758`, `:947` | `T/scene_admission.rs` (8) | I+T |
| Retain all occurrences, no nested validation at parse | `R/scene.rs` | `scene_with_null_elements`, `scene_with_number_elements_and_future_string` | I+T |
| §4.2 canonicalized scene | `R/render/metadata.rs` | `T/artifact_metadata.rs` | I+T |

### §5 Compatibility normalization

| Requirement | Implementation | Tests | Status |
|---|---|---|---|
| Missing ID derivation; duplicate-ID rekey; missing timestamp → 0; zero seed → derived nonzero; cycle removal | `R/normalization.rs`, `R/relationships.rs:138` | `T/normalization.rs`, `T/relationships.rs` | I+T |
| Negative dimension shift (silent) | `R/normalization.rs:1492-1503` | `T/normalization.rs` | I+T |
| Linear point origin normalization | `R/normalization.rs:1970-1980` | lib tests | I+T |
| Fractional-index repair (v3.2.0 vectors) | `R/normalization.rs:2821-3175` | lib + `T/normalization.rs` | I+T |
| Legacy `draw` → `line` | `R/scene_surface.rs:243-263`, `R/normalization.rs:1248` | `T/freedraw_render.rs` | I+T |
| Legacy `boundElementIds` → `boundElements` | `R/relationships.rs:542-555` | indirect | I−T |
| **Legacy `strokeSharpness: "round"` → roundness** (manifest `/strokeSharpness` "supported", normalization: *"Legacy round/sharp values migrate to the corresponding roundness record"*) | **Not found.** `normalized_roundness` (`R/normalization.rs:3320-3350`) reads only `roundness`; `strokeSharpness` appears only in `R/scene_surface.rs:818` as a validated-value path | none | **M** (gap G1) |
| **Legacy `font: "20px Virgil"` → fontSize/fontFamily** (manifest `/font` "supported") | **Not found** | none | **M** (gap G2) |
| §5.1 unknown property → `compatibility.unsupported-property` | `R/scene_surface.rs:353-420, 1059` | `T/scene_surface_policy.rs` (21) | I+T |
| Wrong-typed known property → `unsupported-property-value` + manifest fallback | `R/normalization.rs:1651-1848`, `R/scene_surface.rs:600-660` | many | I+T |
| Unknown element → placeholder + one diagnostic | `R/scene_surface.rs:149-166, 266-344` | `T/compatibility_fallback_render.rs` (23) | I+T |
| `selection` filtered silently | `R/scene_surface.rs:183-185` | yes | I+T |
| Deleted elements silent | `R/normalization.rs:654-660` | yes | I+T |

### §6 Public API / §7 CLI

All requirements implemented and tested (`T/compatibility_identity.rs`,
`T/outcome_contract.rs`, `T/render_request.rs`, `T/frame_export.rs`,
`crates/excalidraw-render-cli/tests/render_cli.rs`).

### §8.1 Shared controls

| Requirement | Implementation | Tests | Status |
|---|---|---|---|
| Scale/MaxDimension sizing rules | `R/render/resolve.rs:1141-1150`, `R/render/svg.rs:609-617` | `T/whole_scene_controls.rs`; evidence variants | I+E |
| Frame name as synthesized Helvetica-14/1.25 text | `R/render/resolve.rs:317-365, 398-417, 592-611`, `R/render/text.rs:21-25, 267-315` | `T/frame_render.rs` (12); evidence `frame-clip-mask-sharing` | I+E |
| Frame outline radius 8, `#bbb`, width 2 | `R/render/geometry.rs:61-70`, `R/render/svg.rs:1000-1024` | same case | I+E (light mode only — G3) |
| Frame export: rotation-aware AABB, zero padding, no name/outline, membership filter, clip | `R/render/frame_target.rs`, `R/render/mod.rs:1326-1343` | `T/frame_export.rs`, `T/frame_conformance.rs` | I+E |
| Colors: Chromium CSS grammar | `R/color.rs`, `R/scene_surface.rs:596` | `T/whole_scene_controls.rs` | I+T |
| Links: pinned sanitizer, unsafe omitted | `R/scene_surface.rs:828-945`, `R/render/svg.rs:674-678` | `T/compatibility_fallback_render.rs`; evidence `embeddable-links` | I+E |
| Embeddable/iframe offline shell + label | `R/render/mod.rs:1367-1389`, `R/render/resolve.rs:822-908` | evidence (embeddable only) | I+E / iframe I+T |

### §8.2 Raster & nested SVG

| Requirement | Implementation | Tests | Status |
|---|---|---|---|
| PNG/APNG, JPEG, GIF, WebP, BMP, ICO; AVIF → `image.unsupported-format` | `R/render/asset.rs`, `asset/{bmp,ico,svg}.rs` | `T/static_*_asset_render.rs` (94); evidence `image-codecs` | I+E |
| SVG passthrough of exact octets | `R/render/svg.rs:1206-1210` | per codec | I+E |
| Frame selection, orientation, ICC/CICP/gain-map, premultiply | asset modules | per codec | I+T |
| Nested SVG sanitization | `R/render/asset/svg.rs` | `T/nested_svg_asset_render.rs` (45) | I+T |
| Missing image recovery | `R/render/image.rs`, `R/render/png.rs:1090-1135` | yes | I+T |
| Image `crop` and `scale` flip | `R/render/resolve.rs:736-749`, `R/render/svg.rs:1170-1216` | evidence | I+E |

### §8.3 Typography

| Requirement | Implementation | Tests | Status |
|---|---|---|---|
| Bundled families; no host fonts | `R/render/text.rs`, `R/font_metadata.rs:41-62` | `T/font_catalog_render.rs` (19); evidence for families 2,3,5,6,9 only | I+T (1,7,8 no evidence) |
| Liberation/Noto substitution diagnostics | `R/render/text.rs` | yes + evidence | I+E |
| Missing glyph → `?` + `font.missing-glyph` | `R/render/text.rs:762+` | `T/missing_glyph_render.rs` | I+E |
| Metadata metrics baseline | `R/font_metadata.rs` | `T/helvetica_text_render.rs` | I+E |
| Restored `lineHeight` | `R/normalization.rs:1594-1628` | 8 unit tests; evidence | I+E |
| Line splitting `\r\n?` | `R/render/text.rs` | evidence | I+E |
| Bidi (UAX#9), RTL runs | `R/render/text.rs:762-960` | lib tests | I+T (no official evidence) |
| CJK trigger once per target over stored `text` | `R/render/resolve.rs:222-234, 564` | evidence `cjk-propagation` | I+E |
| §8.3.2/8.3.3 subset profile, aliases, validation | `R/render/font_subset.rs`, `svg_fonts.rs` | many | I+T |

### §8.4 SVG · §8.5 PNG · §8.6 PDF

| Requirement | Implementation | Tests | Status |
|---|---|---|---|
| SVG preamble, `<metadata>`, `<defs>` order, one `<style>` | `R/render/svg.rs:609-645`, `svg_fonts.rs:151` | `T/artifact_metadata.rs` | I+T |
| Clip IDs keyed by element index; radius 8 in scene export | `R/render/svg.rs:235-265` | evidence | I+E |
| PNG direct typed plan | `R/render/png.rs` | yes | I+E |
| PDF: 1 page, 0.75pt/px, ICC + OutputIntent, XMP, attachment | `R/render/pdf.rs:14, 594-770, 2862` | `T/rectangle_pdf_render.rs`, `T/pdf_backend_qualification.rs` | I+T (viewer gates need harness) |
| Pinned reductions: rough 2 decimals, freedraw token trimming | `R/render/rough.rs:336-406`, `R/render/freedraw.rs` | yes | I+T |
| **Image viewport w/h official integer rounding** (spec §8.6) | **Not found** — `R/render/svg.rs:1206-1231` and `pdf.rs` emit unrounded | none | **M** (gap G6) |
| Raster fallback closure semantics | `R/render/pdf.rs` | `T/nested_svg_asset_render.rs::pdf_*` (17) | I+T |
| Reviewed decodes, `FontFile2` byte-for-byte | `R/catalog.rs`, assets-base | yes | I+T |

### §9–§16

Architecture layers, Rough.js/perfect-freehand ports, `unsafe_code = "forbid"`,
catalog/licenses, empty report byte form, diagnostic order, fallback grammar,
manifest carriers, limits table and crossings, canonical trace: all **I+T**.
Performance protocol and WASM footprint: **D** (roadmap). Supported targets
narrowed to Linux + WASM: ADR-0006.

### Spec-declared exclusions & compatibility exceptions

All declared exceptions (AVIF, Helvetica → Liberation, Segoe UI Emoji → Noto
Emoji, missing glyph, unknown element placeholder, live embeddables off, zero
seed, missing timestamp, unrecognized `fontFamily`, clip rotate degrees vs
radians, PDFium antialiasing #44, rotated-frame partial row #119) are diagnosed
and tested. The gaps below are *undeclared*.

---

## Part 2 — Excalidraw 0.18.1 format & exporter behavior vs. implementation

### Element kinds

| Kind | Official 0.18.1 static export | Renderer | Class |
|---|---|---|---|
| rectangle / ellipse / diamond | Rough.js shapes | `R/render/geometry.rs:176-358` | fully handled (evidence) |
| line | `linearPath` / `polygon` if loop && fill / `curve` if roundness | `geometry.rs:359-427` | handled, **no official evidence** |
| arrow (+12 arrowheads) | `getArrowheadPoints` | `geometry.rs:514-845` (all 12 incl. crowfoot) | fully handled (evidence `arrow-suite`) |
| elbowed arrow | `generateElbowArrowShape(points,16)` | `geometry.rs:438-512` | fully handled |
| freedraw | perfect-freehand 1.2.0 | `R/render/freedraw.rs` | fully handled (evidence) |
| text | stored wrapped `text`; `verticalAlign` **not used** by export | `R/render/text.rs` | fully handled |
| image (`fileId`, `status`, `scale`, `crop`) | `<symbol>/<use>`, `Math.round(w/h)`, crop viewBox, rounded-corner clip when `roundness` | crop/scale `resolve.rs:736-768`; **no `Math.round`**; **no roundness clip** | **gaps G4, G6** |
| frame / magicframe | outline `#bbb`/r8/w2; name text; clip children | `resolve.rs:587-637`; PNG magic recolor `png.rs:893-899` (light color regardless of mode) | handled; dark-mode gap G3 |
| iframe / embeddable | rough rect + centered Helvetica label, painted last | `mod.rs:1367-1389`, `resolve.rs:822-951` | fully handled |
| selection | filtered by restore | `scene_surface.rs:183-185` | fully handled |
| unknown type | `renderElementToSvg` throws | placeholder + diagnostic | compat exception (declared) |

### Style fields

| Field | Official | Renderer | Class |
|---|---|---|---|
| `strokeStyle` solid/dashed/dotted | dash `[8, 8+sw]` / `[1.5, 6+sw]` | `svg.rs:1491-1503`, `resolve.rs:921-930` | fully handled |
| `fillStyle` hachure/cross-hatch/solid/zigzag | Rough.js fillers | `rough.rs:329-334` | fully handled |
| `fillStyle` zigzag-line / dots / dashed | restore does **not** validate → Rough.js renders them | domain rejects → `unsupported-property-value`, substitute `solid` (`normalization.rs:1525-1531, 1669-1676`) | diagnosed exception; visually farther than official (G8) |
| `roughness` 0/1/2 + `adjustRoughness` | size rule | `mod.rs:1521-1528` | handled; roughness=2 **no official evidence** |
| `roundness` type 1/2/3 | `getCornerRadius` | `normalization.rs:3343-3346`, `geometry.rs:925-931` | handled; **no official evidence for any roundness** |
| `opacity` | per-primitive opacity × frame opacity | `resolve.rs:630, 724, 932`, `frame_target.rs:213-226` | handled; **no official evidence** |
| `angle` | `rotate(deg cx cy)` | `ResolvedTransform` | handled; only one rotated frame in evidence |
| `groupIds`, `locked`, `index`, `version*`, `updated`, `customData` | no rendering effect | relationships only | fully handled |
| `link` | `<a href>` wrap; PDF URI annots | `svg.rs:674-678` + transparent hit rect | fully handled |
| `boundElements`/`containerId` | container-then-label order; arrow midpoint | `R/render/bound_text.rs` | fully handled (evidence) |
| `frameId` → clip, frame opacity | immediate frame only | `frame_target.rs:213` | fully handled |

### appState / export options

| Field | Official | Renderer | Class |
|---|---|---|---|
| `viewBackgroundColor`, `exportBackground:false` | | `normalization.rs:616`, `svg.rs:646-660` | handled |
| `frameRendering` (scene ∧ request) | honored | `mod.rs:1327-1334` | handled |
| `exportWithDarkMode`, `exportScale`, `exportEmbedScene`, `exportBackground` **from the scene** | `exportToSvg(…, appState)` reads them | **request-only**; scene values validated but never consumed | by design; asymmetry with `frameRendering` (G9) |
| `exportPadding` default 10 | | `request.rs:416` | handled |
| Dark mode = root `filter: invert(93%) hue-rotate(180deg)`; raster images additionally `invert(100%) hue-rotate(180deg) saturate(1.25)`; SVG-mime images get only the root filter | | Lowered into colors `color.rs:86-99` for bg, strokes, fills, text, labels, frame names, fallbacks. **Not applied to**: frame outline `#bbb` (`svg.rs:1011`, `png.rs:896-899`, `pdf.rs:1820`), image pixels (`png.rs:960-1089`), nested-SVG images, placeholder glyph `#888` (`png.rs:1128`); placeholder bg hard-coded `#2e2e2e` (`png.rs:645`) | **G3, G5** |
| `exportEmbedScene` payload | pako deflate | stored-block zlib (spec 12.1) | handled |
| Fonts `<style class="style-fonts">` | | own subsets, aliases for RFN families | handled |

### `files`

`dataURL` sniffed type wins over `mimeType`, malformed record → recipe,
unreferenced silent, supplemental fills absent IDs only — all handled and
tested.

---

## Prioritized gap list

### (c) Actual gaps — silent, no diagnostic, no test

| # | Gap | Where | Impact |
|---|---|---|---|
| **G1** | `strokeSharpness: "round"` on elements without `roundness` is not migrated to `{type: 1}` (`ROUNDNESS.LEGACY`; rectangle/image/embeddable/iframe) or `{type: 2}` (`PROPORTIONAL_RADIUS`; others) as `restoreElementWithProperties` does — `getCornerRadius` treats both as the proportional radius. Corners render sharp, arrows/lines straight. Manifest promises the migration. Verified in `tests/public-scenes/`: `antibody-amino-acid-chain` (84 live elements: rect, diamond, arrow, text), `antibody-contact-maps` (50: ellipse, line, rect, text), `perfect-freehand-strokes` (12: arrow, freedraw). | `R/normalization.rs:3320-3350` reads only `roundness`; `scene_surface.rs:818` only validates the string | High |
| **G3** | Dark mode not applied to frame outline (`#bbb` → ≈`#4c4c4c`), magic-frame PNG outline, or PDF frame outline. | `svg.rs:1011`, `png.rs:893-901`, `pdf.rs:1820`; `geometry.rs:66` constant never `for_mode`'d | Medium |
| **G4** | Image `roundness` is not applied (official clips to `getCornerRadius(min(w,h))` rounded rect). | `ResolvedImage` has no roundness field (`resolve.rs:1205-1217`); `corner_radius` only reached from rect/diamond (`geometry.rs:190,277`) | Medium-High |
| **G5** | Dark mode not applied to image pixels: official net effect on raster images is `0.07 + 0.86·saturate(1.25)(c)`; nested-SVG images are fully inverted. Spec §8.6 requires "Dark mode is lowered into vector colors *and image pixels*". PNG placeholder glyph `#888` not darkened; placeholder bg `#2e2e2e` ≠ filter result `#262626`. | `png.rs:960-1089, 1128, 645`; no `color_mode` use in `image.rs` | Medium |
| **G6** | Spec §8.6 pinned reduction "image viewport width and height use official SVG integer rounding" unimplemented in SVG and PDF. | `svg.rs:1206-1231`, `pdf.rs` | Low-Medium |
| **G2** | Legacy `font: "<px> <family>"` not migrated. | no handling | Low |
| **G7** | Frame `<g>` gets `opacity` in SVG (`svg.rs:993-996`); official SVG frame `<rect>` may carry no opacity. Needs verification against 0.18.1 `staticSvgScene.ts`. | `svg.rs:993` | Low |

### (a) Diagnosed compatibility exceptions worth revisiting

| # | Item |
|---|---|
| **G8** | `fillStyle` `zigzag-line`/`dots`/`dashed` are Rough.js-renderable and pass official restore unvalidated; renderer substitutes `solid` with a warning. Consider porting the three fillers or substituting `hachure`. |
| G9 | Scene `appState.exportWithDarkMode/exportScale/exportBackground/exportEmbedScene` never consumed (request-only) while `frameRendering` is ANDed with the request — inconsistent with the manifest's identical wording for all five. |

### (b) Evidence-coverage gaps

The 18-case corpus contains **zero** elements with `roundness` (the editor
default for rectangles and arrows), `opacity < 100`, `angle ≠ 0` (except one
frame), `roughness = 2`, `strokeWidth ∉ {1,2}`, `line` type, `groupIds`,
`iframe`, `fontFamily ∈ {1,7,8}`, `exportBackground:false`,
`exportEmbedScene`, RTL/bidi text, or any dark-mode variant containing frames
or images. `T/outline_shape_render.rs:98-110` records that official evidence
already caught one snapshot-only bug (diamond vertex placement), which is the
concrete risk for these families. Recommended new focused cases: rounded
rectangle+diamond (type 3 default and explicit `value`, type 1 legacy), curved
multi-point line/arrow (type 2), rotated+semi-transparent shapes, cartoonist
roughness, `strokeSharpness`-only legacy scene, rounded image, dark-mode
frame+image, RTL/bidi.

### (d) Roadmap deferrals (correctly non-normative)

Performance protocol, canonical WASM footprint (`docs/roadmap.md`);
macOS/Windows targets (ADR-0006); PDF/SVG viewer gates remain `#[ignore]`
pending pinned harnesses.
