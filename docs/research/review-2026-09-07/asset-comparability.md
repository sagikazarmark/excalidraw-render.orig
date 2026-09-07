# Rendered-asset comparability review

Method: read the spec/ADRs/research/conformance docs, the SVG/PDF/PNG/font
code, the approved official evidence under `tests/conformance/cases/**`, and
the vendored official package at
`tools/reference-oracle/node_modules/@excalidraw/excalidraw/dist/dev/chunk-4FTI6OG3.js`
(so "official does X" claims below are verified against 0.18.1 source). The
already-built debug CLI was used to render the conformance scenes and obtain
concrete candidate bytes. Line numbers refer to `a1be429`.

**Short verdict.** Geometry, text placement, and the SVG document skeleton are
genuinely comparable — path `d` data is byte-identical to the official export
across all 17 cases, text baselines agree to the last digit, and the
`<!-- svg-source -->`/`<metadata>`/`<defs>` layout mirrors 0.18.1. Font
*appearance* is comparable (subsets are within ~5% of official byte size and
shape identically). Where the artifacts are **not** comparable is size and
font-embedding *policy*: Helvetica text embeds a Liberation subset the official
never embeds, one emoji adds a 1.15 MB unsubsetted Noto Emoji TTF to the SVG
(official: 0 bytes), every PDF embeds complete uncompressed font programs
(62–863 KB each) plus raw uncompressed image pixels, and every text-bearing SVG
carries 5–21 KB of license text in `<metadata>`. Two behavioural gaps are not
covered by any test: image `roundness` clipping is not implemented, and
dark-mode image handling differs from the official filter chain.

---

## 1. SVG structure comparability

| Aspect | Official 0.18.1 | Renderer | Consumer impact |
|---|---|---|---|
| Root | `<svg version="1.1" xmlns viewBox width height [filter="invert(93%) hue-rotate(180deg)"]>` — no `xmlns:xlink` | Same, minus `filter` (dark mode baked into colours, `color.rs:86-99`) | Dark-mode consumers see pre-inverted colours instead of a filter (better for PDF/Inkscape). See §1.1 for images. |
| Preamble | `<!-- svg-source:excalidraw -->`, `<metadata>` (payload comments inside), `<defs>` (clipPaths then `<style class="style-fonts">`) | Identical order (`svg.rs:626-645`); adds `<excalidraw-render:manifest>` JSON node inside `<metadata>` | Excalidraw re-import uses regex on `payload-start/end`, so embedded scene round-trips. The manifest adds 5–21 KB (§2.2). |
| Empty font style | `<style class="style-fonts">` always present | Omitted when no fonts (`svg_fonts.rs:148-150`) | Trivial. |
| `@font-face` | unquoted family, `"Liberation Sans"` quoted; no `unicode-range` | `@font-face{font-family:'Excalifont';src:url(…)}` (`svg_fonts.rs:151-161`); no `unicode-range` | Equivalent. |
| Shape group | `<g stroke-linecap="round" transform="translate(x y) rotate(deg cx cy)">` + Rough `<path>` | Same; fill path omits `stroke-width="0"` (`svg.rs:1443`); adds `<rect fill="transparent">` hit-rect inside linked groups (`svg.rs:376-381`) | None visible. Path `d` is byte-identical. |
| Text | One `<text x y font-family="Excalifont, Xiaolai, Segoe UI Emoji" font-size="20px" …>` **per line** | One `<text data-source-text xml:space="preserve" fill font-family="<single family>" font-size="20" text-anchor>` **per element**, one `<tspan x y>` per line; mixed/RTL lines become one `<tspan>` per glyph run with per-character `x` lists (`svg.rs:1069-1126`) | (a) DOM `textContent` is `"One two threeFour"` — no separator between lines (deliberate, `svg.rs:1119-1124`); (b) no fallback family list: non-browser consumers that ignore `@font-face` fall to their default font for `ExcalidrawRender-9`; (c) per-character `x` lists disable consumer kerning and break converters that read only the first `x`. |
| Fonts named | `Helvetica`, `Cascadia`, `Virgil`, `Lilita One`, `Liberation Sans`, … | `ExcalidrawRender-1/3/7/9` for Virgil/Cascadia/Lilita/Liberation (`svg_fonts.rs:165-173`, ADR-0001 OFL RFN) | Invisible in browsers. Visible as unknown family to any tool not honouring embedded fonts. |
| Links | `<a href>` wrapping element node | Same (`svg.rs:674-678`) | None. |
| Images | `<defs><symbol id="image-<fileId>"><image …/></symbol>` + `<use href="#image-…" [filter=IMAGE_INVERT_FILTER]/>`; crop via `<mask>`; `roundness` via `<clipPath rx>` | Inline `<g transform opacity><image href="data:…"/></g>` per element; crop via nested `<svg viewBox … overflow="hidden">`; flip via inner `<g scale>` (`svg.rs:1145-1239`) | **No dedup**: identical file used N times is embedded N times (image-codecs: 11 `<image>` vs 8 official). **No roundness clip**. **No dark-mode image filter**. |
| Frame clip | `<clipPath id="<frameId>"><rect transform="translate() rotate(<radians> cx cy)" rx ry/>`; children wrapped in `<g xmlns:nsN="http://www.w3.org/2000/svg" nsN:clip-path="url(#id)">` (`setAttributeNS` bug — see 1.2) | `<clipPath id="frame-clip-<sourceIndex>">` with degrees; `<g clip-path="url(#…)">` (`svg.rs:231-295, 424-430`) | Renderer's clip **works in browsers; the official's does not**. |
| Bound-text mask | `<mask id="mask-<elementId>">` | `<defs><mask id="arrow-label-mask-<index>" maskUnits="userSpaceOnUse">` (`svg.rs:707-735`) | None. |
| Frame name/outline | `<text font-family="Helvetica, …" font-size="14px">`, `<rect width="580px" … stroke="#bbb">` | `<text data-frame-name … font-family="ExcalidrawRender-9" font-size="14">`, `<rect data-frame-outline width="580" … stroke="#bbbbbb">` (`svg.rs:300-364, 1003-1025`) | None; baselines match exactly. |
| Number formatting | ECMAScript `Number#toString` | Rust `{}` Display, `-0`→`0` (`svg.rs:1628-1641`) | Rare 1e-15 differences in `rotate()` centres; paths use the official 2-decimal Rough algorithm so `d` is identical. |

### 1.1 Dark mode

Official: root `filter="invert(93%) hue-rotate(180deg)"`; raster images
additionally get `<use filter="invert(100%) hue-rotate(180deg)
saturate(1.25)">` (net ≈ original, contrast compressed to ~[18,237]); SVG
images stay inverted (chunk L17644). Canvas does the same (L8793). Renderer:
colours are lowered via `CanonicalColor::darkened()` (`color.rs:86-99`); image
bytes/pixels are untouched in both SVG and PNG — verified: `image-codecs`
rendered light vs `--dark-mode` gives identical image pixels and identical
`<image>` payloads. Raster images differ slightly (≤~7% per channel) and
**nested SVG images differ completely**. No dark-mode variant exists for
`image-codecs`, so nothing observes this.

### 1.2 Frame clipping — official SVG is unclipped in Chromium, renderer's is clipped

`tests/conformance/cases/focused/frame-clip-mask-sharing/evidence/official.svg`
contains `<g xmlns:ns1="http://www.w3.org/2000/svg"
ns1:clip-path="url(#clip-frame)">`. Chromium does not treat an SVG-namespaced
`clip-path` as the presentation attribute: in the approved `official-svg.rgba`
(Chromium raster) the pixel at (592,80) inside an overflowing rectangle is
`(165,216,255)` (blue ink) while `official-png.rgba` (canvas) is white; the two
official rasters differ by 6,491 px (1.74%). The renderer emits a real
`clip-path` and therefore matches the canvas, not the official SVG. The
resvg-based `registered_case_svg_raster_parity` cannot see this because usvg
accepts attributes in the SVG namespace
(`usvg-0.45.1/src/parser/svgtree/parse.rs:234-236`). This is a real,
consumer-visible format-specific divergence from the official SVG and is not
documented in spec §8.4 (only the radians `rotate()` quirk is) nor in
`tests/conformance/README.md`.

---

## 2. Font embedding

### 2.1 What is embedded, and how (SVG)

- **Official**: per used family, subsets each intersecting `unicode-range`
  chunk with HarfBuzz, re-encodes WOFF2, one `@font-face` per chunk; Xiaolai
  prepended when Excalifont text contains CJK. Families with `metadata.local`
  (Helvetica, Segoe UI Emoji) are **never embedded** (L6893-6895). On any
  subsetting error it silently embeds the complete chunk.
- **Renderer**: same demand model (`font_demand.rs:30-57`,
  `svg_fonts.rs:579-699`). Subsetting is **per-scalar with layout/composite
  closure** via `skera 0.5.1` (`font_subset.rs:170-197`), name IDs 0–6+13+14
  rewritten deterministically (`font_subset.rs:296-393`), WOFF2 via
  `ttf2woff2` q11/glyf-transform/1 thread (`:203-214`), then
  decode-and-validate (`:215-227`). Failure is fatal, never complete-chunk
  fallback (spec 8.3.3). Bundle: 230 official chunks + Noto Emoji 3.003
  **TTF** (862,680 B, `assets-base/src/lib.rs:462-482`).
- Deviations from official policy:
  - **Helvetica (fontFamily 2) → Liberation Sans, embedded** as
    `ExcalidrawRender-9` (`font_demand.rs:237-248`, `text.rs:534-543`).
    Official embeds nothing. Every Helvetica text, frame name,
    embeddable/fallback label and `?` replacement glyph pulls in a Liberation
    subset (3.4–6.9 KB WOFF2).
  - **Emoji → complete Noto Emoji TTF, unsubsetted, `font/ttf`**
    (`text.rs:731-754`, `svg_fonts.rs:700-729`, spec 8.3.1 "never subset").
    One emoji = +1,150,240 base64 chars. Official: 0 bytes.
  - Names: Reserved Font Name aliases in CSS/`<text>`/name table (ADR-0001).
    Correct under OFL §3.

### 2.2 Measured sizes (candidate rendered with the built CLI vs approved official.svg)

| Case | Official SVG | Candidate SVG | `<metadata>` | Embedded fonts (WOFF2 bytes) official → candidate |
|---|---:|---:|---:|---|
| excalifont-text | 3,627 | 9,146 | 5,597 | Excalifont 2,100 → 2,188 |
| helvetica-text | 1,319 | 15,966 | 5,554 | none → ExcalidrawRender-9 6,908 |
| cjk-propagation | 11,496 | 26,270 | 12,957 | Xiaolai 1,984/1,364/1,432 + Excalifont 2,604 → 2,080/1,472/1,524 + 2,716 |
| legacy-line-height | 13,677 | 45,259 | 21,428 | Cascadia 4,296, Nunito 1,748, Excalifont 2,344 → ExcalidrawRender-3 5,928, Nunito 1,780, Excalifont 2,440, **+ ExcalidrawRender-9 5,996** |
| missing-glyph | 8,548 | 21,192 | 10,928 | Excalifont 1,664, Liberation 4,008 → 1,748, 4,472 |
| emoji-fallback | 8,543 | **1,175,224** | 15,293 | + Noto Emoji TTF 862,680 |
| embeddable-links | 6,362 | 21,818 | 5,554 | none → ExcalidrawRender-9 6,688 |
| image-codecs | 12,874 | 13,648 | 0 | 8 `<image>` → 11 `<image>` (no reuse) |

Subset quality is comparable (+4–8%; Cascadia +38%). The size gap is driven by
(i) `<metadata>` license/notice text — the complete OFL body is repeated once
per *font* license identity (4× in legacy-line-height, 47% of the file); (ii)
Helvetica/label embedding; (iii) the emoji TTF. No test measures subset size
against official. Source-vs-subset typographic parity is proven (851
dual-viewer executions, 0 changed px).

### 2.3 PDF font embedding (`pdf.rs:1037-1144`)

Type0/Identity-H → CIDFontType2, `/CIDToGIDMap` stream, `/W` per code,
complete `ToUnicode` bfchar CMap, line-level `/Span << /ActualText >> BDC`.
**Each used chunk's complete reviewed decode is embedded raw**
(`/Length`==`/Length1`, no `/Filter`, `pdf.rs:1103-1112`), not subset, not
renamed (spec 8.6). Measured: excalifont-text 71 KB (61,948 B font),
helvetica-text 146 KB (133,824), legacy-line-height 490 KB
(223,420+133,824+61,948+39,436), cjk-propagation 335 KB (three Xiaolai chunks
72–87 KB, all `/BaseFont /Xiaolai`), emoji-fallback **1,080 KB** (862,680 Noto
Emoji). No subset tag prefix on `/BaseFont`. Text is selectable/searchable;
each glyph is its own `Tm`+`Tj` (`pdf.rs:2164-2193`). PDF/A-ish:
`/OutputIntents [GTS_PDFA1]` + ICC v2 (`:705-725`), XMP without `pdfaid`
(`:740-749`), no `/Info`, no trailer `/ID`.

### 2.4 PNG text

rustybuzz shaping (`text.rs:593`), glyph outlines filled with tiny-skia at 4×
supersampling then downsampled (`png.rs:695-726`), no hinting/gamma/LCD.
Browser canvas uses Skia with its own AA/gamma; the result fails the declared
`geometry/image` class on every text-bearing case and is carried as a tolerance
exception (§3). Encoded PNG is smaller than official (ADR-0004).

### 2.5 Risks

- OFL/RFN: handled correctly for subsets; PDF embeds unmodified decodes so
  original names are permissible.
- CJK: trigger evaluated on stored `text` (renderer) vs `originalText`
  (official); frozen Chromium-149 script table vs live V8. Non-Excalifont CJK
  → `?` replacement in renderer vs container fontconfig fallback in official.
- Fallback chain vs fontconfig: deterministic by design; Helvetica on
  macOS/Windows official viewers renders real Helvetica/Arial.
- Glyph coverage: emoji become monochrome Noto outlines vs colour bitmaps
  (#121, excepted).

---

## 3. Visual parity evidence

17 registered cases: 14 `scene`, 1 `typography`, 1 `frame`, 1 PDF-only.
Variants beyond `default`: freedraw (`scale-2`, `max-dimension-160`),
ellipse-diamond-fills (`dark-mode`), cjk-propagation (`scale-2`,
`max-dimension-200`, `dark-mode`). There is **no** dark-mode variant with
images, Helvetica, emoji, or missing glyphs, and no RTL/bidi case at all.

Every scene variant is declared `geometry/image` (≤1/400 changed px, SSIM
≥0.999). Direct-PNG tolerance exceptions (all `format: png`, `region: whole`,
ceilings = exact observed values):

| Case / variant | W×H | changed px | % | SSIM | Would pass typography class (1/40, 0.995)? |
|---|---|---:|---:|---:|---|
| ellipse-diamond-fills default / dark | 618×478 | 64 / 26 | 0.02 / 0.01 | 0.9958 / 0.9952 | yes |
| arrow-suite | 730×362 | 326 | 0.12 | 0.9972 | yes |
| bound-text-containers | 702×462 | 495 | 0.15 | 0.9945 | **no** (SSIM) |
| bound-text-placement | 400×235 | 64 | 0.07 | 0.9982 | yes |
| cjk-propagation default / scale-2 / max-dim-200 / dark | 340×275 / 680×550 / 200×161 / 340×275 | 188 / 1 / 323 / 167 | 0.20 / 0.00 / 1.00 / 0.18 | 0.988 / 0.995 / 0.979 / 0.988 | no / borderline / no / no |
| embeddable-links | 620×300 | 551 | 0.30 | 0.9947 | no |
| helvetica-text | 240×173 | 170 | 0.41 | 0.983 | no |
| legacy-line-height | 490×220 | 572 | 0.53 | 0.982 | no |
| text-line-endings | 240×220 | 331 | 0.63 | 0.982 | no |
| missing-glyph (#124) | 280×93 | 762 | 2.93 | 0.805 | no — compatibility exception |
| emoji-fallback (#121) | 280×93 | 1,240 | 4.76 | 0.878 | no — compatibility exception |

In budget with no exception: rectangle-outline, freedraw ×3, image-codecs,
frame-clip-mask-sharing. Text rendering sits at SSIM 0.98–0.99 against browser
canvas — perceptually close, but below even the typography floor in 7 of 11
text variants.

Threshold meaningfulness: 1/400 changed pixels alone would not catch a missing
20×20 element in a 300k-px image; the SSIM ≥0.999 floor is what catches both
that and a 1-px stroke offset. Additional non-raster gates are strong:
`registered_case_svg_path_parity` (exact `d` and extents) and
`registered_case_svg_raster_parity` — with the blind spot in §1.2.

---

## 4. PDF comparability

Vector shapes ✔, selectable text ✔, correct glyph shapes ✔, links ✔ (borderless
`/Link /URI` annots, `pdf.rs:994-1030`), page size ✔, ICC/OutputIntent ✔, XMP
manifest ✔, scene attachment ✔. Not meeting typical expectations: **no stream
compression anywhere** — content stream, fonts, ICC, XMP, and images are raw
(`pdf.rs:700-703, 889-918, 1103-1112`; svg2pdf `compress: false` at `:3499`),
although spec §8.6 says "fixed stream filters and Flate settings". Images are
raw RGB + raw gray SMask, so a 1920×1080 screenshot costs ~8.3 MB per
occurrence. Fonts are complete chunk programs, not subsets; three same-named
`/Xiaolai` fonts; no subset-tag prefix; no `/ID`, no `/Info`.

Visual evaluation: PDFium and PDF.js render at 96 DPI and compare against
Chromium's raster of the *official SVG*. Results: rectangle-outline PDFium
exception 0 px / SSIM 0.99761 (#44); excalifont-text PDFium 97 px / 0.98688
(#73); pdf-isolated-fallback 0 px / 0.99855 (#107); frame-export frame 5
(rotated) 112 px / 0.99981 (PDF.js) and 0.99815 (PDFium) (#119). All
`#[ignore]`d. Note the authority problem from §1.2 for frame scenes.

---

## 5. Determinism vs comparability tension

Documented and deliberate: stored-block zlib for the embedded scene (spec
§12.1; measured 3,408 B vs 653 B deflate(6), 5×); own WOFF2 encoder/profile
bytes; own PNG encoder (ADR-0004); own SVG serializer; frozen CJK table; PDF
"fixed semantic object numbering". Undocumented or under-documented: PDF has
*no* Flate despite the spec sentence; the `<metadata>` license payload cost is
not called out anywhere; the `ns1:clip-path` divergence (§1.2) is not recorded;
image non-reuse in SVG is not mentioned.

---

## 6. Findings

| # | Finding | Location | Severity | Test coverage |
|---|---|---|---|---|
| 1 | One emoji embeds the complete 862,680-byte Noto Emoji TTF in SVG and raw in PDF; official embeds nothing. 137× size blow-up. | `text.rs:731-754`, `svg_fonts.rs:589, 700-729`, spec §8.3.1 | high | Behaviour asserted; no size gate |
| 2 | PDF embeds complete chunk decodes uncompressed and never subsets: 62–223 KB per font, 863 KB emoji; Helvetica/labels/frame names always add 134 KB. | `pdf.rs:1103-1112`, spec §8.6 | high | Structure pinned; no size gate |
| 3 | PDF applies no stream filter to content, fonts, ICC, XMP or images, contradicting spec §8.6. | `pdf.rs:700-703, 889-918, 3499` | high (size) / med (spec drift) | Digest-pinned only |
| 4 | Frame clipping: official SVG serializes `nsN:clip-path` which Chromium ignores → official SVG renders unclipped, canvas clips; renderer clips. Undocumented and invisible to the resvg harness. | `svg.rs:285-295`; `frame-clip-mask-sharing/evidence/official.svg` | med (docs/authority) | Hidden by `registered_case_svg_raster_parity.rs:262-321` |
| 5 | Image `roundness` not implemented. | `resolve.rs:1205-1217`, `svg.rs:1128-1240` | med-high | None |
| 6 | Dark mode: images left untouched. | `color.rs:86-99`, `svg.rs:1208-1211` | med | None |
| 7 | Helvetica substituted by an embedded Liberation subset; official never embeds Helvetica. Consumers without `@font-face` see `ExcalidrawRender-9`. | `font_demand.rs:237-248`, `svg_fonts.rs:165-173` | med | Diagnostic pinned; no size gate |
| 8 | `<metadata>` repeats the full OFL text once per font license identity (47–61% of text-bearing SVGs). | `svg_fonts.rs:730-742`, `notices.rs` | med (size) | Presence tested; size not |
| 9 | Images inlined per element, never shared via `<symbol>/<use>`. | `svg.rs:1208-1232` | med | None |
| 10 | Text-bearing PNGs fail the declared class on every case; 7/11 variants below the typography SSIM floor. | `registry.json`; `png.rs:695-726` | med (tracked) | Exact ceilings held |
| 11 | SVG `<text>` concatenates lines with no separator; no fallback family list; per-character `x` lists. | `svg.rs:1069-1126` | low-med | Concatenation asserted |
| 12 | No RTL/bidi conformance case. | `text.rs:778-819`, `svg.rs:1102-1117` | med | Unit tests only |
| 13 | Three embedded programs share `/BaseFont /Xiaolai`; no subset tag; no `/ID`, no `/Info`; XMP lacks `pdfaid`. | `pdf.rs:1059-1075, 925-933, 740-749` | low | Structure digests only |
| 14 | Stored-block zlib for the embedded scene ~5× larger than pako. | `metadata.rs`, spec §12.1 | low (documented) | Codec tests |
| 15 | `<style class="style-fonts">` omitted when no fonts; `stroke-width="0"` omitted; 1e-15 number differences in `rotate()`. | `svg_fonts.rs:148-150`, `svg.rs:1443`, `svg.rs:1628-1641` | low | Path parity covers `d` |
| 16 | CJK trigger uses stored `text` and a frozen table; non-Excalifont CJK falls to `?`. | `assets-base/src/lib.rs:118-231` | low (documented) | cjk-propagation, #124 |
| 17 | Diagnostic message for ordinary Helvetica text reads "generated fallback label used bundled Liberation Sans". | `helvetica-text/expected-report-svg.json` | low (cosmetic) | Pinned as-is |
