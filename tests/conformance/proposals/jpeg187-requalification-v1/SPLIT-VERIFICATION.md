# Final verification is combined, not a single green workspace invocation

The exact final source freeze remains intact. The parent completed verification
through the following combined commands on those bytes:

1. `cargo test --locked --workspace --no-fail-fast` was killed after the
   1,200,000 ms timeout while capture work was running. The archived raw log
   independently contains **404 library passes, seven ignores, and 584 unique
   fast passes out of 585**, with the long compatibility recovery test still
   pending and no fast completion summary.
2. The parent ran
   `cargo test --locked -p excalidraw-render --test fast compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family -- --exact`.
   It passed in **816.82 seconds**. This completion is a source-bound parent
   session attestation; no unavailable raw log is fabricated.
3. The parent completed all remaining renderer integration targets, including
   PNG parity 10 and SVG parity 8. Exact individual argv/raw logs were not supplied;
   the packet records that limitation. Renderer doctests passed with zero tests.
4. `cargo test --locked --workspace --exclude excalidraw-render --no-fail-fast`
   completed with exactly **five expected but still failing pending-binding tests**:
   two xtask-bin ownership tests, two xtask-coverage ownership tests and one
   representative-consumer test selecting stale V8. Other targets passed,
   including 101 xtask-bin tests. The parent reports 19 ignored tests overall.

The current whole workspace is **not claimed green before approval**. The five
failures remain explicit and must be rerun after the authorized source/owner/
representative metadata transaction. Existing actual stale-gate stderr confirms
the pending state, but is not substituted for unavailable full continuation logs.

## Exact evidence

- `final-checks/workspace-timeout.raw.log`: byte-for-byte copy of
  `/home/laborant/.local/share/opencode/tool-output/tool_0c15b1496001nt41rWaBJDo10Y`.
- `final-checks/parent-split-completion-message.txt`: actual supplied parent message.
- `final-checks/parent-split-verification.json`: commands/outcomes, separately marked
  raw evidence versus parent attestation, exact source/producer bindings, five
  enumerated pending failures and post-approval checks.
- `final-checks/split-checker-successor.json`: frozen checker and new wrapper
  descriptors. The frozen checker and previous draft evidence are unchanged.

## No all-pass workaround

The frozen `qualify.py` requires its focused `checks` list to pass; those checks
really passed and remain unchanged. The timeout and rest-workspace failures are
not inserted with fake `passed:true` flags. The new unbound `split-verification.py`
validates the separate combined record before invoking the authenticated frozen
seal/verify. It rejects missing/duplicate pass counts, a relabeled timeout, missing
long-test completion, any additional/unrelated failure, a falsely green claim,
stale sources and fabricated raw-log claims. No native recapture, code change,
source-binding refresh or authorization waiver is involved.

Read-only final packet check:

```sh
python3 tests/conformance/proposals/jpeg187-requalification-v1/split-verification.py verify
```

The wrapper itself and negative tests are bound into the final pending subject.
The seven focused wrapper tests do not run production tests or change evidence.

## Approval scope remains narrow

Future explicit approval must name the final color-only production/migration
subject and unchanged V9 subject. It covers the reviewed stable-graph CMYK/YCCK
correction, 15 named color-owner additions, 17 existing source-binding refreshes
and one V8→V9 representative replacement. The six sampling gaps/24 exact known
failure pins remain #187; the existing PDFium alpha finding receives no exception.
All 168 full keys, 38 policy criteria, other dispositions and three exclusions are
preserved. No native baseline/#178 or global/CMS resource closure is granted.

After actual approval/application, rerun only the affected xtask ownership/bin,
coverage and representative-consumer targets and strict approved execution checks
to resolve the five pending binding failures. Do not rerun the 15-minute renderer
suite merely to turn this honest split execution into a single invocation.
