# Velox PNG: fractional rectangle stroke coverage

## Conclusion

The original binding-crop SSIM failure is **not caused by text or arrow
bindings**. It persists with rectangle strokes alone, with identical export
bounds and pixel phase. Native continuously antialiased 4× supersampling does
not reproduce the pinned Chromium analytic edge rasterizer's quarter-pixel Y
snapping. The resulting horizontal-edge coverage differs materially over many
long rectangle edges and breaches the unchanged geometry/image SSIM budget.

This is a localized renderer raster-coverage mismatch, **not an approved
non-defect tolerance cause**. No budget, original crop, approved evidence or
production renderer source was changed. A complete edge-rasterization correction
is not implemented by this diagnosis; it needs qualification beyond one
rectangle and must preserve all existing appearance/resource contracts.

## Reproduction and isolation

Baseline renderer: `29f6f08` (unchanged production from the previous proposal).
The executable digest and complete renderer Rust source bindings are retained
in `controls/isolation.json`. Build provenance remains manually recorded:
Rust 1.97.1, `cargo build --locked --release -p excalidraw-render-cli`.

The original PNG comparison reproduces through the real canonical comparator:

```sh
cargo run --locked --release --manifest-path tools/representative-intake/compare/Cargo.toml -- \
  tests/conformance/proposals/velox84-evidence-v1/viewers/light/official-png.rgba.zlib \
  tests/conformance/proposals/velox84-evidence-v1/viewers/light/native.png canonical \
  tests/conformance/proposals/velox84-evidence-v1/viewers/light/regions.json /tmp/velox-repro.json
jq -e '.passed' /tmp/velox-repro.json
```

The final command returns failure. Original light binding crop: **11 changed
pixels**, SSIM bits `3fefee4532dfe2b3`, SSIM gate false. The tool writes valid
measurements even when budgets fail; `jq` supplies the explicit pass/fail signal.

`isolate.py` derives diagnostic scenes by changing only `strokeColor` and
`backgroundColor` to transparent. It retains the original ordering, relationships,
text, dimensions, positions and seed values. Every reference and native SVG
must have the original viewBox exactly:

`0 0 1955.710387025309 515.8543746214824`

PNG dimensions remain **1955×515**. The official renderer runs twice in fresh
pinned browsers; native PNG and SVG run twice in fresh CLI processes with
reversed format order. Artifact/report bytes repeat exactly. Transparent paint
controls are diagnostic modifications, not purported upstream sanitization.

### Binding crop `(944,30,665,161)`

| Visible paint | Light changed pixels | Light SSIM | Dark SSIM | SSIM gate |
| --- | ---: | ---: | ---: | --- |
| Geometry | 11 | 0.9981384381 | 0.9981362607 | fail |
| Text | 1 | 0.9996690688 | 0.9996625908 | pass |
| Rectangles | 8 | 0.9982035292 | 0.9982032516 | fail |
| Arrows | 3 | 0.9999229948 | 0.9999217396 | pass |
| Client rectangle | 5 | 0.9990463492 | 0.9990084360 | pass |
| Client arrow | 3 | 0.9999353554 | 0.9999340865 | pass |
| Connection rectangle | 0 | 0.9994563296 | 0.9994746449 | pass |
| Diff rectangle | 0 | 0.9998389754 | 0.9998412511 | pass |
| JSON rectangle | 3 | 0.9998465397 | 0.9998542607 | pass |
| Rectangle fills only | 0 | 0.9996911940 | 0.9996466271 | pass |
| Rectangle strokes only | 6 | 0.9981740109 | 0.9980863344 | fail |

The individual-rectangle rows pass against the *large original crop*, while
their combined stroke coverage fails. This is not evidence that each individual
edge matches: larger blank areas dilute that crop's SSIM contribution.

## Reduced reproducer and non-text crop

`controls/minimal-rectangle/scene.excalidraw` has just **two elements**:
the visible client rounded rectangle (its bound-text declaration removed), and
an invisible rectangle fixing the original bounds. Dropping the anchor changes
the device phase; retaining unrelated original elements is unnecessary. The
anchor has a distinct preceding fractional index (`Zz`) so official embedded
metadata does not randomly repair duplicate ordering indices.

The new, diagnostic non-text region is **`(1250,26,200,15)`**, on the interior
straight top edge, away from corners. There is no text anywhere in this reduced
scene. Its light result is **zero changed pixels but failing SSIM**, bits
`3fef727d23783ddb` (approximately 0.9827). This distinguishes actual edge-coverage
disagreement from the earlier typography crop's truncated anti-alias context.
This focused region is retained separately, not substituted for the failed
original representative crop.

`stroke_probe.py` reads the native PNG independently and samples the exact
reference RGBA. At interior x=1350:

| Device row | Reference RGBA | Native RGBA |
| ---: | --- | --- |
| 32 | 255,255,255,255 | 255,255,255,255 |
| 33 | 198,198,198,255 | 171,171,171,255 |
| 34 | 30,30,30,255 | 30,30,30,255 |
| 35 | 79,83,85,255 | 105,111,114,255 |
| 36 | 231,245,255,255 | 231,245,255,255 |

The opaque stroke center and fill interior agree; complementary edge coverage
differs. Its horizontal centerline is at `y=34.63299308144707`. X is
`1189.233470223713`. Vertical-edge differences are much smaller, consistent with
the reference rasterizer's asymmetric handling of X and Y.

## Independent browser control and source cause

`canvas-stroke.mjs` paints horizontal/vertical lines and a rounded-rectangle
path directly through the pinned Chromium canvas, with no Excalidraw restore,
fonts, metadata or image decoding. **27 controls repeat exactly in two fresh
browser processes**; `canvas-stroke.json` binds the script/profile and samples.

At phase `0.632993081447`, rounded-rectangle horizontal-edge alpha samples are
`[0,64,255,192,0]`. A straight horizontal line yields
`[0,64,255,191,0]` (the one-unit distinction is path/coverage arithmetic).
At phase `0.233470223713`, a vertical line yields `[0,195,255,59,0]`.
This disproves a symmetric X/Y continuous-coverage assumption and reproduces
the reference behavior without scene relationships or clipping.

Pinned Skia revision **`53348aa333da02b77c4b5797e2de722f5abde7d0`**, selected by
Chromium `149.0.7827.55` (same DEPS identity documented in
`docs/research/stage4-png-device-phase-assessment.md:89–94`):

- [`SkAnalyticEdge.h`](https://raw.githubusercontent.com/google/skia/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkAnalyticEdge.h):
  `kDefaultAccuracy = 2`; `SnapY` adds half a quarter pixel and discards low bits.
- [`SkAnalyticEdge.cpp`](https://raw.githubusercontent.com/google/skia/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkAnalyticEdge.cpp):
  `setLine` applies `SnapY` to Y, not X. `setQuadratic`, `setCubic` and curve
  updates also apply it at their edge-generation stages. This is **edge
  rasterization**, not permission to round shared geometry or translate shapes.
- [`SkScan_AntiPath.cpp`](https://raw.githubusercontent.com/google/skia/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkScan_AntiPath.cpp):
  antialiased filling delegates to `AAAFillPath` after bounded admission/clipping.

`primary-sources.json` records downloaded source lengths and SHA-256 identities.
In contrast, `crates/excalidraw-render/src/render/png.rs:936–967,1003–1016` draws
the exact transformed path on a 4× surface; `render/png/paint.rs:62–120` averages
subpixels. That correctly explains the native samples but omits the reference
edge Y quantization. More supersampling alone cannot supply the missing rule.

## Fix boundary and remaining work

A correct follow-up should operate on PNG device-space raster edges after
stroking, with pinned binary32/fixed-point admission and asymmetric snapping.
It must cover curves, rotated paths, ties/negative coordinates, clipping,
opacity, fills, narrow strokes and semantic-resource equality. Simply rounding
all path Y coordinates or translating the whole rectangle could improve this
one probe while damaging curve geometry and is not a qualified fix.

Use the reduced facade scene and native/reference edge samples as the first
red regression at the already confirmed public renderer seam. Qualify the
adapter independently and rerun the complete corpus before changing production
bytes/approval subjects. This diagnosis does not authorize tolerances, redefine
the spec, or resolve the independent PDF differences.

## Replay

```sh
python3 -B tools/representative-intake/isolate.py verify \
  tests/conformance/proposals/velox84-png-isolation-v1/controls
# Fresh collection (new directory only):
python3 -B tools/representative-intake/isolate.py prepare target/velox-isolation-new
python3 -B tools/representative-intake/isolate.py collect target/velox-isolation-new \
  --renderer target/release/excalidraw-render
python3 -B tools/representative-intake/stroke_probe.py \
  tests/conformance/proposals/velox84-png-isolation-v1/controls/minimal-rectangle
```

`verify` checks file/source identities, exact derivations, reference/native
bounds, repeat bytes and retained report/result consistency. It does not rerun
viewers, authenticate an approval, or independently recompute all comparisons;
the `collect` command runs those canonical comparisons. No test asserts that
the current failing raster is correct.

After strengthening verification, a complete fresh collection reproduced every
retained byte and comparison. `reconcile-replay` updated only its unapproved
source-binding record after requiring identical files, results and renderer
executable. Completed collections cannot be overwritten by `collect`.
