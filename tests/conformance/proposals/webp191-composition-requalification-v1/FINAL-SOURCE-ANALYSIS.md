# Composition-only source/resource boundary at the authorized freeze

The parent reviewed the production/test change before the exact freeze; this note
records the current source shape and qualification boundary without modifying it.
`checks/parent-qualification.json` and `final-freeze.json` bind exact bytes.

## Standalone-frame allocation and lifetime

`standalone_webp_frame` in `src/render/asset/webp.rs` validates the ANMF header,
reserved flags and contained codec/alpha chunks. It computes the standalone RIFF
size with checked additions and a checked u32 conversion, reserves the complete
buffer once via `try_reserve_exact`, and appends RIFF/WEBP plus an ALPH-required
VP8X header and the borrowed frame chunks. It does not clone the entire input or
grow an unbounded secondary intermediate buffer. The same private helper serves
validation and first-frame decoding, avoiding divergent reconstruction logic.

The decoder verifies standalone dimensions equal the ANMF extent and forbids an
animated standalone result. Placement derives x/y with checked multiply and
canvas-bound checks. Output buffers reserve fallibly, validate exact decoded
sample size, and use checked row/column/byte offsets plus a bounds-checked target
slice. Canvas RGBA starts clear; copied RGB data receives alpha 255 only inside
the first-frame rectangle, leaving the surrounding canvas transparent. Zero-alpha
RGB is cleared. First-frame blend/disposal/background hypotheses do not change
the clear-canvas sample placement.

The original animated decoder is dropped before standalone decode. The decoder
and standalone frame bytes are dropped after sample decoding; decoded samples
are dropped after canvas placement and before ICC normalization/orientation.
These are explicit lifetime reductions. The normal decoder workspace limit remains
set by `webp_decoder`, and the existing metadata/color/orientation paths remain
after RGBA composition.

This establishes the reviewed allocation/bounds/lifetime design and passing
bounded controls. It is **not physical allocator fault injection**, a proof of
every transitive library allocation, or end-to-end OOM/resource closure. Existing
global resource limitations stay explicit. No general PNG premultiplication or
PDF straight-sample policy change is part of the fix.

## Profiles, orientation and dimensions

The existing eight `render::asset::tests::webp_codec` tests are rerun source-bound:
lossless straight alpha, lossy opaque decode, first-frame animation/alpha, EXIF
dimension/permutation, ICC precedence, malformed/truncated containers and later
frame/header rejection. No new browser-tagged WebP matrix is inferred from these
unit controls. Complete artifact/report equality over all 168 packaged inventory
requests supplies an independent transformed-output regression matrix.

Static source bytes and static observation records remain unchanged. Five sample
tests retain exact old/new transition discrimination. The old `d31850…` native
record and original browser/input hashes remain immutable; the new `6c9f48…`
record is a separately bound native composition observation, never an oracle
refresh. Exactly 30 vectors/60 raster records change, including eight improved
but still failing partial-alpha vectors. All 12 partial-alpha cells stay gaps.

## Claims deliberately limited to evidence

Full sample pass counts are SVG72 / PNG48 / PDF48, not all 72 raster vectors.
The 24 partial-alpha vectors and 72 failed comparison views remain exact
SmallArtifactMismatch observations; no finite SSIM tolerance or comparison-rule
change is introduced. Lossless straight-PDF samples remain independently required.
The PDF partial-alpha issue concerns synthetic rounded projection versus browser
floor-premultiplied opaque captures. It is not a measured PDFium/PDF.js failure.

The external diagnosis is causal reading/probing, not a newly authorized static
PNG floor fix or PDF color substitution. Those require their own decision and
qualification. Composition owners qualify only the eleven explicitly named
opaque/binary animation cells under their existing unchanged all-format gates.
