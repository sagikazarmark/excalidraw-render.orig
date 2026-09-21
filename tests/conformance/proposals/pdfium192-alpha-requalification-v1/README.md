# #192 PDFium alpha profile: approved and applied

The user approved the exact subject below. The authenticated maintainer decision
is [#35 comment 5767353707](https://github.com/sagikazarmark/excalidraw-render.orig/issues/35#issuecomment-5767353707).
The four proposed metadata files were applied and xtask recorded the actual
approval; see `application.json`. Dependent representative/bridge/ownership
requalification remains pending. The following proposal description is retained
as history. `pdfium192-proposal.py verify` is a pre-application check and now
intentionally rejects changed live metadata; use `conformance verify-pdf-viewers`
and `conformance pdf-viewer-approval-subject` to verify applied state.

Exact new profile/qualification approval subject (Python and staged Rust agree):

```text
sha256:c393a0dd957df1041323b2e736406fc21e30a915176388ca8a1da23460840c01
```

Approval is requested on #35, the viewer qualification's existing approval
authority. This subject binds the corrected adapter, mandatory independent alpha
controls, updated profile semantics and qualification script. It authorizes no
renderer representative, ownership, baseline, exception or comparator migration.

`before/` preserves six exact committed preimages, including old approval.
`pending/` contains four proposed live metadata files: PDFium profile and sidecar,
qualification manifest and sidecar. The manifest approval is null. Existing live
metadata is untouched. Old approval is not transferred.

## Verification

- Independent, hand-authored PDF vector-opacity and image-soft-mask controls fail
  with the old adapter and pass with the corrected adapter. Alpha 1 maps RGB
  `[64,128,192]` to itself, rather than `[255,255,255]`.
- Three controls now pass: full-buffer native PDF checks, all 256 alpha values
  with invisible-RGB normalization, and identical fresh-process outputs.
- `tools/pdf-viewer-qualification.sh reproduce` runs those controls and reproduces
  all 24 old calibration/evidence files byte-for-byte, including negative gates.
- Both existing ignored adapter placement/rejection tests pass with pinned tools.
- `pdfium192-proposal.py verify` passes. An isolated detached checkout at the same
  HEAD, `/tmp/opencode/pdfium192-stage`, staged the proposed files, compiled xtask,
  and reproduced the subject above. Approved verification rejects null approval.
- Independent Standards and Spec reviews found no blockers. The review confirms
  the four-file semantic scope and unchanged live metadata and archived preimages.

Read-only verification:

```sh
python3 tools/pdf-harness/diagnostics/pdfium192-proposal.py verify
PYTHONDONTWRITEBYTECODE=1 python3 tools/pdf-harness/pdfium/test_alpha.py
```

The audit/proposal scripts are directly hash-bound by `proposal.json`; they are
not transitively part of the viewer-profile approval subject. The actual alpha
tests and mandatory qualification runner are part of that subject.

## Historical impact

`../pdfium192-alpha-audit-v1/audit.json` records all 839 tracked PDF paths,
deduplicated into 118 byte identities, including extensionless PDF snapshot blobs.
All rendered; 11 identities changed. The old formula is applied to the same raw
bitmap for diagnosis, with exact matching historical raster paths recorded
separately. It is not represented as a fresh execution of the old adapter.

- Eight identities cover all 24 WebP partial-alpha vectors, with 48 stored viewer
  rasters affected. The corrected buffers match independent straight samples.
- One ICC control changes 1,000 pixels (two stored rasters).
- The structure-coverage calibration PDF changes four pixels when rasterized;
  its existing structural gate does not depend on that raster.
- The transparent transform-opacity baseline scene changes 20,596 pixels.
- V13, bridge-v7, protected and orientation PDF outputs are unchanged. Their
  source/profile approvals still require successor requalification.

## Remaining dependency order

1. Explicitly approve this exact profile subject on #35.
2. Authenticate and apply only the four proposed profile/qualification metadata
   files, retaining the actual decision and preimages.
3. Collect and review successor representative/bridge/protected evidence and the
   narrowly enumerated registry/ownership transitions for the committed #191 PNG
   correction and corrected viewer. Obtain their own exact-subject approvals.
4. Resolve stale-binding checks through fresh successful execution. #178 native
   baseline approval and #191 partial-alpha comparison decisions remain open.

No policy relaxation, historical raster overwrite or blanket baseline refresh is
part of this proposal. The full workspace has not been rerun during this adapter-
only step; focused viewer execution is the relevant verification. Source-bound
approval consumers remain stale until the approved migrations are applied.
