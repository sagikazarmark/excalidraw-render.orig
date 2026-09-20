# Corrected pending formatting packet — review successor v2

**Revised formatting/migration subject:**

```text
sha256:844e401e1601f0f36d8511f709ebe8834224d83340e2b10fd444dd545c39f6ef
```

**Unchanged representative V8 subject:**

```text
sha256:aaa772430f342a60807ef97255bf2296041cbcf23dff4c2aa4284052deb92454
```

Both are pending. No user approval, GitHub decision comment, live migration,
production/xtask edit, native recapture or commit was performed in this review-fix
unit. **Use this v2 checker, not the rejected v1 apply entry point.** V8's original
README and the v1 packet remain immutable historical review evidence; this file
is the authoritative successor pointer.

## Fixed blockers

1. Optimized Python is rejected before workflow imports or dispatch. `-O`, `-OO`
   and `PYTHONOPTIMIZE=1/2` all exit 2 before mutation. The authenticated predecessor
   is additionally compiled with optimization disabled.
2. Applied verification requires persisted formatting `approval.json` and
   `comment.json`, the exact new formatting subject, both subjects in the decision,
   matching author/date/URL/ID, a fresh matching GitHub API response, and the exact
   corresponding V8 approval. V8 approval alone cannot authorize source migration.
3. Actual decision records and exact preimages are persisted before live writes
   and the final audit. Failed attempts are explicitly marked, fully rolled back
   when possible, preserved under a unique archive on safe retry, and never
   reported as applied. Incomplete/failed rollback states require manual recovery.

The v1 formatter, native equality and exact 57-binding/one-representative
migration validation are reused unchanged. See [REVIEW-FIX.md](REVIEW-FIX.md) for
the exact authorization and rollback/retry rules.

## Audit proof

- **14 focused tests passed**, including four optimized-startup modes, missing or
  changed formatting authorization, mismatched subjects/comment/identity/V8,
  authenticated-shaped positive fixtures, unrelated transaction rejection,
  persist-before-audit ordering, rollback and safe retry, rollback failure and
  incomplete-state refusal.
- GitHub/native transports in tests are mocked; fixtures use temporary roots and
  the actual immutable migration templates/validator. No synthetic authorization
  is recorded in the live repository.
- **183 predecessor files**, **54 V8 files** and **2,286 frozen live files** are
  unchanged, including all production and xtask bytes.
- Original rejected subject/checker preserved:
  `sha256:4befa87e77a521aae20223b78fa6da20bb3a38d8555800f844e9de5c4d3f4e52`.

`review-fix.json`, `review-fix.diff` and `audit-proof.json` bind the explicit delta,
checker identities, test command/results and preservation checks. Raw test output
is under `checks/`. The exact current file inventories are in `manifest.json`.
This README is excluded explanatory metadata.

## Reproduce review checks

```sh
P=tests/conformance/proposals/stage4-format-requalification-v2
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_requalify.py"
python3 "$P/requalify.py" verify
```

After independent review and actual explicit approval of **both** subjects, record
one genuine issue #85 decision naming both full `sha256:` values. Supply its real
four-field approval JSON outside this packet, with the **new v2 formatting**
subject—not the rejected v1 hash:

```sh
python3 "$P/requalify.py" apply /absolute/path/to/actual-formatting-approval.json
python3 "$P/requalify.py" verify-applied
```

Applied auditing requires GitHub access and the matching saved records. The
future real application directory is
`tests/conformance/coverage/stage4-format-application-v2/`; failed attempts are
retained with `.failed-<attempt-id>` suffixes after validated rollback and retry.

The live ledger remains unchanged/stale pending actual approval. Counts and
scope remain 1,874 assignments, 1,803 assigned/excluded, 71 gaps, unchanged
criteria/exclusions and all 168 full inventory keys. Baseline/#178 and the
fixed-size CMS allocation/resource gap remain separate.
