# Asymmetric PDF image colour failure: pre-existing producer defect

Subsequent user-authorized fix and bounded requalification:
[direct image ICC binding fix](stage4-image-icc-fix.md). The original failures and
diagnostic observations below remain preserved.

## Concrete cause

Both the clean `629930b` producer and current producer write image XObjects with
`/ColorSpace /CS1`. `/CS1` is a **page resource name**, not a self-contained colour
space definition or an indirect reference suitable for the image dictionary.
The samples, attached soft mask and embedded ICC profile are correct; their
connection through that name is the defect.

PDF 32000-1:2008 §7.8.3, printed page 82 (physical PDF page 90), says named
resources are meaningful only in the context of a content stream and requires
references between objects outside content streams to use indirect references,
not named resources. Image dictionaries are described in §8.9.5/Table 89. A
correct form here is `/ColorSpace [/ICCBased 5 0 R]`, retaining the existing
profile stream. The predefined `/DeviceGray` on the mask is not a resource alias.

Primary standard: [Adobe-hosted PDF 32000-1](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf),
SHA-256 `9de0ca9e8570d6209e8bd48a355be8eb6ec376acfc3fc3ae97cd8730351417ff`.

**Classification:** pre-existing renderer/producer conformance defect, exposed
by PDF.js. It is not a tiny-image limit, NAPI resampling issue, bad ROI, corrupted
diagnostic PNG, or new regression caused by image orientation/interpolation.
It must not be described as an approved viewer tolerance exception.

## Why PDF.js produces grayscale, and why v6 passed

The pinned PDF.js worker (`pdfjs-dist` 6.1.200, upstream commit
`6353acefe5007cd4899247a8c4e83cb7c9435a54`) explains the measured behavior:

- `src/core/image.js`, bundled lines 39071–39079, passes
  `resources: isInline ? res : null` while parsing an image colour space.
- `ColorSpaceUtils.parse`, bundled lines 9549–9554, first checks the local cache
  by colour-space name.
- `ColorSpaceUtils.#parse`, bundled lines 9614–9628, cannot resolve an uncached
  custom name without resources, warns, and falls back to DeviceGray.
- A prior `/CS1 cs` operation resolves the name in the legitimate content-stream
  resource context and populates the cache. The malformed image alias can then
  appear to work. The profile's error-only logging suppresses the fallback warning.
- A directly supplied ICCBased array reaches the `/N 3` and `/Alternate /DeviceRGB`
  logic at bundled lines 9657–9695 and correctly yields RGB.

Bound worker SHA-256:
`dde66d5cd41b7ac11df49b2c130162fceb15d17afe46f552b0fc072d00734a17`.
The production image entry is in `crates/excalidraw-render/src/render/pdf.rs`
(currently line 921). No production change was made in this diagnosis.

## Reproduced failure and byte-level inspection

The original failed control PDFs remain untouched. In both PDFs, qpdf found:

```text
RGB image: object 7, Width=2, Height=2, BitsPerComponent=8
RGB bytes: ff0000 00ff00 0000ff ffff00
SMask: object 8, Width=2, Height=2, DeviceGray, BitsPerComponent=8
Mask bytes: ff ff ff ff
Page /CS1: [/ICCBased 5 0 R]
ICC stream: /N 3, /Alternate /DeviceRGB, RGB/acsp header
ICC SHA-256: 20b2514b38138f0bd775cb08dbe06a7a68b736b3dba0881a2d6fa91636fd87be
```

Fresh runs reproduced the old/current PDF.js failures exactly. PDFium showed the
old vertical flip and the current correct source orientation. The samples are
well inside the texel corners, away from clipping or interpolation boundaries.

Before any NAPI canvas drawing, the PDF.js image object for the malformed alias
already contains white/black/black/black RGBA. This equals treating the **first
four bytes of the 12-byte RGB stream** (`ff 00 00 00`) as four gray samples.
With only the image's colour-space entry changed to the direct ICCBased array,
the pre-canvas data is the correct red/green/blue/yellow RGBA sequence.

## Controlled results (two fresh runs per viewer)

All variants are new diagnostic files created through qpdf JSON reconstruction;
qpdf checks them. The unmodified round-trip reproduces the original pixels.
Neither original control, production source, viewer, profile nor budget changed.

| Diagnostic change | PDFium | PDF.js |
| --- | --- | --- |
| Original old/current alias | Expected old/current orientation | Colour failure |
| Unmodified qpdf round-trip | Pass | Same failure |
| Remove SMask only | Pass | Same failure |
| Disable interpolation only | Pass | Same failure |
| Enlarge source to 4×4 | Pass | Failure |
| Enlarge source to 40×40 | Pass | Failure |
| Explicit DeviceRGB, with or without mask | Pass | Pass |
| Prime `/CS1` using a content-stream colour operation | Pass | Pass |
| Direct ICCBased array, same profile, RGB and mask | Pass | Pass |
| Direct ICCBased array without mask | Pass | Pass |
| Old PDF with direct ICCBased array | Correctly exposes old flip | Same correct old flip |

The direct-ICC controls change neither the ICC bytes, RGB samples, alpha plane,
image transform nor viewer settings. They isolate the illegal resource alias
without abandoning the project sRGB profile. The old-direct-ICC result also
confirms that fixing colour resolution does not conceal the old orientation bug.

### Additional larger qualified control

V6's original 40×40 source remains an independent control, not a replacement for
the failed 2×2 input. All **1,600 alpha samples** and RGB samples match the authored
rule. Both pinned viewers, twice each, reproduce their previously qualified v6
rasters byte-for-byte; raster crop `(178,38,22,44)` is exact against official
Chromium evidence. Its content selects `/CS1` before `/Im0 Do`, explaining why
the cache masks the producer defect there. The separate unprimed 40×40 control
still fails, so size is not the cause.

## Evidence and reproducibility

Retained package:
`tests/conformance/candidates/stage4-image-colour-diagnosis-v1/`.

- `evidence.json`: SHA-256
  `593cd3b519ef9f0926bd61ecd0f16802c6b0c68706c8182b006f68a01fd5fad1`.
- `narrow-pin-proposal.json`: SHA-256
  `b2e4e0c8d19c1e1e053c3d6dce0f1c77e006cbca102a1cf7c7b74777cc82890b`.
- All diagnostic PDFs, qpdf object models, repeated rasters and runtime summaries
  from the four stages are retained in separate subdirectories. Source/profile
  bindings and the standard identity are recorded. The pre-canvas probe was
  re-executed while packaging rather than inferred from the final screenshots.

Drivers:

```sh
# Red-capable original symptom, expected to fail its source-colour assertion:
python3 tools/pdf-harness/diagnostics/stage4-image-colour.py \
  --output NEW_REPO_OUTPUT_DIRECTORY --require-source-colours
# Controlled changes, retained alongside the original failures:
python3 tools/pdf-harness/diagnostics/stage4-image-colour.py \
  --output ANOTHER_NEW_DIRECTORY --variants
python3 tools/pdf-harness/diagnostics/stage4-image-colour.py \
  --output ANOTHER_NEW_DIRECTORY --variant direct-icc \
  --variant direct-icc-no-mask --variant old-direct-icc --large-control
```

The image-data driver `stage4-image-decoded.mjs` uses the same pinned PDF.js
document options and exposes the decoded image object without drawing a canvas.
Run it in the same pinned Playwright container as the PDF.js harness.

## Decision on the protected pin update

The protected project-operation scene's **narrow delta remains justified**:
the source image there is a constant black texel; its interior ROI is correct;
old/current full-page rasters are identical in both pinned viewers; the only
structural changes are the image Y matrix, image/mask interpolation flags and
document IDs. All other geometry, text, fonts, mappings, image/mask data and
metadata are unchanged. Its proposed operation/inventory expectations pass.

Accordingly, **the parent can review a pin-maintenance update limited to that
exact protected scene and orientation/interpolation delta**, with this defect
explicitly tracked as pre-existing. `narrow-pin-proposal.json` states that scope
and records `original_2x2_colour_gate_passed: false` and `approval: null`.
It is not a conformance-case manifest, tolerance exception, or claim that the
full image-colour qualification passed. The original collector's stronger
2×2 colour gate remains failing; no full requalification approval subject was
manufactured.

For final PDF conformance/freeze, fixing the producer first is preferable: use
the direct ICCBased array/profile reference in each image XObject while retaining
legal `/CS1` use by content-stream operators. Add image-only, unprimed colour
regressions with opaque and soft-mask variants in both viewers, and inspect the
actual image dictionary. That fix will require another explicit artifact/
structural/proof rebind, so approving the current narrow pins first may be
temporary. No production fix, pin update, approval, commit or full suite was
performed in this diagnosis unit.
