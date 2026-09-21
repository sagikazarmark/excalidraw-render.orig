# Affirmative designated-maintainer decision — v2 pending review successor

The production v1 subject
`sha256:e3a794f1461248ccf2ad92b73b7d7cd2aa780386d7790ff1310e4678766105a6`
and all of its frozen source/capture/checker bytes are preserved. Its permissive
hash-mention authorization is not the current application entry point. This
packet binds that whole reviewed production packet and the stricter verifier;
the substantive color correction, 15 owners, 17 source refreshes and one
representative replacement are unchanged.

V9 remains:
`sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d`.
No production, xtask, renderer test, capture, approval or live ledger/registry
bytes change in this fix. No approval comment is posted during preparation.

## Required genuine comment protocol

Only the designated maintainer **`sagikazarmark`** may record the affirmative
decision, and only after the user's explicit approval. The GitHub API comment's
actual `user.login` must equal that exact login; a supplied approval login is not
authority. The saved comment and fresh API response must agree in ID, URL, author,
creation timestamp and complete body. The date and URL must also match the four
approval fields, and V9 must carry the same actual decision for its own subject.

For unambiguous interpretation, the **entire issue #85 comment body** must be
exactly one standalone fenced JSON block, apart from surrounding whitespace:

````text
```json
{
  "decision": "approve",
  "scope": "jpeg187-color-only",
  "production_subject_sha256": "sha256:NEW_V2_PRODUCTION_SUBJECT",
  "representative_subject_sha256": "sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d"
}
```
````

Use the actual new v2 production digest after reviewing the sealed packet. This
document is a protocol description, not an approval or a comment to post now.
Context/cross-references may be posted separately; do not add prose to this exact
decision comment. This avoids interpreting a rejected, quoted or hypothetical
block as authorization.

The parser rejects all of:

- Another GitHub author, even when supplied and saved identities agree.
- A maintainer comment merely mentioning hashes, including explicit rejection.
- `reject`, pending/missing/unknown decision values, wrong scope or old subject.
- Missing, unknown or duplicate JSON keys, duplicate blocks, malformed JSON.
- Prose/quotes or contradictory rejection surrounding an affirmative block.
- Saved affirmative text whose genuine API comment is rejected/changed.
- Different representative approval identity/date/comment/subject.

The old in-memory hash-mention positive fixture is now a negative test. A regression
test demonstrates that the authentic old function accepts a rejection containing
both hashes while the new verifier rejects it. Positive fixtures use a mocked
API-shaped exact affirmative decision in memory or a disposable directory, never
a real approval or a write to live metadata.

## Pending and applied verification

`verify.py verify` authenticates the new envelope and the complete unchanged v1
packet, then invokes the original full split-verification and pending evidence/
migration checks. It performs no write or approval operation.

`verify.py verify-applied` first requires persisted application approval, comment
and successful status records under `coverage/jpeg187-application-v2/`. It performs
the strong affirmative-maintainer authentication **before** any migration audit.
It then rechecks the original packet/file inventory, frozen production/tooling
set (excluding only the two already-designed mutable metadata paths), exact
split-completion evidence, JPEG/nested known failures, both 168-key snapshot sets,
unchanged catalog, exact original migration validator and V9 subject.

The applied path calls the existing evidence functions directly, because the v1
top-level applied verifier expects the old subject/application directory. It does
not call or replace the old weaker authorization function. All actual source,
criteria, exception and transaction validation functions remain byte-identical.
The five pre-approval stale-binding failures stay explicitly recorded; only real
post-approval execution can resolve them.

## Parent-owned application after actual approval

There is intentionally no automatic `apply` command in this minimal successor.
After independent review and explicit user approval of **both** new production
and unchanged V9 subjects:

1. The designated maintainer records the exact affirmative JSON comment on #85.
   Avoid duplicates. Fetch its actual API response with `gh`.
2. Persist actual four-field production approval and full comment snapshot outside
   the immutable packet, under `tests/conformance/coverage/jpeg187-application-v2/`,
   together with exact before bytes and a non-applied attempt status, before any
   metadata mutation. `production_subject_sha256` names the new v2 subject.
3. Apply only the v1-reviewed V9 approval/registry/ledger templates; derive V9's
   approved manifest-file hash after adding its four-field approval. This is the
   only deferred template hash. No source/capture or golden changes.
4. Run approved V9/registry/owner/inventory/ownership checks and the affected xtask
   tests. Retain their actual receipts. Mark status `applied` only on success with
   both exact subject fields. Use the established strict-v2 before-byte rollback
   and failed-attempt archival/retry discipline; do not delete failure records.
5. Run this successor's `verify-applied`. Missing decision records or a V9-only
   approval cannot authorize the color-owner migration.

Approval JSON has exactly `approved_by` (`sagikazarmark`), `approved_at` (actual
YYYY-MM-DD), `approval_url` (actual #85 comment URL), and
`approval_subject_sha256` (new v2 production digest). V9 uses the same identity/
date/comment, with its own exact subject. No synthetic values may be applied.

Optimized Python is rejected before workflow dispatch. Both pending and applied
checks are read-only; applied authentication requires live GitHub access and
fails closed. Baseline/#178, six #187 sampling gaps, global closure and fixed-size
CMS resource work remain outside approval scope.
