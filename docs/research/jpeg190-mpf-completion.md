# JPEG #190 MPF validation completion — 2026-09-21

The user explicitly approved both exact subjects, including the 16 recovery
criterion changes. The user-granted affirmative maintainer decision was recorded
by `sagikazarmark` on 2026-09-21 at
[#85 / 5758239064](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5758239064).
No qualification script grants approval.

## Applied subjects and migration

- Production/policy/qualification/migration:
  `sha256:090a8683ca07feff5c95800de8713da14270ceba6ac50f95ab669a3107512963`.
- Representative `representative.stage4-frames-text-assets.v12`:
  `sha256:f6c3a1fb16d0c1ce7d96b0ccd23ea2a4a95635f34ca56de3e669f28f9e81d9bf`.

The guarded four-file transaction persisted the real decision and preimages
before applying the policy, ledger, registry and V12 approval. Its exact scope is:

- 16 malformed-index criteria changed explicitly from interpretation/admission
  requirements to strict `image.decode-failed` recovery requirements;
- 16 new strict-recovery owners and 28 source-only refreshes;
- V11→V12 representative binding and the derived catalog identity.

This is an approved semantic criterion amendment, not an append-only inventory
extension or an implicit hash refresh. All other criteria, three exclusions,
168 inventory keys and the HDR failure pins remain unchanged.

Application evidence is in `tests/conformance/coverage/jpeg190-application-v1/`.
Application and independent `verify-applied` both passed. Saved successful
source/subject-bound receipts and fresh repeated checks establish **109 passed
tests (103 + 5 + 1)** and approved candidate/registry/owner/inventory checks.
All five earlier stale-binding failures are resolved. No rollback was needed.

## Bounded parser and qualification

The primary MP Index validator uses fixed storage and checked TIFF slices. It
requires version 0100, two JPEG MP entries, valid field types/counts, ordered tags,
nonoverlapping bounded value regions and declared image ranges matching actual
marker-aware JPEG boundaries. It supports standard optional UID/TotalFrames tags,
relocated MP entries and the representative-primary flag. The secondary image may
carry the tested version/individual-number Attribute IFD; competing indexes and
duplicate secondary MPF segments are rejected.

This remains a narrow two-image profile: dependencies, primary Attribute IFDs,
other MP types and noncontiguous containers are not claimed supported. Existing
XMP discovery is not a full XMP semantic validator. The old signature-only unit
fixture now correctly proves rejection rather than pretending to be a valid index.

All 104 original vectors retain their applicable gates:

- 24 canonical SDR vectors;
- 64 exact strict-recovery vectors;
- 16 HDR known failures with **32 unchanged exact pins**.

All 46 focused JPEG tests, including 12 gain-map tests, and ten library JPEG
codec tests passed. Optional-field/layout and secondary-attribute positive and
adversarial controls are structural tests, not additional browser qualification.

All 168 packaged artifacts/failures and full reports equal OLD `80670ae` under
two-process/two-session forward/reverse collection. V12 artifacts/reports and six
viewer-pair rasters equal V11; protected-PDF structure and repeated outputs are
unchanged. Dependencies and frozen reference bytes remain identical.

## Verification history

The completed full workspace invocation recorded **1,244 passed, five pending
stale-binding failures and 19 existing ignored tests**, including 613 fast
passes. Its raw result remains explicit, not described as green. Following actual
approval/application, the 109-test rerun and independent applied verification
resolved those five failures. No redundant full-workspace rerun was performed.

Formatting and Clippy passed. Independent review found a secondary-image index
validation gap before freeze; it was reproduced, corrected, and covered by exact
recovery and positive Attribute IFD tests. Production and final packet re-review
found no remaining scoped blocker. Policy/rollback negative tests require all
four preimages, including policy, to be restored on failure.

## Final ledger and deferred HDR work

| Measure | Count |
| --- | ---: |
| Obligations | 2,126 |
| Primary owners | 2,038 |
| Approved exclusions | 3 |
| Assigned/excluded | **2,041** |
| Explicit gaps | **85** |

#190 remains open for eight HDR projection cells. Diagnosis established linear
gain math and, for the larger gain-map control, Chromium's 7/8 reduced-IDCT decode
before bilinear sampling. The pinned decoder exposes no scaled decode or DCT
coefficients. The user chose MPF validation first and deferred that decoder
strategy; ordinary post-decode resizing is not an equivalent solution.

#187 sampling, #178/native-baseline work, global ledger approval and pre-existing
fixed-size CMS allocation/resource limitations remain separate.

Current applied verification:

```sh
python3 tests/conformance/proposals/jpeg190-mpf-requalification-v1/suite_gate.py verify-applied
```

The command requires GitHub access and reruns the bounded approved checks.
