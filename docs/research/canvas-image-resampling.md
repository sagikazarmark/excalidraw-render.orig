# Canvas image resampling (#198)

The renderer's PNG output used to resample every raster image with one
bilinear sample at each destination pixel's centre (#165). That matched the
pinned reference for magnification and for most 1:1 and mild draws. It did
not match strong minification: a 25×25 image drawn at 12×12 differed by up
to 126 channel units on 142 of 144 pixels. Some mild downscales differed by a
few units too. The defect affected every raster codec.

## Finding

The pinned profile launches Chromium 149.0.7827.55 with `--disable-gpu`, so
the Excalidraw export canvas is rasterized in software. A draw goes through
three stages there. A model of them reproduced 1,005 reference probes with
zero differing pixels: 552 direct-canvas draws in fresh contexts, 385 in one
shared page, and 68 Excalidraw exports.

1. **cc's software image decode cache** pre-scales the image whenever
   smoothing is on. It chooses the deepest level whose next level is not
   smaller than the rounded device size in either axis. Level sizes halve,
   rounding up. Anisotropic draws therefore stay at full size.
2. **Skia's `scalePixels`** builds that level. It uses a linear filter with a
   nearest mipmap: it picks its own mip level from the geometric-mean scale,
   uses Skia's round-down mip chain (a truncating 2×2 box, 1-2-1 taps along
   odd extents), then applies the low-precision pipeline's fixed-point
   bilinear kernel.
3. **Skia's legacy bitmap sampler** (`SkBitmapDevice::drawImageRect`) draws
   the level:
   - a sprite copy when the mapped rectangle is the level's size at an
     integer translate;
   - nearest sampling when the inverse is a translate within 1/256 of whole
     pixels;
   - otherwise, 32.32 fixed-point stepping restarted every 127 pixels, 4-bit
     weights and truncated sums.

   Opacity multiplies by `(α8 + 1) >> 8`.

The cache persists within a render. A later draw of the same image builds its
level from the smallest cached level at least its size, so draw order is
observable. A multi-element export confirmed this.

## Implementation

`render/png/canvas_image.rs` ports the model in binary32, with one rounding
per operation and no fused multiply-add:

- `f32::sqrt` is correctly rounded.
- Skia's `log2` goes through `portable_math::log2` and is rounded to f32.

A per-render `DecodeCachePlanner` decides every cache step from sizes alone,
in paint order. The decisions are keyed by asset identity and the rounded-out
source rectangle. Each built level is charged to the render's resource
accounting. Axis-aligned images keep the snapped block painting of #162.
Rotated images get the chosen level but keep their antialiased bilinear
sampler, because rotation has not been probed.

## Evidence

`tests/oracle/image-resample/reference.json` holds 73 rows over 11 seeded
images. It is collected by `tools/reference-oracle/run-image-resample.sh` in
two fresh processes, forward and reverse, with a fresh context per case and
no network. The rows cover:

- minification sweeps down to 1×1;
- magnification;
- anisotropic scales;
- fractional sizes and offsets;
- crops;
- opacity;
- one image drawn at several sizes in either order.

`image_resample_contract::raster_images_resample_exactly_as_the_reference_canvas`
requires every PNG to equal the reference exactly.

Against their official evidence, the focused image cases improve:

- `image-codecs` goes from 6,791 to 3,655 differing pixels;
- `rounded-image` at scale 2 goes from 4,850 to 4,724.

Their native snapshots change accordingly.

## Not covered

- Rotated images, and spans split by antialiased clips (rounded corners,
  frames).
- Two file ids with identical bytes share cache entries. The browser may
  treat them as separate images.
