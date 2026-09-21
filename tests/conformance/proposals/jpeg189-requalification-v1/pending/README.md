# Final pending #189 production subjects

**Production/qualification/migration subject:**

```text
sha256:d3dadca97b24fd4b2489b051fed00b9bb8540dc5f1305ac5c4ab7df50451ec61
```

**V11 subject, unchanged:**

```text
sha256:d8e086187884e412c78b98197957e2793b3f3a7f0c5fd11df72ceac4dbb3f87c
```

Both remain pending independent review and explicit user approval. The original
capture-draft remains immutable checkpoint evidence, not the final approval request.
No actual approval, live migration or commit was performed while sealing.

## Actual completed verification

Raw workspace output establishes **1232 passed / five failed / 19 ignored**, with
all 601 fast and 404 library tests passing. The command completed normally at
exit 1, not timeout. Exactly five old-ownership/representative-binding failures
are retained by exact identities, locations and messages; no unknown failures
are admitted and none are waived. See `FULL-SUITE-REVIEW.md`.

The new unbound `suite_gate.py` authenticates the existing historical strict
parser and adjusts only current numeric totals. Backtrace-note position is
nonsemantic; all failure bodies remain exact. Nine classifier/auth-scope tests
pass, including unknown-message/identity, forged counts, truncation and extra-error
negatives. The frozen source/tooling/capture bytes remain unchanged.

Qualification remains complete: 168/168 old/current packaged records identical;
164 EXIF canonical vectors; ICC160 gates; #18764 canonical plus 12 known failures/
24 exact pins; V11/bridge-v5/protected artifact/report/viewer equality. No new
reference bytes, exception ceiling, dependency or shared orientation changes.

## Bound evidence

`manifest.json.files` is the complete authoritative descriptor list across this
packet, V11 and bridge v5. It includes authenticated helper/preimage records,
source freeze, both 168-key packaged captures and blobs, focused/protected/viewer
results, exact migration templates, raw full-suite output and parent message,
strict failure-classification successor and test proof, receipt adapter/checker,
and original draft records. V11's approval-bearing manifest is separately bound
while pending; its canonical subject excludes only its approval metadata.

Read-only verification:

```sh
P=tests/conformance/proposals/jpeg189-requalification-v1
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_suite_gate.py"
python3 "$P/suite_gate.py" verify
```

## Future actual approval/application

After review and explicit user approval of both subjects, designated maintainer
`sagikazarmark` records an issue #85 comment whose complete body is one JSON fence:

````text
```json
{
  "decision": "approve",
  "scope": "jpeg189-exif",
  "production_subject_sha256": "sha256:d3dadca97b24fd4b2489b051fed00b9bb8540dc5f1305ac5c4ab7df50451ec61",
  "representative_subject_sha256": "sha256:d8e086187884e412c78b98197957e2793b3f3a7f0c5fd11df72ceac4dbb3f87c"
}
```
````

This is only a future protocol template. No comment or approval is generated now.
The checker requires real author/date/URL/ID, exact affirmative scope and subjects,
saved/fresh GitHub agreement and corresponding V11 approval.

Supply the actual four-field production approval JSON outside the packet:

```sh
python3 "$P/suite_gate.py" apply /absolute/path/to/actual-approval.json
python3 "$P/suite_gate.py" verify-applied
```

The transaction preserves preimages/decision/failure evidence, applies only V11
approval and exact registry/ledger templates, and derives only V11's final
manifest-file hash. Applied status requires actual successful 103+5+1 test and
five approved consumer/owner/inventory/ownership receipts, full expected names,
exact commands, zero exits, current source/subject/metadata bindings and rollback
on failure. `verify-applied` independently repeats those bounded checks. Thus
the five historical failures must actually resolve after approval, not be waived.

Scope is 36 new EXIF owners, 46 source refreshes and V10→V11: projected 2074
assignments / 1997 assigned-excluded / 77 gaps. All criteria, 168 keys, other
dispositions and three exclusions remain. #187 sampling, baseline/#178, global
closure and fixed-size CMS resource work are separate. This README is excluded
explanatory metadata and does not alter the subjects.
