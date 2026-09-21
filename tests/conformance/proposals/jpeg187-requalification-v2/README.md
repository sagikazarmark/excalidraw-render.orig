# Pending v2 production decision — affirmative maintainer approval required

**New production/qualification/migration approval subject:**

```text
sha256:0baf0e3f8b7bbf37f575f0e8d32650befa7b73dd7a0a6c0c45e383c1a1693f32
```

**V9 subject, unchanged:**

```text
sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d
```

Both subjects are pending independent review and explicit user approval. This
successor supersedes v1's weak hash-mention authorization. The original v1 packet
(`e3a794f1…`), frozen checker, all source/capture evidence and V9 files are preserved.
No approval, recapture, production/xtask/renderer-test edit, live migration or commit
occurred in this review-fix unit.

## Read-only verification

```sh
P=tests/conformance/proposals/jpeg187-requalification-v2
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_verify.py"
python3 "$P/verify.py" verify
```

The full pending verifier authenticates the v2 envelope, immutable v1 packet,
original full split-qualification record, old/current 168-key captures, frozen
sources, exact 15-owner/17-refresh migration, catalog, known failures and unchanged
V9 subject. The five current pending stale-binding test failures remain explicit.
No passing result is substituted for them.

## Required later decision

Only after actual explicit user approval of **both exact subjects**, the designated
maintainer `sagikazarmark` records one issue #85 comment whose entire body is this
single JSON fence (no surrounding prose or quotes):

````text
```json
{
  "decision": "approve",
  "scope": "jpeg187-color-only",
  "production_subject_sha256": "sha256:0baf0e3f8b7bbf37f575f0e8d32650befa7b73dd7a0a6c0c45e383c1a1693f32",
  "representative_subject_sha256": "sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d"
}
```
````

This is the future protocol template, **not a recorded or granted approval**.
The verifier requires the actual fetched comment author's login, not only the
supplied approval login. It rejects rejection/hash-mention comments, another
maintainer/login, missing/duplicate/malformed/unknown fields, duplicate blocks,
contradictory prose, wrong scope/subjects, changed remote comments, inconsistent
identity/date/URL and a mismatched V9 decision.

The parent owns the later metadata transaction. Persist the genuine four-field
production approval and full comment snapshot, successful status and exact before
bytes under `tests/conformance/coverage/jpeg187-application-v2/`. Use the already
reviewed v1 migration templates, deriving only the final approved V9 manifest-file
hash. Apply only after explicit approval, retain rollback/failure evidence and run
the affected approved consumers/tests. Then:

```sh
python3 "$P/verify.py" verify-applied
```

Applied verification authenticates the affirmative designated-maintainer decision
before auditing the full original evidence and exact transaction. It requires live
GitHub access and matching saved records; V9 approval alone is insufficient.
See [REVIEW-FIX.md](REVIEW-FIX.md) for the exact parent-owned procedure.

## Review proof

**14 focused tests passed.** One directly demonstrates that the actual old
authenticate function accepts rejection text containing both hashes, while this
successor rejects it. Old positive hash-mention fixtures now fail. Positive tests
use isolated API-shaped affirmative fixtures with mocked transport only.

The audit preserved **1,461 v1/V9 files** and **3,433 frozen source/evidence files**.
`audit-proof.json` binds old/new checker identities and raw test output. Pending
V9 manifest bytes are separately bound; after genuine approval its approval field
is checked by strong authorization and the exact original migration validator.

Exact new files: `verify.py`, `test_verify.py`, `REVIEW-FIX.md`, `audit-proof.json`,
`checks/tests.stdout`, `checks/tests.stderr`, `manifest.json`, `approval-subject.json`
and this excluded explanatory README. Existing files are unchanged.

Scope remains stable-graph color only: 15 new color owners, 17 source refreshes,
V8→V9; 1912 assignments / 1835 assigned-excluded / 77 gaps. Six sampling cells and
24 exact failure pins remain #187. No alpha exception, baseline/#178, global
closure or fixed-size CMS resource approval is granted.
