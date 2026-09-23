# WebP umbrella facets (#85)

This closes the eight WebP umbrella facets: alpha, animation, color-profile,
default-frame, limits, malformed, orientation and passthrough. An evidence
audit found real divergences from the pinned reference. As the user chose on
#85, the renderer now reads WebP the way the reference does, including where
that reverses two #191 characterizations.

## Divergences found and fixed

A probe through the pinned browser used container edits of frozen libwebp
vectors plus the 12 #191 controls. Reading the sources behind it (libwebp
demux at the Chromium 149 DEPS pin `c00d83f`, Blink `webp_image_decoder.cc` at
149.0.7827.55, Skia/skcms at `53348aa`) gives these rules:

| Behavior | Reference | Renderer before |
| --- | --- | --- |
| EXIF orientation | never applied | applied |
| ICC profile | only on a still image with the VP8X ICC flag set and an RGB profile; otherwise ignored | applied with the flag clear, in animations, and for gray profiles; an unparseable profile reported `unsupported-format` |
| ICC profile Skia snaps to sRGB | drawn untransformed (truncating premultiply) | transformed |
| EXIF/ICCP chunk over the decoder workspace | metadata | fatal `resource.memory-unavailable` |
| Bytes after the declared RIFF end | ignored | rejected |
| Reserved VP8X flag bits (`~0x3e`) | rejected | accepted |
| Reserved ANMF bits; ANMF size differing from the bitstream | ignored; the bitstream's size | rejected |
| EXIF/XMP flag without its chunk; unknown chunks | ignored | rejected |
| Duplicate VP8X | rejected | accepted |
| Animated alpha flag differing from the frames | accepted | rejected |
| VP8X alpha flag clear, lossless or animated alpha | straight samples treated as premultiplied (drawn as the clamped unpremultiplication) | alpha dropped or straight |
| VP8X alpha flag clear, still lossy with ALPH | ALPH ignored; opaque with the raw VP8 colors | straight alpha |

`canonical_webp` (`render/asset/webp.rs`) reduces the container to what the
reference interprets before `image-webp` decodes it. The ICC profile is read
directly, and `skia_srgb.rs` ports Skia's matrix-and-curve snapping to sRGB
(CICP, 0.01 matrix, 0.001 parametric or 1/512 round-trip curves). Skia's third
route, `skcms_ApproximatelyEqualProfiles`, compares sampled transforms with
skcms's own arithmetic and is not replicated. Spec section 8.2 states the WebP
rules.

## Evidence

`tests/oracle/webp-container/reference.json` holds 195 rows collected by
`tools/reference-oracle/run-webp-container.sh`. The pinned official v0.18.1
export ran in two fresh processes in forward and reverse order, with a fresh
context per case and no network. Every row edits only the container of a
frozen external libwebp vector, and ICC profiles come from the PNG oracle's own
profiles. Rows cover:

- orientation, 52: EXIF values 0-9, prefixed, flag-clear and garbage EXIF,
  over lossless, lossy, alpha and animated bases;
- color profile, 50: v2/v4 matrix, LUT, Lab-LUT, gray, CMYK, Lab-model,
  unparseable and sRGB-equivalent profiles over opaque and alpha lossless and
  lossy bases; flag clear, missing chunk, duplicate, trailing, oversized and
  animated profiles;
- container, 48, and the 12 #191 controls;
- animation, 21: three and five frames, reserved bits, the alpha flag, EXIF,
  duplicate ANIM;
- alpha, 5, and 7 bases.

Each row declares the base it must equal exactly when the reference draws it
like its base. The loader verifies that equality against the reference, and
the test verifies it on our side.

`webp_container_contract` judges every row in all three formats. A row the
reference rejects must recover exactly as a missing asset. An admitted row
needs an exact report, the exact source octets in SVG, and a canonical match
against the reference in PNG. In PDF it needs a canonical match when opaque,
and exact alpha otherwise.

The vectors are 9×7, so the small-artifact rule requires exact views. 34
color-managed views differ from Skia's float arithmetic by at most 4 channel
units on at most 12 pixels, with 0 Pixelmatch changes. They are held at
exact-ceiling exceptions (`tests/oracle/webp-container/icc-exceptions.json`,
spec section 14.4). A dead exception, or one that goes unused, fails.

One row is a known divergence pinned to its exact current output (#197): a
still lossless image with alpha, its VP8X alpha flag clear, and a transforming
ICC profile.

`webp_limit_contract` covers byte, dimension, pixel and work equality and first
crossing for lossless, lossy and animated WebP in every format. It also covers
the 13.2 multi-crossing precedence. The renderer already complied.

## Owners

| Facet | Owner |
| --- | --- |
| orientation | `webp_container_contract::webp_orientation_metadata_is_ignored_as_the_reference_ignores_it` |
| color-profile | `webp_container_contract::webp_color_profiles_apply_as_the_reference_applies_them` |
| malformed | `webp_container_contract::webp_containers_and_malformed_payloads_match_the_reference` |
| passthrough | `webp_container_contract::webp_svg_embeds_every_admitted_payload_exactly` |
| alpha | `webp_container_contract::webp_alpha_matches_the_reference` |
| default-frame, animation | `webp_container_contract::webp_default_frame_and_animation_match_the_reference` |
| limits | `webp_limit_contract::webp_limits_have_exact_boundaries_and_precedence` |

The #191 control rows move to their reference-backed owners.
`vp8x-reserved-flag` joins the nine rejected controls
(`malformed_controls_reject_exactly_as_the_reference_rejects_them`).
`anmf-reserved-flag` and `intrinsic-frame-mismatch` join `vp8x-reserved-byte`
as accepted controls
(`reserved_and_intrinsic_controls_are_accepted_as_the_reference_accepts_them`).
Their policy criteria are updated to match.
