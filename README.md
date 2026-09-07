# excalidraw-render

This repository contains a pure-Rust rendering library and native CLI targeting
compatibility revision 1 of the approved static-renderer contract.

The approved implementation target is the
[Excalidraw Static Renderer Specification](docs/renderer-spec-v1.md): a
self-contained pure-Rust renderer for scene and frame SVG, PNG, and vector-first PDF
with explicit compatibility diagnostics and no production browser or network
dependency.

The performance and canonical WASM footprint protocols are specified but not yet
activated; they live in [docs/roadmap.md](docs/roadmap.md) and gate nothing today.

## Build

```sh
cargo build --release -p excalidraw-render-cli
```

The workspace separates the supported `excalidraw-render` Rust library from the
native `excalidraw-render-cli` package. They share one lockstep version, and the
CLI is a thin adapter over the public library facade.

## Run

```text
excalidraw-render [OPTIONS] <INPUT> --output <OUTPUT>
excalidraw-render frames [OPTIONS] <INPUT> --output <DIRECTORY> --format <svg|png|pdf>
excalidraw-render frames [OPTIONS] <INPUT> --list
```

For scene export, `INPUT` and `OUTPUT` may be `-` for standard input and artifact
standard output. Frame batches accept standard input but require an output
directory. The scene output format is inferred from a recognized path extension
or selected with `--format`; the default padding is 10 logical pixels. Run
`excalidraw-render --help` for the complete request and reporting controls.
For large scenes, prefer `--max-dimension <value>` for PNG and PDF output;
the default scale of 1 preserves the full logical dimensions and may exceed the
raster-surface or PDF page-size limits.

The `frames` command writes one zero-padding artifact per selected live ordinary
or magic frame. It uses readable frame-name basenames, falls back to the stable
source-order name `frame-N`, and resolves unsafe or duplicate names
deterministically. `--name-regex` filters nonempty source frame names. Batch
publication is stage-all by default; `--allow-partial` publishes independent
successes while retaining a nonzero exit status for any failed frame.

`--list` is a dry run: it reports the ordinal, kind, name, and derived basename
of every frame the same selection would export, and writes nothing. `--output`
and `--format` are required only outside `--list`.

```sh
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.svg
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.png --scale 2
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.pdf
target/release/excalidraw-render frames drawing.excalidraw -o frames --format png --name-regex 'Slide'
target/release/excalidraw-render frames drawing.excalidraw --list --report-format json
```

## Reference renderer

The pinned official Excalidraw browser renderer is development-only. Normal Rust
builds and tests do not need Node, a browser, Docker, or network access. See
[`tools/reference-oracle/README.md`](tools/reference-oracle/README.md) for the
opt-in profile verification workflow.

## Revision catalog

Compatibility revision 1 incorporates its approved manifests, license bundle,
and official rendering-font bytes into a sealed catalog. Ordinary verification
is offline and read-only:

```sh
cargo run --locked -p xtask -- catalog verify
```

Maintainers can reproduce the checked asset shards from an explicit local
`@excalidraw/excalidraw` `dist/prod` directory. The importer verifies every
source byte against the approved reference asset manifest and never downloads
or generates production data during a build:

```sh
cargo run --locked -p xtask -- catalog import <local-dist-prod>
```

The catalog also records every official font chunk's reviewed decode, the sfnt
the pinned `wuff 0.2.8` decode of its exact bytes produces and the only bytes
PDF may embed for it (spec 8.6). `catalog verify` re-derives all 230 decodes
and requires the maintainer approval that binds them; `import` and
`materialize` regenerate the record and clear its approval when the decodes
move. The approval round follows the other evidence families:

```sh
cargo run --locked -p xtask -- catalog reviewed-decodes-approval-subject
cargo run --locked -p xtask -- catalog record-reviewed-decodes-approval <login> <date> <issue-comment-url> <subject>
```

## Artifact byte stability

`tests/package-consumer` depends on Cargo's unpacked package output rather than
the workspace crate. It renders the approved rectangle through the public Rust
facade to SVG, PNG, and PDF twice and compares complete artifact and report
bytes with the checked native expectations.

```sh
cargo package --locked --workspace --exclude xtask --exclude excalidraw-render-compare --no-verify
bash tests/package-consumer/check-packaged-cli.sh x86_64-unknown-linux-gnu
cargo test --locked --release --manifest-path tests/package-consumer/Cargo.toml --test public_facade
```

The target workflow executes this consumer on all four supported native
targets and in the pinned browser-hosted `wasm32-unknown-unknown` harness.

## Containerized checks

Every gate the Linux workflows run is also a Dagger check, so one command
reproduces the whole set locally, in the same pinned container CI uses:

```sh
dagger check
```

The devenv shell carries the pinned Dagger CLI (`dagger.enable` in
`devenv.nix`); the engine needs a working container runtime and nothing else.
`dagger check --list` names every check, and a selector runs a subset:

```sh
dagger check --list
dagger check excalidraw-render:lint
dagger check excalidraw-render:test:workspace
```

The checks mirror the workflow jobs one for one: `lint` is the `conformance`
workflow's formatting and lint gate, `test` its unit, parity, and
whole-workspace targets, `conformance` the catalog and approval-bearing
evidence verification the `conformance` and `Packaged public facade` workflows
share, and `packaging` the Linux leg of the packaged public facade. The
randomized expansions the nightly schedule runs are deliberately not checks:

```sh
dagger call nightly font-subset-fuzz
dagger call nightly frame-batch-property --cases 64
```

`.github/workflows/dagger.yaml` is generated from `dagger.toml`. The
`dagger-gha:generate` check fails when the committed file drifts from the
configuration; regenerate it with:

```sh
dagger generate dagger-gha:generate
```

`dagger.lock` pins the module revisions and the base image digest, so a check
run reproduces byte for byte until the lock moves.

What stays on GitHub Actions: the macOS and Windows legs of the packaged public
facade, and the browser-hosted `wasm32-unknown-unknown` leg. The viewer and PDF
harness tests remain `#[ignore]`d in every environment — they still need the
pinned Chromium, PDFium, PDF.js, and qpdf harnesses.
