# Formatting-only source requalification and pending ownership migration

This packet preserves the approved v7, protected PDF and six-case records. It
does not transfer their approvals to new source bindings. Its two pending review
subjects are the new representative v8 subject and this formatting/migration
subject, which binds v8 and the exact proposed transaction.

## Source changes

Six files under `crates/excalidraw-render` are changed to exactly the output of
pinned Rust 1.97.1 rustfmt (edition 2021):

- `src/lib.rs`
- `src/render/asset/color.rs`
- `src/render/png.rs`
- `tests/fast/dark_mode_frame_and_image_render.rs`
- `tests/fast/png_stage4_contract.rs`
- `tests/fast/stage4_interaction_contract.rs`

Their exact current pre-format bytes were archived before editing and matched
the previously recorded hashes. `before/`, `after/`, `rustfmt-output/`, `diffs/`
and `source-transitions.json` bind and reproduce the transformation. Live source
edits used apply_patch; rustfmt stdin/stdout guided the exact patch. Rustfmt may
add trailing commas and remove redundant closure braces as normal formatting;
the invariant is exact formatter output, not an unsafe whitespace-stripping
approximation or a claim that git diff -w proves equivalence.

The sole xtask edit changes `conformance_pdf_bridge.rs`'s immutable bridge
directory from v1 to v2. No validation branch, allowed structural difference,
profile, metric or tolerance changes. V1 is retained unchanged. This avoids
overwriting its old producer bindings while collecting current strict evidence.

The parent's already-applied external `tests/stage4_typography.rs` change is
recorded separately as a **test-only semantic portability fix**, not formatting:
std f64 sin/cos/asin/acos calls become pinned libm calls, with an explanatory
comment and formatted import. Its reconstructed preimage is authenticated by
SHA-256 `93c4d040619c9f9b042caee5b7be3de5ecf7910791877b2a5549beb22cb5dc86`.
It is outside representative production bindings. No other production semantics
are changed. Reference helper files and their digest constants are unchanged;
old constants in historical reconciliation code remain intact.

## Qualification order and evidence

Before new collection, run workspace fmt and Clippy, the three affected test
modules, six external typography tests and the protected PDF canary. Record a
complete live source/tooling/history freeze, including the standalone checker.
Only after that freeze:

1. Collect the strict v2 PDF colour-space bridge, keeping v1 unchanged.
2. Revise a fresh v8 from fixed v4 authoring and capture native SVG/PNG/PDF with
   pinned `--release --locked` builds and two fresh processes per format.
3. Derive resources; collect font-audit controls and all six viewer pairs.
4. Prepare exactly the original three PDF exception scopes; inspect, materialize
   and compute the new pending subject.
5. Independently replay all three representative formats, the protected backend
   PDF and both SVG/PNG formats for the six focused cases—16 comparisons, each
   with two fresh current release processes—and compare complete artifact and
   CLI report bytes to their authenticated old captures. Compare v8 viewer bytes
   to v7, and rerun protected PDF qpdf plus both viewer pairs.

`output-equality.json` records the actual current release build identity. An
executable hash change caused by source locations/codegen is not an artifact
equality waiver: every artifact and complete report must remain byte-identical.
Changed output stops this workflow rather than changing pins or limits.

The protected PDF collector is intentionally not reused: it only handles the
already-applied old pin transition. Its approved packet remains immutable and
its old producer-check CLI becomes stale. Current protected-PDF applicability is
proved by this new source-transition/equality packet, without editing its canary,
structural expectation, old approval or verifier implementation.

## Historical audit versus current validation

The original six-case packet and applied-transition audit bind pre-format source
bytes and the pre-migration ledger. Rewriting them would change reviewed history.
`historical` runs the unmodified preservation audit in a disposable tree of
authenticated before-source bytes, with the same actual metadata approvals. That
result is explicitly historical; it is not a claim that the current stale live
ledger passes. The new packet separately checks every frozen live file and the
actual formatting deltas, output equality, current v8 gate and prospective
migration. All six ordinary case approvals remain valid and unchanged.

The read-only current checker is `requalify.py verify` while pending, and
`requalify.py verify-applied` after the authorized transaction. The old
`pdf-requalification-subject v2`, approved-v7 consumer and live `verify-ownership`
are expected to reject stale current source bindings during the pending phase.
No exception or broad hash bypass is introduced into those gates.

## Exact pending transaction

`pending/registry.json` and `pending/ownership.template-not-live.json` are proposals
only. The existing live registry and ledger remain byte-identical to `before/`.
The migration changes:

- 55 existing `png_stage4_contract` owner-source hashes;
- two existing `stage4_interaction_contract` owner-source hashes;
- the existing representative owner/registry selection from approved historical
  v7 to v8, **only after genuine approval** of v8 and this formatting packet.

All other assignments, criteria, selectors, test commands, catalog authority,
ordinary six-case approvals, 168 full inventory execution keys and three approved
exclusions stay unchanged. Counts remain 1,874 assignments, 1,800 owners, three
exclusions, 1,803 assigned/excluded and 71 gaps. Global ledger approval stays null.

The template binds the pending v8 manifest for review. The only parameterized
binding is its final manifest file SHA-256, recomputed after recording the actual
four-field approval. The canonical v8 subject excludes approval, so that subject
must remain identical. The prospective checker proves the exact 57 source
substitutions and one representative replacement and rejects all other changes.
Pending v8 emits no approved execution receipts; no synthetic authorization is
written into any live or proposed owner.

## Reproducible commands

The driver pins Rust 1.97.1 Linux x86_64 and sets incremental/debug profiles to
zero. Creation phases refuse overwriting evidence. From the repository root:

```sh
P=tests/conformance/proposals/stage4-format-requalification-v1
python3 "$P/requalify.py" verify
```

Its generation phases (already executed in order; do not overwrite/re-run them
in the same directory) are `archive`, source patches, `formatted`, `quality`,
`freeze`, `capture`, `equality`, `historical`, `migration`, `boundary`, `seal`.
Exact cargo/native/viewer commands and their results are recorded in `checks/`.
V8 can independently be reviewed with:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0
D=tests/conformance/candidates/stage4-frames-text-assets-v8
cargo run --release --locked -p xtask -- conformance candidate-inspect "$D"
cargo run --release --locked -p xtask -- conformance candidate-approval-subject "$D"
```

## Actual approval and later application

After independent review, ask the user to approve both exact subjects. Record the
real user-granted decision in an issue #85 comment naming both subjects; avoid
duplicates. Do not reuse v7's earlier approval. Create a four-field JSON record
outside the immutable packet using the actual login/date/comment URL and the
**formatting packet** subject:

```json
{
  "approved_by": "ACTUAL_LOGIN",
  "approved_at": "ACTUAL_YYYY-MM-DD",
  "approval_url": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-ACTUAL_ID",
  "approval_subject_sha256": "sha256:EXACT_FORMATTING_SUBJECT"
}
```

Only after that explicit user decision:

```sh
python3 "$P/requalify.py" apply /absolute/path/to/actual-approval.json
python3 "$P/requalify.py" verify-applied
```

The guarded metadata operation fetches the real comment, requires both exact
subjects, fills only v8's approval, derives its final manifest binding, switches
the registry/ledger to the exact reviewed templates and runs real approved
candidate/registry/strict-owner/inventory/ownership checks. It restores those
three live metadata files if validation fails. It records actual decision and
execution receipts separately under `coverage/stage4-format-application-v1/`.
No source/tooling edit or new capture is required for application.

Baseline/#178, global closure/71 gaps and the fixed-size moxcms execution-table
allocation/resource gap remain open. This workflow performs no commit or broad
workspace test suite and grants no approval during pending preparation.
