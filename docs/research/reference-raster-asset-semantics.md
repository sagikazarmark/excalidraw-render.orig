# Reference raster-asset semantics

Research date: 2026-07-18

Ticket: [#19, Research reference raster-asset semantics](https://github.com/sagikazarmark/excalidraw-render.orig/issues/19)

Map: [#1, Specify perceptually equivalent Excalidraw static exports](https://github.com/sagikazarmark/excalidraw-render.orig/issues/1)

## Executive answer

Pinned Excalidraw `v0.18.1` does almost no raster interpretation in its export
logic. It accepts scene-file MIME labels for PNG, JPEG, GIF, WebP, BMP, ICO,
AVIF, and JFIF. Its SVG painter copies the supplied `dataURL` into an SVG
`<image href>`; its PNG painter loads the same URL into a detached browser
`HTMLImageElement`, draws that image to a default 2D canvas, and asks Chromium
to encode the complete canvas as PNG. JFIF is a JPEG MIME alias in this surface,
not a separate codec. PNG includes APNG even though Excalidraw does not name
APNG separately. Sources: pinned Excalidraw
[`IMAGE_MIME_TYPES`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts#L204-L214),
[`updateImageCache()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts#L14-L81),
[SVG image emission](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L408-L565), and
[public export wrappers](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts#L36-L193).

The observable format split is sharp:

- **Direct SVG export preserves the source representation.** For an ordinary
  base64 data URL, the encoded PNG/JPEG/GIF/WebP/BMP/ICO/AVIF/JFIF bytes,
  metadata, profiles, alpha representation, and animation remain embedded.
  Excalidraw does not select a frame, apply EXIF/container orientation, perform
  color conversion, premultiply alpha, or re-encode the nested asset. The
  consumer's image decoder does those things when the SVG is displayed. An
  APNG, GIF, animated WebP, or animated AVIF therefore remains animated in the
  official SVG.
- **Direct PNG export always decodes and re-encodes.** Chromium content-sniffs
  the data, selects the detached HTML image's default frame, applies supported
  orientation, color-manages and tone-maps into the default canvas's `sRGB`,
  `unorm8`, standard-tone-mapping surface, composites premultiplied source-over,
  and serializes an 8-bit RGBA PNG. Source codec bytes and metadata, EXIF,
  ICC/CICP/gamma/HDR chunks, and animation are not copied to the PNG.
- **The default frame is frame zero, not a timing sample.** Chromium's canvas
  source path for an `HTMLImageElement` explicitly returns
  `ImageForDefaultFrame()`. This applies to APNG, GIF, animated WebP, and
  animated AVIF. ICO is different: it is a non-temporal multi-image container;
  Chromium sorts entries by descending area and then bit depth and uses its
  first entry. Sources: Chromium
  [`ImageElementBase::GetSourceImageForCanvas()`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/core/html/canvas/image_element_base.cc),
  [`BitmapImage::ImageForDefaultFrame()`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/bitmap_image.cc), and
  [`ICOImageDecoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/ico/ico_image_decoder.cc).

This is not one straightforward pure-Rust behavior to copy. Matching official
SVG means preserving original bytes, profiles, and live animation and accepting
viewer/profile-dependent appearance. Matching official PNG means implementing
the pinned browser's first-frame composition, orientation, codec-specific color
metadata precedence, color conversion, HDR/gain-map tone mapping, alpha math,
and final untagged RGBA8 PNG appearance. Flattening and canonicalizing the SVG
instead can satisfy the destination's stronger deterministic/static policy, but
it necessarily ceases to match official SVG byte-preservation and animation
semantics. A human compatibility-policy decision is therefore unavoidable; this
report does not choose it.

## Authority and scope

### Pinned sources

The reference renderer is Excalidraw tag `v0.18.1`, commit
[`a2ec2889babf7d2295469c6d90ebe77fae57df84`](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84).
The reference oracle selected by the map is Playwright `1.61.0` with Chromium
`149.0.7827.55` in the pinned Linux/amd64 image. Chromium's
[`DEPS`](https://github.com/chromium/chromium/blob/149.0.7827.55/DEPS)
pins Skia commit
[`53348aa333da02b77c4b5797e2de722f5abde7d0`](https://github.com/google/skia/tree/53348aa333da02b77c4b5797e2de722f5abde7d0)
and CrabbyAvif commit
[`c433c9a32320aed983e4106931596fbbae3f77ee`](https://github.com/webmproject/CrabbyAvif/tree/c433c9a32320aed983e4106931596fbbae3f77ee).

### Two input seams must not be conflated

The renderer destination begins with a scene document. The public
`exportToSvg()` and `exportToBlob()` wrappers therefore receive already-stored
`BinaryFiles` records containing a MIME label and `dataURL`; they do not invoke
the editor's file picker, resize, or compression path. `BinaryFileData` permits
every `IMAGE_MIME_TYPES` value plus `application/octet-stream`, while canvas
loading rejects only that binary label. Sources:
[`BinaryFileData`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/types.ts#L90-L120) and
[`updateImageCache()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts#L29-L81).

This report covers the declared raster labels in `IMAGE_MIME_TYPES` and excludes
`image/svg+xml`, which is a nested vector-image path. At runtime the public
export wrapper does not revalidate the TypeScript union: any non-binary file
record whose `dataURL` Chromium can content-sniff may render. That larger
accidental decoder surface is not an accepted-format promise.

The interactive editor insertion path is observably different and is relevant
only if a conformance case is defined as "import this host file through the
editor, then export". It accepts the same image MIME list, normalizes SVG, and
passes every raster through `resizeImageFile(..., maxWidthOrHeight: 1440)`
before storing a data URL. Even an image that does not need downscaling is
drawn through Pica and canvas and encoded again; resize failure is caught and
falls back to the original file. Sources:
[`initializeImage()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/components/App.tsx#L9707-L9804),
[`resizeImageFile()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/blob.ts#L296-L343), and
[`DEFAULT_MAX_IMAGE_WIDTH_OR_HEIGHT`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts#L307-L313).

The pinned lock selects `image-blob-reduce 3.0.1` and `pica 7.1.1`.
`image-blob-reduce` always creates an output canvas, resizes into it, and calls
Pica `toBlob()` with the input MIME. Pica delegates to `canvas.toBlob()`, whose
pinned Chromium encoder supports PNG, JPEG, and WebP only. For GIF, BMP, ICO,
and AVIF, canvas serialization falls back to PNG, after which Excalidraw wraps
the bytes in a `File` carrying the original MIME label. Thus an editor-inserted
asset can contain PNG bytes under `image/gif`, `image/bmp`, `image/x-icon`, or
`image/avif`; Chromium later succeeds because it content-sniffs. JFIF bytes are
recognized by the JPEG plugin and become re-encoded JPEG under the
`image/jfif` label. Sources: pinned Excalidraw
[`package.json`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json),
[`yarn.lock`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/yarn.lock),
[`image-blob-reduce 3.0.1`](https://github.com/nodeca/image-blob-reduce/blob/6749ecde7d176591a65fe894fcdffe1b09d2a4d4/index.js),
[`pica 7.1.1`](https://github.com/nodeca/pica/blob/4228a45898036ab7cf0232d1ee4a70e0c3579d28/index.js), and
Chromium's pinned
[`ImageEncoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-encoders/image_encoder.cc).

JPEG insertion has one additional Excalidraw-dependency behavior. The
`image-blob-reduce` JPEG plugin reads EXIF orientation, patches it to `1` before
browser decode, physically rotates/flips the resized output canvas, re-encodes
JPEG, and reattaches original APP segments and comments except APP2 ICC. The
result keeps some original metadata, drops ICC, normalizes orientation to `1`,
and never preserves original compressed image bytes. Source:
[`jpeg_plugins.js`](https://github.com/nodeca/image-blob-reduce/blob/6749ecde7d176591a65fe894fcdffe1b09d2a4d4/lib/jpeg_plugins.js).

Unless a test explicitly performs interactive insertion, the rest of this
report describes the direct scene-document export authority.

## Excalidraw export logic

### SVG

For an initialized image element with a matching file record, Excalidraw creates
an SVG `<image>`, assigns `fileData.dataURL` directly to `href`, and places it
inside a reusable `<symbol>`. Crop, mirror, element rotation, opacity, rounded
clip, and frame clip are outer SVG operations; they do not alter the nested
raster bytes. In dark mode, all non-SVG image MIME labels receive the project's
additional CSS filter before the root dark filter. Source:
[pinned SVG image branch](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L408-L565).

Consequences for every accepted raster format:

- Excalidraw performs no frame selection or animation flattening.
- Excalidraw performs no orientation, profile, gamma, gamut, HDR, tone-map, or
  alpha conversion.
- Base64 payload bytes are preserved. XML serialization can escape attribute
  syntax, but decoding the emitted URL yields the same supplied bytes.
- The displayed result is delegated to the SVG consumer. Under pinned
  Chromium, APNG, GIF, animated WebP, and animated AVIF continue to advance;
  profile and HDR appearance is evaluated against that consumer's color and
  display configuration.

### PNG

Canvas export loads every initialized file concurrently into a detached
`new Image()`, waits for all `onload` promises, then invokes the canvas renderer.
The image branch calls `drawImage()` with either the full oriented natural
dimensions or the stored crop rectangle. Missing/failed images receive the
canvas placeholder. Sources:
[`exportToCanvas()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L153-L255),
[`loadHTMLImageElement()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts#L14-L81), and
[canvas image painting](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts#L429-L469).

Excalidraw asks for `canvas.getContext("2d")` without color, alpha, pixel-format,
or tone-map options. Chromium `149` defaults that context to alpha enabled,
`sRGB`, `unorm8`, and standard tone mapping. The initial bitmap is transparent
black; Excalidraw optionally paints the scene background, then uses ordinary
source-over compositing and element/frame opacity. Sources:
[`bootstrapCanvas()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/helpers.ts#L30-L74),
Chromium's pinned
[`CanvasRenderingContext2DSettings`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/modules/canvas/canvas2d/canvas_rendering_context_2d_settings.idl), and
the HTML Standard's
[canvas color/conversion and serialization sections](https://html.spec.whatwg.org/multipage/canvas.html#colour-spaces-and-colour-correction).

`exportToBlob()` then calls `canvas.toBlob(..., "image/png")`. Chromium reads
the canvas into an unpremultiplied pixmap and uses the pinned Rust-backed Skia
PNG encoder at low compression. A default sRGB canvas does not receive an
embedded ICC profile because the encoder writes one only for a non-sRGB source
color space. Source metadata is not consulted by this stage. Sources:
[public `exportToBlob()`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts#L98-L159),
Chromium
[`ImageDataBuffer`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/image_data_buffer.cc) and
[`ImageEncoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-encoders/image_encoder.cc), and pinned Skia
[`SkPngRustEncoderImpl`](https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/encode/SkPngRustEncoderImpl.cpp) and
[`SkPngEncoderBase`](https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/encode/SkPngEncoderBase.cpp).

If scene embedding is requested, Excalidraw inserts its own `tEXt` chunk before
`IEND` after Chromium encoding. That metadata is unrelated to source image
profiles or metadata. Source:
[`encodePngMetadata()` call](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts#L126-L155).

## Shared Chromium decoder behavior

Chromium content-sniffs JPEG, PNG, GIF, WebP, ICO, BMP, and AVIF signatures,
then creates the matching decoder. Untagged decoded content is assumed sRGB.
Normal HTML images use `ColorBehavior::kTag`: the decoder keeps or approximates
the embedded source color space and attaches it to the decoded SkImage; drawing
that image to the sRGB canvas performs the destination conversion. Unsupported
profiles can be transformed at decode time to a representable gamut/transfer
function, with sRGB as the fallback when no usable gamut exists. Sources:
[`ImageDecoder` sniffing and color behavior](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/image_decoder.cc),
[`ColorBehavior`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/color_behavior.h), and
[`BitmapImage::SetData()`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/bitmap_image.cc).

Canvas `drawImage()` from an HTML image does not sample its live animation
clock. `ImageElementBase::GetSourceImageForCanvas()` returns
`source_image->ImageForDefaultFrame()`, and `BitmapImage` marks that generated
paint image non-animating. Frame zero is the fully composited first frame, so
format disposal/blend rules still affect it where applicable. The same source
path respects supported image orientation. Sources:
[`ImageElementBase`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/core/html/canvas/image_element_base.cc),
[`BitmapImage`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/bitmap_image.cc), and
[`Canvas2DRecorderContext`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/modules/canvas/canvas2d/canvas_2d_recorder_context.cc).

Decoded frames use premultiplied alpha for this path. Canvas compositing remains
premultiplied internally; PNG serialization reads unpremultiplied pixels. Color
under alpha zero is observably canonicalized to transparent black, while
partially transparent channels can incur the expected 8-bit premultiply/
unpremultiply rounding. Sources:
[`ImageDecoder::AlphaOption`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/image_decoder.h),
[`ImageDataBuffer`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/image_data_buffer.cc), and
the HTML Standard's
[premultiplied-alpha section](https://html.spec.whatwg.org/multipage/canvas.html#premultiplied-alpha-and-the-2d-rendering-context).

## Format-specific matrix

The table describes direct scene-file export. "SVG: pass through" means the
source behavior is deferred to the SVG consumer. "PNG: frame 0" means the
pinned detached-HTML-image canvas path described above.

| Accepted label | Animation/frame behavior | Orientation | Color, gamma, HDR | Alpha | SVG versus PNG bytes |
| --- | --- | --- | --- | --- | --- |
| `image/png` | Static PNG or APNG. SVG preserves APNG animation. PNG draws APNG's composited frame 0. | PNG `eXIf` orientation is parsed and honored by Chromium. | Valid PNG `cICP` has highest precedence, then `iCCP`, then `sRGB`, then usable `gAMA` plus optional `cHRM`; no usable tag means sRGB. Pinned Skia also reads `mDCV`/`cLLI` HDR metadata. High-bit-depth input can decode above 8 bits before the canvas conversion. | Grayscale/RGB transparency, palette `tRNS`, gray-alpha, and RGBA are decoded; APNG blend/disposal is honored. | SVG preserves the complete PNG/APNG. PNG emits new 8-bit RGBA pixels with no source chunks. |
| `image/jpeg` | One base image; JPEG gain maps are auxiliary HDR data, not temporal animation. | EXIF Orientation is parsed and honored. | RGB, gray, and compatible CMYK ICC profiles are parsed. Untagged input is sRGB. Pinned Chromium can extract JPEG gain-map data and apply it in the paint image. | Opaque. | SVG preserves JPEG bytes, EXIF, ICC, and gain-map metadata. PNG bakes orientation/color/tone mapping and discards all JPEG metadata. |
| `image/jfif` | Same JPEG bitstream behavior. | Same as JPEG. | Same as JPEG. | Opaque. | Chromium's content sniffing decodes JPEG bytes despite the `image/jfif` label. Probe output exactly matched `image/jpeg`; SVG retains the supplied label and bytes. |
| `image/gif` | SVG remains animated. PNG draws composited frame 0. | No supported EXIF/container orientation. | No ICC/CICP/gamma/HDR path in the pinned GIF decoder; pixels are treated as sRGB palette colors. | GIF transparency is binary; frame disposal/composition is decoded. | SVG preserves GIF bytes. PNG emits RGBA8. |
| `image/webp` | SVG remains animated. PNG draws composited frame 0. | The pinned WebP decoder does not apply the EXIF chunk's Orientation. | Static WebP reads RGB ICC. Pinned Chromium explicitly disables `ICCP` for animated WebP, so animated WebP is treated as untagged sRGB. No CICP/HDR path is present. | Static and animated WebP alpha is decoded; animated blend/disposal is honored. | SVG preserves all chunks, including ICC/EXIF/XMP and animation. PNG emits converted frame-zero RGBA8 and no WebP metadata. |
| `image/bmp` | Static. | Top-down/bottom-up storage is normalized; there is no EXIF orientation. | Windows V4/V5 calibrated RGB endpoints and per-channel gamma, explicit sRGB/Windows space, and embedded ICC are supported. Linked external profiles are ignored. | Explicit V4+ alpha masks and `ALPHABITFIELDS` are supported. Standalone Windows V3 32-bit BMP alpha is deliberately ignored; BMP-in-ICO has separate alpha handling. | SVG preserves BMP bytes. PNG emits converted RGBA8. |
| `image/x-icon` | ICO is multi-resolution, not animated. Entries are sorted by descending area then bit depth; the best entry is frame 0. | Top-level ICO exposes no orientation; an embedded PNG orientation is not propagated as top-level ICO orientation. | Entries are decoded through the pinned BMP or PNG path. | Embedded PNG alpha is retained; BMP entries use alpha masks and/or the ICO AND mask. | SVG preserves the full ICO and lets its consumer choose/decode; pinned Chromium PNG uses its sorted default entry and emits RGBA8. |
| `image/avif` | SVG remains animated when an image-sequence track is present. PNG draws frame 0. | AVIF `irot`/`imir` item properties are honored in clean-aperture, rotate, then mirror order. The decoder explicitly ignores EXIF metadata, including EXIF Orientation. | ICC overrides CICP. Otherwise CICP primaries, transfer, matrix, and range are used; unspecified values receive documented BT.709/sRGB/BT.601 defaults. High-bit-depth, P3/Rec.2020, PQ/HLG, CLLI, and AVIF gain maps enter Chromium's HDR pipeline before conversion/tone mapping to the default canvas. | AVIF alpha planes are decoded, including high-bit-depth input, then quantized to canvas RGBA8. | SVG preserves AVIF bytes and HDR/animation metadata. PNG emits tone-mapped/converted frame-zero RGBA8. |

Owning primary sources for the matrix are pinned Skia
[`SkPngRustCodec`](https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/codec/SkPngRustCodec.cpp),
Chromium
[`JPEGImageDecoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/jpeg/jpeg_image_decoder.cc),
[`GIFImageDecoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/gif/gif_image_decoder.cc),
[`WEBPImageDecoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/webp/webp_image_decoder.cc),
[`BMPImageReader`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/bmp/bmp_image_reader.cc),
[`ICOImageDecoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/ico/ico_image_decoder.cc), and
[`AVIFImageDecoder`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/avif/avif_image_decoder.cc).

## Observable PNG serialization

Under the pinned default canvas path, the final PNG is a new complete-scene
encoding, not a container for any source raster asset. The pinned encoder uses
low compression and chooses unpremultiplied RGBA for an alpha-capable 8-bit
canvas. This remained PNG color type 6 even when the one-pixel test canvas was
fully painted opaque. The source image's codec, palette, bit depth, profile,
gamma, EXIF, HDR, textual metadata, and compressed bytes leave no direct trace.
Sources: Chromium
[`ImageEncoder::Encode()`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-encoders/image_encoder.cc) and pinned Skia
[`SkPngEncoderBase::getTargetInfo()`](https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/encode/SkPngEncoderBase.cpp).

The oracle probe serialized a two-pixel transparent canvas as a 91-byte PNG
with `IHDR` (`8` bits, color type `6`), two `IDAT` chunks, and `IEND` only. It
contained no `sRGB`, `gAMA`, `cHRM`, `cICP`, `iCCP`, `eXIf`, `pHYs`, or HDR
chunk. A supplied invisible pixel `[255,17,33,0]` read back and round-tripped as
`[0,0,0,0]`; `[200,100,50,128]` became `[199,100,50,128]` before and after
serialization. This is executable behavior of the selected oracle, not a
portable PNG-standard guarantee.

## Environment- and profile-dependent observations

The following are observable under a fully specified reference profile but are
not universal Excalidraw rules:

- SVG appearance inherits the consuming viewer's decoder versions, animation
  clock/policy, output/display color profile, HDR capability and headroom,
  tone-mapping policy, and image resampling. Excalidraw fixes none of these in
  the SVG bytes because it preserves the nested source.
- PNG source conversion has a fixed logical destination (`sRGB`, `unorm8`,
  standard tone mapping), but exact ICC approximation, gamut mapping, HDR/gain-
  map tone mapping, resampling, premultiply rounding, and filter pixels are
  Chromium/Skia implementation behavior. GPU/software selection and launch
  color flags are part of the reference profile, not the scene document.
- Unsupported or malformed profiles can be ignored, approximated, transformed,
  or reject the image according to the individual pinned decoder. The table
  states valid-profile routes; it is not a promise that every ICC class,
  version, or malformed metadata combination decodes.
- Untagged final PNG pixels are intended as sRGB by the canvas pipeline, but the
  serialized file has no sRGB/profile/gamma chunk in the pinned probe. A
  different PNG consumer can therefore apply a different untagged-image
  assumption even though canonical Chromium treats it consistently.

Chromium's source explicitly distinguishes tagging, conversion to sRGB, and
ignoring profiles in
[`ColorBehavior`](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/graphics/color_behavior.h).
Playwright likewise documents that screenshot pixels depend on the browser,
OS, settings, hardware, and headless mode; this is why the map selected one
canonical image rather than a browser-independent oracle. Source:
[`Playwright v1.61.0 visual comparisons`](https://github.com/microsoft/playwright/blob/v1.61.0/docs/src/test-snapshots-js.md).

## Oracle probes

Disposable probes ran in the selected Linux/amd64 Playwright image
`sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9`
with package `playwright@1.61.0`, Chromium `149.0.7827.55`, locale `en-US`,
timezone `UTC`, device scale factor `1`, and light color scheme. A Node setup
step fetched fixtures from the immutable Chromium tag; the browser received
only data URLs and made no resource requests. The probe reported
`HeadlessChrome/149.0.7827.55` and canvas attributes `alpha: true`,
`colorSpace: "srgb"`, `colorType: "unorm8"`, and
`toneMapping.mode: "standard"`.

Verified observations:

- PNG, JPEG, JFIF-labelled JPEG, GIF, WebP, BMP, ICO, and AVIF all loaded.
  JPEG and the same bytes labelled `image/jfif` had identical oriented natural
  dimensions (`82x1281`) and identical canvas pixel hash.
- Repeated detached-HTML-image canvas draws remained on one identical pixel
  hash for APNG, GIF, animated WebP, and animated AVIF through approximately
  1.9 seconds. This independently confirms the source-level default-frame rule.
- The same bytes embedded as live SVG `<image>` nodes changed rendered
  screenshot hashes for all four formats: APNG by 1 second, GIF by 50 ms, WebP
  by 1 second, and AVIF by 50 ms. Exact transition times are fixture/scheduler
  observations; only preservation of animation is the compatibility finding.
- A 10-bit BT.2020/PQ AVIF fixture loaded and read back as sRGB RGBA8; an AVIF
  alpha fixture retained transparent and half-alpha pixels. These verify the
  route, not an exhaustive HDR tone-map function.
- Base64 `href` values round-tripped unchanged through the SVG DOM for every
  fixture.
- Both transparent and fully opaque default canvases serialized as 8-bit RGBA
  PNG color type 6. The exact transparent two-pixel PNG SHA-256 was
  `b472ad7edcd28dce0c66199a274421b547858b5e3c77c4a0e6169ead971df781`.

Representative immutable fixtures:

- [APNG](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/png-animated-three-independent-frames.png)
  SHA-256 `6dae5081855e61c8c2a867bed804018cead17fc36102ac42b32dcf876fceae33`.
- [EXIF-oriented JPEG](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/jpeg-height-exif-orientation.jpg)
  SHA-256 `054be6141f397726f79e1afdc6226f4dc54bcf7b32807c66d6cda681a374c2c0`.
- [Animated GIF](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/animated.gif)
  SHA-256 `1efe744f4f55c13a616396539c07cdfcf9941210c2b430fa73a1bae092c4e6f2`.
- [Animated WebP](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/webp-animated.webp)
  SHA-256 `292753f066add623af1e30bbeeca58f15b3d6d1052e12b13e30f21f8d3c14505`.
- [BMP](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/gracehopper.bmp)
  SHA-256 `728db5e97ec4723f086b3b72439301b78d5b738dcfa3dc4cf2d84a6f19060edb`.
- [Multi-entry ICO](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/2entries.ico)
  SHA-256 `3085fb0456e4f109cf630262c7055aca7fdcb4df686ed8a86ae7ee1659caca33`.
- [Animated AVIF](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/avif/star-animated-8bpc.avif)
  SHA-256 `ae35b161de67a5afeb195ee401f369c34990f0ff8662f70ab4065bc6931f0a66`.
- [10-bit BT.2020/PQ AVIF](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/web_tests/images/resources/avif/red-full-range-bt2020-pq-444-10bpc.avif)
  SHA-256 `89dc8e9c16ee0e468e2bebfb45b83a4f404cf04fd08d38b55003687dbce9157b`.

## Compatibility consequence

The evidence sharpens, and partially contradicts, the prior compatibility
resolution. That resolution says to decode frame zero of animated GIF, WebP,
and AVIF "for every static format." Pinned Chromium proves frame zero for PNG,
but official SVG preserves live animation; it also reveals APNG as a fourth
accepted animated raster container. The same conflict exists for color and
bytes: official SVG preserves original profiles/HDR metadata and delegates
conversion, while official PNG converts to its sRGB canvas and discards the
source representation.

A self-contained pure-Rust renderer can straightforwardly match the official
SVG *representation* by validating and embedding accepted source bytes without
decoding them. That leaves viewer-dependent appearance and live animation. It
can target official PNG appearance only with individually pinned decoders plus
the browser's orientation, frame composition, ICC/CICP/gamma precedence,
color conversion, HDR/gain-map tone mapping, premultiplied compositing, and
RGBA8 projection. Common SDR formats are tractable; AVIF/HDR/gain maps and exact
browser color behavior are a substantial compatibility implementation, not a
simple generic RGBA decode.

It cannot simultaneously preserve official SVG's original bytes/profiles/live
animation and enforce deterministic first-frame canonical-sRGB SVG assets. One
must be a documented compatibility exception with diagnostics and its own
conformance authority.

### Proposed HITL decision ticket

**Title:** Choose revision-1 raster normalization and SVG passthrough policy

**Question:** For direct scene-document raster assets, should compatibility
revision 1 preserve official `v0.18.1` format-specific behavior (SVG embeds the
validated original bytes and retains APNG/GIF/WebP/AVIF animation and source
profiles; PNG uses the oriented, color-managed/tone-mapped frame-zero sRGB
canvas result), or declare a compatibility exception that flattens and
normalizes raster assets to deterministic first-frame sRGB in SVG as well?
Specify per-format diagnostics, whether original-byte preservation survives,
and whether conformance uses direct scene-file bytes or editor-insertion
re-encodings. This report makes no selection.

## Residual unknowns

1. The map selects a canonical Playwright image and browser, but the literal
   reference-profile manifest and digest, including exact Chromium launch/color
   flags and software-versus-GPU controls, remain to be materialized by the
   open reference-profile work. The probes used the pinned image's default
   headless launch. Exact profile-dependent SVG display and HDR tone-map pixels
   remain unknown until those literal settings are frozen.
2. No exhaustive pixel corpus was run over every valid and malformed ICC
   version/class, PNG cICP/gamma/chromaticity/HDR combination, JPEG/AVIF gain
   map, AVIF CICP transfer/primaries/range combination, or BMP calibrated/ICC
   variant. The owning source establishes routing and precedence, but exact
   output vectors for that combinatorial surface remain unknown.
3. The exact canonical pixels for HDR and gain-map inputs are not yet known for
   the eventual reference profile. The source establishes high-bit-depth
   decode, metadata propagation, and conversion into the standard sRGB/unorm8
   canvas; the one PQ and one alpha probe do not specify Chromium's complete
   tone-mapping function.
4. Appearance of an official SVG in non-reference viewers remains intentionally
   unknown and cannot be inferred from the artifact alone. Such viewers can
   differ in animation policy, profile support, untagged assumptions, HDR,
   alpha rounding, and ICO entry choice.
5. The product authority between direct scene-document `BinaryFiles` and the
   editor's insertion re-encoding is not a source fact. The renderer's stated
   input is a scene document, which favors direct bytes, but the proposed HITL
   decision must state this explicitly so conformance cases do not mix the two
   seams.

These unknowns do not obscure the policy edge: official SVG passthrough and a
flattened canonical-sRGB SVG are mutually exclusive observable contracts.
