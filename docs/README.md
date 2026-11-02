# Documentation

## Guides

Start here if you want to render drawings.

- [Getting started](guides/getting-started.md): a tutorial that installs the
  command-line tool, renders a first drawing, and then does the same from Rust.
- [Command-line guide](guides/command-line.md): formats, sizing, appearance,
  pipes, images, metadata, reports and exit codes.
- [Rust library guide](guides/library.md): requests, reports, errors and
  images in the library.
- [Exporting frames](guides/frames.md): one file per frame, from the CLI and
  from Rust; slide decks.
- [Serving untrusted input](guides/untrusted-input.md): resource limits,
  blocking pools and process isolation for services.
- [Features and WebAssembly](guides/features-and-wasm.md): Cargo features,
  SVG-only builds and the browser.
- [Reproducible output](guides/reproducible-output.md): what makes output
  bytes change, and how to pin them.

The [examples](../crates/excalidraw-render/examples) are complete programs:
`render` (one scene to any format), `frames` (frames to files) and
`untrusted` (a bounded render with JSON errors, as a service would do it). Run
one with `cargo run --release --example render -- <scene> <output>`.

## Reference

- [API documentation](https://docs.rs/excalidraw-render) on docs.rs.
- `excalidraw-render --help` and `excalidraw-render frames --help` for every
  CLI option.
- [Known limitations](reference/known-limitations.md): what differs from
  Excalidraw, and why.
- [0.1.0 release policy](release-0.1.0.md): what this release promises.
- [Renderer specification](renderer-spec-v1.md): the normative rendering
  design and conformance target for compatibility revision 1.
- [Roadmap](roadmap.md): the performance and WebAssembly-footprint protocols
  that are specified but not yet active.
- [Glossary](../CONTEXT.md): the project's domain vocabulary.

## Development

For working on the renderer itself.

- [Development](development/README.md): building, the reference renderer, the
  font catalog, byte stability and the test tiers.
- [CI and evidence operations](development/ci.md): the gates, the golden
  record, release checks, scheduled runs and restaging.
- [Releasing](development/releasing.md): the release runbook, and the
  [first-release checklist](development/first-release.md) until 0.1.0 is out.
- [Conformance corpus](../tests/conformance/README.md): registered cases, the
  comparator, viewer qualification and coverage ledgers.
- Fuzzing: [codec and nested-SVG assets](development/asset-fuzzing.md) and
  [complete renders](development/render-fuzzing.md).
- [CSS tokenization](development/css-tokenization.md): the nested-SVG
  stylesheet compatibility policy.
- Tooling: the [reference oracle](../tools/reference-oracle/README.md), the
  [PDF harness](../tools/pdf-harness/README.md), the
  [pure-Rust SVG viewer](../tools/svg-viewer-rust/README.md),
  [representative intake](../tools/representative-intake/README.md) and the
  [side-by-side demo](../tools/demo/README.md).

## Design records

- [Architecture decision records](adr): why the project works the way it
  does, one decision per file.
- [Research notes](research): investigations that code, the specification or
  kept evidence still cite.
