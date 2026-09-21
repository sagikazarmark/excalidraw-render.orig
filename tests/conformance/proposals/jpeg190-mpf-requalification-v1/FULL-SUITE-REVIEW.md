# #190 completed full suite and exact policy-aware pending failures

The parent completed `cargo test --locked --workspace --no-fail-fast` normally,
exit 1, within 1,800,000 ms on pinned Rust 1.97.1 and memory-saving profiles. The
raw log `tool_0c317b2e5001j4sIC9NDTQfgyV` is copied byte-for-byte to
`checks/full-workspace.raw.log`.

Strict parsing of 39 complete target summaries and individual pass listings gives
**1244 passed / five failed / 19 ignored**: 1249 tests executed pass/fail, 1268
including ignores. All **613 fast** and **404 library** tests pass (seven library
ignores). This is not a green invocation or a timeout.

The only accepted pending failures are exact identities, locations and bodies:

| Test | Observed message |
| --- | --- |
| `coverage::ownership::tests::inventory_extension_preserves_live_owners_and_approved_exclusions` | unwrap error `stale ownership evidence` |
| `coverage::ownership::tests::test_owner_checks_compiled_module_exact_selector_and_source_digest` | same exact unwrap error |
| `ownership_recording_rejects_unknown_obligations_without_changing_the_ledger` | assertion for stderr containing `unknown ownership identity` fails |
| `ownership_recording_validates_the_entire_batch_before_writing` | `error: stale ownership evidence` |
| `representative_registry_is_consumed_through_approval_aware_native_execution` | `error: stale adapter production/source bindings` |

The assertion-only test's exact CLI was separately reproduced; it fails with
`stale ownership evidence`, emits no stdout and leaves the ledger byte-identical.
No unrelated criterion/MPF behavior failure is being excused as a binding failure.

The new unbound `suite_gate.py` authenticates the original strict parser, changes
only aggregate/fast numeric totals, and preserves exact five-name/location/message
checks, failed summary lists, permitted standard backtrace note and final trailer.
Unknown errors, extra details, substituted identities, truncated completion or
forged counts are rejected. Frozen workflow/receipt/production/test/capture bytes
and prior draft records remain unchanged.

## Policy-aware approval scope

This packet binds **exactly 16 before/after criterion rows** under
`policy/criterion-transition.json`, the normal isolated catalog derivation, and
the corresponding policy proposal. Canonical-admission language changes only for
bad-offset/bad-size/bad-count/missing-secondary cells to exact strict recovery,
independent structural validation and browser base-fallback characterization.
There is no visual exception. All other criteria, spec authorities, pointers,
candidates, eight HDR-gap definitions and three exclusions stay unchanged.

The transaction contains **four live files**: V12 approval, representative registry,
policy and ledger. New ledger catalog digest is the exact proposed policy-derived
catalog. Pending/live catalogs are verified by phase; old immutable history is not
rehashed or used to mask unapproved live changes. Post-approval bindings include
the actual policy and ledger bytes.

The strict applied gate requires actual `sagikazarmark` affirmative standalone
JSON scope **`jpeg190-mpf`** naming both final subjects, real GitHub consistency,
saved successful 109-test/six-command receipts and independent bounded fresh reruns.
Four-file preimages are persisted before mutation; any failure restores all four
files and preserves error/partial receipts. Automatic overwrite/retry is refused.

Ten focused successor tests pass, including a real adapted transaction in a
disposable fixture that changes four files, injects receipt failure, then proves
policy and all three other files are restored exactly with retained failure state.
No synthetic decision or application is written to live metadata by tests.

Read-only pending check:

```sh
python3 tests/conformance/proposals/jpeg190-mpf-requalification-v1/suite_gate.py verify
```

After independent review and actual user approval of the exact production/policy
subject and V12 subject, `suite_gate.py apply ACTUAL_APPROVAL_JSON` runs the guarded
four-file transaction. `verify-applied` authenticates the real decision and requires
successful resolution of the five old failures via the existing 103+5+1 targets
and five approved consumer/owner/inventory/ownership commands. No historical
failure is relabeled passing; no full-suite rerun is required here.

Scope stays 16 new strict-recovery owners, 28 source refreshes, 16 criterion changes
and one representative replacement: 2126 assignments / 2041 assigned-excluded /
85 gaps. Eight HDR cells/16 vectors/32 unchanged pins, #187 sampling, baseline/#178,
global closure and CMS resource work remain deferred. No actual approval, live
policy/ledger change, recollection, production edit or commit occurred while sealing.
