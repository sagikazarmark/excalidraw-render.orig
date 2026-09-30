# Stage 4 scoped exception proposal tooling

The user authorized preparation of case-specific PDF exceptions, not approval.
This bounded unit implements that preparation and its gates. It changes no
production rendering/ICC code, global budget, v4 scene or crop, protected baseline,
or approval record. Fresh v5 native/viewer collection is deferred to the next unit.

## Exact scope

`xtask/src/conformance_candidate_exceptions.rs` admits only these three records:

| Viewer | Region | Max changed pixels | Minimum SSIM bits |
| --- | --- | ---: | --- |
| PDFium | whole `(0,0,200,120)` | 118 | `3fefacd49b980278` |
| PDFium | typography `(10,12,124,41)` | 118 | `3fee1f978eeaac28` |
| PDF.js | whole `(0,0,200,120)` | 16 | `3feff3e226be27cf` |

PDF.js typography and both nontext crops retain canonical limits. PNG and both
SVG comparisons retain canonical limits. The fixed `judgment-plan.json` remains
unchanged: exceptions are a separate, explicit proposal, not edits to that plan.

Each scope binds the candidate case ID, default variant, PDF format, comparison
ID, exact viewer profile descriptor, region bounds, exact limits, non-defect
cause and approving issue #85. The proposal also binds the comparator, current
inputs, diagnosis documents, their explicit repository producer chain, and the
upstream source identities recorded by the diagnosis.

The reviewed proof is pinned to:

- `docs/research/stage4-pdf-typography-diagnosis.md`:
  `8bba3edd8b7b3928aed1794e53bda1b2f21b42b449e01c87d1087be68571b766`
- `docs/research/stage4-pdf-typography-evidence.json`:
  `5efa8854d7cecd731427b3a18ce69d728e791e2d420ba8d5d06640a7357ace72`

All explicit repository bindings in the proof are checked against current bytes.
Its historical v4 collection is not recursively rebound to today's collector
implementation. Fresh candidate provenance is checked separately by the normal
candidate gate. The diagnosed PDF, reference SVG, scene, crop plan and three
relevant viewer rasters must remain byte-identical to the diagnosis's inputs;
equal aggregate metrics on different inputs cannot reuse this diagnosis.

## Preparation versus approval

`candidate-propose-exceptions DIRECTORY` writes `exception-proposal.json` only
for a fresh unmaterialized revision. It first recomputes canonical image
judgments and requires exact agreement with the reviewed measurements, with no
margin. It refuses unused scopes that now pass canonically, drift in either
measurement, unknown/duplicate scopes, changed causes, widened limits, or stale
proof. It emits `proposed-not-approved`, never an approval subject or approval.

The gate recomputes canonical judgments and, separately, the proposed effective
judgments using the real comparator's `ComparisonLimits` and `Crop::with_limits`.
It never reads stored judgment booleans as evidence. `judgments.json` retains
the canonical results alongside the effective rows and full proposal.

Materialization may return `ready-for-approval` when all effective visual gates,
native replay, reports, semantics and provenance checks pass. The manifest has
`approval: null`. The approval subject includes the full proposal and its file
descriptor, diagnosis and source bindings. Approved execution still requires a
real maintainer approval of that exact case subject. Any scope/cause/limit/proof
change invalidates it. The supplemental registry must reproduce the manifest's
three exception records exactly; declarations alone cannot authorize execution.

## Targeted checks run

Rust 1.97.1; `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_TEST_DEBUG=0`,
`CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo test --locked -p xtask --bin xtask scoped_ -- --nocapture
cargo test --locked -p xtask --bin xtask provenance_ -- --nocapture
cargo clippy --locked -p xtask --bin xtask -- -D warnings
git diff --check
```

All pass: five scoped-filter tests (four new), six provenance-filter tests, and
Clippy. Tests reject missing/duplicate/unknown scopes, looser pixel/SSIM limits,
missing cause, changed viewer/region/issue/proof, changed proof bytes, unused
exceptions and moved measurements. Isolated scratch evidence with forged stored
`passed`/`approved` booleans cannot hide actual failures. Recomputed v4 PDF
comparisons pass only their proposed scopes; v4 PNG remains a canonical failure.
In-memory synthetic authorization tests prove the actual case subject binds
exception content and that approval is still required; no synthetic approval is
written into corpus evidence.

## Next bounded unit: fresh v5 evidence

After the parent confirms final production sources, run from the repository root:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
D=tests/conformance/candidates/stage4-frames-text-assets-v5
cargo run --locked -p xtask -- conformance candidate-revise "$D"
mkdir "$D/assets" "$D/viewer-evidence"
cargo run --locked -p xtask -- conformance candidate-native-capture "$D"
node tools/reference-oracle/scripts/stage4-viewer-assets.mjs "$D"
cargo run --locked -p xtask -- conformance candidate-prepare-assets "$D"
node tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs "$D" collect
node tools/reference-oracle/scripts/stage4-candidate-views.mjs collect "$D"
cargo run --locked -p xtask -- conformance candidate-propose-exceptions "$D"
cargo run --locked -p xtask -- conformance candidate-inspect "$D"
cargo run --locked -p xtask -- conformance candidate-materialize "$D"
cargo run --locked -p xtask -- conformance candidate-approval-subject "$D"
```

The revision command copies v4 authoring/reference inputs unchanged. Do not reuse
an existing directory or overwrite failed observations. Verify both PNG whole and
typography gates, both SVG gates, every nontext crop, and exact PDF measurements.
The last command issues a subject only if effective gates and fresh replay pass.

Before approved registry execution, update only the supplemental candidate
registry to the new case/directory/three format keys and copy the exact
`exception-proposal.json` `exceptions` array into its entry. Preserve the original
execution registry and approvals. Parent review and a real maintainer approval
remain necessary. No fresh v5 approval subject is claimed by this tooling unit.
