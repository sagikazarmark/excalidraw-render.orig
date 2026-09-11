# CI and evidence operations

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

## Pending single refresh (ADR-0003)

The brotli, ttf2woff2 and skera forks have all retired. The remaining serialized
refresh updates **three** historical profile tokens (brotli 9.0.0, ttf2woff2
0.13.3, skera 0.7.0), the relevant spec versions, and the WASM logging call site.
It then regenerates the catalog/profile, 977 per-cell font pins, cross-target
measurement, and dual-viewer font qualification, and obtains the required
re-approvals. Other evidence-touching work freezes during that round.

Dagger supplies the execution environment for that round. The aggregate's first
release-evidence binding belongs to the refreshed evidence, as ADR-0003 requires.
The CI migration itself does not refresh those tokens or record approval.
The unsigned pending release manifest's orchestration descriptors now bind
the Dagger implementation and inputs rather than the retired Actions job body.
Changing those files requires updating their SHA-256 descriptors; the verifier
checks them recursively. Viewer approvals and generated-byte pins are separate.

## Updating the runner

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
manual until the single refresh is ready. Native and WASM qualification and
packaged-facade checks still run on every push/PR through the generated adapter.

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

The aggregate export requires the migration to be committed first and was not
executed end to end on the uncommitted working tree. The font-subset viewer
round, PDF viewer evidence reproduction, and frame-profile update were not
rerun for this migration; their Dagger entry points call the existing pinned
scripts. The serialized font identity refresh and its approvals remain pending.
