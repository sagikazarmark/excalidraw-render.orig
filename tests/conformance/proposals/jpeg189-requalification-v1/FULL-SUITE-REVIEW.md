# #189 completed workspace — five exact pending failures remain explicit

The final parent command completed normally with exit 1 within its 1,800,000 ms
timeout, using pinned Rust 1.97.1, incremental disabled and dev/test/release debug
profiles set to zero:

```sh
cargo test --locked --workspace --no-fail-fast
```

`checks/full-workspace.raw.log` is the byte-for-byte supplied tool output
`tool_0c281dfba001UIHGAkuDc6S5rs`. Its 39 complete summaries and independent pass
listings establish **1232 passed / five failed / 19 ignored** (1237 executed
pass/fail; 1256 including ignored). All **601 fast** and **404 library** tests pass;
the library retains seven ignored tests. This is neither a timeout nor a green run.

Exactly five failing tests remain pending old-owner/representative bindings:

1. `coverage::ownership::tests::inventory_extension_preserves_live_owners_and_approved_exclusions` — unwrap `stale ownership evidence`.
2. `coverage::ownership::tests::test_owner_checks_compiled_module_exact_selector_and_source_digest` — same exact unwrap error.
3. `ownership_recording_rejects_unknown_obligations_without_changing_the_ledger` — assertion that stderr contains `unknown ownership identity` fails.
4. `ownership_recording_validates_the_entire_batch_before_writing` — `error: stale ownership evidence`.
5. `representative_registry_is_consumed_through_approval_aware_native_execution` — `error: stale adapter production/source bindings`.

These correspond exactly to xtask bin 101/2, coverage 3/2, representative 0/1.
The assertion-only test's precise CLI invocation was independently reproduced;
actual stderr is `stale ownership evidence`, stdout empty, and ledger bytes remain
unchanged. Its logs bind the causal attribution instead of guessing hidden output.

## Strict authenticated successor

The frozen `workflow.py`, `receipt_gate.py` and approved historical helpers remain
byte-identical. New unbound `suite_gate.py` authenticates the original #188 parser
under its sealed subject, adapting only numeric totals 1226→1232 and 595→601 and
derived totals. Exact identities, source locations, error bodies, summary lists,
final target trailer and allowed-note handling are unchanged.

The only ignored nonsemantic line is the exact standard
`note: run with RUST_BACKTRACE=1 ...` text. Its presence/placement is allowed;
arbitrary notes, extra details/errors, changed failures or missing summaries are
rejected. This does not waive any failure or merely allow a failure count.

`checks/full-suite-record.json` binds raw output, actual parent message, source
freeze, final producer/test identities, classification and diagnostic reproduction.
Existing focused passing logs and the earlier draft remain unchanged. The five
failures are not inserted into passing focused checks with false pass flags.

Nine negative/classification/auth-scope tests pass without rerunning renderer or
workspace tests. The actual strict post-approval receipt gate still requires all
109 exact successful test identities, six exact commands, zero exits, bound logs,
current source/subjects/metadata and independent fresh bounded reruns.

## Review and eventual application

Read-only final pending check:

```sh
python3 tests/conformance/proposals/jpeg189-requalification-v1/suite_gate.py verify
```

After independent review and explicit user approval of both exact final subjects,
actual designated `sagikazarmark` must record one standalone affirmative JSON
decision on #85 with scope **`jpeg189-exif`**, production subject and V11 subject.
Rejections/hash mentions/other authors/wrong or duplicate fields do not authorize.

Parent can then use `suite_gate.py apply ACTUAL_APPROVAL_JSON`. It retains the
existing guarded transaction, real comment authentication, persisted preimages,
rollback/failure evidence and no-overwrite retry rule. Applied status is allowed
only after successful actual 109-test/six-command receipts and exact migration
validation. `suite_gate.py verify-applied` authenticates the actual decision first,
checks this historical failure record, validates saved success receipts, and
independently reruns those bounded commands to establish resolution.

No full-suite repeat is needed or performed here. No actual approval, live
migration, baseline refresh or commit is part of this sealing operation. Scope
remains 36 new EXIF owners, 46 source refreshes and V10→V11, projected 2074 /
1997 assigned-excluded / 77 gaps, with all criteria/168 keys/three exclusions
preserved. #187 sampling, baseline/#178, global and CMS resource work stay open.
