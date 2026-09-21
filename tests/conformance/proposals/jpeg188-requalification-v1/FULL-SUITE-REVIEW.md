# Completed full workspace: exact pending-binding failures retained

The parent's final command completed normally within its 1,800,000 ms timeout:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_PROFILE_RELEASE_DEBUG=0 PATH=<pinned-1.97.1-bin>:$PATH \
cargo test --locked --workspace --no-fail-fast
```

The byte-for-byte raw log is `checks/full-workspace.raw.log`, copied from
`/home/laborant/.local/share/opencode/tool-output/tool_0c1f93690001WBgc0K8u82mEbm`.
It contains 39 complete target summaries and independently counted pass listings:
**1226 passed, 5 failed, 19 ignored**. Thus 1231 tests executed pass/fail, 1250
including ignores. All 595 renderer fast and 404 library tests pass; the library
has seven existing ignores. This is a completed failed invocation, not a timeout,
split completion or a falsely green suite.

Exactly five failures are admitted as *pending binding failures*, never as passes:

| Test | Exact observed message |
| --- | --- |
| `coverage::ownership::tests::inventory_extension_preserves_live_owners_and_approved_exclusions` | unwrap error `stale ownership evidence` |
| `coverage::ownership::tests::test_owner_checks_compiled_module_exact_selector_and_source_digest` | unwrap error `stale ownership evidence` |
| `ownership_recording_rejects_unknown_obligations_without_changing_the_ledger` | assertion that stderr contains `unknown ownership identity` failed |
| `ownership_recording_validates_the_entire_batch_before_writing` | `error: stale ownership evidence` |
| `representative_registry_is_consumed_through_approval_aware_native_execution` | `error: stale adapter production/source bindings` |

These occur only in xtask bin (101 passed/2 failed), coverage (3 passed/2 failed),
and representative_candidate (0 passed/1 failed). The parser requires exact test
identities, panic source locations and message bodies, exact failure-summary lists,
exact target error trailer and totals. Unknown errors, additional panic details,
different failure names/counts, missing summaries, relabeled outcomes or removed
pass listings are rejected. It grants no general permission for five failures.

For the assertion-only unknown-owner test, the log does not expose subprocess
stderr. Its exact normal CLI call was separately reproduced under current frozen
bindings. It fails with `stale ownership evidence`, emits no stdout, and preserves
ledger bytes. `checks/unknown-owner-reproduction.*` binds that causal evidence.

## Frozen sources and strict new audit entry point

`checks/full-suite-record.json` binds the raw log, actual parent message, final
freeze, exact final test/producer bytes, classification and CLI reproduction.
The original `workflow.py`, production/tests, xtask, captured data and previously
written draft/qualification records remain unchanged. The new unbound
`suite-verification.py` authenticates the completed-suite evidence before invoking
the hash-authenticated frozen packet seal/verify. Actual focused checks retain
their true passing status; the full-suite failures are stored separately, never
inserted as `passed:true` checks.

```sh
python3 tests/conformance/proposals/jpeg188-requalification-v1/suite-verification.py verify
```

Nine focused checker tests cover actual classification, unknown/different errors,
same-count substituted tests, extra errors, truncated completion, altered counts,
false pass flags, missing/duplicate pass listings, optimized-startup refusal and
the exact affirmative `jpeg188-icc` authorization scope. No renderer or full suite
was rerun to produce this audit.

## Future user decision and parent-owned application

Both final production and V10 subjects remain pending independent review/user
approval. The exact transaction is 16 ICC additions, 64 test-source refreshes and
V9→V10 representative binding; all criteria, 168 full keys, three exclusions and
the six #187 sampling gaps are preserved. Projected counts: 1992 assignments,
1915 assigned/excluded, 77 gaps. No baseline/#178 or global/CMS closure is granted.

Only after explicit user approval, designated maintainer `sagikazarmark` records
one standalone fenced JSON comment on #85 with exactly `decision: approve`,
`scope: jpeg188-icc`, the final `production_subject_sha256` and V10
`representative_subject_sha256`. No hash-mention/rejection/other-author comment
can authorize the migration. Persist real approval/comment/preimages and attempt
status before changing live metadata; apply the reviewed templates, deriving only
the approved V10 manifest-file digest. Use exact rollback and preserved failed
attempts on any failure.

After actual approval/application, the required rerun is:

```sh
cargo test --locked -p xtask --bin xtask --test coverage \
  --test representative_candidate --no-fail-fast
python3 tests/conformance/proposals/jpeg188-requalification-v1/suite-verification.py verify-applied
```

Also run actual approved candidate/registry/owner/inventory checks. Applied
verification authenticates the real decision before auditing exact migration.
It continues to describe this pre-approval full-suite log as failed; new post-
approval receipts must establish resolution of the five failures. No redundant
broad-suite rerun or automatic approval is part of packet preparation.
