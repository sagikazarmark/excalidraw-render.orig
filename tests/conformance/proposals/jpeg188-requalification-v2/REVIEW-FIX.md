# Immutable v2: applied state requires actual resolution receipts

The original v1 subject
`sha256:6dbac083ea0d744880bebe0aab6a259f9530bad0635540faeba156edad23c1fa`
and every frozen producer, checker, test, capture and approval remain unchanged.
V10 remains
`sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15`.
This successor binds the original complete packet and adds a post-approval
resolution protocol. No actual approval, migration, recapture or commit is made
while preparing it. Use this v2 verifier/application entry point after review.

## Why the old applied status was insufficient

V1 authenticates an actual affirmative decision and the exact metadata migration,
but those do not prove that the five known stale-binding tests were rerun and
resolved. V2 requires both stored successful execution evidence and an independent
fresh repeat of exactly the bounded checks, after authenticating the actual decision.
A status marker, declared count or hand-written green summary cannot satisfy it.

## Exact mandatory command set

1. `cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast`
2. `cargo run --release --locked -p xtask -- conformance candidate-verify tests/conformance/candidates/stage4-frames-text-assets-v10`
3. `cargo run --release --locked -p xtask -- conformance candidate-executions`
4. `cargo run --release --locked -p xtask -- conformance verify-representative-owner representative.stage4-frames-text-assets.v10`
5. `cargo run --release --locked -p xtask -- conformance verify-inventory`
6. `cargo run --release --locked -p xtask -- conformance verify-ownership`

All use pinned Rust 1.97.1 Linux x86_64 with incremental/dev/test/release debug
disabled and explicit recorded environment. No broad renderer/workspace suite is
added. `verify-applied` reruns this same bounded command set independently rather
than accepting self-authored exit codes as cryptographic proof of execution.

## Receipt protocol

Application evidence lives outside the sealed packets in
`tests/conformance/coverage/jpeg188-application-v2/`. `receipts.json` requires:

- Exact production and representative subjects.
- Descriptor of the original full source freeze, verified against every actual
  current source; post-application V10 manifest, registry, ledger and inventory/
  policy hashes; exact xtask source/test inputs, Cargo files, v2 verifier and bound
  test-identity inventory.
- Exactly six ordered command records with exact flags/cwd/environment, integer
  exit code zero, stdout/stderr descriptors and unchanged before/after bindings.
- Parsed outcome recomputed from the actual log bytes, never a trusted pass flag.

The three test binaries must emit **all 103 + 5 + 1 unique expected successful
test identities**, matching `target-identities.json`, three exact successful
zero-failure/zero-ignore summaries and the three actual binary runner lines on
stderr. Missing/duplicated/substituted names, count-only green summaries, wrong
flags/targets, nonzero exits and failure/panic stderr are rejected.

The identity inventory is independently reconstructed from the immutable raw
completed workspace log, accounting for Cargo stderr interleaving after the first
libtest stdout lines. It includes the five originally failing identities; counts
alone cannot substitute for their observed successful rerun.

Direct and registry receipts must exactly match the current candidate's three
SVG/PNG/PDF keys, native artifact/report hashes, two fresh processes, canonical
judgment/semantic digests and approved V10 subject. Owner/inventory/ownership
commands must return their precise current output, including 1992 assignments /
1915 assigned-excluded / 77 gaps. Pending/null-approval or stale totals fail.

After validating saved receipts, `verify-applied` independently reruns the exact
six commands, parses actual output and compares semantic results to the saved
ones. It writes no receipt or source metadata during that audit (Cargo may use
its normal build/test scratch). A fabricated but syntactically plausible saved
result cannot conceal an actual failing current rerun.

## Authentication and unchanged original evidence

Optimized Python is unconditionally rejected before workflow dispatch. The old
checker and suite parser are loaded only after their original approved-envelope
descriptors are authenticated, with optimization disabled. The strict original
`sagikazarmark` standalone fenced JSON policy is retained, using the **new v2
production subject**, unchanged V10 subject and scope `jpeg188-icc`. The actual
GitHub comment must explicitly approve both subjects; wrong author/scope/subject,
rejection/hash-mention text, duplicate/unknown fields or saved/live disagreement
cannot grant authority.

Applied authentication precedes all evidence/receipt checks. The wrapper then
uses the exact original source/qualification/migration functions and original
full-suite classification without invoking v1's obsolete applied-status gate.
Only registry and ledger have the original designed freeze exclusions; V10's
approval is checked against the genuine decision and exact migration template.
No protected artifact, reference, criterion, exclusion or tolerance is relaxed.

## Guarded future application

After independent review and explicit user approval of both exact subjects,
the designated maintainer records one actual #85 comment, entirely one JSON fence:

````text
```json
{
  "decision": "approve",
  "scope": "jpeg188-icc",
  "production_subject_sha256": "sha256:ACTUAL_NEW_V2_SUBJECT",
  "representative_subject_sha256": "sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15"
}
```
````

Supply the actual four-field production approval JSON outside the packet:
`approved_by`, `approved_at`, `approval_url`, `approval_subject_sha256`. No sample
identity/comment in tests is permission to apply this transaction.

```sh
P=tests/conformance/proposals/jpeg188-requalification-v2
python3 "$P/verify.py" apply /absolute/path/to/actual-production-approval.json
python3 "$P/verify.py" verify-applied
```

The guarded operation verifies pending evidence and authenticates the genuine
decision before mutation, stores exact preimages/approval/comment/attempt state,
applies only V10 approval plus exact registry/ledger templates, and derives only
the approval-bearing V10 manifest hash. It runs and persists all six mandatory
checks, revalidates decision/evidence/receipts, and marks **applied only afterward**.
No fictitious receipt or synthetic approval is written by preparation/tests.

Failure restores and byte-checks all three metadata preimages. Partial logs,
decision records and error remain under `failed_rolled_back` or `rollback_failed`.
Automatic retry/overwrite of any existing application directory is refused;
inspect and preserve the entire attempt under a unique failed archive before a
separately authorized retry. Incomplete/failed rollback requires manual recovery.
No failed status can pass `verify-applied`.

## Tests and bounded scope

Seventeen isolated tests cover successful complete receipts, missing logs/commands,
failed exit with green text, wrong flags, forged summary/full-name substitution,
source/subject/metadata drift, stderr/binary mismatch, incorrect consumer payloads,
pending totals, independent-rerun failure despite parseable forged logs, success
with all commands reexecuted, rollback/preimage preservation, applied-state ordering,
optimization refusal and strict affirmative scope. Native/GitHub boundaries are
mocked in fixtures; live metadata is not approved or changed. The source freeze,
v1 packet and V10 bytes are checked before and after tests.

The original full workspace remains accurately recorded as 1226 passed / five
pending binding failures / 19 ignored. Post-approval 109-test/consumer receipts,
not a changed historical claim, must resolve those five failures. Scope remains
16 ICC additions, 64 source refreshes and V9→V10; six #187 gaps, baseline/#178,
global closure and fixed-size CMS allocation limitations remain outside approval.
