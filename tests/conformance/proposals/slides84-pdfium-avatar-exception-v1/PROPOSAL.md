# Proposed tolerance exceptions: PDFium nested-SVG rasterization (slides, #84)

**Status: pending review. Not approved, not applied.** `exceptions.json` has
`approval: null` and every `approving_issue` is null.

| Case | Variant | Format | Viewer profile | Region | Max changed px | Minimum SSIM bits |
| --- | --- | --- | --- | --- | --- | --- |
| `representative.excali-local-slides.v1` | light | pdf | `canonical-pdf-pdfium-v1` | `slide1-avatar` | 0 | `3feff378e3937646` (0.998471) |
| `representative.excali-local-slides.v1` | dark | pdf | `canonical-pdf-pdfium-v1` | `slide1-avatar` | 0 | `3feff4fa706910a9` (0.998655) |

Ceilings are the exact judgments in `../slides84-evidence-v2`, bound by report
digest. These are the only failing comparisons in that collection.

## Non-defect cause

The crop is a nested-SVG cartoon avatar embedded as vectors through `svg2pdf`.
It has **0 changed pixels** in both modes; only SSIM falls under the
geometry/image limit (0.999). PDF.js passes the same crop of the same PDF
(0.999088 light, 0.999152 dark). Canonical rasterizers disagree as much on
identical input (`rasterizer-pairs.json`, canonical comparator):

| Same input | Rasterizers | Light | Dark |
| --- | --- | --- | --- |
| official SVG | Chromium vs pure Rust | 0.998864 | 0.997364 |
| this PDF | PDFium vs PDF.js | 0.998584 | 0.998677 |
| official SVG vs this PDF | Chromium vs PDFium | 0.998471 | 0.998655 |

The difference is PDFium's antialiasing of the avatar's fine vector detail, not
a geometry, color or placement difference.

## Required before this can be applied

Exact-entry approval through the case approval subject, schema support for PDF
crop exceptions, and registration of the representative case (see
`../varmq84-pdf-subpixel-exception-v1/PROPOSAL.md`).
