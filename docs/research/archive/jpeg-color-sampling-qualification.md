# JPEG color/sampling matrix (#82 / #85 / #187)

## Scope and reference identity

This tranche enumerates **38 untagged 8-bit DCT cells / 76 vectors**: gray, raw
RGB, YCbCr, CMYK and YCCK; baseline and progressive; eight encoder sampling
factors for YCbCr/YCCK and 1×1 for the other models. Each cell has an odd 33×25
color ramp and a 48×32 asymmetric sharp-edge pattern. Chroma, channels and black
vary independently. This is a bounded matrix, not exhaustive JPEG admission.

`jpeg_matrix_inputs.rs` reconstructs the frozen compressed inputs using pinned
`jpeg-encoder 0.6.1`, independently of production `zune-jpeg 0.5.15`. Raw RGB
uses unchanged channel samples through the encoder's direct-YCbCr input, with
JFIF removed, R/G/B component identifiers in SOF/SOS and Adobe transform 0.
An independent header test checks the actual precision, component identifiers,
sampling, dimensions, Adobe/JFIF signaling and scan mode of every source.

Reference collection calls official Excalidraw v0.18.1 SVG and direct PNG export
under the pinned Chromium 149.0.7827.55 profile. Official SVG is independently
rasterized for the PDF sample-color authority. Two fresh browser processes use
forward/reverse case order and fresh contexts, with network disabled. SVG and
PNG outputs must repeat exactly. All 76 JPEG inputs are browser-admitted.

- Inputs SHA-256: `acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258`.
- Reference SHA-256: `9b5ab60a18e83b52c582ccb38728b71633b11b592ffe540bab2c796857694638`.
- Reference profile: `1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71`.
- OCI: `mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9`.

The final collection binds the input author, actual wrapper/build configuration,
package lock, profile/assets, collector/runner and comparator profile. An initial
unapproved collection was superseded before publication to relocate the input
author outside the production-source set and complete its provenance chain;
no approved evidence was refreshed. The final inputs are exactly reproducible.

## Judgments and ownership

All 76 vectors execute in SVG/PNG/PDF twice with fresh renderer instances. Gates
include complete empty reports, dimensions, opaque alpha, exact compressed
octets in the actual SVG image, deterministic artifacts/reports, untagged RGBA8
PNG, and PDF page/Im0/ICC/OutputIntent/soft-mask sample bindings. PNG is judged
against direct canvas PNG; extracted PDF samples against the official SVG raster.
This does **not** replace whole-PDF PDFium/PDF.js viewer qualification.

The canonical GeometryImage budget is unchanged: changed-pixel fraction 1/400
and minimum SSIM 0.999, with exact semantic gates independent of the visual gate.

**17 cells pass both patterns and both color authorities:** gray and RGB in both
scan modes (four); YCbCr in all eight progressive modes and five baseline modes
(1×1, 2×1, 1×2, 2×2, 4×1). Their single explicit test owner binds the reference
and its source descriptors plus the reused PDF/PNG inspectors.

**21 cells remain gaps under [#187](https://github.com/sagikazarmark/excalidraw-render.orig/issues/187):**
three baseline YCbCr cells, both CMYK cells, and all 16 YCCK cells. Of their 42
vectors, 27 fail PNG and PDF color comparison, giving **54 exact defect pins**.
Passing companions still enforce the canonical budget. Pins require the actual
canonical failure and exact changed-pixel count/SSIM bits; a movement in either
direction fails. They grant no tolerance exception, approval, or primary owner.

The ADR-0011 append-only extension used the authenticated prior catalog
`stage4-six-case-application/final-catalog.json`. It preserved all **1,874** old
obligations/dispositions and three exclusions, added 38 narrow requirements,
then assigned only the 17 passing cells. Final counts:

| Measure | Count |
| --- | ---: |
| Obligations | 1,912 |
| Primary owners | 1,817 |
| Approved exclusions | 3 |
| Assigned/excluded | 1,820 |
| Explicit gaps | 92 |

The increase in gaps exposes 21 concrete failures previously hidden inside
coarse facets. All eight broad JPEG facets and all previous gaps remain open.
The 168 inventory executions, criteria, existing approvals and V8 producer
bindings are unchanged. Global ledger approval remains null.

## Defect diagnosis

For baseline 4×2, 1×4 and 2×4 YCbCr/YCCK the independent header test confirms
**non-interleaved sequential scans** (one component per SOS). The corresponding
progressive sources interpret sampling correctly; baseline samples are severely
corrupted. This narrows the issue to baseline/non-interleaved decoding or its
interaction with generic sampling; a minimal causal decoder fix remains open.

For CMYK, an isolated probe decodes raw four-component samples, then compares
rounded versus truncated component-times-K division by 255 to the independently
collected browser. Truncation raises ramp SSIM from **0.9988312919302899** to
**0.9999999971938845** and passes both patterns in both scan modes. Rounded
multiplication reproduces the production ramp failure. `worker.rs::blinn_8x8`
in zune-jpeg uses rounding for both CMYK and YCCK. This isolates CMYK; analogous
YCCK causality still requires its own controlled probe. Production is unchanged.

## Reproduction and follow-up

```sh
cargo test --locked -p excalidraw-render --test jpeg_matrix_inputs
cargo test --locked -p excalidraw-render --test fast jpeg_color_contract::
bash tools/reference-oracle/run-jpeg-color.sh verify
node tools/reference-oracle/scripts/verify-jpeg-ownership.mjs
cargo run --locked -p xtask -- conformance verify-ownership
```

Use pinned Rust 1.97.1 and disabled incremental/debug profiles. The ordinary
tests need no browser/Node/container; reference recollection is explicit.
Native/WASM equality and native-baseline reapproval remain deferred. Next work
is #187 diagnosis/fix and affected qualification, then ICC/EXIF precedence and
real MPF/gain-map containers. No reference evidence here is a new blanket release
approval or a codec-wide coverage claim.

## Verification and independent review

- Input reconstruction passed; all 76 JPEG byte sequences match the frozen input.
- A separate `run-jpeg-color.sh verify` collection repeated both fresh browser
  processes and reproduced the entire final reference record byte-for-byte.
- Full `cargo test --locked --workspace --no-fail-fast`: **1,217 passed, zero
  failures, 19 existing ignored tests**, including 581 fast tests. This run
  preceded the final test-oracle strengthening described below.
- Independent Standards and Spec reviews each identified the same missing
  artifact-extent assertions. Added exact SVG root width/height/viewBox checks
  for native and official artifacts and exact PDF MediaBox/CropBox checks,
  rejecting Rotate/UserUnit. Mutation controls alter extents without changing
  embedded image bytes and must fail the gates. Both reviewers confirmed closure.
- All **five final JPEG matrix tests** passed after that correction; only the
  17 new owner-source bindings were refreshed. No full suite was redundantly
  repeated after the local test-only strengthening.
- Workspace formatting, Clippy with warnings denied, ownership verification,
  append-only preservation audit and approved V8 representative-owner validation
  passed. No native/WASM equality or whole-PDF viewer claim is made.
