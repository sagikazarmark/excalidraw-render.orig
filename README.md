# excalidraw-render

This repository contains a pure-Rust rendering library and native CLI targeting
compatibility revision 1 of the approved static-renderer contract.

The approved implementation target is the
[Excalidraw Static Renderer Specification](docs/renderer-spec-v1.md): a
self-contained pure-Rust renderer for scene and frame SVG, PNG, and vector-first PDF
with explicit compatibility diagnostics and no production browser or network
dependency.

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

```sh
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.svg
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.png --scale 2
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.pdf
target/release/excalidraw-render frames drawing.excalidraw -o frames --format png --name-regex 'Slide'
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
