# Final #187 readiness — wait for explicit parent freeze

The parent selected and applied the **minimal consumer lock**, SHA-256
`500a95474dd043905056db92324f3f1cc8d8e9b908aa7de8d6e9d8b0d39d63ca`.
The aligned alternative is retained as exploratory history, not adopted.
Pre-existing bytemuck/proc-macro2/quote drift stays documented; it is not silently
claimed as identical workspace/package feature or version resolution.

No final C capture, bridge v3 collection, V9 collection, approval or live ownership
migration has started. Parent owns production/tests and will send the final freeze.

## Historical reference authentication for the parent test

The exact root Cargo.lock from commit `4e5b624` is archived at:

```text
tests/conformance/proposals/jpeg187-requalification-v1/before/Cargo.lock
```

From `crates/excalidraw-render/tests/fast/jpeg_color_contract.rs`:

```rust
include_bytes!("../../../../tests/conformance/proposals/jpeg187-requalification-v1/before/Cargo.lock")
```

Authenticate its bytes as
`27717cb869ee8b99a6a401f0c14cb4953c3e19813a69033c9121d8beca513cd2`,
and keep the frozen reference source descriptor equal to that historical digest.
Separately authenticate live Cargo.lock as
`d10fd6fcfe6b7efe18249485e7410e1a9edd47414a6573dc1503803983d877da`.
The committed origin, old/current root locks, old/minimal consumer locks and frozen
reference identity are in `historical-lock-transition.json`. The archive is actual
`git show 4e5b624:Cargo.lock` bytes, not reconstructed current bytes.

## Exact ownership scope

`qualify.py` has no live mutation/apply command. It prepares and verifies:

- The 21 exact `codec:jpeg:color-sampling:*` rows currently `gap` under #187 become
  owned by `jpeg_color_contract::qualified_gray_rgb_and_ycbcr_cells_pass_both_reference_authorities`.
- The 17 existing rows for that function refresh only the test-source hash.
- The existing representative selection moves V8→V9 with the exact registry,
  approved-subject, manifest-file binding and release command.
- Every other assignment, rationale, command, policy criterion, three exclusions,
  catalog and 168 full inventory keys remains unchanged.

Expected counts: **1912 assignments / 1838 owned / 3 exclusions / 1841 assigned or
excluded / 71 gaps**. Global ledger approval stays null. No automatic 38-cell
policy expansion or baseline approval is performed.

The pending template uses a null-approved V9 manifest. After actual user approval,
only its approval-bearing file hash is parameterized. The exact unchanged
exception scopes (including reasons/profiles) must match V8 except case ID.

## Final nested scope and pre-existing finding

Use the ten already-authored immutable inputs; recollect final native artifacts,
qpdf models and both viewer pairs once. `check_final_findings` requires:

- All final JPEG/PNG/GIF independent sample comparisons and viewer comparisons
  pass canonical gates, except the one enumerated historical PDFium alpha finding.
- All non-JPEG old/final artifacts, reports and viewers stay byte-identical.
- Every diagnostic list stays unchanged; the filter case retains its bounded
  fallback report. No artificial blur oracle is claimed.
- `png-rgba8` PDFium remains exactly **24 / `3febf9303648d4c8`** and its two final
  raster runs equal the corresponding OLD runs byte-for-byte. Sample/soft-mask
  binding and PDF.js remain exact.

That last observation is a **pre-existing known discrepancy**, not an approved
tolerance exception, not a passing canonical gate and not a new #187 regression.
It does not block this unrelated JPEG correction when unchanged. Any movement,
new viewer failure or unrelated output change requires diagnosis before sealing.

## Source/tooling freeze

The sole bound xtask edit is `conformance_pdf_bridge.rs::DIRECTORY` v2→v3.
Its logic is unchanged and xtask formatting passes. Finish any parent formatting,
minimal regression/review changes and Clippy **before** invoking freeze. No parent
changes are inferred as frozen from an idle workspace.

After the explicit parent message, place this authorization record outside the
packet, with the actual parent message text:

```json
{"final_current_capture_authorized":true,"parent_freeze_message":"ACTUAL PARENT FREEZE MESSAGE"}
```

Commands (not run yet):

```sh
P=tests/conformance/proposals/jpeg187-requalification-v1
python3 "$P/qualify.py" freeze /tmp/opencode/jpeg187-parent-freeze.json
python3 "$P/qualify.py" stage-final /tmp/opencode/jpeg187-final-stage
python3 "$P/collect.py" final /tmp/opencode/jpeg187-final-stage "$P/final-freeze.json"
python3 "$P/nested.py" final /tmp/opencode/jpeg187-final-stage final-current "$P/final-freeze.json"
python3 "$P/nested.py" qualify final-current
python3 "$P/nested.py" compare final-current
python3 "$P/qualify.py" representative
python3 "$P/qualify.py" protected
python3 "$P/qualify.py" migration
```

All cargo/native operations use pinned Rust 1.97.1 and memory-saving profiles.
The V9 command uses the established full pipeline: fresh bridge v3, native capture,
resource derivation, font audit, six viewer pairs, exact exceptions, inspection,
materialization and subject. The protected command proves two fresh native full
artifact/report matches and repeated qpdf/PDFium/PDF.js equality to the existing
protected artifact. No protected pin is refreshed.

Final 168 capture uses the already-selected **minimal** consumer lock. OLD capture
is reused. Do not run a B full capture or another blind full workspace suite.

## Parent test evidence and sealing

After the parent runs/reports its final suite or bounded qualification, provide
`final-checks/production-qualification.json` containing:

- `jpeg_reference_sha256`: the frozen `9b5ab60a…` full hash;
- `frozen_input_reconstruction_passed: true`, `qualified_vectors: 76`;
- `test_source`: exact final `jpeg_color_contract.rs` path/length/hash;
- `producer_sources`: the complete exact V9 native producer descriptor set;
- `checks`: named commands with `passed: true` and existing log descriptors;
- `diagnosed_inventory_changes`: the exact old/new key/sample records returned
  by `collect.py compare`. Empty only if all 168 are unchanged. Every changed
  output requires explicit parent-reviewed diagnosis; never normalize it away.

Then `qualify.py seal` binds all final data and emits a **pending** production/
dependency/migration subject plus the separate V9 subject. Sealing refuses absent
final captures, missing source-bound test results or unreviewed changed inventory
outputs. `qualify.py verify` recomputes packet/file/source/graph/migration checks
and asks the strict candidate gate for the unchanged V9 subject.

## Authorization and parent-owned application

The checker unconditionally rejects optimized Python before workflow dispatch.
Its applied audit requires real production approval and comment records, exact
both-subject text, actual identity/date/URL consistency, a fresh matching GitHub
API response and the same decision in V9 approval. V9 approval alone cannot
authorize the 21 new owner selections or 17 source refreshes.

Parent owns application. Only after user approval of both final subjects: record
the genuine decision on #85 (and cross-reference #187), persist `approval.json`,
`comment.json`, exact preimages and attempt status **before** any live metadata
mutation; apply the three-file V9 approval/registry/ledger transaction; run actual
approved representative/registry/owner/inventory/ownership checks; then mark
`tests/conformance/coverage/jpeg187-application-v1/status.json` as `applied`, naming
`production_subject_sha256` and `representative_subject_sha256`. Reuse strict v2's
rollback and failed-attempt archival/retry protocol, not an unaudited hash update.
`qualify.py verify-applied` requires these persisted records and authenticates them.

The helper intentionally does not grant approvals or write the live ledger.
Negative tests use in-memory API-shaped fixtures only. Current preparation leaves
live owners stale pending final approval and parent transaction.

## Current readiness checks

- 15 new isolated authorization/migration tests pass, including mandatory saved
  production approval/comment records in applied auditing and incomplete-state rejection.
- Eight capture-integrity/explicit-freeze/optimized-startup tests pass.
- `cargo fmt -p xtask -- --check` and `git diff --check` pass.
- Historical root lock authenticated; minimal consumer lock matches parent.
- No final capture or approval subject has been created before freeze.

Baseline/#178, global closure and the pre-existing fixed-size CMS allocation gap
remain separate. The parent may organize eventual color/RC changes into two commits
after final combined qualification; this packet does not assert intermediate
commits have separately current representative approval.
