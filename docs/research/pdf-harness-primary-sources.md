# PDF harness primary-source findings

Research for GitHub issue [#35, "Qualify PDF visual and structural harnesses"](https://github.com/sagikazarmark/excalidraw-render.orig/issues/35).
Sources were checked on 2026-07-19. The approved renderer specification fixes
the two viewer families, 96 DPI resolution, canonical raster protocol,
comparator, structural fields, and hard-failure behavior; it deliberately leaves
concrete tool versions as implementation evidence.

## Selected identities

| Role | Exact identity | Immutable acquisition |
| --- | --- | --- |
| PDFium viewer | PDFium `152.0.7947.0`, build `7947`, default `pdfium-binaries` flags, driven through the `pdfium-render` Rust bindings (`tools/pdf-harness/pdfium-rust`); the pypdfium2 `5.12.1` wheel at commit [`b3e7e67a1e35c9436b52cb043d476b89ec8c38cb`](https://github.com/pypdfium2-team/pypdfium2/tree/b3e7e67a1e35c9436b52cb043d476b89ec8c38cb) was the historical Python adapter and remains the source of the engine identity | upstream `pdfium-binaries` archive, checksum-pinned by `tools/pdf-harness/install.sh` |
| PDF.js viewer | `pdfjs-dist` at the version in `tools/pdf-harness/pdfjs/package-lock.json` (`6.1.200` at commit [`6353acefe5007cd4899247a8c4e83cb7c9435a54`](https://github.com/mozilla/pdf.js/tree/6353acefe5007cd4899247a8c4e83cb7c9435a54) when this was written; a PDF.js update needs no requalification of its own, ADR-0019), `@napi-rs/canvas` `1.0.2` | npm lock plus canonical Playwright image digest `sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9` |
| Structural parser | qpdf `12.3.2` at signed tag commit [`a898bb3a7289d1d05789d6d3f0d5dd534943a8da`](https://github.com/qpdf/qpdf/tree/a898bb3a7289d1d05789d6d3f0d5dd534943a8da) | relocatable Linux x86_64 archive SHA-256 `44f2c53bf784c0143128d80d2b9946e9793962c5bb403b75c0024cb4d8e346b9` |

The pypdfium2 wheel records PDFium's origin, build, version, and enabled flags in
its packaged `pypdfium2_raw/version.json`; the Rust worker records the same
identity from the installed library. pypdfium2's API documentation says the build
number uniquely identifies the PDFium version and warns that PDFium is not
thread-safe, which supports a fresh single-threaded process per render.
[Source: pypdfium2 version and threading documentation](https://pypdfium2.readthedocs.io/en/stable/python_api.html)

The qpdf project describes the selected archive as a nearly standalone Ubuntu
LTS build with relative runpaths and bundled nonstandard shared libraries,
intended for minimal Linux environments. This avoids ambient distribution
packages while retaining a reviewable upstream binary.
[Source: qpdf download guidance](https://github.com/qpdf/qpdf/blob/a898bb3a7289d1d05789d6d3f0d5dd534943a8da/README-what-to-download.md)

## PDFium rendering decisions

`PdfPage.render` defines scale as pixels per PDF canvas unit and explicitly says
DPI converts through `dpi / 72`. It computes output dimensions with `ceil`,
creates top-left bitmaps, exposes reverse byte order for RGB(A), and permits a
forced BGRA format and transparent initialization. Because `ceil` sizes the
bitmap and `FPDF_RenderPageBitmap` then stretches the page to fill it, a
fractional 96 DPI page would be scaled by up to a pixel; the adapter therefore
keeps `PdfPage.render`'s bitmap, fill, format, and flag choices but issues the
render through `FPDF_RenderPageBitmapWithMatrix`, whose base display matrix
maps the page's top-left corner to the device origin at one point per pixel
without rounding the page size. The selected adapter fixes:

- scale `96 / 72`: binary64 for the admission check, then carried to PDFium as
  the binary32 `FS_MATRIX` render matrix on top of the unrounded display
  matrix, with the page origin at the bitmap origin;
- one unrotated PDF 1.7 page;
- a bitmap of exactly the caller-supplied size, admitted when each 96 DPI page
  dimension is within one pixel of it (exact equality for integral pages),
  clipped to the bitmap so uncovered pixels stay transparent;
- transparent initialization and forced BGRA with reverse byte order;
- forms and annotations off;
- default display rendering with path, image, and text smoothing on;
- no print/LCD optimization and no forced halftone;
- PDFium reinitialization with a checked empty directory as the complete Linux
  user-font path set, replacing default system font directories;
- deterministic integer unpremultiplication and zero RGB below zero alpha.

[Source: pypdfium2 page renderer](https://github.com/pypdfium2-team/pypdfium2/blob/b3e7e67a1e35c9436b52cb043d476b89ec8c38cb/src/pypdfium2/_helpers/page.py)
[Source: bitmap layout](https://github.com/pypdfium2-team/pypdfium2/blob/b3e7e67a1e35c9436b52cb043d476b89ec8c38cb/src/pypdfium2/_helpers/bitmap.py)
[Source: PDFium matrix render entry point, release branch `chromium/7947` of build 7947](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/fpdfsdk/fpdf_view.cpp)
[Source: PDFium display matrix over the float page rect, release branch `chromium/7947` of build 7947](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/core/fpdfapi/page/cpdf_page.cpp)

## PDF.js rendering decisions

PDF.js `getDocument` accepts owned typed-array data and exposes explicit controls
for error recovery, system fonts, external CMaps/ICC/standard-font/WASM data,
range and stream loading, XFA, image decoders, OffscreenCanvas, and hardware
acceleration. `PDFPageProxy.render` separately controls annotations and the
canvas background. The selected adapter fixes all of those rather than
inheriting environment defaults:

- owned `Uint8Array` only, no URL or base URL;
- `stopAtErrors=true`;
- range, stream, auto-fetch, system fonts, font-face loading, XFA, WASM,
  ImageDecoder, OffscreenCanvas, and HWA disabled;
- no CMap, ICC, standard-font, or WASM URL;
- display intent, annotations disabled, transparent background;
- exact `96 / 72` viewport drawn onto a canvas of exactly the caller-supplied
  size, admitted when each 96 DPI viewport dimension is within one pixel of it
  (exact equality for integral pages); `PageViewport.transform` places the
  page origin at the canvas origin, so a fractional page is cropped by the
  canvas rather than scaled to it and uncovered canvas pixels stay transparent;
- fresh Node process and PDF worker;
- straight RGBA from `getImageData`, with transparent RGB canonicalized to zero.

[Source: PDF.js display API](https://github.com/mozilla/pdf.js/blob/6353acefe5007cd4899247a8c4e83cb7c9435a54/src/display/api.js)
[Source: PDF.js viewport transform](https://github.com/mozilla/pdf.js/blob/6353acefe5007cd4899247a8c4e83cb7c9435a54/src/display/page_viewport.js)

## Structural inspection decisions

qpdf documents exit status `0` for clean input, `2` for errors, and `3` for
warnings. It also documents `--suppress-recovery`, but the harness intentionally
allows recovery so warnings and a report can be retained; every warning remains
a hard gate failure. qpdf JSON v2 supplies the object graph, page resources, and
inline stream bytes. Generalized decoding is selected so content streams can be
tokenized without rewriting the PDF.

[Source: qpdf command and exit-status documentation](https://qpdf.readthedocs.io/en/stable/cli.html)
[Source: qpdf JSON documentation](https://qpdf.readthedocs.io/en/stable/json.html)

The owned inspector reports:

- PDF version, page count, boxes, rotation, and `UserUnit`;
- every ordered content operator and operands, plus vector and text subsets;
- font resources, embedding stream digests, `ToUnicode`, and `ActualText`;
- annotation subtype, geometry, border, flags, action, and URI;
- clips, alpha, blend modes, soft masks, and image soft masks;
- every page Image XObject resource, including unused resources, dimensions,
  color space, bit depth, filters, recursively inventoried mask and soft-mask
  bytes and digests, and occurrence count;
- complete catalog, page, and annotation action graphs including `OpenAction`,
  `AA`, and `Next`, with only safe HTTP, HTTPS, mailto, and relative URI actions
  admitted;
- encryption, signatures, IDs, incremental revisions, linearization, Info, XMP,
  attachments, output intents, complete ICC digest/header identity, and tags;
- normalized parser warnings and errors.

## Qualification control boundary

Native PDFium path antialiasing did not meet the fixed geometry/image SSIM floor
for the project-authored rough rectangle even though Pixelmatch reported zero
changed pixels. Altering the approved comparator or adding an unapproved
tolerance was rejected. Instead, the valid qualification PDF remains vector
only, retains the cubic rough path for structural extraction, covers it, and
uses pixel-grid PDF rectangle fills to reproduce the already approved Chromium
raster exactly. This control qualifies viewer transport, canonicalization,
determinism, comparator wiring, and structural extraction only. It is explicitly
not renderer output, official PDF evidence, or permission for a renderer to use
calibration overlays.

The independent negative controls are:

- a visible red vector rectangle that fails both visual gates while preserving
  the accepted structural contract;
- an unused but reachable undeclared Image XObject that leaves both visual gates
  unchanged while failing exact structural inventory;
- a damaged `startxref` that qpdf repairs while emitting warnings, proving that
  recoverable parser input still fails closed.
