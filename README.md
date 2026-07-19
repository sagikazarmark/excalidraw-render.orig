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
cargo build --release
```

## Run

```text
excalidraw-render <input.excalidraw> -o <output.{svg|png|pdf}> [--scale <f32>] [--padding <f32>]
```

`--scale` affects PNG output only. Its default is `1.0`; the default padding is
`20.0` pixels on each side.

```sh
target/release/excalidraw-render tests/fixtures/basic-shapes.excalidraw -o basic.svg
target/release/excalidraw-render tests/fixtures/basic-shapes.excalidraw -o basic.png --scale 2
target/release/excalidraw-render tests/fixtures/basic-shapes.excalidraw -o basic.pdf
```

## Reference renderer

The pinned official Excalidraw browser renderer is development-only. Normal Rust
builds and tests do not need Node, a browser, Docker, or network access. See
[`tools/reference-oracle/README.md`](tools/reference-oracle/README.md) for the
opt-in profile verification workflow.
