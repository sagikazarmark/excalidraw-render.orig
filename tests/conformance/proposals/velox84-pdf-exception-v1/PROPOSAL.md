# Proposed tolerance exceptions: Velox PDF (#84)

**Status: pending review. Not approved, not applied.** The case
`tests/conformance/cases/representative/velox-design` (subject
`sha256:14937dda9e20fae9cb723c0835e15418f87d46f3b5b49fc8a4b603c96fc36c79`)
was verified with these exceptions but not approved. Velox is registered for
SVG only (subject `sha256:e81cf856…49b2`), and PDF waits on the budget
recalibration in #195.

`exceptions.json` holds 16 exact-ceiling entries: every region (`whole`,
`client-typography`, `client-binding`, `client-arrow`) × light/dark × PDFium/PDF.js,
copied from `../velox84-evidence-v2`. SVG passes every gate, and PNG is not
registered (it waits on #194).

## Causes

- **PDF.js: rasterizer variance.** Its difference from the official raster
  matches the canonical viewers' own disagreement on the identical official SVG
  (`rasterizer-pairs.json`). Light: whole 1655 px / 0.9916 against Chromium vs
  pure Rust 1571 / 0.9914; typography 37 / 0.9769 against 37 / 0.9778; binding
  213 / 0.9867 against 184 / 0.9871.
- **PDFium: overlap summing plus glyph rasterization.** At a horizontal 2 px
  Rough edge, PDFium paints 89 / 30 / 30 where Chromium and PDF.js paint 198 /
  30 / 79. It sums the overlapping partial coverage of Rough.js's two passes, as
  the `band-twice` control in `../varmq84-pdf-subpixel-exception-v1` shows
  (0.502 vs 0.251).
- **The non-text `client-arrow` crop** (added in plan v2 to satisfy spec §14.3)
  fails even between Excalidraw's own references: the official SVG in Chromium
  vs the official canvas in the same Chromium scores 0.9948 light and 0.9933
  dark, under the 0.999 geometry limit.

## Decision needed

Each entry is diagnosed, and exact ceilings still fail on any regression. But
16 entries waive every PDF comparison of this case, and the arrow evidence
suggests the geometry budget is tighter than the references agree for
Rough.js edge-dominated crops. The alternatives:

1. approve these 16 entries and register the case;
2. revisit the geometry and typography budgets in a comparator-profile v2
   (touches every approval that binds the profile), then re-judge;
3. register Velox for SVG only for now.
