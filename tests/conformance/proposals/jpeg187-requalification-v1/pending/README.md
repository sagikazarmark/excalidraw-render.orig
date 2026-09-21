# Final sealed pending subjects — independent review required

**Color-only production/qualification/migration subject:**

```text
sha256:e3a794f1461248ccf2ad92b73b7d7cd2aa780386d7790ff1310e4678766105a6
```

**Unchanged V9 subject:**

```text
sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d
```

These are pending exact review subjects. No actual approval, live ledger/registry
migration or commit has been recorded. The prior capture-draft hash remains
immutable historical evidence and is superseded for final review by this subject.

## Verification result, stated precisely

This is **combined verification, not one clean workspace run**. The raw timed-out
log confirms library 404 passed/7 ignored and 584 unique fast successes of 585.
The parent attests the exact missing long test passed separately in 816.82 seconds,
all remaining renderer integrations passed (including PNG 10/SVG 8), renderer
doctests passed with zero tests, and the remaining-workspace invocation passed
everything except five anticipated current stale-binding tests (two xtask-bin
ownership, two xtask-coverage ownership, one V8 representative consumer). Parent
reports 101 xtask-bin passes and 19 overall ignored tests.

The five tests are still failing before authorization; they have not been waived
or marked passing. Raw logs and parent-only completion attestations are explicitly
distinguished in `final-checks/parent-split-verification.json`. See
`SPLIT-VERIFICATION.md`. The strict new wrapper authenticates that record before
calling the unchanged frozen checker. Seven negative wrapper tests pass.

All 168 OLD/current packaged key outcomes/artifacts/reports are identical.
V9/bridge-v3/protected-PDF checks pass with existing artifacts and tolerances.
64 JPEG vectors pass canonically; 12 retain all 24 exact sampling-failure pins.
Neutral missing-row control and unchanged PDFium alpha discrepancy stay explicit.

## Exact approval scope

Approve only the stable-graph CMYK/YCCK color correction, its captured applicability
evidence and exact proposed migration:

- 15 named color-cell owners (CMYK two + YCCK thirteen).
- 17 existing JPEG owner-source hash refreshes.
- One V8→V9 representative binding/registry replacement, using real V9 approval.
- Preserve all 38 policy criteria, 168 full keys, other assignments and three
  exclusions; six sampling gaps and 24 known-failure pins remain #187.

Projected counts are 1912 assignments / 1832 owners / three exclusions /
1835 assigned-excluded / 77 gaps. No baseline/#178, global closure or fixed-size
CMS resource approval is included. PDFium's unchanged alpha finding receives no
exception or closure claim.

## Read-only independent verification

```sh
python3 tests/conformance/proposals/jpeg187-requalification-v1/split-verification.py verify
```

This authenticates the split-result record, all bound packet and frozen source
bytes, the exact pending transaction, independent snapshot checks and V9's normal
strict approval subject. It writes no live metadata and runs no broad suite.

After independent review, the user must explicitly approve both exact subjects.
Parent owns the actual authenticated approval/atomic metadata transaction. Once
applied, rerun the affected xtask ownership/bin, coverage and representative
consumer targets, plus approved execution/inventory/ownership checks, so the five
pending stale-binding failures are resolved rather than silently waived.

This README is excluded explanatory metadata and does not alter either subject.
