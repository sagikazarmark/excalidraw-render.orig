# Final pending v2 — actual post-approval resolution receipts required

**New production/qualification/migration subject:**

```text
sha256:9ac73ac1c6ae2e3622c08d4254ba7dcb49d350047a12651e31590b1bacb2aa01
```

**Unchanged V10 subject:**

```text
sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15
```

Both remain pending independent review and explicit user approval. The original
v1 packet/subject is preserved unchanged as historical review evidence. Use v2
for future application and applied verification. No production, xtask, renderer
test, capture, approval, live ledger/registry or commit changed in this fix.

## Blocking finding resolved

Applied status and subject authentication alone no longer suffice. V2 requires:

- Real affirmative `sagikazarmark` decision for the **new v2 subject** and V10,
  scope `jpeg188-icc`, using the unchanged strict one-fence JSON policy.
- Saved successful receipts for exactly the 103+5+1 xtask targets and approved
  candidate, registry, representative-owner, inventory and ownership commands.
- Exact commands/flags, zero exits, stdout/stderr descriptors, production/V10
  subjects, complete source-freeze identity and before/after actual approved
  manifest/registry/ledger/test-source bindings.
- Every expected unique successful test identity, exact summaries and binary
  runner evidence. Blank/count-only green summaries cannot pass.
- Exact approved three-format artifact/report/judgment/semantic execution receipts.
- An independent fresh rerun of that same bounded command set on every
  `verify-applied`, compared with saved parsed results. Plausible fabricated
  saved logs cannot conceal a failing actual current check.

The guarded `apply` persists decision and exact preimages before writing the
three reviewed files. It marks `applied` only after actual checks and receipt
validation succeed; failures restore all three files and preserve explicit failed
attempt evidence. Existing attempts are never automatically overwritten/retried.
See [REVIEW-FIX.md](REVIEW-FIX.md) for the complete protocol.

## Checks

**17 isolated tests passed**, covering missing/failed/forged receipts, wrong
commands, source/subject/post-approval metadata drift, substituted test identities,
stderr errors, wrong consumers/totals, current rerun failure despite parseable
saved logs, successful independent reruns, rollback and applied-status ordering,
optimized startup and strict affirmative scope. Process/authorization boundaries
are mocked in disposable fixtures; no synthetic approval reaches live metadata.

Full pending verification passed against the original v1 evidence and split-free
completed full-suite record. That record remains honestly **1226 passed, five
pending binding failures, 19 ignored**. No post-approval green result is claimed
until the future receipt-producing transaction actually runs.

Preserved: **553 original v1/V10 files and 4315 frozen files**. `audit-proof.json`
binds old/new checker sources and test outputs. `target-identities.json` binds all
109 names back to the actual archived full-suite output.

## Read-only review commands

```sh
P=tests/conformance/proposals/jpeg188-requalification-v2
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_verify.py"
python3 "$P/verify.py" verify
```

## After actual explicit approval only

The designated maintainer records one issue #85 comment whose complete body is
exactly this JSON fence, after the user approves both subjects:

````text
```json
{
  "decision": "approve",
  "scope": "jpeg188-icc",
  "production_subject_sha256": "sha256:9ac73ac1c6ae2e3622c08d4254ba7dcb49d350047a12651e31590b1bacb2aa01",
  "representative_subject_sha256": "sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15"
}
```
````

This is a future protocol template, not an approval record. Supply the actual
four-field production approval JSON outside the immutable packet, then:

```sh
python3 "$P/verify.py" apply /absolute/path/to/actual-approval.json
python3 "$P/verify.py" verify-applied
```

The latter is read-only to source/metadata/receipts but deliberately executes the
same bounded tests/consumers again (ordinary Cargo/test scratch is permitted).
Application evidence resides under `coverage/jpeg188-application-v2/`. No full
workspace/renderer rerun is included.

Substantive scope is unchanged: 16 ICC owner additions, 64 source refreshes and
V9→V10, projected 1992 assignments / 1915 assigned-excluded / 77 gaps. All original
criteria, 168 keys, three exclusions, six #187 sampling gaps and their exact pins
remain. Baseline/#178, global closure and fixed-size CMS resource work are separate.

New files only: `verify.py`, `test_verify.py`, `target-identities.json`,
`REVIEW-FIX.md`, `audit-proof.json`, `checks/tests.stdout`, `checks/tests.stderr`,
`manifest.json`, `approval-subject.json`, and this excluded explanatory README.
