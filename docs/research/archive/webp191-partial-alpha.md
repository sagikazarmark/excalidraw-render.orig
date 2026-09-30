# #191 partial alpha: PNG correction and PDF viewer diagnosis

Status: implemented and tested against `852c093`, committed as a checkpoint at
the user's request, and not yet production-requalified. The ledger and approved V13 remain untouched. Twelve
partial-alpha cells remain unqualified because their PDF gates still fail.

## PNG correction

For WebP with `ColorNormalization::None`, derive the draw-time representation
using integer `c*a/255`, matching the pinned browser's libwebp path. Preserve
canonical straight samples for PDF and preserve rounded conversion for real
color transforms. The regression first failed on the lossless partial-alpha
static ramp (37 differing pixels), then passed all 72 vectors.

Nested SVG-to-PNG normalization previously discarded this draw-time copy. It now
encodes a derivative straight PNG that round-trips to the same premultiplied
channels through resvg, only for untransformed WebP targeting PNG. PDF and tagged
color paths retain canonical straight storage. The cache is request-scoped.
Seventy-two nested comparisons against independently captured browser PNGs
through the same nested pipeline pass exactly. This tests alpha-quantization
preservation, not independent browser parity for nested geometry/resampling.

Current direct sample results: **SVG 72/72, PNG 72/72, PDF 48/72**. The historical
composition record remains immutable. Current SVG and PDF measurements still
match it exactly; all 24 partial PNG rows now pass the unchanged gates.

## Actual viewer collection

Create-only diagnostic tooling:

```sh
python3 tools/pdf-harness/diagnostics/webp191-partial-viewers.py <new-repository-directory>
PYTHONDONTWRITEBYTECODE=1 python3 tools/pdf-harness/diagnostics/webp191-pdfium-raw.py <same-directory>
```

Evidence: `tests/oracle/webp-sample/partial-viewer-diagnosis/`. It includes all
72 PDFs, two independent native renders with identical complete JSON reports,
two independent executions per pinned viewer, artifact hashes, runtime metadata,
and raw PDFium buffers. The reference record is authenticated by its existing
`b325e8a2…` digest. Native collection used Rust 1.97.1 and `--release --locked`.
These are diagnosis artifacts, not approved reference replacements or new gates.
An initial discarded target-directory probe captured the wrong CLI report stream;
the retained collection captures stderr and verifies entire success records.

Both adapters exactly match the browser for 48 opaque/binary controls. Every
partial-alpha vector differs. PDF.js differs at 6–43 pixels, with maximum
black/white composite channel delta one. Since the complete 9×7 artifact has no
11×11 SSIM window, these differing views cannot pass the small-artifact exact
rule. Replacing the synthetic PDF test with actual viewer output alone would
therefore not qualify these cells.

### PDFium adapter defect: #192

The unchanged adapter unpremultiplies bitmap output which this pinned backend
already returns as straight RGBA. Across **all 72 vectors**, buffers captured
before `canonicalize()` exactly equal independently decoded straight samples,
including placement and invisible-RGB canonicalization. The adapter's partial
outputs instead show 12–49 differing pixels and composite errors up to 64.

Minimal example: straight source and raw PDFium pixel `[1,128,128,1]` become
`[255,255,255,1]` through the adapter; browser readback is `[0,0,0,1]`, and PDF.js
readback is `[0,255,255,1]`. On the lossless static ramp, raw canonical projection
differs at 37 pixels by at most one; adapter projection differs at 49 pixels by
up to 64. The larger error is an adapter defect, not evidence of a PDF renderer
or source-sample defect.

Tracked as [#192](https://github.com/sagikazarmark/excalidraw-render.orig/issues/192).
Canonical adapter/profile bytes remain unchanged. Correcting them requires
independent translucent controls, auditing existing alpha-bearing evidence, and
explicit viewer/profile/representative requalification. It must precede relying
on this adapter for new partial-alpha approval. Raw probes do not themselves
constitute a qualified replacement adapter.

## Verification and review

- Full workspace completed: all **405 library tests and 628 fast tests passed**;
  **seven stale source/ownership/representative binding failures** remained in
  xtask, with 19 existing ignored tests. This is not a green full-suite result.
- Raw log: `tool_0c538fa47001p2EdjfmvyO7ysy` in the session tool-output directory.
  Four xtask-bin failures cover the bridge, representative-positive path and two
  owner-source checks; two coverage tests and the representative consumer fail.
- After a checked-division Clippy cleanup and test-selector rename, all nine
  WebP library and twelve WebP fast tests passed again. Workspace/all-target
  Clippy with `-D warnings`, formatting and ordinary diff checks pass.
- Standards review found no hard violation. Its report-stream comment and test
  naming advisories were addressed. Spec review found the nested projection
  omission (fixed and re-reviewed) and the PDFium adapter defect (open #192).
- Dependencies, frozen reference observations, historical pins, PDF source
  samples, comparator thresholds, policy criteria, live owners and V13 approval
  have not been refreshed.

## Required next decision/work

Correct and requalify #192 before sealing a trusted successor representative for
the PNG change. Then evaluate the remaining one-unit PDF/browser quantization
boundary with qualified viewer output and an explicitly reviewed disposition.
Neither modifying canonical PDF samples nor silently dropping composite gates is
supported by these measurements. No new all-format owners can be assigned yet.
