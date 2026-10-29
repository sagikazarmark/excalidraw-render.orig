# GIF umbrella facets (#85)

This closes the eight GIF umbrella facets: alpha, animation, color-profile,
default-frame, limits, malformed, orientation and passthrough. A probe through
the pinned browser showed the reference is far more lenient than the renderer
was. It admitted 92 of 98 probe variants, most of which the renderer rejected.
That includes all but 4 of the 48 frozen GIF controls, which no earlier
tranche had collected.

## Reference decoder

Chromium 149 decodes GIF through Skia's `SkWuffsCodec`, which uses Wuffs 0.3.4
with only the `IGNORE_TOO_MUCH_PIXEL_DATA` quirk. Excalidraw admits an image
through `onload`/`onerror`, and Blink paints a partially decoded frame.

## Policy (chosen by the user on #85)

- Match everything the reference draws as a complete, well-defined image:
  container and metadata tolerance, out-of-range indices, a missing palette,
  expanded frame bounds, zero-size frames, and data after a complete first
  frame.
- Keep strict decode-failed recovery where the first frame's data ends or
  fails before its pixels are complete. The reference paints the partial
  frame. This is the declared divergence.
- Fix the fatal `resource.memory-unavailable` that a large ICC or XMP
  application extension caused.

`render/asset/gif.rs` is rewritten around its own block walk and LZW decoder,
and no longer uses the `gif` crate for decoding. Spec section 8.2 states the
rules.

## Evidence

`tests/oracle/gif-container/reference.json` holds 97 rows collected by
`tools/reference-oracle/run-gif-container.sh`, in two fresh processes forward
and reverse, with a fresh context per case and no network. Every row is drawn
at its natural size. The collector asserts each row's admission and the
browser-reported natural size, which independently checks the canvas-growth
rule (for example 11×9, 25×25, 17×7 and 10×7). The rows are:

- 5 bases;
- container tolerance: trailing bytes, a NUL or missing trailer, unknown
  labels, wrong-size application and plain-text blocks, GIF87a extensions,
  reserved bits, disposal 5, aspect ratio, an out-of-range background, and a
  zero logical screen;
- ICC and XMP application extensions, including oversized ones;
- palette semantics;
- frame bounds and zero-size frames;
- complete-frame data tails;
- later-frame faults;
- the 48 frozen controls.

Each row declares `match`, `partial` (the declared divergence), `rejected`, or
`resource-rejected`. `huge-logical-screen` is the one resource-rejected row:
the reference rejects it for its size, the renderer's resource limits decide
in PNG and PDF, and direct SVG, which derives no pixels, embeds it.

`gif_container_contract` judges every row in every format. A `match` row must
equal the reference exactly in PNG and PDF, since GIF alpha is binary, and
keep its octets in SVG. The other rows must recover exactly as a missing
asset.

The three earlier GIF suites now run their strict-recovery loops only over the
controls the record declares partial or rejected: 3 first-frame, 7 two-frame
and 7 LZW. The complete-drawing controls move to the record's owner.
`gif_limit_contract` covers byte, dimension, pixel and work equality and first
crossing, and the 13.2 precedence, for still, transparent and animated GIFs.
The renderer already complied.

Downscaling a large image into a small element differs from Chromium in every
codec. The first probe drew all rows into a 9×7 element, and a 25×25 image
then differed by up to 96 channel units. At natural size, every row matches
exactly. The resampling difference is outside this tranche.

## Owners

| Facet | Owner |
| --- | --- |
| orientation | `gif_container_contract::gif_orientation_metadata_is_ignored_as_the_reference_ignores_it` |
| color-profile | `gif_container_contract::gif_palettes_are_the_only_color_model_as_in_the_reference` |
| malformed | `gif_container_contract::gif_containers_and_malformed_payloads_match_the_reference` |
| passthrough | `gif_container_contract::gif_svg_embeds_every_admitted_payload_exactly` |
| alpha | `gif_container_contract::gif_alpha_matches_the_reference` |
| default-frame, animation | `gif_container_contract::gif_default_frame_and_animation_match_the_reference` |
| limits | `gif_limit_contract::gif_limits_have_exact_boundaries_and_precedence` |

The 27 frozen control rows that now draw completely move to the malformed
owner. The 17 still-strict ones stay with their suites, and their criteria are
rewritten to the reference-backed behavior.
