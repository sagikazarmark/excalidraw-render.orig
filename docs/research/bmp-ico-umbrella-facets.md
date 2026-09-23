# BMP and ICO umbrella facets (#85)

This closes the fourteen BMP and ICO umbrella facets: alpha, color-profile,
default-frame, limits, malformed, orientation and passthrough for each family.
A 78-variant probe through the pinned browser showed that the renderer was
stricter than the reference, and differed in pixels, in about half of the
variants.

## Reference decoder

Chromium 149 decodes BMP with Blink's `BMPImageReader`. The Rust BMP decoder
is behind a field trial that the pinned browser leaves off. ICO and CUR go
through Blink's `ICOImageDecoder`, which reads DIB entries with the same
reader in icon mode and PNG entries with the PNG decoder at an offset.
Excalidraw admits an image through `onload`/`onerror`. Blink fires `onload`
once the headers give a size, so a file whose pixels then fail loads and
paints only what was decoded, sometimes nothing.

## Policy

This follows the GIF policy the user chose on #85:
- Match everything the reference draws as a complete, well-defined image:
  - header and directory tolerance;
  - OS/2 2.x headers;
  - out-of-range palette indices;
  - empty and wide masks;
  - the all-zero alpha rule;
  - run-length holes, early ends and overruns;
  - calibrated RGB and embedded profiles;
  - cursors;
  - icon entry selection;
  - the AND-mask rule;
  - ignored PNG-entry orientation.
- Keep strict decode-failed recovery where the reference loads a file but
  paints it partially or not at all. This covers truncated non-run-length
  pixel data, overlapping or non-contiguous masks, an icon image outside the
  file, and a PNG entry whose size disagrees with the directory. This is the
  declared divergence.
- Do not replicate a reference defect. The reference paints a PNG icon entry
  that needs a color transform fully transparent: a gAMA other than sRGB's, a
  linear cICP, or a non-sRGB ICC profile. The renderer draws such an entry as
  the same PNG standalone. sRGB-equivalent entries draw untransformed in both.

`render/asset/bmp.rs` is rewritten as a port of `BMPImageReader`, with an icon
mode. `render/asset/ico.rs` is rewritten as a port of `ICOImageDecoder`. Both
module headers list the rules. Spec section 8.2 states them.

The standalone PNG path now leaves an ICC profile that Skia snaps to sRGB
untransformed, as the WebP path already did. The reference does the same.
Before this change, an sRGB v4 profile drew one channel unit off.

## Evidence

`tests/oracle/bmp-ico-container/reference.json` holds 104 rows. It is
collected by `tools/reference-oracle/run-bmp-ico-container.sh` in two fresh
processes, forward and reverse, with a fresh context per case and no network.
The fixtures are built from one 4×3 picture. The ICC profiles are the frozen
PNG color vectors. The collector asserts each row's admission and natural
size.

Each row declares `match`, `partial` (the declared divergence), `rejected` or
`divergent` (the transparent PNG entries). `bmp_ico_container_contract` judges
every row in every format:
- A `match` row must pass the canonical comparison in PNG, and in PDF where
  the reference is opaque. PDF alpha must be exact, and SVG must keep the
  source octets.
- A `partial` or `rejected` row recovers exactly as a missing asset.
- A `divergent` row must draw exactly as its PNG standalone, and the test
  checks that the reference still paints it transparent.

Three color-managed rows differ from Skia's float arithmetic by one channel
unit on one pixel, in PNG and PDF: calibrated near-sRGB, embedded P3 and
embedded linear. On a 4×3 artifact, the small-artifact rule needs exact views.
These six views are held at approved exact-ceiling exceptions
(`tests/oracle/bmp-ico-container/color-exceptions.json`, spec section 14.4). A
dead or unused exception fails.

`bmp_ico_limit_contract` covers byte, dimension, pixel and work equality and
first crossing, and the 13.2 precedence, for direct and RLE BMP and for DIB and
PNG icons.

## Owners

| Facet | Owner |
| --- | --- |
| bmp orientation | `bmp_ico_container_contract::bmp_row_order_is_its_only_orientation` |
| bmp color-profile | `bmp_ico_container_contract::bmp_color_spaces_apply_as_the_reference_applies_them` |
| bmp alpha | `bmp_ico_container_contract::bmp_alpha_matches_the_reference` |
| bmp default-frame | `bmp_ico_container_contract::bmp_default_image_matches_the_reference` |
| bmp malformed | `bmp_ico_container_contract::bmp_containers_and_malformed_payloads_match_the_reference` |
| bmp passthrough | `bmp_ico_container_contract::bmp_svg_embeds_every_admitted_payload_exactly` |
| bmp limits | `bmp_ico_limit_contract::bmp_limits_have_exact_boundaries_and_precedence` |
| ico orientation | `bmp_ico_container_contract::ico_orientation_metadata_is_ignored_as_the_reference_ignores_it` |
| ico color-profile | `bmp_ico_container_contract::ico_color_profiles_apply_as_png_entries_apply_them` |
| ico alpha | `bmp_ico_container_contract::ico_alpha_matches_the_reference` |
| ico default-frame | `bmp_ico_container_contract::ico_default_entry_matches_the_reference` |
| ico malformed | `bmp_ico_container_contract::ico_containers_and_malformed_payloads_match_the_reference` |
| ico passthrough | `bmp_ico_container_contract::ico_svg_embeds_every_admitted_payload_exactly` |
| ico limits | `bmp_ico_limit_contract::ico_limits_have_exact_boundaries_and_precedence` |
