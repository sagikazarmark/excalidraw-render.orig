# Proposed tolerance exceptions: PDF sub-pixel sketch strokes (varmq, #84)

**Status: pending review. Not approved, not applied.** `exceptions.json` has
`approval: null` and every `approving_issue` is null. Nothing reads this
directory.

## Scope

Spec §14.4: a tolerance exception names one case, variant, format, viewer and
region, documents a non-defect cause, and sets ceilings to the exact observed
count and binary64 score with no margin. Two are proposed:

| Case | Variant | Format | Viewer profile | Region | Max changed px | Minimum SSIM bits |
| --- | --- | --- | --- | --- | --- | --- |
| `representative.varmq-queue-diagram.v1` | light | pdf | `canonical-pdf-pdfium-v1` | `worker-arrow` | 18886 | `3fe98f674257a6fe` (0.798755) |
| `representative.varmq-queue-diagram.v1` | light | pdf | `canonical-pdf-pdfjs-v1` | `worker-arrow` | 15156 | `3fe9e824845e5a96` (0.809588) |

Ceilings are copied from the judgments in
`tests/conformance/proposals/varmq84-evidence-v2` (renderer at `ca5c3a2`,
including the PDFium geometry-loss fix). They are bound by report digest in
`exceptions.json`.

## Non-defect cause

1. **The PDF encodes the correct geometry.** The crop is dominated by the
   cross-hatch fill, stroked at `0.5 w` under a 0.375 CTM (0.25 device px). Its
   32240 path numbers are identical, in order, to the native SVG's sketch path
   (`attribution-light.json`). The native SVG passes this crop in Chromium with
   0 changed pixels and SSIM 1.0.
2. **Both pinned PDF viewers widen sub-pixel strokes.** Hand-authored
   single-stroke PDFs (`controls/thin-lines.json`, produced by
   `tools/representative-intake/pdf_thin_lines.py`) paint exactly one device
   pixel of coverage for every stroke from 0.125 to 1 px, and proportional
   coverage above that. PDF.js does this in `getScaleForStroking` /
   `rescaleAndStroke`.
3. **The widening accounts for the density.** Hatch coverage on a pure-hatch
   patch: official and native SVG 0.317; PDF.js 0.812. The official SVG with
   only the hatch's stroke width raised to one device pixel, rendered in the
   canonical Chromium viewer, gives 0.8125. PDFium reaches 0.931 because it also
   sums overlapping partial coverage: in the same controls, one 0.25 px band
   paints 1.0 and two identical bands in one path paint 0.502, where PDF.js
   paints 0.251 for both. Rough.js draws every sketch line twice.
4. **What the ceilings admit is texture, not displacement.** At one-pixel width
   the hatch nearly saturates the region, and each rasterizer places partial
   pixels differently. Even against the one-pixel Chromium control, the crop
   stays well below the SSIM limit, so no single-pixel-width reference would
   pass it either.

No renderer change can make the pinned viewers draw a 0.25 px stroke as
0.25 px. Drawing sketch fills as filled outlines was prototyped
(`docs/research/pdf-sketch-fill-outline-prototype.md` on branch
`pdf-sketch-outline-prototype`). It cut the difference but did not reach parity,
because the viewers also combine overlapping sub-pixel fills differently from
Chromium, and PDFs grew 3–6×. It is not adopted.

## Deliberately excluded

- **Dark variant.** The native *SVG* already fails this crop in dark mode (SSIM
  0.989 with 0 changed pixels, in both SVG viewers). That cause is undiagnosed,
  so a dark PDF exception cannot claim a single non-defect cause yet.
- **Whole-page PDF comparisons.** Besides the hatch, about 73000 pixels per
  viewer differ outside the cross-hatch rectangle for undiagnosed reasons.
- **`queue-manager-typography`.** PDF text differs for an unrelated,
  undiagnosed reason (SSIM about 0.988–0.993).
- **PNG.** Direct PNG does not have this cause.

With these exceptions, varmq light PDF still fails on the whole page and the
typography crop. The exceptions narrow the open work; they do not make the case
pass.

## Required before this can be applied

1. Maintainer review and approval of these exact entries (`exceptions.json`),
   recorded through the case approval subject, which binds exceptions (§14.4).
2. Schema support. The focused-case `toleranceException` admits only
   `format: png` and `region: whole` with a null viewer profile. A PDF crop
   exception needs a schema that admits `format: pdf`, a named crop region and a
   viewer profile id.
3. Registration of the representative case, which does not exist yet.
4. If the renderer's PDF output for this crop changes, the ceilings are stale
   and must be re-proposed from a fresh collection, never widened by hand.

## Reproduction

```sh
python3 -B tools/representative-intake/compact.py check tests/conformance/proposals/varmq84-evidence-v2
python3 -B tools/representative-intake/pdf_thin_lines.py <new-dir>
python3 -B tools/representative-intake/pdf_subpixel_attribution.py target/varmq-viewers-v2/light \
  --stroke-width 0.5 --patch 470 2250 30 40 --red 255 165 --control <control.rgba.zlib> <out.json>
```

The one-pixel control is `target/varmq-viewers-v2/light/official-substituted.svg`
with its single `stroke-width="0.5"` changed to `"2"` (one device pixel at
scale 0.5), rendered with `tools/representative-intake/svg-viewer.sh` and the
official asset declaration, and clipped to the 3262×2996 grid.
`control/receipt.json` is its viewer receipt. `attribution-light.json` binds
the control raster by digest.
