# CI and evidence operations

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

`just` lists every recipe. The per-change gates are `fmt`, `clippy`,
`features`, `msrv`, the `test`, `test-render`, `test-features` and `test-doc`
tiers, `conformance` and `packaging`. They run on any host the devenv shell
supports; CI runs each as its own job (`.github/workflows/ci.yaml`), except that
the fast and render tiers share one job and one compilation. Cargo's own
fingerprints decide what a recipe rebuilds; rust-cache keeps the dependencies
between runs, saved from `main`. Each job pulls the devenv shell from the
`rust-projects` Cachix cache and pushes the paths it builds, chiefly the
rust-overlay toolchain that cache.nixos.org does not carry.

Some xtask tests run commands that build the qualified release CLI (LTO, one
codegen unit) on first use. `.config/nextest.toml` puts them in a one-thread
test group, so they take turns instead of holding test slots while they wait on
Cargo's build lock.

The release checks run on the supported targets (ADR-0006). Browser WASM runs in
headless Firefox through geckodriver on any host: the renderer makes no browser
calls, and the browser build cannot change artifact bytes. Native evidence needs
`x86_64-unknown-linux-gnu`; its recipes refuse other hosts. A run on another host
is a check, not target evidence.

```sh
just release                          # everything below
just qualification wasm32-unknown-unknown
just wasm-packaging                   # packaged facade and no-CJK consumer in WASM
just wasm-snapshot-replay
just snapshot-replay                  # native; Linux x86-64
just full-equality                    # traces and the joined comparison
```

The nightly expansions take a case count:

```sh
just nightly-font-subset-fuzz
just nightly-frame-batch-property 64
just nightly-scene-fuzz 512
```

The [release policy](release-0.1.0.md) and [runbook](../release.md) define the
initial scope. Experimental-WASM checks execute every key but accept only the
exact #178 differences in `tests/package-consumer/experimental-wasm.json`.
`target/debug/xtask conformance verify-joined-results target/release-evidence/results`
after `just full-equality` retains the strict qualification goal.
Representative evidence uses fresh replay against expected outputs with
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
| Viewer qualification | Reproducibility and correctness of the tools used to judge rendered output | `just viewer-evidence svg` (also `pdf`, `font-subset`; `generate` as the second argument regenerates) |
| Font corpus | Per-cell generated WOFF2 byte pins and packaged corpus copy | `dagger api call evidence font-corpus` |
| Native baseline | The canonical native artifact and report bytes for the inventory's 168 keys | `just stage-native-baseline` |
| Representative candidate | A numbered successor of the stage-4 representative case, collected from its frozen authoring | `just candidate-successor tests/conformance/candidates/<directory>` |
| Target aggregate | Both supported targets agree on generated font bytes and resource traces | `just target-evidence` |
| Catalog / manifests | Derived catalog records and evidence bindings | `dagger api call evidence xtask --args catalog,materialize` |

The viewer scripts (`tools/svg-font-subset-qualification.sh`,
`tools/svg-viewer-qualification.sh`, `tools/pdf-viewer-qualification.sh`) and
the candidate viewers run approval-pinned Docker images by digest, bind-mount
the checkout and render with `--network none`. `just viewer-evidence` and
`just candidate-successor` acquire what the scripts need first (the oracle's
`npm ci`, the PDF harness, the pure-Rust viewer's crates) and then run them
verbatim against the host's Docker. The reference oracle stays in the Dagger
module (`dagger.dang`, `dagger-evidence.dang`, `dagger-oracle.dang`), which
runs the same scripts in a private nested Docker daemon for hosts without a
suitable Docker; it needs a Dagger engine that permits
`insecureRootCapabilities` and never mounts the host Docker socket.
Acquisition can use the network. The devenv shell carries the Dagger CLI.

Directory-returning calls take `--output PATH`. Use a staging directory, inspect
the output, then copy the intended generated files into the checkout. For example:

```sh
dagger api call evidence reference --mode verify --output target/dagger-reference
dagger api call evidence font-corpus --output target/dagger-corpus
```

`font-corpus` returns the staged JSON and digest sidecar, not an edited checkout.
The corpus JSON also belongs at
`crates/excalidraw-render/tests/qualification/font-subset-corpus.json`.
Reference calls accept `--frame` for the frame profile. Case calls default to
`reproduce`; use `--mode generate` for explicit generation. Reference profile
changes use `--mode update`. Existing generators retain their refusal
to overwrite manifested evidence. The `xtask` entry point accepts its ordinary
argument list, including the materialization commands.

## Target evidence and provenance

After committing the source to be qualified, run `release-evidence.yaml` from
that commit, or on Linux x86-64:

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

To bind the aggregate to release evidence, download the workflow's
`target-evidence` artifact and record it with the run as its provenance:

```sh
cargo run --locked -p xtask -- conformance record-svg-font-subset-target-aggregate \
  target/release-evidence/aggregate.json \
  https://github.com/sagikazarmark/excalidraw-render.orig/actions/runs/RUN
```

When the aggregate was produced outside Actions, record the tested commit,
`devenv.lock`, command and output digests in a maintainer attestation comment
on the originating issue instead, and pass that comment's URL; the ADR-0006
provenance format accepts either.

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

## Coverage, snapshots and traces

The shared #85/#86 inventory is checked by `just conformance`
(`conformance verify-inventory`). See
[the inventory/result contract](../tests/conformance/coverage/README.md).
Its consistency check does not grant release closure: `conformance verify-closure`
rejects the inventory until ownership and canonical snapshot evidence are complete.
The adjacent global ledger is checked by `conformance verify-ownership`; see
[ownership semantics](../tests/conformance/coverage/OWNERSHIP.md). Its counts
describe assignments, not an approved coverage percentage.

`just snapshot-replay` collects native snapshot candidates for the inventory's
168 keys into `target/release-evidence/native-snapshots`. It runs the packaged
public facade twice per key in separate forward/reverse processes, keeps
complete artifact/report bytes and package provenance for review, validates the
baseline and rerenders the current package against all canonical native
expectations (`verify-native-baseline`); see the coverage README for the exact
workflow. `just wasm-snapshot-replay` executes all 168 keys in browser WASM and
compares them with the native baseline. Four PNG keys differ by the tiny-skia
rounding tracked in #178 and are accepted exactly as recorded in
`tests/package-consumer/experimental-wasm.json`; see
[the investigation record](research/wasm-snapshot-mismatches.md). Its logs stay
in `target/release-evidence/wasm-snapshots`.

Test-only traces cover all 168 keys on both targets: `just trace-logs <target>`
collects them into `target/release-evidence/results/{native,wasm}`, and
`just full-equality` joins them to the packaged artifact/report runs and gates
the combined comparison. Three WASM trace keys follow the #178 PNG output
lengths; the public join has zero disagreements. See
[trace collection](research/trace-collection.md) for the distinction between
report export and a pass.

GitHub Actions runs only recipes in the devenv shell: `ci.yaml` the per-change
gates, `nightly.yaml` the `just nightly-*` expansions and the browser-WASM
release checks, `release-evidence.yaml`, on demand, `just target-evidence`
with its output as an artifact, and `restage.yaml`, on demand, the restaging
recipes below. The PDF harness gates are not scheduled; run them with
`dagger api call evidence pdf`.

## Restaging

A change that moves approved evidence (a dependency that changes output bytes,
a subset profile token, a catalog descriptor) stales several records at once.
The restaging recipes regenerate them on `x86_64-unknown-linux-gnu` and write
candidates for review; none edits a manifest or the registry, and
`restage.yaml` runs them with `contents: read` and exports the results as
artifacts. Adopt each in a commit, then run the verifiers:

| Recipe | Writes | Adopt by |
| --- | --- | --- |
| `just stage-native-baseline` | `target/release-evidence/native-baseline` from a clean commit | replacing `tests/conformance/coverage/native-baseline`; `verify-native-baseline`, `verify-pdf-judgments`, `verify-ownership` |
| `just viewer-evidence font-subset generate` | the four files under `tests/conformance/svg-font-subset-viewer-qualification` (delete `revision-1.json` first; generation refuses to replace it) | `verify-svg-font-subsets` |
| `just candidate-successor tests/conformance/candidates/<dir>` | the successor directory through `candidate-materialize` | pointing `registry.json` at it, `record-representative-owner`, `verify-ownership`, `candidate-executions` |

The baseline job runs nothing before the collector: it records the tree's
status, and staging refuses a dirty tree, so the other two recipes (which
write into the checkout) belong in their own jobs. The candidate recipe
pre-builds the pure-Rust viewer and the PDFium worker because the views
script gives each viewer run 180 seconds. After adopting, set the release
manifest back to `pending_target_aggregation`, remove the stale target
aggregate, and run `release-evidence.yaml` from the resulting commit to
record a new one (above).

Codec and nested-SVG mutation fuzzing runs through `just nightly-asset-fuzz`,
and complete-render fuzzing through `just nightly-render-fuzz`; their PR smoke,
seed matrix, resource envelope and failure-replay procedure are documented in
[asset fuzzing](asset-fuzzing.md) and [render fuzzing](render-fuzzing.md).
