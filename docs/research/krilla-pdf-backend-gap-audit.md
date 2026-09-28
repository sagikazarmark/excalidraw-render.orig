# Krilla PDF backend gap audit

A throwaway spike that asks one question ahead of ADR-0021's port: what can
Krilla 0.8.2 and krilla-svg 0.8.1 not express the way the PDF backend's
specification, ADRs and qualification evidence require? It is an audit, not a
candidate: nothing here is pinned, and no expectation moved.

## Why Krilla at all

svg2pdf is deprecated, and 0.13 is the standalone converter on usvg 0.45 and
pdf-writer 0.12, not a wrapper over Krilla. Since #209 the renderer's trees are
usvg 0.47, so every nested-SVG tree crosses a write-and-reparse bridge
(`render/pdf/nested_svg.rs`, `svg2pdf_tree`). krilla-svg 0.8.1 depends on
exactly the renderer's generation: usvg and resvg 0.47.0, tiny-skia 0.12.0,
fontdb 0.23.0.

Krilla has no counterpart to `svg2pdf::to_chunk`. Its only output is
`Document::finish() -> Vec<u8>`, a complete file, and krilla-svg draws only onto
a Krilla `Surface`. Krilla also writes through pdf-writer 0.15, where the
renderer's splice (`renumber_svg_chunks`) is typed on 0.12. Using krilla-svg
therefore means the whole PDF writer moves onto Krilla. Grafting objects out of
a separate Krilla file was not pursued: it would parse and renumber Krilla's
output, which is more fragile than the bridge it replaces.

## Method

The spike is `krilla-pdf-backend-gap-audit/spike.rs` with
`spike-Cargo.toml`, a standalone crate outside the workspace. It has three
modes:

- `svg`: parses the renderer's own SVG export of a scene with usvg 0.47, loading
  the fonts from the export's WOFF2 `@font-face` data URLs (decoded with the
  workspace's `wuff`), and draws it with krilla-svg on a page of the renderer's
  PDF size (SVG pixels × 0.75). It uses the renderer's viewer image resolver
  (PNG and nested SVG only). This approximates a Krilla backend closely enough
  to judge drawing, but carries no links or text tagging, since the SVG does not.
- `probe`: drives Krilla's API directly. It draws text in the official
  Liberation Sans (`pdfjs-dist` standard font, 139,512 bytes), adds a URI link
  annotation, sets metadata, and optionally configures PDF/A-2b.
- `actualtext`: draws three lines, each inside a `ContentTag::Span` carrying
  its source line as `/ActualText`, with Krilla's tagging on or off. The last
  line draws the glyphs "ABC" with the actual text "XYZ", so an extractor that
  returns "XYZ" is provably reading the span.

`pdfium-text.py` extracts page text through the harness's pinned
`libpdfium.so`. `wasi-run.mjs` runs the spike built for `wasm32-wasip1` under
Node's WASI.

The inputs are the three backend qualification scenes: `project-operations`,
`nested-native-operations`, and the isolated blur fallback. The current PDFs
come from the renderer on `deps/209-usvg-0.47`. Both were judged by the
repository's own tools: the qpdf structural inspector (`run-structure.sh`
against the current expectations) and the pinned PDFium and PDF.js adapters
at 96 DPI.

## Findings

### Compatible

- **Drawing.** Where the inputs match, the pages match. `project-operations`
  differs in 0.4% of pixels in PDFium (15 pixels by more than 32 levels) and
  1.2% in PDF.js. The differences are anti-aliasing at one raster image's edge
  and a speck after "PDF text". Shapes, dashes, patterns, gradients, clips,
  masks, blend modes and text all render the same.
- **Determinism across runs, compilers and targets.** Every Krilla file was
  byte-identical on a second render. All five outputs (the three scenes, the
  probe and the ActualText file) were identical between native x86_64 and
  `wasm32-wasip1`, both built with Rust 1.97.1. The three scene files were
  also identical between native builds on Rust 1.97.1 and 1.98.1. The spike
  measures wasm through WASI, not the browser target, but the arithmetic is
  the same wasm32 code. The full corpus is still the real canary (ADR-0002).
- **Line-level `/ActualText`.** With tagging on (Krilla's default), a span
  writes `/Span<</MCID n/ActualText(…)>>BDC` around its line. PDFium's text
  extraction returns `XYZ` for the mismatch line, so it reads the span, and the
  other lines come back as their source text. With tagging off, Krilla writes
  no span, and PDFium returns the glyphs' `ABC`. The Krilla render of
  `project-operations` extracts the same text as the current PDF
  (`Frame`, `PDF text`, `https://example.com/embed`). qpdf `--check` is clean on
  every Krilla file. pdf.js ignores `/ActualText` for both producers, so it
  cannot judge this.
- **Links.** `LinkAnnotation` with a URI action produces the `/Subtype /Link`
  and `/URI` structure the expectations name.
- **Type0 fonts with ToUnicode.** Text is embedded as `Type0` over
  `CIDFontType2` with `Identity-H` and a `ToUnicode` CMap.
- **Flate.** Every stream is zlib at level 6, the level ADR-0008 fixed, but
  through flate2 on miniz_oxide 0.9.1, not the pinned 0.8.9.
- **ICC tags.** Krilla's output-intent profile (sRGB, ICC v4.2, 480 bytes)
  carries all seven `required_icc_tags`.
- **Size.** `project-operations` is 21,598 bytes, down from 226,887, mostly
  from font subsetting (below).

### Conflicts with earlier decisions

Items 1–4 are settled under [Decisions](#decisions).


1. **Krilla always subsets embedded fonts** (`text/cid.rs:237`, no option). #120
   and ADR-0008 allow PDF to embed only the reviewed decodes, and foreclose
   subsetting. The subset also keeps the Reserved Font Name: the official
   program became `RTBOJP+LiberationSans`. ADR-0001 requires a Modified Version
   to replace `Liberation` with `ExcalidrawRender-9`. Options:
   - Hand Krilla the renderer's own SVG-path subsets. They are already
     ADR-0001 Modified Versions with renamed name tables, and Krilla's subset of
     them keeps that name. This amends #120 for PDF.
   - Ask upstream for an embed-without-subsetting option.
2. **No custom XMP.** `Metadata` exposes only fixed fields (title, creator,
   producer, dates, document ID and a few more). The renderer's packet carries
   the `er:manifest` property (`urn:excalidraw-render:pdf:metadata:1`) with
   the mandatory font notices and the OFL body. Options: an upstream hook for
   extra XMP properties, or carrying the manifest as an embedded file, which
   changes spec §8.6.
3. **`/Info` and `/ID`.** Krilla always writes an `/Info` dictionary; ADR-0008
   deliberately has none. Its trailer `/ID` is two different base64 strings
   from Krilla's own stable hash (seeded by `Metadata::document_id` when set).
   ADR-0008 derives both equal entries from the SHA-256 of the preceding bytes.
   Both are spec §8.6 changes unless upstream adds control.
4. **Output intent only with a PDF/A claim.** Krilla writes `/OutputIntents`
   only when a PDF/A validator is configured. It then also writes the `pdfaid`
   schema and refuses to finish without a creation date (`MissingDocumentDate`).
   The renderer has a `GTS_PDFA1` intent without claiming PDF/A (ADR-0008), and
   no timestamps. The choices:
   - Claim PDF/A with a fixed date. That date is invented metadata, and a
     real clock breaks determinism.
   - Drop the output intent and draw in ICC-based colour (`no_device_cs`).
     Either is a spec change.

   Krilla's profile is its own; the renderer's is not used.
5. **Colour spaces.** The renderer draws in `ICCBased` spaces over its own
   profile. Krilla draws `DeviceRGB` by default, or its own sRGB ICC space with
   `no_device_cs`. The structural operator minimums (`CS`, `SCN`, `cs`, `scn`)
   change either way.
6. **Filters.** krilla-svg rasterizes a filtered group itself, with its own
   resvg at `filter_scale`. That bypasses ADR-0009's single closure plan, which
   owns admission, diagnostics, rasterization and embedding. It is not a
   blocker: the renderer already lowers every raster closure to PNG content
   before conversion (`prepared.lower`), so krilla-svg would see no filter. The
   isolated fallback, which the spike let krilla-svg rasterize at
   `filter_scale: 1.0`, differs in 14% of pixels; that is the expected cost of
   bypassing the plan, not a Krilla defect.

### Differences the port accepts

- **Spans carry MCIDs.** The renderer writes `/Span << /ActualText … >> BDC`
  with no MCID and is not a tagged PDF. Krilla's spans always carry an MCID, and
  without a tag tree the file has no structure tree for them to point into.
  Viewers and extractors ignore the unused MCIDs. Giving the document a real
  tag tree would make it a tagged PDF, which is an accessibility feature to
  decide separately.

### Needs changing in the port

- **The structural inspector.** It classifies images by the renderer's resource
  names (`Im0`, `Svg0/xo…`), so every Krilla image reads `undeclared`. It also
  reads `/OutputIntents` only as a direct array (`xtask/src/pdf_harness/inventory.rs:685`),
  and Krilla writes it as an indirect one, so it reported 0 intents for a file
  that has one. Both need changing with the expectations.

### Spike artifacts, not gaps

- The nested `ABC` text in `nested-native-operations` differs because that
  scene's export carries no fonts, so the spike's usvg fell back to another
  face. The renderer's nested-SVG font database would supply the right one.
- Operation counts, form counts and the digests all differ. The port rewrites
  every structure expectation regardless, under ADR-0019 review.

## Artifacts

Under `target/tmp/krilla-spike/` (not retained):

| file | SHA-256 |
| --- | --- |
| `project-operations.krilla.pdf` | `b1ce4d62e20fdbc229546e98e8079cb27f03dc29fe64054898b5b8cd367749fb` |
| `nested-native-operations.krilla.pdf` | `ffbc413d9dbebc2ecf50d7f61fc22b9ea99bbbbc1ac23f199a7f19e4baea6c70` |
| `isolated.krilla.pdf` | `89deff9822e293cd81594ff179e3cdae4bdb05afafdc1876c20371c0dc27787e` |
| `probe.plain.pdf` (repeat identical) | `a45ae903ae3eb3bcbd2522c924e965c2a23038bf475e9c2acf596357f3038df9` |
| `probe.pdfa.pdf` | `f0b398eb1b3685b27ac213e718b85ad4de9db4397cf5aa830091ec645690869b` |

| `actualtext` (native = wasm32) | `60986d54c15737da95bf3824917f2991f7d6c25c7062b8fa90a7f43a036510c0` |

## Decisions

The four conflicts are settled by spec changes reviewed with the port, without
upstream requests (ADR-0021):

1. PDF embeds the renderer's own font subsets, the ADR-0001 Modified Versions
   the SVG path already produces, and lets Krilla subset them further.
2. The artifact manifest moves out of XMP into an attached file beside
   `scene.excalidraw`.
3. Spec §8.6 requires deterministic, timestamp-free document metadata, not a
   particular `/Info` or `/ID` derivation.
4. The output intent is dropped, and colour is drawn in Krilla's ICC-based sRGB
   (`no_device_cs`).

## Outcome in the port

The port settled some points differently from what the audit assumed:

- **Fonts.** The SVG path's subsets are not reused. PDF embeds each reviewed
  decode with only its names rewritten (the same effective alias and legal
  records), and Krilla subsets that program. Each program carries its own
  mandatory notice.
- **krilla-svg.** Not a dependency. Its `tiny-skia` and `resvg` default
  features would reach the PNG rasterizer, so its drawing is ported as
  `render/pdf/svg_draw.rs`. Spread gradients are lowered to padded ones,
  because PDF.js mis-samples the function shadings Krilla writes for them.
- **`/Info`.** Krilla writes it only when metadata is set. The renderer sets
  none, so the port's PDFs have no `/Info`, and the trailer `/ID` entries are
  equal.
- **Spans.** The port writes a real tag tree (paragraphs, line spans,
  artifacts), so its MCIDs are referenced.

ADR-0021 records the result.
