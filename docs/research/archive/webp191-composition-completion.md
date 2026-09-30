# #191: approved first-frame composition correction

## Correction and reproduction

Starting at `dd3389a`, a focused native PNG/PDF test against the frozen browser
record reproduced 48 incorrect alpha pixels outside a 5×3 opaque frame placed
at (2,2) on a 9×7 canvas. Standalone ANMF decoding followed by placement onto
transparent RGBA makes that test pass. It also avoids sample changes from
blending the first frame against an empty canvas. The final regression selector
is `webp_sample_contract::corrected_composition_cells_match_reference`.

The private extraction helper is shared with full animation validation. Every
later frame is still validated; source-preserving SVG behavior, ICC normalization,
orientation, diagnostic reports, dependencies and generic premultiplication are
unchanged. The helper uses one fallibly reserved standalone buffer with checked
lengths and placement, and drops temporary buffers early. This does not establish
physical allocator/OOM closure.

## Measured scope

- 11 newly qualified cells / 22 vectors; 24 of 36 sample cells now qualify.
- SVG 72/72, PNG 48/72 and PDF 48/72 pass the complete existing gates.
- Alpha passes for every vector. Twelve partial-alpha cells / 24 vectors remain
  gaps, with 72 exact failing comparison views.
- Exactly 30 vectors / 60 raster records change: the 22 corrected vectors plus
  eight partial-alpha blend vectors that improve but remain failing.
- Historical inputs, browser observations and native defect measurements remain
  immutable. New composition measurements have SHA-256
  `6c9f48c8903604c7112085d64fd53325d30975f3552c01a3ace5e50dae5de668`.

## Residual partial-alpha diagnosis

An external diagnostic compared actual libwebp 1.3.2 RGBA/rgbA output with
image-webp 0.2.4 and the frozen browser surfaces. All twelve static vectors have
identical decoded straight samples. Floor premultiplication reproduces all 36
raw browser surfaces and 72 observed black/white surfaces. Rounded projection
differs by one composite channel unit on 37–43 pixels in each partial-alpha
static vector. The diagnostic Rust compiler was 1.98.1, so these are causal
probes, not canonical production qualification.

[Chromium 149.0.7827.55 selects premultiplied libwebp output for these untagged
alpha inputs](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/webp/webp_image_decoder.cc#L200-L217).
Its bundled libwebp revision
[`c00d83f6642e7838a12bb03bca94237f03cc2e00` uses floor premultiplication](https://github.com/webmproject/libwebp/blob/c00d83f6642e7838a12bb03bca94237f03cc2e00/src/dsp/alpha_processing.c#L209-L245).
The formula `(c*a*32897)>>23` equals `floor(c*a/255)` for all 65,536 input pairs;
the renderer's generic path rounds instead.

PNG therefore has a separate WebP premultiplication parity defect. PDF's extracted
straight samples are correct: its failing synthetic comparison rounds them under
§14.3 and compares against already floor-premultiplied browser observations.
For example, straight `[1,128,128,1]` projects onto black as `[0,1,1]` under the
canonical rounded rule but `[0,0,0]` through libwebp. This is not a measured
PDFium/PDF.js defect and does not justify changing PDF source samples. Actual
viewer evidence and an explicit comparison-boundary decision are needed before
qualifying those cells. Both straight and composite gates remain enforced.

## Verification and approval boundary

- Full workspace, Rust 1.97.1, `--locked`: **1,257 passed, five stale-binding
  failures, 19 ignored**. All 626 fast and 404 library tests passed. The full
  suite is not green before the approved migration.
- All 168 packaged records match the prior capture exactly, including complete
  reports and failures. V13 native outputs and six viewer-pair rasters equal V12.
- Formatting, Clippy, focused codec regressions, and 37 isolated packet tests
  pass. Independent Standards and Spec reviews found no blockers; their stale
  README finding was corrected in excluded explanatory metadata.
- Applied migration: 11 new owners, 25 source-only binding refreshes, V12→V13.
  Current ledger: 2,418 obligations, 2,321 assigned/excluded, 97 gaps.

Sealed production/qualification subject:
`sha256:8c706d759ad01650eb34eac247ca0ba2d61cf3ac101ede290ffd9c398edbc801`.

V13 subject:
`sha256:520b71395cd48b8768caba6eedadf6c6c40f996a9e0c36019f5ef9dafe62c5f5`.

Packet: `tests/conformance/proposals/webp191-composition-requalification-v1/`.
The user explicitly approved both presented subjects. The actual designated
maintainer decision is [issue #85 comment 5762673976](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5762673976),
dated 2026-09-21. Authenticated decision, three metadata preimages, successful
109-test/six-command receipts and applied status are retained under
`tests/conformance/coverage/webp191-application-v1/`.

Application and independent `suite_gate.py verify-applied` both passed, including
fresh reruns of all 109 bound tests and required approved consumers. All five
stale-binding failures are resolved; the historical full-suite failure log is
preserved. The first application attempt stopped before migration because the
local qpdf executable was absent; installing the exact pinned PDF harness restored
the required environment without changing source or evidence.

#191 remains open for partial alpha; #178, #187 sampling and #190 HDR remain
separate deferred work.
