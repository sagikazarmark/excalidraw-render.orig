# Native snapshot reconciliation after font registry approval

Status: **baseline approved and packaged replay passing**.

## Evidence

- Clean tested commit: `b419e23a1f133f03b559009973a6f22d507a6f8e`.
- Canonical collection: `dagger api call packaging snapshots --output target/font-registry-native-snapshots` (passed in 4m8s).
- `verify-native-snapshots` validates 168 execution keys, two sessions in each of separate forward/reverse processes: **672 renders**, clean source and package provenance.
- Staging: `cargo run --locked -p xtask -- conformance stage-native-baseline target/font-registry-native-snapshots target/font-registry-native-baseline` passed, including archive/source verification and Git ancestry.

## Comparison with the previous approved baseline

Join keys are `(case, variant, export target, format)`, deliberately excluding the changed input digest for this review. Every previous key has exactly one new match, with no additions or omissions.

- All **168 complete artifact/report samples are identical** to the previous approved expectations.
- All **187 content-addressed blobs**, totaling **6,146,996 bytes**, are identical by both filename and actual bytes. No blob was changed, added or deleted in the repository.
- All 168 `inputs_sha256` fields change because the inventory now binds the reconciled scene-surface registry, derived catalog and reapproved case manifests.
- The inventory descriptor and fresh collection/package/compiler provenance are updated through the baseline producer. Collector and producer logic are unchanged.

Only `native-baseline/revision-1.json` and its digest sidecar were replaced with the newly staged candidate. It was staged with null approval, then explicitly reapproved; the previous approval remains in Git history.

## Recorded #85 approval

`sha256:f6e6a9e6a538b60b088038d19586c943b92166a18f5a9dee8a1b68c37984c957`

This is the repository-computed `native-baseline-approval-subject`, covering the refreshed input bindings and clean-commit provenance with unchanged artifact/report expectations. Approval of the 36 case subjects did not approve this separate baseline subject.

The maintainer explicitly approved this subject in the coding session on 2026-09-13. Approval is recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5652971255) and in the baseline through `record-native-baseline-approval` under `sagikazarmark`.

`verify-native-baseline` passes. `dagger check excalidraw-render:packaging:snapshot-replay` passed in 5m45s on the working tree based on `0546304`, freshly collecting packaged output and matching all 168 approved artifact/report expectations. Full cross-target equality and global ownership closure remain separate gates; #178 and the broader conformance gaps are not waived.

## Checks

The candidate verifier, baseline staging/provenance validation, approval validation, explicit old/new byte comparison and canonical packaged replay pass. `git diff --check` passes. Expected output blobs have not been repinned to conceal any renderer drift.
