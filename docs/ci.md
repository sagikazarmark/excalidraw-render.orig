# CI and evidence operations

Acceptance uses Git/PR review and reproducible tests for all evidence families
(ADR-0019). Older approval-subject and approval-recording examples are historical;
those commands are retired. CI retains input/output integrity checks, exact
tolerances, target completeness and fresh replay.

## The gates

Every gate is a recipe in the [justfile](../justfile), run in the devenv shell
(ADR-0024). `devenv.lock` pins every tool the recipes use: the Rust toolchain,
nextest, qpdf 12.3.2, Firefox and geckodriver, and the wasm-bindgen runner at the
`wasm-bindgen` version in `Cargo.lock`. A laptop and CI run the same recipes
with the same tools; GitHub Actions is a caller, not the authority.

| Recipe | When | What it establishes |
| --- | --- | --- |
| `just ci` | Every change | Formatting, lints and the feature matrix, every test tier, the conformance verifiers and the packaged consumers |
| `just msrv` | Every change | The published crates build on the workspace `rust-version`; run it as `devenv --option languages.rust.version:string 1.92.0 shell -- just msrv` |
| `just release` | Release | Native and browser-WASM qualification, packaged consumers in WASM, native and WASM snapshot replay, traces and full equality |
| `just target-evidence` | Release | Both targets' qualification reports and their aggregate, from a clean commit |
| `just nightly-*` | Scheduled | The bounded randomized expansions |

`just` lists every recipe. The per-change gates run on any host the devenv shell
supports; CI runs each as its own job (`.github/workflows/ci.yaml`). Cargo's own
fingerprints decide what a recipe rebuilds.

The release checks run on the supported targets (ADR-0006). Browser WASM runs in
headless Firefox on any host: the renderer makes no browser calls, and the
browser build cannot change artifact bytes. Native evidence needs
`x86_64-unknown-linux-gnu`; its recipes refuse other hosts. A run on another host
is a check, not target evidence.

The [release policy](release-0.1.0.md) and [runbook](../release.md) define the
initial scope. Experimental-WASM checks execute every key but accept only the
exact #178 differences in `tests/package-consumer/experimental-wasm.json`.
`target/debug/xtask conformance verify-joined-results target/release-evidence/results`
after `just full-equality` retains the strict qualification goal.
Representative evidence uses fresh replay against approved outputs with
historical source provenance (ADR-0017), so source-only edits do not require
another numbered case. Existing accepted evidence is not rewritten.

The release checks write their logs, snapshots and reports under
`target/release-evidence`. Trace collection stages its instrumentation in a Git
worktree of the current tree under `target/trace-source`, with its own Cargo
target directory, so the staged sources never touch the checkout.

## What the evidence is

| Operation | What it establishes | Entry point |
| --- | --- | --- |
| Per-change gates | Formatting, lints, workspace tests, approved-file digests and packaged consumers | `just ci` |
| Release checks | Native and WASM qualification, snapshot replay and full equality | `just release` |
| PDF harness gates | PDFium/PDF.js rendering and qpdf structure tests | `dagger api call evidence pdf` |
| Reference profile | Pinned official Excalidraw wrapper, assets, environment and smoke test | `dagger api call evidence reference --mode verify` |
| Registered case | Expected reference SVG/PNG output for one scene | `dagger api call evidence case --case-id focused.rectangle-outline.v1` |
| Viewer qualification | Reproducibility and correctness of the tools used to judge rendered output | `dagger api call evidence viewers --kind svg` (also `pdf`, `font-subset`) |
| Font corpus | Per-cell generated WOFF2 byte pins and packaged corpus copy | `dagger api call evidence font-corpus` |
| Target aggregate | Both supported targets agree on generated font bytes and resource traces | `just target-evidence` |
| Catalog / manifests | Derived catalog records and evidence bindings | `dagger api call evidence xtask --args catalog,materialize` |

The reference oracle and the viewer harnesses stay in the Dagger module
(`dagger.dang`, `dagger-evidence.dang`, `dagger-oracle.dang`): they run
approval-pinned Docker images, and Dagger executes them verbatim in a private
nested Docker daemon, preserving their exact image digests, bind mounts and
`--network none` rendering. This needs a Dagger engine that permits
`insecureRootCapabilities`; it never mounts the host Docker socket. Acquisition
can use the network. The devenv shell carries the Dagger CLI.

Directory-returning calls take `--output PATH`. Use a staging directory, inspect
the output, then copy the intended generated files into the checkout. For example:

```sh
dagger api call evidence reference --mode verify --output target/dagger-reference
dagger api call evidence viewers --kind font-subset --output target/dagger-viewers
dagger api call evidence font-corpus --output target/dagger-corpus
```

`font-corpus` returns the staged JSON and digest sidecar, not an edited checkout.
The corpus JSON also belongs at
`crates/excalidraw-render/tests/qualification/font-subset-corpus.json`.
Reference calls accept `--frame` for the frame profile. Case and viewer calls
default to `reproduce`; use `--mode generate` for explicit generation. Reference
profile changes use `--mode update`. Existing generators retain their refusal
to overwrite manifested evidence. The `xtask` entry point accepts its ordinary
argument list, including materialization and approval-recording commands.

## Target evidence and provenance

After committing the source to be qualified, on Linux x86-64:

```sh
just target-evidence
```

This runs the release checks on both supported targets, captures complete logs,
materializes one JSON report per target under
`target/release-evidence/<target>/` and runs `aggregate-svg-font-subset-targets`
into `target/release-evidence/aggregate.json`. Failed tests cannot be hidden by
log capture. The aggregate rejects missing targets, mismatched
identities/measurements, and drift from the committed cross-target measurement.
Exported evidence requires a clean tracked and untracked source tree; the other
recipes support edits.

Record the tested commit, `devenv.lock`, command and output digests in a
maintainer attestation comment on the originating issue. The ADR-0006
provenance format accepts that comment without an Actions run URL. To bind the
aggregate to release evidence, run:

```sh
cargo run --locked -p xtask -- conformance record-svg-font-subset-target-aggregate \
  target/release-evidence/aggregate.json \
  https://github.com/sagikazarmark/excalidraw-render.orig/issues/ISSUE#issuecomment-COMMENT
```

The recorder still accepts only URLs under the development repository's
pre-rename name, `excalidraw-render.orig`, which the recorded aggregate's
provenance uses; GitHub redirects those URLs to `excalidraw-render`. Use that
spelling until the recorder learns the new name.

This recording step also checks Git ancestry. Keep it in the checkout carrying
the exported report and history. `pending_ci` is the existing manifest spelling
for “awaiting a supported-target aggregate”; it does not mean “awaiting Actions”.
Generating outputs does not grant human approval or change publication policy.

The release manifest's workflow and runner descriptors record how the retained
qualification was produced (the Dagger module at the time). They are historical
provenance: the verifier checks their form, not today's files, so retiring a
runner file leaves the recorded evidence valid.

## Completed single refresh (ADR-0003)

The brotli, ttf2woff2 and skera forks have all retired. The serialized refresh
is complete: all three tokens name their upstream implementations (brotli 9.0.0,
ttf2woff2 0.13.3, skera 0.7.0), the spec and WASM logging are updated, and the
regenerated catalog/profile, 977 font pins and 857 viewer executions are approved.
Dagger's clean-commit aggregate on `3f06c91` is bound into release evidence as
`aggregated` / `pass`. The round's publication block and evidence freeze are
lifted. See [the refresh record](font-identity-refresh.md) for approvals,
measurements and run provenance.

## Coverage, snapshots and traces

The shared #85/#86 draft inventory is checked by `just conformance`
(`conformance verify-inventory`). See
[the inventory/result contract](../tests/conformance/coverage/README.md).
Its consistency check does not grant release closure: `conformance verify-closure`
rejects the draft until ownership and canonical snapshot evidence are complete.
The adjacent global ledger is checked by `conformance verify-ownership`; its 928
obligations have 777 proposed owners and 151 explicit gaps. See
[ownership semantics](../tests/conformance/coverage/OWNERSHIP.md). These counts
describe assignments, not an approved coverage percentage.

`just snapshot-replay` collects native snapshot candidates for the inventory's
168 keys into `target/release-evidence/native-snapshots`. It runs the packaged
public facade twice per key in separate forward/reverse processes, keeps
complete artifact/report bytes and package provenance for review, validates the
baseline and rerenders the current package against all canonical native
expectations. The initial #85 baseline is approved and its full replay passes;
see the coverage README for the approval record and exact workflow.
`just wasm-snapshot-replay` executes all 168 keys in browser WASM and compares
them with the approved native baseline. Its initial run exposed seven
mismatches; the ICO accounting fix resolves three, and four PNG mismatches
remain tracked in #178. See
[the investigation record](research/wasm-snapshot-mismatches.md). Its logs stay
in `target/release-evidence/wasm-snapshots`.

Test-only traces cover all 168 keys on both targets: `just trace-logs <target>`
collects them into `target/release-evidence/results/{native,wasm}`, and
`just full-equality` joins them to the packaged artifact/report runs and gates
the combined comparison. Initial collection found 30 differing trace keys.
Deterministic release-record tie breaks fix 27; the remaining three follow PNG
output lengths under #178. The public join has zero disagreements. See
[trace collection](research/trace-collection.md) for the distinction between
report export and a pass.

The PDF workflow only schedules Dagger calls. The nightly workflow runs the
`just nightly-*` expansions and the browser-WASM release checks. Target-evidence
export is manual.

Codec and nested-SVG mutation fuzzing runs through `just nightly-asset-fuzz`;
its PR smoke, seed matrix, resource envelope and failure-replay procedure are
documented in [asset fuzzing](asset-fuzzing.md).

## History: the Dagger runner

From 2026-09-11 to ADR-0024 the Dagger module was the canonical CI and
evidence runner, and the checks above were `dagger check` functions with the
same commands. Its records stay valid; the
[performance notes](ci-performance.md) describe that runner. Before retiring
it, its checks passed on the working tree based on `11c1901`, and its
end-to-end export passed on `2cb598e2b53fa87448bc5ebe8933dbf353501f78` in
27m46s: native qualification passed 358 tests (5 ignored), pinned Chromium
WASM qualification passed all 6 selected tests, and both reports agreed on 977
successful cells and 1204 traced cells, matching the committed cross-target
measurement at the time:

- Generated bytes: `bb4e7dcdc4793db511aa7d75e77d54bd6b739477cc6e064153f9609e02fa9283`.
- Resource traces: `79ea93cdb58f7f348a14df21358be0066360fde67aa3b0cbbd7c6556bccf90fa`.
- Exported `aggregate.json` SHA-256: `1eeb31f3e01b58bbb2133bfd213d872552cb77d661ddfebfcfabeeba21876beb`.
- Native report SHA-256: `234a2cf8cf7124fc2a62d6a591a13136a64d7edec0b1f0db4c32050da93a7121`.
- WASM report SHA-256: `cb2451fdd0027aef220cbb3e4701cbf06ad38447d1ef4511196a7eaaba73e1ad`.
