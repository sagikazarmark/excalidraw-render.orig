# varmq representative: pending evidence decision

## Result

**Do not promote yet.** Two fresh reference renders per variant, two native
renders per format/variant and two fresh executions of each viewer all repeat
exactly. Two of ten complete visual comparisons pass: both light SVG viewers.
No tolerance exception or approval was created.

Source: the immutable `tests/conformance/intake/varmq-queue-diagram-v1` board,
89 live elements inside one named frame on a `#fffce8` view background,
Excalifont (family 5) text throughout. Light and dark whole-scene exports,
background on, padding 10, **scale 0.5**, all optional metadata on. Official SVG
`3262.225314958285 × 2996.5945323202386`; canvas/native PNG grid **3262 × 2996**.
Native reports are empty in all six executions. Native capture used the Rust
1.97.1 `--release --locked` CLI built from `70ebc55` (production unchanged since);
`native/capture.json` binds its digest.

At scale 1 the native PNG fails with `resource.raster-surface-exceeded`
(39098332 attempted pixels against `max_raster_surface_pixels` 33554432). That
deliberate outcome is recorded in the plan and not proposed as a variant here.

## Results under canonical budgets

| Comparison | Light | Dark |
| --- | --- | --- |
| SVG / Chromium | pass (whole 4 changed px) | fail: `worker-arrow` SSIM 0.98924, 0 changed px |
| SVG / pure Rust | pass (identical) | fail: `worker-arrow` SSIM 0.99080, 0 changed px |
| PNG / official canvas | fail: whole SSIM 0.98831; `worker-arrow` 311 changed px | fail: same pattern |
| PDF / PDFium | fail: whole 543794 changed px, `worker-arrow` SSIM 0.686 | fail |
| PDF / PDF.js | fail: whole 713345 changed px, `worker-arrow` SSIM 0.810 | fail |

Exact measurements are in `viewers/{light,dark}/*-comparison.json`.

## Frame name: observed Helvetica substitute

Official frame names use `Helvetica`, which the exporter does not embed. In the
pinned reference browser both CDP and `fc-match` resolve it to Liberation Sans
(`/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf`, sha256
`4659bc0c…b144`, a pinned profile system font); see
`viewers/*/official-font-resolution.json`. `viewer_assets.py` injects exactly
those bytes, WOFF2-wrapped with the renderer's encoder settings, into
`official-substituted.svg`. The Chromium audit then reports only custom fonts.
Both light SVG viewers pass with the substituted frame name. This substitution
is a **proposed adapter choice requiring review**, like the fractional-viewport
adapter.

## Diagnosed: PDF viewers widen sub-pixel strokes

The native PDF encodes the cross-hatch rectangle's hachure lines at `0.5 w`
under a `0.375` CTM: 0.1875 pt, **0.25 device px** at 96 DPI, which is the
correct projection of Excalidraw's `strokeWidth / 2` hachure width at scale 0.5.
Hand-authored single-stroke PDFs, independent of the renderer
(`pdf-thin-lines/`, `tools/representative-intake/pdf_thin_lines.py`), show both
pinned viewers paint **exactly one device pixel of coverage for every stroke
narrower than one pixel** (0.125, 0.25, 0.5, 0.75 and 1 px all render
identically) and proportional coverage above it. PDF.js does this in
`getScaleForStroking`/`rescaleAndStroke`. Chromium's SVG and canvas paths draw
the same 0.25 px lines with fractional coverage, so the PDF hachure appears
about four times denser.

Roughly 90% of the whole-page PDF differences (pixels with any channel delta
over 32: PDFium 638971 of 711988, PDF.js 816908 of 890693) lie inside the
cross-hatch rectangle's bounds, against 93 for native SVG in Chromium. This
also applies at scale one: `strokeWidth` 1 hachure is 0.5 px there.

This is a demonstrated viewer rendering rule, not a PDF encoding defect. The
decision is a product one:

1. **Lower sub-pixel strokes to filled outlines in PDF.** Viewers antialias thin
   fills by coverage, so PDF matches SVG at every zoom. Changes the PDF vector
   contract for affected strokes and increases size. Recommended for evaluation.
2. **Scoped exception** for PDF comparisons of sub-pixel strokes, justified by
   these controls. Leaves the visible density difference in PDF viewers.
3. Keep the gap as an explicit known-defect pin.

## Not yet diagnosed

- PDF differences outside the cross-hatch rectangle (about 73000 px per viewer).
- Dark-mode `worker-arrow` SSIM failures with zero changed pixels in both SVG
  viewers while light passes.
- PNG whole SSIM and `worker-arrow` changed pixels; the Velox quarter-pixel
  coverage mismatch (#194) is a candidate, not an established cause.

## Retention and reproduction

Full collections are about 115 MB (92 MB viewers). `compact.py retain` keeps
only JSON records plus a digest inventory of every file. Rerunning all three
collections from scratch at the recorded `target/varmq-*-v1` paths reproduced
every artifact, raster, derived resource and judgment digest.

```sh
bash tools/representative-intake/reference.sh tests/conformance/intake/varmq-queue-diagram-v1 target/varmq-reference-v1
python3 -B tools/representative-intake/intake.py capture tests/conformance/intake/varmq-queue-diagram-v1 \
  target/release/excalidraw-render target/varmq-native-v1 --timeout 400
python3 -B tools/representative-intake/evidence.py tests/conformance/intake/varmq-queue-diagram-v1 \
  target/varmq-reference-v1 target/varmq-native-v1 target/varmq-viewers-v1
python3 -B tools/representative-intake/compact.py check tests/conformance/proposals/varmq84-evidence-v1
```

`compact.py verify` checks the retained records offline and lists stale
producers; it does not re-execute anything or authenticate approval.

## Promotion work still required

1. Review the sanitization/no-change record, scale 0.5, region classes and the
   Helvetica substitute adapter.
2. Decide the PDF sub-pixel stroke handling above; diagnose the remaining PDF,
   dark SVG and PNG differences.
3. Independently authored semantics, notices/metadata, reports and resource
   outcomes (including the scale-one surface limit).
4. Registry and shared target integration, native/WASM/process-order equality
   and exact subject approval.
