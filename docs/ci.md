# CI and evidence operations

Acceptance uses Git/PR review and reproducible tests for all evidence families
(ADR-0019). Older approval-subject and approval-recording examples are historical;
those commands are retired. CI retains input/output and downloaded-tool integrity
checks, exact tolerances, target completeness and fresh replay.

## 0.1.0 preparation

Use Dagger `v1.0.0-beta.14`, with `dagger --engine cloud check` as the CI
authority. GitHub Actions is an optional caller and its billing-related failures
do not determine CI status. Beta.14 names checks relative to the entrypoint:
`test:fast`, `conformance:catalog`, `packaging:snapshot-replay`, etc.; discover
the current names with `dagger check --list`. Older prefixed commands below
describe historical runs.

The [release policy](release-0.1.0.md) and [runbook](../release.md) define the
initial scope. Experimental-WASM checks execute every key but accept only the
exact #178 differences in `tests/package-consumer/experimental-wasm.json`.
`dagger api call strict-full-equality` retains the strict qualification goal.
Representative evidence uses fresh replay against approved outputs with
historical source provenance (ADR-0017), so source-only edits do not require
another numbered case. Existing accepted evidence is not rewritten.

The Rust module's automatic Cargo cache mounts are disabled: their `LOCKED`
sharing holds registry locks for whole container commands. `dagger-builds.dang`
instead owns immutable, profile-specific cargo-chef 0.1.78 dependency layers.
Workspace recipes depend on manifests/locks, so ordinary source edits reuse
their compiled dependencies. Native workspace tests and the qualified release CLI are compiled
once; conformance runs the resulting `xtask` directly. Native and WASM consume
one shared set of package archives, and each target compiles its public-facade
and snapshot consumer together. Artifact/report comparisons remain unchanged.
The standalone consumers keep their separate dependency lock and release profile:
their builds are shared within each target, but do not reuse the workspace's
different-profile dependency objects. A dedicated consumer recipe cache is a
further optimization, not a reason to unify the consumer and workspace graphs.

Heavy real compilations have explicit dependencies: native workspace/CLI,
native release-library and trace builds, then WASM release-library and trace
builds. Each target's consumers wait for its trace build. Profile/target-specific
dependency cooks can run concurrently, but qualification, trace and consumer
checks no longer each start their own competing dependency compilation. Native
checks remain native-only. Staged trace code and unpacked consumer sources are
also compiled from narrow views before restoring runtime data.

Inputs have four views (`dagger api call ci-inputs --kind <view> --output PATH`):

- `format`: Rust source and Cargo/configuration files, without asset blobs,
  external fixtures, or Git history (about 8 MB in the initial measurement).
- `code`: workspace source and bundled assets.
- `compile`: code plus compile-time `include_bytes!`/`include_str!` fixtures
  (about 216 MB, versus the roughly 1 GiB full checkout).
- `runtime`: the full working tree and history, attached after compilation for
  evidence, ancestry, and source-binding checks.

`input-boundaries` rejects accidentally broad format/compile inputs. Real Cargo
builds check compile-input completeness. Cargo target output lives in `/target`,
outside the source mount; package archives retain real paths under
`/work/src/target/package` for the snapshot collector's checked path boundary.
Browser tools are installed from their own small input view before code/evidence
is attached. Dagger module loading includes only the Dang implementation files.
Harness scratch files live in the checkout's `target/tmp` independently of the
external Cargo compilation directory, retaining safe repository-relative paths.

For cache misses, start with the engine's available disk space, GC policy, and
pruning logs. Beta.14 can evict even fresh layers under free-space pressure;
[the measured diagnosis and bounded probes](ci-performance.md#local-cache-eviction-diagnosis-beta14)
distinguish that behavior from source-input invalidation.

`dagger.dang`, `dagger-evidence.dang`, `dagger.toml`, and `dagger.lock` are the
canonical CI implementation. GitHub Actions is an optional scheduler, caller,
and artifact store. A Dagger run on a laptop or another runner has the same
meaning; Actions availability is not a prerequisite for evidence.

## What the evidence is

| Operation | What it establishes | Dagger entry point |
| --- | --- | --- |
| Push checks | Formatting, lints, workspace tests, approved-file digests, native and WASM qualification and packaged consumers | `dagger check` |
| PDF harness gates | PDFium/PDF.js rendering and qpdf structure tests | `dagger api call evidence pdf` |
| Reference profile | Pinned official Excalidraw wrapper, assets, environment and smoke test | `dagger api call evidence reference --mode verify` |
| Registered case | Expected reference SVG/PNG output for one scene | `dagger api call evidence case --case-id focused.rectangle-outline.v1` |
| Viewer qualification | Reproducibility and correctness of the tools used to judge rendered output | `dagger api call evidence viewers --kind svg` (also `pdf`, `font-subset`) |
| Font corpus | Per-cell generated WOFF2 byte pins and packaged corpus copy | `dagger api call evidence font-corpus` |
| Target aggregate | Both supported targets agree on generated font bytes and resource traces | `dagger api call target-evidence` |
| Catalog / manifests | Derived catalog records and evidence bindings | `dagger api call evidence xtask --args catalog,materialize` |

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

The reference/viewer scripts are themselves approval-pinned. Dagger executes
them verbatim in a private nested Docker daemon, preserving their exact image
digests, bind mounts and `--network none` rendering. This needs a Dagger engine
that permits `insecureRootCapabilities`; it never mounts the host Docker socket.
Acquisition can use the network. Browser WASM uses its separate manifest-pinned
Chrome-for-Testing archive, not the reference profile's Playwright browser.

## Target evidence and provenance

After committing the source to be qualified:

```sh
dagger api call target-evidence --output target/dagger-evidence
```

This executes qualification and packaged consumers for both supported targets,
captures complete logs, materializes one JSON report per target and runs
`aggregate-svg-font-subset-targets`. Failed tests cannot be hidden by log capture.
The aggregate rejects missing targets, mismatched identities/measurements, and
drift from the committed cross-target measurement. Exported evidence requires a
clean tracked and untracked source tree; ordinary `dagger check` supports edits.

The output contains `native/`, `wasm/`, `reports/`, and `aggregate.json`.
Record the tested commit, Dagger lock, command, output digests and (when available)
Dagger trace URL in a maintainer attestation comment on the originating issue.
The existing ADR-0006 provenance format accepts that comment without an Actions
run URL. To bind the aggregate to release evidence, run:

```sh
cargo run --locked -p xtask -- conformance record-svg-font-subset-target-aggregate \
  target/dagger-evidence/aggregate.json \
  https://github.com/sagikazarmark/excalidraw-render.orig/issues/ISSUE#issuecomment-COMMENT
```

This recording step also checks Git ancestry. Keep it in the checkout carrying
the exported report and history. `pending_ci` is the existing manifest spelling
for “awaiting a supported-target aggregate”; it does not mean “awaiting Actions”.
Generating outputs does not grant human approval or change publication policy.

## Completed single refresh (ADR-0003)

The brotli, ttf2woff2 and skera forks have all retired. The serialized refresh
is complete: all three tokens name their upstream implementations (brotli 9.0.0,
ttf2woff2 0.13.3, skera 0.7.0), the spec and WASM logging are updated, and the
regenerated catalog/profile, 977 font pins and 857 viewer executions are approved.
Dagger's clean-commit aggregate on `3f06c91` is bound into release evidence as
`aggregated` / `pass`. The round's publication block and evidence freeze are
lifted. See [the refresh record](font-identity-refresh.md) for approvals,
measurements and run provenance.

The release manifest's orchestration descriptors bind
the Dagger implementation and inputs rather than the retired Actions job body.
Changing those files requires updating their SHA-256 descriptors; the verifier
checks them recursively. Viewer approvals and generated-byte pins are separate.

## Updating the runner

The shared #85/#86 draft inventory is checked by
`dagger check excalidraw-render:conformance:coverage-inventory` and staged with
`dagger api call evidence inventory --output target/inventory.json`.
See [the inventory/result contract](../tests/conformance/coverage/README.md).
Its consistency check does not grant release closure: `conformance verify-closure`
rejects the draft until ownership and canonical snapshot evidence are complete.
The adjacent global ledger is checked by
`dagger check excalidraw-render:conformance:coverage-ownership`; its 928 obligations
have 777 proposed owners and 151 explicit gaps. See
[ownership semantics](../tests/conformance/coverage/OWNERSHIP.md). These counts
describe assignments, not an approved coverage percentage.

Native snapshot candidates for the inventory's 168 keys can be exported with
`dagger api call packaging snapshots --output target/native-snapshots`. This runs
the packaged public facade twice per key in separate forward/reverse processes
and exports complete artifact/report bytes and package provenance for review.
`dagger check excalidraw-render:packaging:snapshot-replay` validates baseline
approval and rerenders the current package against all canonical native
expectations. The initial #85 baseline is approved and its full replay passes;
see the coverage README for the approval record and exact workflow.
The corresponding `excalidraw-render:wasm:snapshot-replay` check now executes all
168 keys in pinned browser WASM and compares with the approved native baseline.
Its initial run exposed seven mismatches; the ICO accounting fix resolves three,
and four PNG mismatches remain tracked in #178. See
[the investigation record](research/wasm-snapshot-mismatches.md). Export diagnostic
logs with `dagger api call wasm browser-snapshot-logs --output target/wasm-snapshot-logs`.

Test-only traces now cover all 168 keys on both targets:
`dagger api call traces --output target/trace-results`. `dagger api call full-results
--output target/full-results` joins these to actual packaged artifact/report runs;
`dagger check excalidraw-render:full-equality` gates the combined comparison.
Initial collection found 30 differing trace keys. Deterministic release-record
tie breaks fix 27; the remaining three follow PNG output lengths under #178.
The public join has zero disagreements. See [trace collection](research/trace-collection.md) for sequential
commands on small engines and the distinction between report export and a pass.

Recent `daggerverse-beta` changes at `b0671fd12a` add toolchain/MSRV overrides,
manifest-configured cargo-hack, automatic nextest plus doctests, cargo selection
flags for clippy, and the `cargoProfile` CLI collision fix. The module lock now
includes them. Repository-specific checks still select the exact nextest GNU
archives in `tests/package-consumer/tool-manifest.json`; the reusable Rust
module's automatic nextest uses a different (musl) archive.

After changing checks/configuration, regenerate the thin Actions adapter:

```sh
dagger generate dagger-gha:generate
dagger check dagger-gha:generate
```

The PDF/nightly workflows only schedule Dagger calls. Target-evidence export is
manual. Native and WASM qualification and
packaged-facade checks still run on every push/PR through the generated adapter.

Codec and nested-SVG mutation fuzzing runs through `dagger api call nightly
asset-fuzz`; its PR smoke, seed matrix, resource envelope and failure-replay
procedure are documented in [asset fuzzing](asset-fuzzing.md).

## Verification of this migration

The following passed through Dagger on the working tree based on `11c1901`:

- All 18 registered checks, run by group: conformance (6), lint (2), workspace
  test tiers/doctests (3), native packaging/qualification (3), WASM (3, including
  the native-only CLI no-op), and generated workflow freshness (1).
- All nine PDF harness tests.
- Scene reference-profile verification and rectangle-outline case reproduction.
- Font-corpus generation and dual-viewer SVG qualification reproduction.

The all-at-once check run exceeded this 8-GB runner's resources/time budget;
run the same checks in groups on a small engine:

```sh
for group in conformance lint test packaging wasm; do
  dagger check "excalidraw-render:$group" || exit
done
dagger check dagger-gha:generate
```

After committing the migration, the end-to-end export passed on
`2cb598e2b53fa87448bc5ebe8933dbf353501f78` with:

```sh
dagger api call target-evidence --output target/dagger-evidence
```

The run completed in 27m46s. Native qualification passed 358 tests (5 ignored),
and pinned Chromium WASM qualification passed all 6 selected tests, including
the scene-fuzz regression corpus. Packaged-consumer gates passed before export.
Both reports name that commit and agree on 977 successful cells and 1204 traced
cells, matching the committed cross-target measurement:

- Generated bytes: `bb4e7dcdc4793db511aa7d75e77d54bd6b739477cc6e064153f9609e02fa9283`.
- Resource traces: `79ea93cdb58f7f348a14df21358be0066360fde67aa3b0cbbd7c6556bccf90fa`.
- Exported `aggregate.json` SHA-256: `1eeb31f3e01b58bbb2133bfd213d872552cb77d661ddfebfcfabeeba21876beb`.
- Native report SHA-256: `234a2cf8cf7124fc2a62d6a591a13136a64d7edec0b1f0db4c32050da93a7121`.
- WASM report SHA-256: `cb2451fdd0027aef220cbb3e4701cbf06ad38447d1ef4511196a7eaaba73e1ad`.

This verified Dagger's aggregate path on the pre-refresh evidence. The subsequent
refreshed aggregate and release-evidence binding are recorded above.
The font-subset viewer round, PDF viewer evidence reproduction, and frame-profile update were not
rerun for this migration; their Dagger entry points call the existing pinned
scripts. The font-subset viewer round subsequently passed in full during the
completed identity refresh; see its record above.
