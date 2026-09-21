# Final pending #188 production packet — independent review required

**Production/qualification/migration subject:**

```text
sha256:6dbac083ea0d744880bebe0aab6a259f9530bad0635540faeba156edad23c1fa
```

**V10 subject, unchanged:**

```text
sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15
```

Both subjects are pending explicit user approval. The earlier capture-draft
subject is preserved as historical checkpoint evidence and is not the final
production approval request. No actual approval or live migration was recorded.

## Completed qualification

- All 168 old/current packaged full-key records are byte-identical in outcomes,
  artifact/failure blobs and reports, with two sessions in fresh forward/reverse
  processes. Fresh OLD evidence names b01f2db rather than relabeling prior results.
- All 160 ICC applicable gates pass: 104 canonical-color and 56 strict-recovery
  vectors. The frozen independent reference remains unchanged.
- Existing #187 gates retain 64 canonical vectors, 12 known-failure vectors and
  24 exact sampling-failure pins. Six sampling cells remain gaps.
- V10 native artifacts/reports and all viewer rasters equal approved V9;
  bridge v4 and protected PDF native/report/qpdf/viewer equality pass.
- Full workspace completed normally: **1226 passed, five failed, 19 ignored**.
  All 595 fast and 404 library tests pass. The five current failures are exact
  pending ownership/representative binding failures, not waived or reported green.
- Strict raw-log classification matches every failing identity, source location,
  message, target summary and final error trailer. Nine negative checker tests pass.

See `FULL-SUITE-REVIEW.md` and `checks/full-suite-record.json` for exact evidence,
including the assertion-only failure's independently reproduced CLI cause.

## Bound packet contents

`manifest.json.files` enumerates all bound files across this packet, V10 and bridge
v4, including:

- Authenticated before-source/Cargo/ledger/policy/helper records and final freeze.
- OLD/current 168-key package archives, complete blobs, metadata and repeat proofs.
- Frozen source-bound ICC/#187 focused logs, protected equality and representative
  producer/viewer/font/resource/semantic evidence.
- Exact 16-owner/64-refresh/V9→V10 templates and catalog identity.
- Raw completed full-suite log, parent message, exact classification and pending
  CLI reproduction.
- Immutable original checker plus strict unbound suite-verification successor,
  its source-bound negative tests/proof and recorded orchestration.
- Earlier capture-draft records, preserved without rewriting history.

The V10 manifest's approval field is intentionally outside its canonical subject;
only its approved file digest is deferred in the owner-binding template. All
production/test/xtask/frozen tooling bytes remain unchanged.

## Read-only verification

```sh
P=tests/conformance/proposals/jpeg188-requalification-v1
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_suite_verification.py"
python3 "$P/suite-verification.py" verify
```

This verifies both pending subjects, all bound evidence and current exact proposed
migration; it runs no broad suite and writes no live metadata.

## Future actual approval/application protocol

After independent review and actual explicit user approval of **both subjects**,
designated maintainer `sagikazarmark` records one issue #85 comment whose entire
body is exactly one standalone fenced JSON block:

````text
```json
{
  "decision": "approve",
  "scope": "jpeg188-icc",
  "production_subject_sha256": "sha256:6dbac083ea0d744880bebe0aab6a259f9530bad0635540faeba156edad23c1fa",
  "representative_subject_sha256": "sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15"
}
```
````

This is a future protocol template, **not a granted or recorded approval**. The
checker fetches the actual comment and requires exact author, ID, URL, date,
affirmative scope/subjects, unique fields and saved/live body agreement. V10 must
carry the same actual identity/date/comment for its own subject.

Parent owns the guarded three-file transaction: persist actual production approval,
comment, exact V10/registry/ledger preimages and attempt status before mutation;
apply only the reviewed templates and derive V10's final manifest-file digest.
Preserve rollback/failure evidence on errors; mark applied only after real approved
consumer/registry/owner/inventory/ownership checks pass. Rerun exactly the three
affected xtask targets to resolve the five pending failures, then run
`suite-verification.py verify-applied`. No redundant full-workspace run is needed.

Projected totals: **1992 assignments / 1915 assigned-excluded / 77 gaps**. All
80 ICC criteria, 168 full keys, other dispositions and three exclusions survive.
No baseline/#178, #187 sampling, global or fixed-size CMS resource closure is
granted. This README is excluded explanatory metadata. No commit was made.
