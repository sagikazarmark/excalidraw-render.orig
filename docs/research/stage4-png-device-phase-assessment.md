# Stage 4 PNG device phases: draft-rule approval and assessment

Date: 2026-09-19. Base: `629930b`, current uncommitted working tree.

## Approval scope

The user explicitly authorized this normative **draft** amendment in the current
agent session: replace whole-line PNG snapping with measured Chromium per-glyph
device phases; quarter pixels along an axis-aligned baseline, whole pixels
perpendicular to it, quarter pixels on both axes for non-axis-aligned rotation,
and exact origins beyond the 256-device-pixel path cutoff. Shared layout and
SVG/PDF coordinates remain exact.

The user subsequently explicitly authorized fixing the demonstrated quarter-turn
and near-axis bug using the pinned Skia 1/1024 font-cache matrix classification
at the correct device-matrix stage. The clarification below implements that
authorization; it does not broaden the case or tolerance approval scope.

This record is the session approval assessment, not a fabricated GitHub approval
identity or issue decision. It covers the §8.5 draft rule and that narrow PNG
classification fix. It does not
approve a representative case, PDF tolerance exceptions, new budgets, crops,
producer records, or final Stage-4 qualification.

**Implementation assessment: the demonstrated axis-classification defect is
fixed and the six focused regressions pass.** The pinned browser's 30 boundary
controls repeat in two fresh processes. A separate native Excalifont PNG byte
canary still disagrees; its exact result and scope are recorded below.

## Exact approved rule and implementation mapping

For local glyph origin `(gx,gy)`, the full local-to-final-device affine transform
maps it to `X=a*gx+c*gy+e`, `Y=b*gx+d*gy+f`. Use the grid `(qx,qy)` appropriate
to the transformed baseline: horizontal `(4,1)`, vertical `(1,4)`, otherwise
`(4,4)`. Each glyph gets its own device-space displacement:

```text
snap(v,q) = floor(v*q + 0.5)/q
(dx,dy) = (snap(X,qx)-X, snap(Y,qy)-Y)
```

Alignment uses only the font-cache copy of the device matrix. Skia admits each
coefficient to binary32, multiplies by 1024 in binary32, then promotes to
binary64 for `floor(product + 0.5)`. Division by 1024 gives the cache coefficient;
the implementation needs only its zero/nonzero result. Cached `b == 0` selects
horizontal first; otherwise cached `a == 0` selects vertical. Font size and the
internal supersample are excluded; export scale is included. No shared layout,
outline point, actual transform coefficient, or path cutoff is rounded to this
cache representation.

Halfway ties select the more positive device coordinate, including negatives:
`0.125 → 0.25` and `-0.125 → 0` on the quarter grid; `-0.5 → 0` on the whole
grid. The code uses `floor`, not integer truncation, ties-to-even, or absolute
value rounding. Positive eighth-pixel ties are exercised by the new focused
test; the negative examples follow the formula and pinned source, not a new
negative-coordinate browser measurement.

Bypass snapping when either `F*F*(a*a+b*b) > 256*256` or
`F*F*(c*c+d*d) > 256*256`. The strict comparison leaves equality on the mask
route. The transform already includes export scale. The supersample multiplier
is applied only after the glyph's device displacement has been selected.

Current code inspection:

- `PngTransform::canvas_text_path` in `render/png.rs` performs that cutoff and
  per-glyph rounding, then transforms each contour with the glyph's displacement.
  It does not quantize individual contour points to the phase grid.
- Ordinary text, generated embeddable text, and frame names all call this helper.
  Compatibility-placeholder labels still use the unsnapped `label_path` route.
- `ResolvedGlyph::outline` in `render/text.rs` identifies each glyph's range in
  the shared outline. Both `build_label` and `shape_official_line` record it;
  fallback-run concatenation adjusts the ranges by the prior operation count.
  Glyph positions, shaping advances and shared outlines are not snapped there.
- `render/png/paint.rs::append_label_path` applies the caller's coordinate map
  to the complete glyph contour. Layout and SVG/PDF coordinates remain exact.
- The grid selection now uses the pinned cache-zero classification with
  horizontal-first precedence. It replaces raw `a == 0` / `b == 0`, which
  misclassified `libm::cos(π/2)`'s tiny residue and near-axis rotations.

Final production SHA-256 values for this narrow fix (`png.rs` changed;
`paint.rs` and `text.rs` were inspected but not edited):

| File under `crates/excalidraw-render/src/` | SHA-256 |
| --- | --- |
| `render/png.rs` | `a089e88fb04f58f40ab186ead55a973048e5099995ad13a653be15006d0d2ade` |
| `render/png/paint.rs` | `c7d105d9dad483742df6e586fcd8fbee718d51fc95bce0144b669e8a8b9689ec` |
| `render/text.rs` | `f8f36b8377a9ce437c1734099e6118941e33081998463d1035361e26addc24f7` |

## Pinned primary sources

Chromium `149.0.7827.55`'s [DEPS](https://chromium.googlesource.com/chromium/src/+/149.0.7827.55/DEPS)
pins Skia **`53348aa333da02b77c4b5797e2de722f5abde7d0`**. The existing
[PDF typography evidence](stage4-pdf-typography-evidence.json) binds that DEPS
file, `SkGlyph.h`, `SkGlyphRunPainter.cpp` and `SkStrikeSpec.cpp` by decoded bytes
and SHA-256; those records were not modified for this amendment.

- [SkGlyph.h](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkGlyph.h),
  lines 51–70: two subpixel bits and `kSubpixelRound = 1/8`.
- [SkGlyph.cpp](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkGlyph.cpp),
  lines 683–705: horizontal alignment uses `(1/8,1/2)` rounding biases;
  vertical uses `(1/2,1/8)`; unaligned uses `(1/8,1/8)`.
  Decoded length **24,760**, SHA-256
  `6fb66cfe529a54e6b569babd3b7077723edeed14b310292d595ee3f579803419`.
- [SkGlyphRunPainter.cpp](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkGlyphRunPainter.cpp),
  lines 121–150: transform each origin, add the axis bias, pack its subpixel
  phase, then floor its integer device origin.
- [SkStrikeSpec.cpp](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkStrikeSpec.cpp),
  lines 101–126: `ShouldDrawAsPath` uses the two transformed text-matrix column
  squared lengths and strict `> 256²` comparisons.
- [SkScalerContext.cpp](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkScalerContext.cpp),
  lines 989–1009: baseline classification tests the font-cache matrix;
  1041–1048 and 1104–1117: `sk_relax` first rounds its coefficients to 1/1024.
  Decoded length **51,163**, SHA-256
  `6dbe1656b2442c7c43b63dbbe0edaf504a0cbe8a965d4c1154369229db3d736a`.
- [SkScalar.h](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/include/core/SkScalar.h),
  line 32, maps `SkScalarRoundToScalar` to `sk_float_round`. Decoded length
  **5,033**, SHA-256
  `d8af31c19c5f13f2536a5a073f25f4b7f5f80bb81c8916b75f3000448ce79fff`.
- [SkFloatingPoint.h](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/include/private/base/SkFloatingPoint.h),
  lines 35–38 and 126, promotes the binary32 argument to binary64 before adding
  0.5 and taking floor. Its comment explicitly warns that adding the bias in
  binary32 rounds values such as 0.49999997 incorrectly. Decoded length
  **6,945**, SHA-256
  `f5a23b3a6af862ba964f5aabc0803ac1513158467e21e6e03f602fb6339463b1`.

### Resolved boundary detail

Skia's axis classification uses its rounded font-cache matrix, not the raw
full-f64 device matrix. The browser control confirms that a 0.0001-radian
rotation at scale 1 behaves like horizontal text. A quarter-turn also gets the
proper vertical classification despite its floating-point cosine residue.

The approved high-level rule requires the correct grid for horizontal/vertical
baselines; the original quarter-turn failure was a concrete code/spec mismatch.
The now-authorized adapter uses the font-cache matrix only for axis selection.
The boundary controls distinguish signed rounding, binary32 admission, font size,
export scale, quarter-turn cosine residue and the unchanged path cutoff. This is
qualification of this narrow classifier, not a claim of complete Chromium parity.

## Initial boundary control and evidence

A temporary canvas-only browser probe uses the reviewed Excalifont program
(61,948 bytes, SHA-256
`a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb`), loaded as
a custom `FontFace`. It draws one `S` at device origin `(50.1,300.1)` on a
transparent 512×512 canvas, then repeats with predefined shifts `(0.25,0)` and
`(0,0.25)`. These are raw unequal-alpha-pixel counts, **not Pixelmatch counts**.
All three full RGBA hashes for every row repeated in two fresh browser processes.

| Font size | Rotation | Changed pixels for +0.25 device x | Changed pixels for +0.25 device y |
| ---: | ---: | ---: | ---: |
| 20 | 0 | 76 | 0 |
| 20 | 0.0001 | 76 | 0 |
| 20 | 0.125 | 73 | 89 |
| 20 | π/2 | 0 | 82 |
| 256 | 0 | 1016 | 0 |
| 256.25 | 0 | 1010 | 1080 |

The 0.0001 row's three RGBA hashes are also identical to the horizontal row's.
The quarter-turn control's original and +x images both hash to
`4c313294b421ed1f0f5d0a527c253b1168590c3834e5010c414904d3eb526e19`.
The 256 px control's original and +y images both hash to
`a7986130a60a80b15123f97dab0300cd36e0137d69047b90ea6abdb515fa76aa`.
At 256.25 px those hashes differ:
`248dd53c908e6d5701432199c21a07b265a51136556bc8708a2f0bb62bd5f994`
versus `e5b243cfcc3f0247a15e3f4cb1e0aa01663b7cce0f5e01ca41d0506057e2eb20`.

Temporary probe: `/tmp/opencode/stage4-png-phase-boundaries.mjs`, SHA-256
`fdc6cf0b510e793cdbc7a1179ea47ee667613ad6ba9ae826cc95a71bac2b0d93`.
Result: `/tmp/opencode/stage4-png-phase-boundaries.json`, SHA-256
`e5e18cdd9979f68734b92626cc3756c538bb71cbd5c708c3bb48a9a97f0d1271`.
The profile is `tests/oracle/reference-profiles/revision-1.json`, SHA-256
`1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71`.
It supplies browser launch arguments and context settings. The executed command:

```sh
docker run --rm --init --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --mount type=bind,src=/home/laborant/excalidraw-render.orig,dst=/workspace,readonly \
  --mount type=bind,src=/tmp/opencode,dst=/probe \
  --workdir /workspace/tools/reference-oracle \
  mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9 \
  node /probe/stage4-png-phase-boundaries.mjs
```

This is a diagnostic canvas control, not new official case evidence or a
candidate collection. Its rendering operation, sufficient to reconstruct each
row after loading the identified face as `Probe`, is:

```js
const canvas = document.createElement("canvas");
canvas.width = canvas.height = 512;
const ctx = canvas.getContext("2d");
ctx.font = `${size}px Probe`;
ctx.textBaseline = "alphabetic";
ctx.setTransform(Math.cos(angle), Math.sin(angle),
                 -Math.sin(angle), Math.cos(angle), 50.1 + dx, 300.1 + dy);
ctx.fillText("S", 0, 0);
const rgba = ctx.getImageData(0, 0, 512, 512).data;
```

The retained v4 PNG crop test independently checks the actual renderer against
official canvas evidence. The earlier PDF.js quarter-phase control in
`stage4-pdf-typography-diagnosis.md` is additional causal evidence of Chromium's
glyph phases, not authority to snap PDF coordinates.

## Fix qualification: signed cache boundaries and matrix stage

The extended pinned-browser probe passed **30 controls in two fresh processes**;
all three RGBA hashes per control repeat. It uses the same font, origin,
dimensions, displacements, browser profile and container as the initial probe.
For export scale `s`, its four linear canvas coefficients are
`s*cos(angle), s*sin(angle), -s*sin(angle), s*cos(angle)`; origin/displacements
remain in final device pixels. Font size stays a separate `ctx.font` parameter.

Let `h = 2^-11`, `inside = h - 2^-35`, and `outside = h + 2^-34`: these are
the positive half-step and its adjacent binary32 coefficients. Horizontal
boundary controls use `angle = asin(coefficient)`; vertical controls use
`angle = acos(coefficient)`. At 20 px, scale 1, observed unequal-alpha counts
for the predefined (+x,+y) displacements are:

| Coefficient | Horizontal-boundary control | Vertical-boundary control |
| --- | --- | --- |
| `inside` | `(76,0)` | `(0,82)` |
| `h` | `(76,86)` | `(84,82)` |
| `outside` | `(76,86)` | `(84,82)` |
| `-inside` | `(76,0)` | `(0,82)` |
| `-h` | `(77,0)` | `(0,82)` |
| `-outside` | `(76,86)` | `(84,82)` |

Additional controls discriminate the stage rather than just an epsilon:

- `asin(h - 2^-37)` admits to the positive binary32 tie: `(76,86)`, unaligned.
  `asin(-h - 2^-36)` admits to the negative tie: `(77,0)`, horizontal. Direct
  full-f64 threshold comparisons would get both of these classifications wrong.
- At font size 80, `asin(inside)` gives `(298,0)`; `asin(outside)` gives
  `(297,320)`. Multiplying the classification matrix by font size is incorrect.
- At scale 2 and size 20, unchanged `asin(inside)` gives `(163,177)`;
  `asin(inside/2)` gives `(163,0)`; `asin(outside/2)` gives `(163,177)`.
  Export scale must participate in classification, but the supersample must not.
- Both signs of π/2 keep whole-device-x phases: `(0,82)` and `(0,83)`;
  π keeps whole-device-y phases: `(77,0)`.
- At scale 2, font size 128 (256 device pixels) gives `(1016,0)`; 128.125
  (256.25 device pixels) gives `(1013,1082)`. The path cutoff remains strict,
  and is evaluated on the original transformed font size.
- The initial 0, ±0.0001, 0.125, 256 px and 256.25 px cases remain included.

Extended probe: `/tmp/opencode/stage4-png-alignment-probe.mjs`, SHA-256
`d55723fb32683abc3ea93079609c373369967b6513c60b35425d7d1a9038d1d7`.
Result: `/tmp/opencode/stage4-png-alignment-probe.json`, SHA-256
`e93cb7c27b7e587f5af59311fe22f0e0660e6c2de7faf5db6172556924802b0c`.
The command is the same pinned-container invocation above, with the final
argument changed to `/probe/stage4-png-alignment-probe.mjs`. It was executed
after the fix as well as during qualification. These are independent controls;
no candidate scene, crop, budget, or reference capture was fitted or replaced.

## Focused test result and handoff

The narrow production change is in `render/png.rs::canvas_text_path`.
`crates/excalidraw-render/tests/stage4_typography.rs` exercises the public
renderer with in-memory one-glyph controls and fixed frame bounds. The two
documentation files clarify the authorized classifier. Frozen v4 files and
candidate-producer records stay intact.

```sh
PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
cargo test --locked -p excalidraw-render --test stage4_typography -- --nocapture
```

Before the fix, the expanded target returned **3 passed, 3 failed**: quarter-turn
(86 changed pixels where zero was required), tiny positive angle (96), and
the just-inside positive horizontal threshold (97) were red. After the fix:
**6 passed, 0 failed**, in 0.49 seconds of test execution:

- Pass: original rotated v4 typography crop, canonical typography comparison.
- Pass: positive eighth-pixel tie selects the next quarter-pixel phase.
- Pass: 20 and exactly 256 px snap perpendicular placement; 256.25 px does not.
- Pass: both quarter-turn signs and the half-turn use the proper axis grid.
- Pass: signed inside/tie/outside cache boundaries and binary32 admission cases.
- Pass: font-size independence, export-scale dependence and scaled path cutoff.

Additional bounded native font checks used the same pinned toolchain and Cargo
environment, with `--test fast <module>:: -- --nocapture`:

| Module | Result |
| --- | --- |
| `font_catalog_render` | 17 passed |
| `helvetica_text_render` | 1 passed |
| `excalifont_text_render` | 5 passed, 1 failed |

The Excalifont failure is the PNG artifact-byte canary in
`excalifont_text_renders_semantically_in_every_format`, line 93:

```text
expected cb90c943bc71d5eec983bf2c933e1c3c16dd4ca7b5fc14ba79f0230404209534
observed 9b2da5d13b6a2c672ab80be51edbeeb9491195b57939dea9f48e80e3887f3b95
```

Its scene is 20 px text rotated 0.125 radians at scale 1; both the previous
raw-zero classifier and this cache classifier select `(4,4)`, so the narrow
axis fix does not change its phase branch. The separate approved direct-PNG
typography-budget check passes. The byte canary needs separate assessment under
the preceding per-glyph amendment; it was not repinned here. That test aborts
at PNG, so its later PDF canary is not claimed as checked by this invocation.

The demonstrated axis-classification bug is resolved, with its red-capable
regressions now green and no known remaining classifier mismatch in these
controls. The parent can use the final document/production hashes for rebinding
and final capture, while tracking the outstanding byte canary. No full-workspace
green result, final representative-case approval, or PDF-exception approval is
claimed.
