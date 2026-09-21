# JPEG #187 color-only completion — 2026-09-21

## Actual scoped approval and application

The user explicitly approved both exact subjects. The designated maintainer
`sagikazarmark` recorded the affirmative `jpeg187-color-only` JSON decision at
**2026-09-21T01:20:15Z**:

https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5754180693

The strict v2 verifier fetched the real API comment and checked exact author,
decision/scope, production/representative subjects, ID, URL, date and saved/live
body agreement. No new or duplicate comment was posted during application.

Approved production/qualification/migration subject, unchanged:

```text
sha256:0baf0e3f8b7bbf37f575f0e8d32650befa7b73dd7a0a6c0c45e383c1a1693f32
```

Approved representative V9 subject, unchanged:

```text
sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d
```

The delegated parent-owned transaction wrote exactly three live metadata files:

1. `tests/conformance/candidates/stage4-frames-text-assets-v9/manifest.json` — only
   the actual four-field approval object.
2. `tests/conformance/candidates/registry.json` — exact reviewed V9 template.
3. `tests/conformance/coverage/ownership.json` — exact reviewed 15 additions,
   17 source refreshes and V8→V9 binding; only V9's final manifest-file hash was
   derived after writing actual approval metadata.

Before mutation, the transaction persisted actual approval/comment records,
exact three-file preimages, a broader source snapshot and its guarded driver.
Failure handling restores and verifies all preimages and records explicit
`failed_rolled_back` or `rollback_failed` status. No rollback was needed: the
attempt completed with `state: applied`.

Application evidence:
`tests/conformance/coverage/jpeg187-application-v2/`.
The immutable pending packets and all OLD/RC exploratory history remain unchanged.
This completion note is outside the approved packet/source bindings.

## Approved execution and resolved pending failures

Pinned Rust 1.97.1 Linux x86_64 was used with incremental compilation and dev/test/
release debug information disabled. Passed:

- Release `candidate-verify` for V9: exactly three approved SVG/PNG/PDF receipts,
  two fresh native processes per format, with exact artifact/full-report replay.
- Release `candidate-executions`: the same three receipts via the registry.
- Strict `verify-representative-owner` for V9.
- Normal `verify-inventory`: 320 coverage rows, 168 keys, eight unresolved milestones.
- Normal `verify-ownership`: 1912 assignments, 1835 assigned/excluded, 77 gaps;
  global ledger approval remains pending.
- Strong v2 `verify-applied`: affirmative actual maintainer decision, all frozen
  evidence, exact migration and both approved subjects verified.
- `git diff --check`.

The only test rerun was the requested affected-target command:

```sh
cargo test --locked -p xtask --bin xtask --test coverage \
  --test representative_candidate --no-fail-fast
```

Results: **103 + 5 + 1 = 109 passed**, zero failures or ignored tests. This resolves
the five previously reported pending stale-binding failures. Raw stdout/stderr
for every command is retained under the application directory's `logs/`.

## Combined test history — not a single clean workspace invocation

The approved packet accurately preserves the earlier split verification:

- The full workspace command timed out after 1,200,000 ms under capture load.
  Its raw log confirms 404 library passes/seven ignored and 584 unique fast
  successes of 585, with one long test incomplete at termination.
- Parent separately completed
  `compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family`
  successfully in 816.82 seconds on the same frozen bytes.
- Parent attested completion of all other renderer integrations, including PNG
  parity 10/SVG parity 8, and zero renderer doctests. The completed remaining
  workspace command had only the five then-stale binding failures. Those are now
  resolved by the 109-test rerun above.
- Parent continuation results remain explicitly session attestations where raw
  logs were unavailable. No raw output or single-run green result was invented.

No redundant full-workspace or long renderer suite was run during application.

## Exact qualification scope and counts

The source remains color-only on the exact stable graph from `4e5b624`:
zune-jpeg 0.5.15 and the original root/consumer lockfiles. Published RC2 sampling
work is retired exploratory history after the minimized neutral failure; it is
not represented as qualified or approved production.

All 76 frozen inputs reconstruct. **64 vectors pass canonically**, while 12 retain
all **24 exact known-failure PNG/PDF pins**. The neutral regression retains the
known wrong-pixel counts; the corrected pinned-browser 9×9 probe confirms 81
exact gray pixels in two fresh processes. All 168 OLD/current packaged full keys,
artifacts/failures and reports remain identical. Representative and protected PDF
artifacts/reports/viewer bytes are preserved.

The applied owner additions cover CMYK two cells and YCCK thirteen cells. All
38 policy criteria, other assignments/rationales/commands, 168 full inventory
keys and three approved exclusions remain intact.

| Measure | Final count |
| --- | ---: |
| Assignments | 1,912 |
| Primary owners | 1,832 |
| Approved exclusions | 3 |
| Assigned/excluded | **1,835** |
| Explicit gaps | **77** |
| JPEG color/sampling cells owned | 32 |
| JPEG color/sampling cells still gaps | 6 |

The six remaining #187 sampling gaps are baseline YCbCr and YCCK at 4×2, 1×4
and 2×4. Their failures are not tolerance exceptions or owner evidence. The
pre-existing PDFium asymmetric-alpha discrepancy remains unchanged at 24 pixels /
`3febf9303648d4c8`; it receives no exception or closure claim.

## Final identities and preservation

| File / identity | SHA-256 |
| --- | --- |
| Approved V9 manifest file | `473f95c569ece715bf531d893bc6e9eb82ed346f2a2be968ba08cf6e687be0cf` |
| Supplemental registry | `fbb3fb617089b2204433916cf8887626fb692a50b269a0ffc23c2c48c903aae4` |
| Ownership ledger | `e20d715727a4a203006f1744397a597814292088b1a7b3360601acc6134219bd` |
| Applied transition record | `ad9dacb0ba2cc62d687c78be76a2e4e9de10cdc211d9ea76b3e124bdc09edd35` |
| Catalog canonical digest, unchanged | `1507cde0e7d246bc09b5be3c4689810838833975e4b14993a449643c7dd44ff7` |

The approved packet envelopes remain null-approved immutable review records;
the actual production decision is stored in the application directory, and V9's
manifest stores its actual corresponding approval. These are intentional distinct
roles. `OWNERSHIP.md` was not edited because it is part of frozen historical
source evidence; current counts are recorded here and by the live ledger verifier.

Read-only current applied verification:

```sh
python3 tests/conformance/proposals/jpeg187-requalification-v2/verify.py verify-applied
```

It requires GitHub access and matching actual decision records. Historical pending
checks correctly described their original state; use this applied entry point for
the current approved transaction.

## Remaining work outside this approval

- Six noninterleaved baseline sampling cells and minimized missing-row defect: #187.
- Native baseline refresh/reapproval and native/WASM equality: #178, deferred.
- Global ownership approval/release closure: 77 gaps and eight draft milestones.
- Pre-existing fixed-size moxcms execution-table allocation/resource gap.

No commit was made. The approved color-only transaction and previously failing
binding targets are ready for the parent's final inspection and commit.
