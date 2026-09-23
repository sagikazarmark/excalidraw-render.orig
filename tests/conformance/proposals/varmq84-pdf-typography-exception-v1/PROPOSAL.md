# Proposed tolerance exceptions: PDF glyph rasterization (varmq, #84)

**Status: pending review. Not approved, not applied.** `exceptions.json` has
`approval: null` and every `approving_issue` is null.

## Scope

| Case | Variant | Format | Viewer profile | Region | Max changed px | Minimum SSIM bits |
| --- | --- | --- | --- | --- | --- | --- |
| `representative.varmq-queue-diagram.v1` | light | pdf | `canonical-pdf-pdfium-v1` | `queue-manager-typography` | 11 | `3fefc17216d8c415` (0.992364) |
| `representative.varmq-queue-diagram.v1` | light | pdf | `canonical-pdf-pdfjs-v1` | `queue-manager-typography` | 2 | `3fef9ebf1e368a9e` (0.988128) |

Ceilings are the exact v3 judgments
(`tests/conformance/proposals/varmq84-evidence-v3`), bound by report digest.
The crop is 79 px Excalifont text, "Queue Manager", on a solid fill.

## Non-defect cause

1. **The PDF places glyphs exactly.** Each glyph is shown with its own
   full-precision absolute text matrix (`1 0 0 -1 x y Tm`) at the positions the
   native SVG uses, and the native SVG passes this crop in Chromium with 0 changed
   pixels and SSIM 1.0.
2. **The viewers rasterize glyphs differently.** Measured on the crop: PDFium
   paints 2.7% more ink than Chromium with the same centroid (within 0.07 px);
   PDF.js paints 0.65% more, with its centroid about 0.19 px right and up. No
   integer shift improves either.
3. **Canonical rasterizers disagree this much on identical input**
   (`rasterizer-pairs.json`, canonical comparator, same crop):

   | Same input | Rasterizers | SSIM |
   | --- | --- | --- |
   | official SVG | Chromium vs pure Rust | 0.988381 |
   | this PDF | PDFium vs PDF.js | 0.982131 |
   | official SVG vs this PDF | Chromium vs PDFium | 0.992364 |
   | official SVG vs this PDF | Chromium vs PDF.js | 0.988128 |

   The PDF-versus-official results fall inside the range the canonical viewers
   show against each other. The comparator profile already admits this for the
   pure-Rust SVG and PDF.js viewers with relaxed typography budgets (minimum SSIM
   0.99). PDFium is held to the canonical typography budget (0.995). Large glyphs
   with long antialiased edges put all four results just under those limits.

## Alternatives considered

Drawing text as filled outlines, with invisible selectable text on top, would
move glyph rendering into each viewer's path rasterizer. It would not make
viewers agree with Chromium's glyph rasterizer, and it changes the PDF text
contract (spec section 8.6). Not proposed.

## Also consider

A budget change instead of per-case exceptions: a PDFium typography budget
matching the existing PDF.js and pure-Rust ones. That is a comparator-profile
decision affecting every case, so it is only raised here, not proposed.

## Required before this can be applied

The same as `../varmq84-pdf-subpixel-exception-v1`: exact-entry approval
through the case approval subject, schema support for PDF crop exceptions, and
registration of the representative case.
