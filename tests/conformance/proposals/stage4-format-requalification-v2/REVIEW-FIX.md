# Review-fix successor: authorization and optimized execution

This is a **pending packet-only successor** to formatting review subject
`sha256:4befa87e77a521aae20223b78fa6da20bb3a38d8555800f844e9de5c4d3f4e52`.
That original packet, subject, checker and evidence are preserved byte-for-byte
as the rejected review baseline. **Do not use its v1 apply entry point.**

Representative V8 remains unchanged:
`sha256:aaa772430f342a60807ef97255bf2296041cbcf23dff4c2aa4284052deb92454`.
No production, xtask, V8/bridge evidence, existing approval or live ledger/registry
bytes changed. No native recapture or broad suite is part of this fix.

## Blocker 1: optimized Python

The v2 checker rejects `not __debug__` or any nonzero `sys.flags.optimize` before
workspace-dependent imports, path calculation or dispatch. `python -O`, `-OO`
and `PYTHONOPTIMIZE=1/2` exit 2 with:

```text
Optimized Python execution is forbidden; no workflow started.
```

This protects all assert-based checks delegated to the authenticated immutable
predecessor. Its source is hash-checked before loading and compiled with
`optimize=0`, without generating pycache in the archived packet. New authorization
and transaction-state checks also use unconditional `require` checks.

## Blocker 2: formatting decision authentication

The common `authenticate_decision` path is used by both application and applied
verification. It requires:

- Exactly four formatting approval fields, naming the **new v2 formatting
  subject**, with a valid login/date and issue #85 comment URL.
- A persisted `comment.json` whose ID, URL, author and creation date agree with
  `approval.json`, and whose body names both complete formatting and V8 subjects.
- A fresh `gh api` read of that actual comment. The saved ID, URL, creation date,
  author login and full body must match the live GitHub response. Mutable reaction
  counters or other incidental API fields are not approval authority.
- V8's approval must be the same identity/date/comment decision, changing only
  `approval_subject_sha256` to the unchanged V8 subject.

`verify-applied` reads the mandatory application `approval.json` **and**
`comment.json`. V8 approval alone cannot authorize the 57 owner-source updates.
It refuses missing records, wrong subjects, identity/date/URL mismatches, changed
or deleted decision comments, inconsistent V8 approval and incomplete/failed
application states. Authentication happens before the exact migration audit.

The predecessor's immutable formatter/source/output proofs and
`validate_migration` remain unchanged. The successor delegates their verification
verbatim, preserving the exact 57 source replacements, one representative
replacement, frozen criteria/exclusions, counts, catalog and 168 full keys. It
does not bypass source validation to address this authorization defect.

## Persist-before-audit and rollback protocol

Applications use a new directory:
`tests/conformance/coverage/stage4-format-application-v2/`.
No application directory is created by pending verification or these review fixes.

Each real approved attempt has a unique ID and states:

```text
preparing → applying → validating → applied
                       failure → failed_rolled_back | rollback_failed
```

The checker stores exact before bytes, actual `approval.json` and fetched
`comment.json` **before any live mutation**. It then changes only the reviewed
V8 approval, registry and ledger transaction, runs the real approved consumers,
and performs the authenticated applied audit. `applied` is written only after
that audit succeeds. Public `verify-applied` requires `applied`; the internal
final audit additionally admits the in-progress `validating` state.

On an exception, all three live files are restored and checked byte-for-byte.
The attempt remains with its decision records, backups, partial receipts and
error, explicitly marked `failed_rolled_back`. A subsequent explicitly authorized
`apply` first revalidates pending evidence and the genuine decision. It permits
retry only when the failed attempt's subject, exact file set, backup bytes and
live preimage hashes match. The entire old attempt is renamed to
`stage4-format-application-v2.failed-<attempt-id>`; nothing is overwritten or
mistaken for successful authorization evidence. A new attempt uses the normal
active directory.

`rollback_failed`, interrupted/incomplete states, an applied attempt, missing
backups or preimage drift block automatic retry. Preserve those records for
manual inspection/recovery; do not delete the directory to evade validation.
Restore only authenticated before bytes after resolving the failure, then
revalidate pending evidence before an explicitly authorized retry. The checker
does not claim to recover automatically from a killed process or storage failure.

## Meaningful isolated tests

Run:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tests/conformance/proposals/stage4-format-requalification-v2/test_requalify.py
```

Tests use disposable roots under `/tmp/opencode`, a mocked GitHub transport and
mocked native process boundary. They reuse the actual immutable migration
templates and unchanged full `validate_migration`, including all 57 source
updates and the real representative binding shape. Tests never post comments or
write synthetic authorization to the live repository; each test checks live V8,
registry and ledger bytes are unchanged.

The 14 tests cover optimized startup (four interpreter/environment modes),
missing approval/comment, altered formatting subject/fields, missing either
subject in the decision, author/date/URL/ID disagreement, saved-versus-live
comment mismatch, inconsistent V8 approval, a successful authenticated-shaped
fixture, rejection of unrelated ledger mutations, records existing before live
writes/final audit, late-authentication-failure rollback and retry, rollback
failure, and refusal of incomplete application states.

The positive fixture is **not genuine user approval**: only its API shape and
authentication boundary are simulated. Genuine authorization in the production
CLI always comes from the mandatory `gh api` read after explicit user approval.

## Pending review and eventual authorized commands

```sh
P=tests/conformance/proposals/stage4-format-requalification-v2
python3 "$P/requalify.py" verify
```

After independent review and actual explicit approval of **both** the new v2
formatting subject and unchanged V8 subject, record the real decision in an
issue #85 comment naming both full `sha256:` values. Supply a four-field approval
JSON outside the packet with actual `approved_by`, `approved_at`, `approval_url`
and the **v2 formatting** `approval_subject_sha256`. Do not reuse v1's subject.

```sh
python3 "$P/requalify.py" apply /absolute/path/to/actual-formatting-approval.json
python3 "$P/requalify.py" verify-applied
```

`verify-applied` requires GitHub access and matching persisted decision records;
it fails closed on an unavailable, changed or inconsistent comment. The old
formatting packet and V8 README remain immutable review history; this successor
is the corrected approval/application entry point. All subjects are still pending,
the live ledger remains stale, and no migration is applied during preparation.
