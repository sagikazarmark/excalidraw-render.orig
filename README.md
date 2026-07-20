# excalidraw-render

This repository currently contains a small proof-of-concept pure-Rust CLI. Its
supported element set, defaults, and fallback behavior are not the product
contract.

The approved implementation target is the
[Excalidraw Static Renderer Specification](docs/renderer-spec-v1.md): a
self-contained pure-Rust renderer for whole-scene SVG, PNG, and vector-first PDF
with explicit compatibility diagnostics and no production browser or network
dependency.

## Build

```sh
cargo build --release -p excalidraw-render-cli
```

The workspace now separates the future supported Rust library package,
`excalidraw-render`, from the native `excalidraw-render-cli` package. They share
one lockstep version. The current CLI still uses proof-of-concept rendering
internals while the approved public library facade is implemented; those
internals are not the compatibility revision 1 contract.

## Run

```text
excalidraw-render <input.excalidraw> -o <output.{svg|png|pdf}> [--scale <f32>] [--padding <f32>]
```

`--scale` affects PNG output only. Its default is `1.0`; the default padding is
`20.0` pixels on each side.

```sh
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.svg
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.png --scale 2
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.pdf
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
cargo package --locked --workspace --exclude xtask --exclude excalidraw-render-compare
cargo test --locked --release --manifest-path tests/package-consumer/Cargo.toml --test public_facade
```

The target workflow executes this consumer on all four supported native
targets and in the pinned browser-hosted `wasm32-unknown-unknown` harness.
