# Contributing

This is the entry point for testing, generating references and reviewing
expectation changes. [Development details](docs/development.md) cover build/run
options; [CI operations](docs/ci.md) cover the gates and release evidence.

Work in the `devenv shell`: it pins every tool the gates use, including Rust
1.97.1, nextest, qpdf 12.3.2, Firefox and the wasm-bindgen runner (ADR-0024).
Every gate is a [`just`](justfile) recipe; `just` lists them.

## Run tests

| Level | Command | What it checks |
| --- | --- | --- |
| Unit feedback | `cargo nextest run --locked --workspace --exclude xtask --lib --bins --profile ci-fast` | Renderer, CLI, assets, and comparator unit targets |
| Full Rust | `just test test-render test-doc` | Quick tests, official-reference parity, parser limits, and doctests |
| Per change | `just ci` | Formatting, lints, every test tier, the conformance verifiers and the packaged consumers |
| Release | `just release` | Native and browser-WASM qualification, snapshot replay, traces and full equality |

The per-change gates run on any host the devenv shell supports, macOS included.
CI runs the same recipes, one job each. Native release evidence needs
`x86_64-unknown-linux-gnu`, the supported native target; the browser-WASM
release checks run anywhere.

`cargo test --locked --workspace` also works without nextest. Neither ordinary
Rust command runs the ignored viewer harnesses or release-only qualification.
Optional CJK coverage uses an isolated packaged consumer so workspace feature
unification cannot accidentally restore the payloads: `just package-no-cjk`
checks it natively and `just wasm-packaging` in browser WASM.
The full workspace includes `xtask` evidence tests that invoke Node and qpdf
12.3.2; the devenv shell provides both, and links qpdf at
`target/pdf-harness/qpdf`.
The full integration suite includes the unsupported field/value families as
independent tests, with a checked test-group owner (ADR-0018). Use the unit
targets or a focused test while iterating. For CLI integration changes, run
`cargo test --locked -p excalidraw-render-cli --test render_cli`.
Release preparation additionally runs the SVG/PDF viewer harnesses. See
[the release runbook](release.md).

## Understand a failure

- **Ordinary assertion:** investigate the implementation or test expectation.
- **Reference parity:** our output diverged from pinned official Excalidraw
  output; inspect the geometry, report, and visual comparison.
- **Snapshot drift:** exact renderer artifact/report bytes changed. An intended
  fix can cause this, but regenerating a snapshot does not establish correctness.
- **Evidence binding:** a manifest no longer describes its referenced files.
  Follow the relevant generator/verification instructions before updating it.
- **Experimental WASM:** known native/WASM differences are tracked explicitly;
  new differences, changed reports, or within-target nondeterminism must fail.

## Change expected output deliberately

Five-layer coverage mapping is optional. Adding regression fixtures, cases or
tests does not require regenerating or approving `layers.json`, or assigning
`layer:*` ownership rows. Use `cargo run --locked -p xtask -- conformance
layers-report` for a read-only coverage report. Mapping gaps and stale
declarations are advisory findings. Ordinary tests, registered execution inputs,
reference evidence and expected-output checks still apply.

Adding a regression test or editing an existing test module requires no coverage
source-hash refresh. Test ownership checks resolve existing selectors against the
current Rust tree; milestone links retain historical source identities. A removed
or renamed owned test still needs its selector updated. Changes to registered
render inputs or expected output still use the workflow below.

1. Explain the intended behavior change and add a focused regression when needed.
2. Generate candidate evidence in a staging directory using the commands in
   [conformance operations](tests/conformance/README.md) or
   [CI operations](docs/ci.md). Keep generation separate from verification.
3. Review the changed outputs, diagnostics, and visual/structural measurements.
   Do not broaden tolerances just to make a failure pass.
4. Review changed expectations and comparison rules through normal Git/PR review.
   No separate approval hash, issue-comment acceptance or approval-writing command
   is required for any evidence family.
5. Run the affected checks, then `just ci`, and `just release` before a release.

Running tests needs no fresh approval. An unchanged accepted baseline remains
accepted. Contributors and agents may prepare updates; maintainer acceptance
is the decision that a new expectation is correct.
For cases in `tests/conformance/cases/representative/registry.json`, normal code
review is the acceptance step: no separate issue-comment approval or
`record-approval` command is required. `representative case materialize` writes
`approval: null`; review the case and add it to the registry through a PR.
Registry verification and coverage's independent-case validation accept null
approval. Retained approval fields are historical metadata, not acceptance gates;
`case subject` remains diagnostic. See the
[representative intake workflow](tools/representative-intake/README.md#registered-cases).
The same policy applies to catalogs, viewer qualification, Stage-4 candidates,
native baselines, PDF judgments, exceptions and coverage records. See
[ADR-0019](docs/adr/0019-review-expectations-in-git-without-approval-hashes.md).

Representative cases treat source/build identities as historical provenance:
code-only changes are checked by fresh replay against the accepted outputs,
without a new numbered candidate or approval. See
[ADR-0017](docs/adr/0017-separate-representative-expectations-from-producer-provenance.md).

Font-subset qualification also treats implementation hashes as historical
provenance. Check subsetter changes with the corpus test, and viewer-adapter or
input-generator changes with fresh viewer reproduction; see
[SVG font subset qualification](tests/conformance/README.md#svg-font-subset-qualification).

Native snapshot baselines also retain historical collector and generator source
identities. Refactoring those tools needs no expectation refresh: recollect a
candidate and replay it against the accepted baseline to check current inputs,
execution coverage and exact output. See
[native baseline replay](tests/conformance/coverage/README.md#canonical-baseline-approval-and-replay).

PDF-judgment derivation also records historical source identity. Source-only
refactors need no judgment-record refresh: verification rebuilds the reports,
rules and evidence classifications and checks them against the accepted record.
See [PDF judgments](tests/conformance/coverage/README.md#pdf-judgments-closurepdf-judgments).

Release font-subset evidence retains historical workflow, runner and generator
identities too. Its corpus, profile, schema, cross-target measurements, viewer
qualification and aggregate still verify. JPEG reference tests authenticate the
captured inputs/profiles and compare actual decoded/rendered pixels; today's
whole `Cargo.lock` and test/helper source bytes are not expectation inputs.

## Generate and check references

| Work | Current workflow |
| --- | --- |
| Official Excalidraw reference exports | [Reference oracle](tools/reference-oracle/README.md) |
| Registered focused cases and viewer qualification | [Conformance operations](tests/conformance/README.md) |
| Representative scenes, captures, comparisons and retained evidence | [Representative intake](tools/representative-intake/README.md) |
| Native/WASM snapshots and replay | [Snapshot operations](tests/conformance/coverage/README.md#native-snapshot-candidates) |
| PDF fixtures, structural checks and pinned viewers | [PDF harness](tools/pdf-harness/README.md) |

Python remains in CI/release tooling, legacy representative bindings and
approval recording, and selected research/audit tools; see the
[Python inventory and CI handoff](tools/python-inventory.md).

Completed evaluations and proposal migrations are historical research. Their
command transcripts are not additional contribution steps; see
[retired tooling](tools/historical-tooling.md). Older approval records and command
transcripts remain historical. Current verification checks outputs, input/profile
bindings, explicit tolerances and complete execution results directly.
`devenv.lock` pins the tools, and the harness images keep their digests.

## Release scope

0.1.0 prioritizes useful, tested scene/frame rendering with documented
limitations. Exhaustive coverage-ledger closure, additional formats, historical
revision retention are follow-up work. See
[the release policy](docs/release-0.1.0.md) and
[known limitations](docs/known-limitations.md).
