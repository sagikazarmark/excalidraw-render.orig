# Stage 4: case-specific PDF typography diagnosis

2026-09-19, base `629930b`, fixed candidate
`tests/conformance/candidates/stage4-frames-text-assets-v4/`.

**Status: verified diagnosis supporting preparation of scoped tolerance
exceptions; not an approval or an installed exception.** The user selected
preparation of case-specific PDF viewer exceptions. Adapter integration and the
exception proposal are separate work. The failed v4 captures and judgments
remain the evidence; diagnostic controls do not replace them.

## Cause ready for the exception proposal

**PDFium:** The correctly embedded, exactly positioned Excalifont 20 px text at
0.125 radians is rasterized by the pinned PDFium viewer using whole-device-y
glyph origins, LCD-mask x phases, and its text coverage pipeline. Chromium's
reference SVG text uses quarter-pixel glyph phases. A one-glyph perturbation
confirms PDFium's whole-y rounding directly; path/clip controls at unchanged
coordinates substantially reduce the mismatch. The original PDF's transform,
baseline and advances are correct. This is a diagnosed viewer rasterization
limitation for this case, rather than an identified renderer geometry defect.

**PDF.js:** The pinned profile explicitly uses `disableFontFace: true` and its
built-in path font renderer. It paints exact fractional glyph origins without
Chromium's quarter-pixel mask placement. Applying that independently specified
quarter-pixel rule only in a temporary diagnostic PDF raises whole-image SSIM
from `0.998520923299333` to `0.9999370752309106` and removes all Pixelmatch changed
pixels. The unmodified typography crop already passes PDF.js's allowance; only
its whole-image comparison needs the proposed exception.

These controls isolate viewer behavior, not every possible PDF encoding. They
do not prove that no semantics-preserving alternative could ever improve it.
Font-program hinting is ruled out for the used glyphs; all FreeType grid-fitting
or coverage behavior is not. The PDFium coverage-gamma contribution is supported
by source and changed coverage, but was not separately ablated quantitatively.

### Exact inputs to the scoped limits

The fixed whole-image class is **geometry/image**, not mixed representative:
200×120, SSIM ≥ 0.999. The typography crop is `(10,12,124,41)`, with SSIM ≥ 0.995
for PDFium and ≥ 0.990 for PDF.js. These canonical budgets remain unchanged.

| Viewer / region requiring exception | Changed-pixel ceiling, no margin | Observed SSIM floor | Exact binary64 bits |
| --- | ---: | ---: | --- |
| PDFium / whole | 118 | 0.9898474730435529 | `3fefacd49b980278` |
| PDFium / typography | 118 | 0.9413564483014669 | `3fee1f978eeaac28` |
| PDF.js / whole | 16 | 0.998520923299333 | `3feff3e226be27cf` |

This table supplies measured inputs to the integration agent, not exception
objects. It applies only to the bound PDF, reference, viewer profiles and regions.
PDF.js typography, both nontext crops, and every other execution retain their
canonical gates. The existing bound `judgments.json` records both nontext crops
as exact. Structural and selectable-text obligations also remain required.

## Geometry and font evidence

The probe independently derives the element matrix from authored
`x=12,y=20,width=120,height=25,angle=0.125` and checks the actual PDF content:

```text
page:    0.75 0 0 -0.75 0 90 cm
element: 0.992197667229329 0.12467473338522769
        -0.12467473338522769 0.992197667229329
         14.026574133555604 12.617045156519724 cm
```

- `/F2 20 Tf`; each baseline is `17.62`, matching SVG's
  `17.619999999999997` within `4e-15` logical pixels.
- Local glyph x values are `0,12.44,23.5,35.02,46.12,56.86,64.86,74.8,86.8,97.76`.
  Independent `hmtx` accumulation differs by at most
  `1.4210854715202004e-14` logical pixels (assertion limit `1.43e-14`).
- All ten PDFium-parsed page-object origins differ from analytic device origins
  by **less than `6e-6` pixels**. First expected origin:
  `(11.829805331307892,30.099568053100505)`; parsed:
  `(11.829806009928385,30.09956868489583)`.
- PDFium reports the font embedded. The FontFile2 program is 61,948 bytes,
  SHA-256 `a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb`,
  UPEM 1000. All ten used glyphs have zero instruction bytes; the program has
  no `fpgm`, `prep`, or `cvt ` tables.
- Use `FPDFPageObj_GetMatrix` for individual positions. The line-wide
  `ActualText` makes `FPDFText_GetCharOrigin` report one replacement-text origin
  for all extracted characters; that is not their actual drawing placement.

## Executed controls

Both original viewers were rerun under their pinned profiles and matched the
retained v4 captures **byte-for-byte**. The single-run control replaces ten
separate glyph shows with one `Tj` using the same exact advances. The single-S
controls remove the other text glyphs in temporary PDFs and apply predefined
device-y displacements transformed back into local coordinates:

| Displacement | Pixels different from unmoved S | Observation |
| ---: | ---: | --- |
| 0 | 0 | Control |
| 0.125 | 0 | Identical mask |
| 0.25 | 0 | Identical mask |
| 0.5 | 136 | Exactly the original mask one row down |

This is a causal placement perturbation, not alignment search or fitting of the
candidate. The quarter-pixel control likewise uses Skia's rule, not a searched
offset. No scene, crop, reference raster, or comparator budget was changed.

| PDF/control | Viewer | Whole SSIM | Typography SSIM | Typography changed pixels |
| --- | --- | ---: | ---: | ---: |
| Original | PDFium | 0.9898474730435529 | 0.9413564483014669 | 118 |
| Single `Tj` | PDFium | 0.9898474730435529 | 0.9413564483014669 | 118 |
| Extracted outlines, same origins | PDFium | 0.9985056026038821 | 0.9916320519628264 | 19 |
| Original text clip (`7 Tr`), solid fill | PDFium | 0.9985448858140402 | 0.9918711425117517 | 15 |
| Quarter-pixel origins | PDFium | 0.9910480791988634 | 0.9484572958568168 | 85 |
| Original | PDF.js | 0.998520923299333 | 0.9920016961872306 | 16 |
| Quarter-pixel origins | PDF.js | 0.9999370752309106 | 0.9996543146625062 | 0 |

The outline control uses PDFium's `FPDFFont_GetGlyphPath` with Unicode input,
scaling its normalized coordinates by 20. That extraction passes through
PDFium's 64-pixel FreeType path machinery: it is not an assertion of exact
font-unit contour identity. The independent text-clip control avoids external
outline extraction. Both controls still fail the canonical gates.

`FPDF_LCD_TEXT`, `FPDF_PRINTING`, and `FPDF_RENDER_FORCEHALFTONE` each produced
byte-identical original output. `FPDF_RENDER_NO_SMOOTHTEXT` changed it; disabling
antialiasing is not a geometry remedy. All four raster hashes are retained.

Spec §8.5 confines canvas snapping to PNG; §8.6 requires full-f64 PDF geometry.
Copying PNG snapping into PDF would violate that requirement and still fails
PDFium. §14.3 requires independent viewer gates, and §14.4 requires maintainer
approval. This record supplies the diagnosed non-defect cause for that review.

## Pinned primary sources

The [evidence record](stage4-pdf-typography-evidence.json) binds decoded source
bytes by exact URL, byte length and SHA-256. `--verify-sources` fetches the
Gitiles base64 representation and verifies the decoded bytes.

- [PDFium `cfx_renderdevice.cpp`](https://pdfium.googlesource.com/pdfium/+/d3e62278db7489af9e0b686835d1c8aca218c3d7/core/fxge/cfx_renderdevice.cpp):
  lines 1189–1194 select normalized LCD masks for alpha output; 1254–1258 round
  origin y and floor LCD x; 1353 selects thirds-of-pixel x phase; 99–121 and
  180–207 implement the coverage-gamma table and application.
- [PDFium `cfx_face.cpp`](https://pdfium.googlesource.com/pdfium/+/d3e62278db7489af9e0b686835d1c8aca218c3d7/core/fxge/cfx_face.cpp):
  lines 887–907 describe the 64-pixel outline extraction and hinting flags.
  This commit was resolved from `refs/heads/chromium/7947` on 2026-09-19. The
  wheel identifies build 7947 but carries no source commit hash. Thus source
  correspondence is by version branch; the actual pinned binary's behavior is
  independently demonstrated by the controls, not inferred solely from source.
- [Chromium 149.0.7827.55 `DEPS`](https://chromium.googlesource.com/chromium/src/+/149.0.7827.55/DEPS)
  explicitly pins Skia `53348aa333da02b77c4b5797e2de722f5abde7d0`.
- At that Skia commit, [SkGlyph.h](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkGlyph.h)
  lines 51–70 encode two subpixel bits and the eighth-pixel rounding bias;
  lines 224–230 explain the rounding rule.
  [SkGlyphRunPainter.cpp](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkGlyphRunPainter.cpp)
  lines 121–150 map individual origins with that bias.
  [SkStrikeSpec.cpp](https://skia.googlesource.com/skia/+/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkStrikeSpec.cpp)
  records the 256-device-pixel path cutoff.
- The bound local `tools/pdf-harness/pdfjs/render.mjs` and PDF.js profile record
  `disableFontFace: true`, `useSystemFonts: false`, exact 96/72 scaling, and the
  path-font renderer. Its locked dependencies and container identity are bound.

## Reproduction and evidence retention

New files are this report, `stage4-pdf-typography-evidence.json`, and the two
drivers under `tools/pdf-harness/diagnostics/`. The JSON binds the original
PDF/reference/captures (including repeated captures), judgment plan, profiles,
viewer/comparator/diagnostic sources, upstream sources, and **23 exact control
artifact hashes**. It also records all 14 whole/crop comparison results with
binary64 SSIM bits. Binary controls are regenerated in temporary storage rather
than duplicated into the repository.

Prerequisites: Linux x86-64, Python 3.12.3, Rust 1.97.1, the locally available
pinned Playwright image, and the existing PDF.js `node_modules` installed from
its lockfile. The PDFium wheel is the artifact specified in
`tools/pdf-harness/pdfium.lock.json`; the probe verifies its complete hash before
extracting it under the new output directory. On this host it is already at
`/tmp/opencode/pdfium.whl`. From the repository root:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
python3 tools/pdf-harness/diagnostics/stage4-typography.py \
  --wheel /tmp/opencode/pdfium.whl \
  --output /tmp/opencode/stage4-typography-reproduction \
  --verify-sources
```

Use a fresh output directory for each run. Omit `--verify-sources` for an offline
replay. The probe uses the unmodified PDFium harness configuration, mounts the
repository read-only for PDF.js, and builds only the small canonical comparator
with pinned `rustc`, `opt-level=2`, `debuginfo=0`. It never builds the renderer.
Its success means the diagnosis reproduced, **including the original failures**.
It writes the geometry observations and verified hashes/metrics to
`observations.json` in that output directory.

The command above was executed successfully on 2026-09-19, including the six
upstream source checks, all 23 control hashes, and all 14 comparison results.
The resulting `observations.json` SHA-256 is
`cc5d7405149e4b6dbe98823d3c64f839b801aa5b4ebaa99ad8efebe0fcc8f104`.
Its runtime-identification fields are informative; a different toolchain build
string can change that summary digest without changing the separately checked
artifact hashes or SSIM bits. The direct original-PDF comparison below was also
executed and returned exit 1 with both regions failed, as required.

The comparator itself remains red-capable; this command exits 1 on the original:

```sh
/tmp/opencode/stage4-typography-reproduction/compare \
  tests/conformance/candidates/stage4-frames-text-assets-v4/viewer-evidence/official-chromium.rgba \
  /tmp/opencode/stage4-typography-reproduction/stage4-original.rgba pdfium
```

Bindings fail closed if evidence, controls, sources or exact metrics change.
There is no refresh/approval mode. The integration agent can cite this report
and bind its evidence record into a case-specific exception proposal; final
candidate approval must still bind the actual reviewed artifacts and decision.
