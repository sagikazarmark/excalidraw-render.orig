---
status: accepted
---

# Carry the reference's dark-mode root filter in SVG and round PDF lowering

The reference's dark-mode export applies `THEME_FILTER`
(`invert(93%) hue-rotate(180deg)`) to the composited image: its SVG carries the
filter on the root `<svg>`, and its canvas sets it on the context. Revision 1
lowered the filter into every painted color instead, in every format, and
floored the result to 8 bits.

The varmq representative (#84) showed that per-color lowering cannot reproduce
the reference SVG. At an antialiased pixel the browser filters a blended color,
and `hue-rotate(180deg)` clamps several ordinary scene colors below zero, so
filtering after compositing and compositing lowered colors disagree. The dark
`worker-arrow` crop failed SSIM in both canonical SVG viewers with zero changed
pixels. Rounding the exact filter of the official light raster reproduces
99.2% of the official dark crop's pixels (at most 3 levels off), while the
lowered native SVG matched 0.9% (up to 29 levels off); see
`tests/conformance/proposals/varmq84-dark-diagnosis-v1/DIAGNOSIS.md`. The same
evidence showed the pinned browser **rounds** the root filter where the
renderer floored, leaving flat colors one level low in some channels.

## Decision

- **SVG** carries the reference's own root attribute,
  `filter="invert(93%) hue-rotate(180deg)"`, and paints light colors beneath
  it; the frame name keeps the dark-theme constant, as the reference SVG's does.
  Raster images keep `IMAGE_INVERT_FILTER` on their `<use>` as Filter Effects
  primitives; nested-SVG images need no filter of their own.
- **PDF**, whose authority is the official SVG, keeps lowering (a PDF cannot
  carry a CSS filter) but **rounds** the filter's result.
- **PNG**, whose authority is the canvas, keeps lowering and flooring.

The root filter is the CSS function spelling, not a `<filter>` of primitives
referenced from the root or from a wrapping group. On varmq, both primitive
encodings stayed measurably different from the official SVG in Chromium, while
the CSS function reproduced it exactly once the one known structural difference
was removed.

## Consequences

- Dark SVG matches the reference in the pure-Rust viewer exactly on varmq. In
  Chromium the remaining difference is the recorded frame-clip authority
  divergence (spec section 8.4): the reference's namespaced `clip-path` is not
  applied, the renderer's plain one is, and Chromium draws a clipped group
  differently inside a filter layer. With the reference's clip spelling
  substituted, the crop is identical.
- An SVG consumer that ignores CSS filter functions shows a dark-mode export in
  light-mode colors, as it would show the reference's own dark export.
- Dark PDF still cannot match antialiased pixels exactly. It inherits the
  compositing-order difference, now with correctly rounded flat colors.
- Every approved dark-mode SVG and PDF artifact changes bytes and needs
  requalification, including the approved native snapshot baseline.
