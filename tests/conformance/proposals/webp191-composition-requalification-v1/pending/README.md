# Final pending #191 composition-only subjects

**Production/qualification/migration subject:**

```text
sha256:8c706d759ad01650eb34eac247ca0ba2d61cf3ac101ede290ffd9c398edbc801
```

**V13 subject, unchanged:**

```text
sha256:520b71395cd48b8768caba6eedadf6c6c40f996a9e0c36019f5ef9dafe62c5f5
```

Both are pending independent review and explicit exact-subject user approval.
The earlier capture-draft remains immutable history. Permission to continue
preparation was not interpreted as approval of these newly sealed subjects.

## Actual completed verification

The supplied full-suite log is archived verbatim at `checks/full-workspace.raw.log`.
Strict parsing gives **1257 passed / five failed / 19 ignored**, including all
626 fast and 404 library passes. The five exact old-owner/representative binding
failures remain explicit; the full suite is **not called green**. No unknown
failure/count/message/trailer substitution is accepted.

The actual command's output, parent message, exact classification, assertion-only
CLI reproduction and frozen source/producer descriptors are in
`checks/full-suite-record.json`. `FULL-SUITE-REVIEW.md` explains the scope.

The original **28 isolated mutation/receipt tests** were rerun and passed, and
**nine suite-classification/auth-scope tests** passed. No full workspace rerun,
production edit or recapture was performed. All frozen files and old/new native
measurement hashes remain unchanged.

## Exact approval scope

- First-ANMF standalone decode and clear RGBA canvas placement only.
- Eleven named corrected composition-cell owners; 25 source-only owner refreshes;
  one V12→V13 representative binding.
- Preserve all criteria, catalog, 168 full inventory keys, other dispositions
  and three exclusions.
- Preserve exact 30-vector/60-raster observation changes, including eight improved
  but still failing partial-alpha vectors. Twelve partial cells/24 vectors stay
  unqualified. Full sample passes remain SVG72/PNG48/PDF48, not all72 raster pass.
- No static PNG floor-premultiplication fix, PDF straight-sample/color substitution,
  synthetic comparison-policy change, tolerance exception, or PDF-viewer defect
  claim. Allocation/bounds/early-drop evidence is not physical OOM closure.

Projected totals: **2418 assignments / 2321 assigned-excluded / 97 gaps**.

## Read-only review

```sh
P=tests/conformance/proposals/webp191-composition-requalification-v1
python3 "$P/suite_gate.py" verify
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_suite_gate.py"
```

`manifest.json.files` is the complete bound file list: original/final source and
graph proof, old/current 168 packaged captures/blobs, exact old/new native
measurements and transition, source/resource analysis, focused logs, V13/bridge-v7/
protected evidence, exact metadata templates, raw full suite/parser/tests and
all earlier checkpoint records. `composition-defect-measurements.json` is bound
explicitly outside the packet as well. V13 pending approval metadata is separately
bound and excluded only from its canonical approval subject.

## Future affirmative decision — do not post without actual user approval

After independent review and explicit user approval of **both exact subjects**,
actual designated maintainer `sagikazarmark` records an issue #85 comment whose
complete body is exactly this standalone JSON fence:

````text
```json
{
  "decision": "approve",
  "scope": "webp191-composition",
  "production_subject_sha256": "sha256:8c706d759ad01650eb34eac247ca0ba2d61cf3ac101ede290ffd9c398edbc801",
  "representative_subject_sha256": "sha256:520b71395cd48b8768caba6eedadf6c6c40f996a9e0c36019f5ef9dafe62c5f5"
}
```
````

These are the exact four decision keys. The application approval record separately
has exactly `approved_by`, `approved_at`, `approval_url`, and
`approval_subject_sha256`, using real identity/date/comment URL and the production
subject above. A rejection, hash mention, different author/scope/subject or extra/
duplicate fields does not authorize application.

With actual approval only:

```sh
python3 "$P/suite_gate.py" apply /absolute/path/to/actual-approval.json
python3 "$P/suite_gate.py" verify-applied
```

The guarded three-file transaction stores genuine decision/preimages and restores
all three files on any failure, retaining failure evidence. Applied status requires
actual successful 109-test/six-command source/subject/log-bound receipts; applied
verification independently repeats those same bounded checks. Thus the five pending
failures must resolve, not be waived. Baseline/#178, #187 sampling, #190 HDR,
partial-alpha adjudication and global/CMS resource work remain separate.

No actual approval, live application or commit occurred during sealing. This
README is excluded explanatory metadata and does not change the final subjects.
