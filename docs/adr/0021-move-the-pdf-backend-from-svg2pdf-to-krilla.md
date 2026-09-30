---
status: accepted
---

# Move the PDF backend from svg2pdf to Krilla

svg2pdf is deprecated, and 0.13, the release the PDF backend uses for nested-SVG
content, is built on usvg 0.45 and pdf-writer 0.12. Krilla is its maintained
successor, and krilla-svg 0.8 is built on the renderer's own usvg 0.47. The PDF
backend moves to Krilla. The move is required, not optional: staying on
svg2pdf pins the renderer to an unmaintained converter and a second usvg
generation.

Krilla cannot be dropped in where svg2pdf is used. svg2pdf's `to_chunk` hands
back PDF objects that the renderer's own writer renumbers and splices into its
file. Krilla's only output is a complete document, and its SVG drawing targets
only a Krilla page. Moving means the document writer itself moves onto Krilla:
serialization, fonts, metadata, colour and annotations, not only the
nested-SVG step. Rendering nested SVGs as separate Krilla files and grafting
their objects into ours was rejected, because it parses and renumbers another
producer's output.

The work is sliced. Slice 1 (#209) moves the renderer to usvg 0.47 and keeps
svg2pdf behind a temporary bridge: each tree is written back to SVG at full
`f32` precision and reparsed by svg2pdf's usvg 0.45, using the viewer's image
resolver. The bridge changes the spelling of PDF bytes but not what they
render, and both viewer gates hold. It exists only until the port, and goes
with svg2pdf, pdf-writer 0.12 and the older resvg/tiny-skia generation that
`parity_registered_case_svg.rs` tolerates only while svg2pdf is locked.

The port changes every PDF artifact byte and every structure expectation, and
requalifies the PDF evidence once, reviewed in Git (ADR-0019). The PDFium,
PDF.js and qpdf gates remain the judges, and ADR-0009's closure plan still
owns rasterization: Krilla receives lowered, filter-free content.

krilla-svg is not a dependency. Its `tiny-skia` and `resvg` default features
would unify into the PNG rasterizer's, so the drawing it does for the
renderer's trees (groups, clip paths, masks, paths, patterns, gradients and
images) is ported as `render/pdf/svg_draw.rs`, and a filter reaching it is an
error rather than a rasterization. Nested SVGs are drawn inline wherever they
appear, not reused as one shared object. Spread (`repeat`, `reflect`)
gradients are lowered to padded gradients with repeated stops over the painted
bounds, because PDF.js mis-samples the function shading Krilla writes for them.

## Where Krilla differs from earlier decisions

The gap audit (`docs/research/archive/krilla-pdf-backend-gap-audit.md`) found four
places where Krilla 0.8.2 cannot produce what earlier decisions require. Each
is settled here, by a spec change reviewed with the port rather than by an
upstream request:

- **Fonts are subset.** Krilla always subsets. #120 and ADR-0008 embedded only
  the reviewed decodes, and a subset of an official program keeps its Reserved
  Font Name, against ADR-0001. PDF now embeds an ADR-0001 Modified Version of
  each reviewed decode: the whole program with its names rewritten to the
  effective alias the SVG path uses, its legal records kept, and Krilla
  subsets that program to the glyphs drawn. Each program carries a mandatory
  notice (`pdf-font-program@sha256:…`) naming its source program's digest and
  both transformations. This replaces #120's PDF rule and #150's choice to
  embed the emoji fallback whole. PDF font bytes then come from Krilla's
  subsetter, which the viewer and text-extraction gates judge. The subset tag
  is Krilla's own, not ADR-0008's digest-derived tag.
- **The manifest is an attachment.** Krilla cannot write custom XMP. The
  artifact manifest, with the mandatory notices, moves from the `er:manifest`
  XMP property to the attached file `artifact-manifest.json`
  (`application/json`) beside `scene.excalidraw` (spec §12.2). XMP is off.
- **Metadata is deterministic, not a recipe.** Krilla derives the trailer `/ID`
  from its own stable hash, and writes `/Info` whenever metadata is set (the
  renderer sets none, so there is none today). Spec §8.6 stops prescribing
  ADR-0008's derivation and absent `/Info`, and requires document metadata to
  be deterministic and free of timestamps.
- **No output intent.** Krilla writes an output intent only when claiming
  PDF/A, which also requires a creation date. The intent is dropped, and colour
  is drawn in Krilla's ICC-based sRGB (`no_device_cs`), so every colour stays
  characterized. Claiming PDF/A remains a separate decision.

## Allocation inside the writer

Krilla, pdf-writer 0.15, subsetter and Krilla's flate2 calls allocate
infallibly (Stage 1 gate 1 of #201, `docs/research/archive/krilla-pdf-feasibility.md`).
If the allocator refuses during serialization, the process aborts. It does not
return `resource.memory-unavailable`. svg2pdf already had this property for
nested-SVG content; with Krilla it covers the whole PDF serialization stage.
Upstream fallible allocation is not in prospect.

Spec §13.3 is scoped rather than broken silently. The renderer still does its
semantic preflight before the writer runs: every limit and aggregate is
admitted, and every ledger record, including the PDF image planes, font
resources and stream workspace, is charged. Every renderer-owned buffer, before
or during the stage, is still reserved fallibly. Only the writer's own physical
allocations, from the first Krilla call to `finish()`, fall outside §13.3's
fallible-allocation rule. Their size is bounded
by the admitted content, since every input they copy or compress was admitted
first, but the ledger charges an estimate for them, not their exact size. A
host that must survive physical refusal there isolates the call in a terminable
process or worker, as it already does for deadlines. Injected allocator refusal
is exercised up to the writer; the conformance suite does not inject refusal
inside it.



- Line-level `/ActualText` (spec §8.6) needs Krilla's tagging on, which gives
  each span an MCID. The port therefore writes a real tag tree, and every PDF
  is a tagged PDF: each text element, label, frame name and fallback label is
  a paragraph, each of its lines a span carrying its source text, and every
  other drawing an artifact. PDFium reads the spans, and extraction is
  unchanged. The tree claims no conformance level (no PDF/UA).
- Krilla writes `f32` numbers, composes transforms into `cm` and writes clips
  in page coordinates, so no PDF number or operator sequence survives from the
  svg2pdf writer. Tests that matched colour or coordinate spellings match
  Krilla's shortest single-precision spellings.
- ADR-0008's Flate level survives (Krilla uses level 6), but its compressor is
  flate2 over miniz_oxide 0.9.1, not the pinned 0.8.9 the PNG encoder uses.
  Krilla needs its `raster-images` feature for its image module; images
  arrive as sample planes, so it decodes nothing, and the `zune-jpeg` features
  it enables are already enabled through resvg. Krilla is pinned exactly,
  and a Krilla bump is a byte-affecting change, as svg2pdf's was.
- The spike's output was byte-identical between native and wasm32 and across
  two Rust releases. The ported backend still has to pass the full
  cross-target canary (ADR-0002).
- The structural inspector learns Krilla's resource naming, and every
  structure expectation is rewritten, with an output intent count of zero.
