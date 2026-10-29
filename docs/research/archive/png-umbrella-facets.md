# PNG umbrella facets and HDR peak luminance (#85)

The four remaining PNG umbrella facets were `codec:png:alpha`,
`codec:png:color-profile`, `codec:png:limits` and `codec:png:hdr-gain-map`.
An audit of the owned atomic rows (`png-stage4-ownership.md`, "honest remaining
scope") found the gaps below. This work closes them, fixing one renderer
defect along the way.

## HDR peak luminance: a renderer defect

The pinned reference tone maps PQ and HLG with Skia's default reference-white
tone map (RWTMO). Its baseline headroom is `log2(max(peak / 203, 1))`, and the
peak comes from the content's metadata (Skia `53348aa…`,
`src/codec/SkHdrAgtm.cpp` `get_max_luminance` and `PopulateToneMapAgtmParams`):
a positive `cLLI` MaxCLL, else a positive `mDCV` maximum luminance, else
1000 nits. `SkPngRustCodec::CreateEncodedInfo` reads both chunks through the
`png` crate before the first `IDAT`. The renderer used a fixed 1000 nits, so
PQ/HLG PNGs carrying these chunks differed from the reference by up to 66
channel units.

A diagnostic probe through the pinned browser mapped the behavior; PQ and HLG
behave identically:

| Metadata | Observed peak |
| --- | --- |
| None, zero values, short or long chunks, chunks after `IDAT` | 1000 (default) |
| `cLLI` MaxCLL at or below 203 | no tone map |
| Positive `cLLI` | MaxCLL, over `mDCV` in either order |
| Zero or absent `cLLI`, positive `mDCV` | `mDCV` maximum |
| Duplicates | the first valid chunk; a malformed first chunk is skipped |
| SDR cICP with `cLLI` | unaffected |

That is png 0.17.16's parsing, which the renderer already pins. The fix in
`asset/hdr.rs` and `asset/color.rs` builds the RWTMO curve from the peak ratio
and applies no tone map at zero headroom. At the default peak the curve stays
bit-identical to the old fixed curve; a unit test checks the bits. Spec
section 8.2 now states the rule.

One divergence is out of scope and unchanged: a `cLLI` with a bad CRC makes
the renderer's strict PNG chunk validation reject the image
(`image.decode-failed`), while the browser ignores the chunk. That is the
existing strict CRC policy, not tone mapping.

Evidence: `tests/oracle/png-color/hdr-metadata-reference.json`, 170 rows
collected by `tools/reference-oracle/run-png-hdr-metadata.sh` in two fresh
processes in forward and reverse order, and reproduced by its `verify` mode.
The rows are 26 metadata variants over PQ (BT.2020 and BT.709 primaries) and
HLG bases at 8 and 16 bits, SDR controls, and gain-map signaling. Each row
declares the variant it must equal exactly. The record's loader checks that
equality on the reference side, and that the distinct peak classes of each base
differ pairwise. `hdr_peak_luminance_and_tone_mapping_paths_match_the_reference`
checks both in all three formats. Against the old fixed-peak code it fails
(SSIM 0.9475 on the first peak variant).

AGTM is unreachable from PNG: `SkPngRustCodec` sets no AGTM metadata, so
`PopulateToneMapAgtmParams` always takes the RWTMO branch for PNG.

## Alpha and color-profile: profile × layout crossings

The frozen composition matrix (1,968 crossings) did not cross the sRGB chunk,
v2 matrix/TRC ICC, SDR/linear/BT.709/Display-P3 cICP or narrow-range cICP with
partial alpha on every layout; an equivalence argument covered them. The
supplemental collection `tests/oracle/png-color/supplemental-composition-reference.json`
(840 crossings, `run-png-supplemental-composition.sh`) replaces that argument
with vectors. It covers eight RGB families and five gray families (sRGB chunk,
gAMA, SDR/linear cICP, PQ) across every RGB/gray/palette layout, intrinsic,
key, palette and absent transparency, both interlace modes and all three APNG
containers, with early `eXIf`. Like the frozen matrix, its reference is an
independently authored static canvas of the selected frame, with the same
profile, samples, alpha and `eXIf`. That avoids Chromium 149's
animated-orientation/natural-size defect. A first collection that rendered the
APNG directly hit exactly that defect, with the wrong alpha on `included` rows,
and was discarded. The original APNG remains the renderer input.
`png_alpha_and_profiles_compose_across_every_layout` runs both matrices in all
formats, including PDF's grayscale soft mask and transparent RGB zero.

## Limits: multi-crossing precedence

The atomic validation and projection owners cover each limit's equality and
first crossing. Spec section 13.2's order for multiple raster crossings (width
precedes height, then area; the image side precedes per-identity pixels) was
unexercised. `png_limit_crossing_contract` covers it over 8-bit, 16-bit and
Adam7 sources in every format, with exact reports and recovery. The renderer
already complied. `png_limits_have_exact_boundaries_and_precedence` runs the
validation boundaries and this precedence.
