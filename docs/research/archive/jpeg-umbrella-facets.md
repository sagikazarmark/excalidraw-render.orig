# JPEG umbrella facets (#85)

This closes the eight coarse JPEG facets: alpha, color-profile,
default-frame, hdr-gain-map, limits, malformed, orientation and passthrough.
Fourteen JPEG cells stay open as their own gaps: the six baseline-sampling
cells wait on #187 and the eight HDR-base projection cells on #190.

## Reference decoder

Chromium 149 decodes JPEG with Blink's `JPEGImageDecoder` over libjpeg-turbo.
Blink reads EXIF orientation and ICC profiles itself and applies them through
Skia. Excalidraw admits an image through `onload`/`onerror`. Blink fires
`onload` once the header gives a size, so a damaged file loads and paints only
what the decoder reached.

## Findings

A probe of about 30 container variants showed the renderer's strict JPEG path
rejecting many files that the reference draws completely, exactly like the
clean image:

- trailing data;
- an appended image;
- junk and fill bytes between segments;
- stray `RST` and `TEM` markers;
- a missing DHT, where libjpeg uses the standard tables;
- a progressive file missing only its EOI.

The existing ICC record showed the same for 56 malformed or incomplete ICC
rows, which the reference draws untagged.

In other cases the reference paints only part of the image:

- truncated scans;
- a baseline file missing its EOI;
- a missing DQT;
- a restart interval without its markers.

Every file the renderer rejected that the reference also rejects agreed.

## Policy (chosen by the user on #85)

This is the same rule as for GIF and BMP:

- Match what the reference draws completely.
- Keep strict decode-failed recovery where the reference paints only partially.
- Keep the strict recovery the user approved for an invalid MPF gain-map
  index, as a declared divergence; the reference draws the primary image
  there.
- Close hdr-gain-map with HDR-base projection carried as the pinned #190
  divergence, as the WebP #197 row was.

Spec section 8.2 states the JPEG rules. `render/asset/jpeg_markers.rs` reads
the first image as libjpeg reads it and, where zune-jpeg would differ, builds
a corrected copy for it to decode. The rules cover marker skipping, standard
Huffman tables, progressive coverage, restart counting, color-space
precedence and ICC assembly. JPEG ICC now snaps sRGB-equivalent profiles and
ignores unparseable or mismatched ones, as the PNG and WebP paths do.

## Evidence

`tests/oracle/jpeg-container/reference.json` holds 65 rows. It is collected by
`tools/reference-oracle/run-jpeg-container.sh` in two fresh processes, forward
and reverse, with a fresh context per case and no network. The rows edit
frozen project-authored JPEGs, and each row is drawn at its natural size.
The groups cover:

- malformed markers and entropy;
- partial data;
- default images;
- orientation: all eight values, out-of-range values, other tag types and
  counts, IFD1-only, XMP-only, EXIF over XMP, the `Exif\0\xff` header and a
  looping IFD;
- color: sRGB v4, P3 and linear profiles, a truncated profile, and profiles on
  gray and CMYK images, plus JFIF over Adobe and component identifiers;
- ISO 21496-1 and GContainer-only gain-map containers.

`jpeg_container_contract` judges each row:

- A `match` row passes the canonical comparison in PNG and PDF, draws opaque,
  and keeps its octets in SVG.
- A `partial` or `rejected` row recovers exactly as a missing asset.

Each facet owner also runs the existing record suites for its domain:
`jpeg_exif_contract`, `jpeg_icc_contract`, `jpeg_color_contract` and
`jpeg_gain_map_contract`. `jpeg_limit_contract` covers byte, dimension, pixel
and work equality and first crossing, and the 13.2 precedence, for baseline,
progressive, CMYK, EXIF-rotated and gain-map JPEGs.

Twenty-eight JPEG ICC rows used to require strict recovery for malformed
profiles. Their criteria are rewritten to the reference-backed drawing, and
they move to `jpeg_icc_contract::malformed_profiles_and_sequences_draw_untagged_as_the_reference`.

## Owners

| Facet | Owner |
| --- | --- |
| malformed | `jpeg_container_contract::jpeg_containers_and_malformed_payloads_match_the_reference` |
| default-frame | `jpeg_container_contract::jpeg_default_image_is_the_first_as_in_the_reference` |
| orientation | `jpeg_container_contract::jpeg_orientation_matches_the_reference` |
| color-profile | `jpeg_container_contract::jpeg_color_profiles_apply_as_the_reference_applies_them` |
| hdr-gain-map | `jpeg_container_contract::jpeg_gain_maps_match_the_reference_with_the_190_divergence_pinned` |
| passthrough | `jpeg_container_contract::jpeg_svg_embeds_every_admitted_payload_exactly` |
| alpha | `jpeg_container_contract::jpeg_draws_opaque_in_every_raster_view` |
| limits | `jpeg_limit_contract::jpeg_limits_have_exact_boundaries_and_precedence` |
