# excalidraw-render

A small, pure-Rust CLI that renders Excalidraw rectangles, ellipses, diamonds,
lines, and arrows to SVG, PNG, or PDF. Unsupported and deleted elements are
ignored.

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

## Reference oracle

The pinned official Excalidraw browser oracle is development-only. Normal Rust
builds and tests do not need Node, a browser, Docker, or network access. See
[`tools/reference-oracle/README.md`](tools/reference-oracle/README.md) for the
opt-in profile verification workflow.
