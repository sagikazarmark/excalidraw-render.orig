---
status: accepted
---

# Lower dark mode into nested-SVG content in PDF

#154 recorded a revision-1 limitation. In dark mode the reference's root
`THEME_FILTER` inverts a nested-SVG image with the rest of the scene, but the
PDF backend embedded the image's vector content through `svg2pdf 0.13.0` in its
light-mode colors. `usvg::Tree` is immutable and `svg2pdf::to_chunk` owns the
conversion, so the issue listed three contract-level choices: fork `svg2pdf`,
rasterize the whole image with a filter, or keep the content unlowered under a
compatibility exception.

## Decision

A fourth option avoids all three costs. The PDF backend already serializes
sanitized nested-SVG trees through a lossless normalized writer and parses
them again (for raster-fallback planning). In that normalized form, every paint
is a `#rrggbb` attribute and every nested raster is a normalized PNG data URL.
In dark mode, the backend lowers the rounded theme filter (ADR-0012) into:

- `fill`, `stroke`, `stop-color`, `flood-color` and `lighting-color` colors;
- each visible pixel of a nested raster, alpha untouched, re-encoded with the
  sanitizer's normalized PNG encoder;
- nested-SVG images inside the image, which the writer inlines.

`<mask>` content is left alone: a mask acts through the luminance of its
light-mode content, and the reference filters only the composite. The lowered
tree replaces the original for every PDF consumer, including raster-fallback
closures. The content stays vector, needs no fork, and emits no
`pdf.raster-fallback`.

## Consequences

- On the `excali-local-slides` representative (#84), the dark nested-SVG avatar
  crop goes from about 12000 changed pixels (SSIM 0.766) to 0 changed pixels in
  both pinned PDF viewers. PDF.js passes the whole dark comparison.
- Like all PDF lowering, the colors are lowered before compositing, so
  antialiased edges keep the compositing-order difference ADR-0012 describes.
  Filter primitives inside a nested SVG see lowered inputs, where the reference
  filters their output; the two agree for the linear primitives and can differ
  for color-matrix and component-transfer primitives.
- Dark-mode PDF artifacts with nested-SVG images change bytes and need
  requalification.
