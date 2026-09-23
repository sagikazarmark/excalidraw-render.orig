# excali-local-slides representative: pending evidence decision

## Result

**7 of 10 comparisons pass.** Two fresh reference renders per variant, two
native renders per format and variant, and two fresh executions of each viewer
all repeat exactly. No tolerance exception or approval was created.

Source: `tests/conformance/intake/excali-local-slides-v1`, an upstream slide
deck (MIT). It has eight named frames clipping sixteen nested-SVG image
occurrences of two distinct assets, and framed Virgil text. Light and dark
whole-scene exports: background on, padding 10, scale 1, all optional metadata
on. Grid 2831 × 3847. Native reports are empty in all six executions. Frame
names use the observed Helvetica substitute (Liberation Sans).

The upstream export predates fractional indices, which the pinned reference
oracle requires. The intake uses `reviewed-index-repair`: it inserts
`"index":"a0"` … `"aV"` in array order (the keys `restore()` assigns, verified
against the pinned `fractional-indexing` 3.2.0) and keeps every other byte.
Verification removes exactly those members and checks the pinned upstream
digest. The oracle and its profile are unchanged.

| Comparison | Light | Dark |
| --- | --- | --- |
| SVG / Chromium | pass (whole 0 changed px) | pass (whole 13 px) |
| SVG / pure Rust | pass | pass |
| PNG / canvas | pass | pass |
| PDF / PDF.js | pass | fail: `slide1-avatar` 12037 px, SSIM 0.766863 |
| PDF / PDFium | fail: `slide1-avatar` 0 px, SSIM 0.998471 | fail: `slide1-avatar` 11948 px, SSIM 0.765717 |

Exact measurements are in `viewers/{light,dark}/*-comparison.json`.

## Findings

- **Dark SVG passes in both viewers,** including sixteen nested-SVG images
  that invert with the root filter (ADR-0012). Under the previous per-color
  lowering they carried a separate image filter.
- **Dark PDF fails on the nested-SVG avatars: the #154 limitation.** PDF embeds
  a nested SVG's vector content in its light-mode colors. This scene makes the
  limitation visible (about 12000 px per avatar crop) and is a direct test for
  whichever #154 option is chosen.
- **Light PDFium avatar crop:** SSIM 0.998471 against the geometry limit 0.999,
  with 0 changed pixels. The nested SVG is embedded as vectors through
  `svg2pdf`, and PDF.js passes the same crop (0.999088). Not yet diagnosed.
- **Frame clipping at scale:** eight frames each clipping two images render
  within budget in every format, which the pool audit flagged as a PNG failure
  risk before #112.

## Promotion work still required

1. Review of the index-repair sanitization record, region classes and the
   Helvetica substitute adapter.
2. #154 decision for dark PDF nested-SVG content; diagnose the light PDFium
   avatar crop.
3. Independent semantics, notices/metadata, reports and resource outcomes;
   registry and target integration; exact subject approval.
