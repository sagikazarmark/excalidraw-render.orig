# varmq #84 evidence proposal, v3

Unapproved compact collection for `representative.varmq-queue-diagram.v1`,
re-collected after the dark-mode root filter change (`a9d9e5e`, ADR-0012). The
reference collection is unchanged from v1 and v2. `compact.json` binds the
digest of every file of the full collection.

3 of 10 comparisons pass (v2: 2). All five light comparisons are identical to
v2, so the proposed light PDF `worker-arrow` exceptions keep their ceilings.

| Dark comparison | v2 | v3 |
| --- | --- | --- |
| SVG / pure Rust | fail (`worker-arrow` SSIM 0.990801) | **pass** (1.000000 everywhere) |
| SVG / Chromium | fail (`worker-arrow` 0.989242) | fail (`worker-arrow` 0.997467; whole and typography pass) |
| PDF / PDFium | whole 713569 changed px, `worker-arrow` 0.777583 | whole 732920, `worker-arrow` 0.778235 |
| PDF / PDF.js | whole 442201, `worker-arrow` 0.792512 | whole 459713, `worker-arrow` 0.794221 |
| PNG / canvas | unchanged | unchanged |

**Dark SVG, Chromium.** The remaining crop difference is the recorded
frame-clip authority divergence (spec section 8.4). The reference SVG's
namespaced `clip-path` is not applied in Chromium, the renderer's plain one is,
and Chromium draws a clipped group differently inside a filter layer. With the
reference's clip spelling substituted, the crop is identical (SSIM 1.000000);
see `../varmq84-dark-diagnosis-v1/DIAGNOSIS.md` and ADR-0012.

**Dark PDF.** PDF now rounds the lowered filter. Sampled flat pixels that were
one level off the official dark raster (about 1.88 million of every fourth
pixel) are now exact. Antialiased edges moved slightly the other way (about
244000 sampled edge pixels worse, 35000 better), which raises whole-image
changed pixels. The edges are the compositing-order difference no lowering
can remove: the PDF composites lowered colors, the reference filters the
composite. The dark PDF also carries the sub-pixel stroke widening documented in
`../varmq84-pdf-subpixel-exception-v1`.
