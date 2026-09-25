# #191 PDF partial-alpha comparison boundary — decision proposal

Status: proposed, no policy/spec/owner changes and no closure claimed.

**Decision (2026-09-23):** the user chose exact-ceiling tolerance exceptions
instead of keeping the gaps. The 48 failing black/white PDF views of the 24
partial-alpha vectors carry exceptions at their exact observed ceilings
(`tests/oracle/webp-sample/pdf-partial-alpha-exceptions.json`), spec section 14.4
gains the small-artifact exception form, and
`webp_sample_contract::partial_alpha_cells_match_reference_under_approved_pdf_exceptions`
owns the twelve cells. It rejects growth and dead exceptions. No comparator,
budget or production change.
Base: `1776fc6`; approved PDFium profile subject `c393a0dd…`, representative V14.

## Recommendation

**Separate demonstrated codec/sample correctness from unresolved visual parity.**
Keep the twelve existing all-format partial-alpha obligations as gaps. Retain
their exact failure pins and the canonical small-artifact rule. If narrower
coverage is useful, append separately named PDF sample/alpha obligations using
the new independent controls, through ADR-0011's authenticated append-only flow.
Those owners must explicitly exclude browser visual parity; they cannot replace
or silently narrow the twelve existing criteria.

This completes the diagnosis, not the visual promise. Closing the twelve cells
without a production change would require an explicit revision to the current
compatibility contract or a demonstrated compliant production fix. The fixed
straight samples cannot match browser composites under the current §14.3
projection; the two measured PDF representations and pinned viewers reproduce
that mismatch. This does not exclude every possible PDF rendering construction.
No compliant alternative fix is demonstrated, and no contract revision is
implemented here.

## Fresh approved-profile evidence

- `tests/oracle/webp-sample/approved-profile-viewer-diagnosis/`: all 72 native PDFs,
  two fresh native renders with identical complete reports, both pinned viewers
  in two independent processes. PDFium uses the corrected and approved profile.
  Measurement digest:
  `6d47db1399e8c857b8f402d7604d21bd0ed8474f4422fa2128bcab6a40833879`.
- `tests/oracle/webp-sample/comparison-boundary-controls/`: 72 independent PDFs
  authored directly as DeviceRGB image + grayscale SMask. Sample bytes come from
  the original external libwebp decoding, never production or viewer readback.
  Canvas placement and PDF objects are independently authored. Both viewers run
  twice. Digest:
  `36dc6cff0076b013b6086be1fbe6290392440f6a6c9676e73e9067ae4b89e079`.
- All **144 independent-PDF vs production viewer buffers are byte-identical**.
  PDFium also equals the external straight samples exactly across all 72 vectors.
  Thus the production ICCBased encoding and independent DeviceRGB control agree
  in the measured scope; this is not proof for other profiles or transformations.
- Both viewers match the browser on 48 opaque/binary controls and fail exact
  projected-view identity on all 24 partial-alpha vectors. Alpha always matches.
  Black and white views differ at **6–43 pixels**, maximum channel delta **one**.
  Counts are identical between viewers in this matrix, though raw straight
  readbacks differ. A readback delta of 255 at alpha 1 is not a 255-unit composite
  error.

## Boundary proof

For straight source `[1,128,128,1]`, the pinned WebP browser decode truncates
premultiplication to `[0,0,0,1]`. The required canonical rounded projection of the
exact PDF straight source gives black `[0,1,1,255]`. Both facts are independently
observed and required by the current test contract.

The same difference occurs for hand-authored PDFs made from the independent
decoder samples. Each viewer's production black/white buffers equal the rounded
projection of those straight samples; browser composites equal floor projection.
The removed PDFium double-unpremultiplication is no longer involved.

§8.2 requires lossless unpremultiplied canonical PDF samples. §14.3 fixes rounded
alpha projection, prohibits color fitting/resizing/alignment search, and requires
byte-identical complete views for 9×7 artifacts. A zero Pixelmatch count does not
override that exact-view failure. An ordinary SSIM tolerance exception has no
finite score to relax here.

## Options evaluated

| Option | Measured/contract outcome |
| --- | --- |
| Use actual viewers instead of synthetic composites | Necessary evidence improvement; all 24 partial cases still fail per viewer. Does not close cells. |
| Compare with independent canonical-sample PDFs | All 144 buffers match exactly. Valid bounded sample/representation evidence; not a replacement for browser visual authority. |
| Fit PDF samples to browser readback | Violates exact unpremultiplied sample requirement; negative checks reject it. Not recommended. |
| Apply floor projection to both sides | Changes the prescribed comparator and may double-quantize readback; not an authorized correction. |
| Add a general ±1 RGB tolerance | Weakens every covered comparison, hides unrelated defects, and contradicts the issue's constraints. Rejected. |
| Resize/pad these fixtures to obtain SSIM | Does not resolve the existing complete-artifact criterion and can dilute error. Rejected as a way to close these cells. |
| Explicit case-bound non-defect disposition | Requires a new reviewed contract/disposition mechanism for this small-artifact incompatibility, not an ordinary tolerance exception or automatic pin refresh. Possible policy choice, not currently authorized. |
| Preserve gaps; separately qualify exact codec semantics | Recommended under the current contract. Honest coverage without changing the visual promise. |

## Proposed narrower obligation, if approved for implementation

For each existing partial-alpha cell, require both patterns and:

1. Authenticated original bytes, external decoder samples, browser reference and
   exact fixture reconstruction.
2. Source-bound PDF image/SMask extraction with exact canonical samples and alpha
   for VP8L **and VP8**, correct dimensions/orientation/placement, complete reports
   and deterministic replay. Lossy samples mean external decoded pixels, not the
   pre-encoding authored pixels.
3. Actual production PDF output equals the independently authored sample-control
   PDF in each approved viewer, separately, across repeated processes.
4. Original browser comparisons continue to run and fail with exact observations;
   improvements or regressions require diagnosis. They remain separate parity
   gaps and never become ignored pixels or passing judgments.

No new owners are assigned by this proposal. It does not establish general WebP,
ICC, arbitrary scale/background, layered composition or full-scene PDF parity.
The ledger remains 2,418 obligations / 2,321 assigned-excluded / 97 gaps.

## Checks

`node tools/pdf-harness/diagnostics/webp191-verify-boundary.mjs` checks approved
PDF viewer profile dependencies and both
evidence sets, reconstructs independent placement, checks all
three canonical views and actual failure outcomes, and rejects **336 mutations**
(RGB drift, alpha drift, and substitution with passing browser pixels). These
mutations exercise the independent-buffer guard; additional paired mutations
exercise the alpha, exact-sample and failure-preservation guards separately.
The Node audit replaces the retired Python verifier and runs in the xtask suite.
Collector/CLI identities describe the historical capture; inputs, profiles and
output bytes remain live checks. There are 336 independent-buffer and 336 paired
semantic rejections, with all 48 partial-alpha viewer comparisons still failing
their canonical browser comparison.
All seven existing WebP contract tests pass. Production, approved profiles, spec,
comparator, frozen historical records and the ledger are unchanged. No full
workspace rerun was needed for this evidence-only decision preparation.
