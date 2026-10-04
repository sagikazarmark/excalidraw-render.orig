# excalidraw-render-cli

A command-line tool that renders [Excalidraw](https://excalidraw.com) drawings
to **SVG, PNG, and vector-first PDF**, for a whole scene or frame by frame. It
is a single native binary with every font bundled: no browser, Node.js,
network access, or installed fonts.

The compatibility target is Excalidraw **0.18.1**. Output is close to, but not
pixel-identical with, the browser exporter; see the
[known limitations](../../docs/known-limitations.md).

## Install

```sh
cargo install --locked excalidraw-render-cli
```

This installs the `excalidraw-render` binary (Rust 1.92 or newer). Each GitHub
release also carries a prebuilt Linux x86-64 (glibc) archive with a SHA-256
checksum.

## Usage

```sh
excalidraw-render drawing.excalidraw -o drawing.svg
excalidraw-render drawing.excalidraw -o drawing.png --scale 2
excalidraw-render drawing.excalidraw -o drawing.pdf
excalidraw-render frames drawing.excalidraw --list
excalidraw-render frames drawing.excalidraw -o frames --format png
```

The output extension selects the format, or use `--format`. Input and output
may be `-` for standard input and output. An existing output is kept unless you
pass `--force`. For large drawings, `--max-dimension 4096` bounds the PNG/PDF
size.

Warnings describe substitutions, omissions, and fallbacks the renderer made;
`--report-format json` prints a machine-readable report. Run
`excalidraw-render --help` and `excalidraw-render frames --help` for every
option.

## License

Code is licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option. The binary bundles fonts and
adapted third-party code under their own licenses, collected in
[`LICENSE-BUNDLE.txt`](LICENSE-BUNDLE.txt).
