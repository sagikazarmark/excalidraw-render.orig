# varmq dark SVG crop: diagnosis (#84)

Status: diagnosis only. Nothing changed in production, and no approval or
exception is proposed here.

## Symptom

In `varmq84-evidence-v2`, the dark `worker-arrow` crop fails the SSIM gate in
**both** SVG viewers (Chromium 0.989242, pure Rust 0.990801) with 0 changed
pixels, while the light crop is pixel-identical. Almost every pixel in the crop
differs by 1–11 levels (maximum 29 over the full region).

## Cause

The reference's dark SVG carries `filter="invert(93%) hue-rotate(180deg)"` on
its root, so the browser filters the **finished, composited light image**. The
renderer, per spec §8.1, emits no filter and lowers the filter into **each color
before compositing**. The two orders agree on flat color, up to quantization,
but not on antialiased pixels, where the filter sees blended colors and clamps
(`hue-rotate(180deg)` drives several of this board's colors below zero).

`filter-order.json` (from `tools/representative-intake/dark_filter_order.py`)
applies the exact filter to the official **light** Chromium raster and compares
the result with the official **dark** raster:

| Over the `worker-arrow` region (30401 px) | Exact pixels | Max delta |
| --- | --- | --- |
| round(filter(official light)) vs official dark | 30143 | 3 |
| floor(filter(official light)) vs official dark | 2854 | 3 |
| native dark vs official dark | 274 | 29 |

Over the whole image, rounding reproduces 99.9% of sampled pixels. The official
dark SVG in Chromium is, to within a few levels, the rounded filter of the
composited light image.

## Secondary finding: flat colors round, the renderer floors

Spec §8.1 says the lowered filter is "floored to 8 bits", as the pinned browser
evaluates it. For the official SVG, the pinned browser **rounds**. Every flat
color on this board matches the rounded exact filter and differs from the
floored one by one level in some channel: `#fffce8` gives `181605` (native
`181504`), `#a5d8ff` gives `154163` (native `154162`), `#ffec99` gives `362600`
(native `352500`), and `#b2f2bb` gives `043b0c` (native `043b0b`). The canvas PNG
path fits neither rule; it quantizes between the two filter functions in some
fixed-point form (`#ffd43b` gives `5e3900` against exact 95.0, 58.0).

Rounding alone does not fix the crop. A renderer build with `round` in
`theme_filter` moves the dark Chromium crop from SSIM 0.989242 to 0.989173 and
the whole image from 0.998267 to 0.998669
(`round-conversion-svg-chromium-comparison.json`). The crop failure is the
compositing order, not the rounding.

## Consequences

- No per-color lowering can match the reference's dark SVG at antialiased
  pixels. Fine textures, such as sketch fills, fail SSIM in dark mode even when
  every flat color is right.
- Dark PDF cannot carry a CSS filter, so it inherits the same difference on top
  of the sub-pixel stroke widening.
- Dark PNG is compared with the canvas, which filters after compositing too, but
  with its own quantization.

## Decision required

1. **SVG: emit the reference's structure.** Keep light colors and put
   `THEME_FILTER` on the root (and `IMAGE_INVERT_FILTER` on raster image `<use>`
   elements), as the official export does. The native SVG would then match in
   any viewer that implements CSS filter functions. That includes both canonical
   viewers, and nested-SVG images would invert with the root. Cost: a spec §8.1
   change, requalification of every approved dark SVG artifact, and SVG
   consumers that ignore CSS filter functions would show light colors.
2. **Keep lowering** and treat compositing order as a documented non-defect
   cause, with exact-ceiling exceptions for affected dark crops (evidence above).
3. Independently: the lowering rounds, per this evidence, rather than floors for
   the formats whose authority is the official SVG (SVG and PDF). That is a small
   correction, but it changes every approved dark SVG and PDF artifact's bytes.
