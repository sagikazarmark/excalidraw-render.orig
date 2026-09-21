# #191 completed full suite — five exact pending failures, not green

The parent supplied the completed `cargo test --locked --workspace --no-fail-fast`
output at `tool_0c449b1de001sz9cnHVgDTvSaK`. Only that log was inspected for this
qualification. Its bytes are copied verbatim to `checks/full-workspace.raw.log`.

Strict parsing of 39 complete summaries and individual success listings establishes
**1257 passed / five failed / 19 ignored**: 1262 executed pass/fail, 1281 including
ignores. All **626 fast** and **404 library** tests pass; seven library tests remain
ignored. The completed invocation is **not labeled green**.

Exactly these five pending old-ownership/representative binding failures remain:

| Test | Exact observed cause |
| --- | --- |
| `coverage::ownership::tests::inventory_extension_preserves_live_owners_and_approved_exclusions` | unwrap error `stale ownership evidence` |
| `coverage::ownership::tests::test_owner_checks_compiled_module_exact_selector_and_source_digest` | same exact unwrap error |
| `ownership_recording_validates_the_entire_batch_before_writing` | `error: stale ownership evidence` |
| `ownership_recording_rejects_unknown_obligations_without_changing_the_ledger` | stderr assertion for `unknown ownership identity` fails |
| `representative_registry_is_consumed_through_approval_aware_native_execution` | `error: stale adapter production/source bindings` |

The assertion-only test's exact CLI was independently reproduced: nonzero exit,
empty stdout, `stale ownership evidence`, unchanged ledger bytes. The raw log's
parallel ordering of failure detail blocks is immaterial; names/locations/bodies,
summary lists and target trailer remain exact. Only the standard backtrace note
is ignored as nonsemantic. Unknown errors, extra failure content, substituted
test names, missing results, changed totals or passing relabels are rejected.

## Immutable source and successor audit

The original frozen workflow, receipt entry, production, tests, native evidence
and capture-draft are unchanged. New unbound `suite_gate.py` authenticates the
prior strict parser under its original sealed subject and changes only numeric
aggregate/fast totals. It checks the actual parent message, frozen source record,
complete producer/test descriptors and captured diagnostic before invoking the
unchanged qualification/receipt gate. Actual focused checks remain passing;
the five failures are not inserted into that list with false pass flags.

`checks/full-suite-record.json`, `checks/suite-successor.json` and the test proof
bind this classification. Nine suite/scope negatives and the original 28 isolated
measurement/migration/authorization/receipt mutations pass. No broad suite was
repeated. The 28-test rerun uses only isolated fixtures and immutable captured
measurements, not production image recapture or synthetic live approvals.

## Composition-only approval boundary

The pending subject covers first-ANMF standalone decoding and clear-canvas
placement, eleven corrected owner cells, 25 old owner-source refreshes and
V12→V13. No policy/criteria change or partial-alpha tolerance is included.
All old/new measurements remain exact: 30 changed vectors/60 raster records;
eight improved partial blend vectors still fail. Sample pass counts remain
SVG72/PNG48/PDF48; twelve partial-alpha cells stay gaps. Synthetic PDF rounded
projection failures are not measurements of PDFium/PDF.js.

After independent review, only actual designated `sagikazarmark` may record the
user's explicit decision, as one standalone fenced JSON with exactly
`decision: approve`, `scope: webp191-composition`, and the final production and V13
subject keys. A instruction to continue preparation is not that exact approval.

The guarded future transaction stores real decision/preimages, writes only V13
approval/registry/ledger, requires successful exact 109-test/six-command receipts,
and restores all three files with preserved error evidence on failure. Applied
verification authenticates the real decision first, checks saved successful
results, and independently reruns those bounded commands to resolve the five
historical failures. No blanket waiver or redundant full-workspace rerun.

Read-only final verification:

```sh
python3 tests/conformance/proposals/webp191-composition-requalification-v1/suite_gate.py verify
```

Projected counts remain 2418 assignments / 2321 assigned-excluded / 97 gaps.
Criteria, all 168 keys, three exclusions and prior codec limitations remain exact.
Baseline/#178, partial-alpha/#191 adjudication, #187 sampling, #190 HDR and CMS
resource work remain separate. No actual approval, live migration or commit was
performed during sealing.
