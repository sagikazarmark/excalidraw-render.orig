# excalidraw-render

A pure-Rust library and command-line tool for rendering Excalidraw scene
documents to **SVG, PNG, and vector-first PDF**. Export a whole scene or each
frame separately. Fonts and other required rendering assets are bundled by default;
rendering needs no browser, Node.js, network access, or installed fonts.

**Status:** preparing the first `0.1.0` release. The intended public home is
[`sagikazarmark/excalidraw`](https://github.com/sagikazarmark/excalidraw).
The compatibility target is Excalidraw **0.18.1**, with
[known limitations](docs/known-limitations.md). This is an initial release,
not a claim of complete Excalidraw conformance.

## Install

Build from this checkout with Rust 1.97 or newer (release checks use 1.97.1):

```sh
cargo build --locked --release -p excalidraw-render-cli
target/release/excalidraw-render --help
```

After 0.1.0 is published, install the CLI from crates.io:

```sh
cargo install --locked excalidraw-render-cli --version 0.1.0
```

The initial prebuilt archive targets Linux x86-64 with glibc. Other native
platforms are not release-qualified. Browser-hosted WASM is tested but
experimental; no JavaScript bindings or standalone WASM package are shipped.

## Render

```sh
excalidraw-render drawing.excalidraw -o drawing.svg
excalidraw-render drawing.excalidraw -o drawing.png --scale 2
excalidraw-render drawing.excalidraw -o drawing.pdf
excalidraw-render frames drawing.excalidraw --list
excalidraw-render frames drawing.excalidraw -o frames --format png
```

The output extension selects the format, or use `--format`. Scene input and
output may be `-` for standard input/output. For large drawings, use
`--max-dimension 4096` to bound PNG/PDF dimensions. Compatibility diagnostics
describe substitutions, omissions, and fallbacks; use `--report-format json`
for machine-readable reporting. See `--help` for all options.

## Rust library

After publication, add `excalidraw-render = "=0.1.0"` to your dependencies.
The library exposes `SceneDocument`, `Renderer`, `RenderRequest`, and the frame
export API. It returns artifact bytes together with a compatibility report.
Generate the API documentation locally with:

```sh
cargo doc --no-deps -p excalidraw-render --open
```

Rendering dependencies are pinned to exact versions: dependency updates can
change output bytes. These pins can constrain a consuming application's
dependency resolution. Pin the renderer version and retain your lockfile when
reproducible output matters; later 0.x releases may intentionally change
rendering and diagnostics. Revision `1` identifies the current rendering
profile, not a promise to preserve every historical output in future releases.

### Optional CJK fonts

The default `cjk` feature bundles Xiaolai, Excalifont's Chinese/Japanese/Korean
fallback. Applications that do not need it can omit its roughly 12 MiB payload:

```toml
[dependencies]
excalidraw-render = { version = "=0.1.0", default-features = false }
```

Every other supported font, including Noto Emoji, remains bundled. A render that
needs to consult or embed a Xiaolai chunk returns `render.font-unavailable`, with
category `capability` and `ErrorDetails::FontUnavailable` identifying the family
and asset. It does not silently substitute another font. This follows actual
font demand, including SVG font embedding; it does not reject every scene that
contains CJK characters. The CLI retains the complete default bundle.

`excalidraw-fonts` holds the catalog and non-Xiaolai fonts. Its default-enabled
`cjk` feature includes `excalidraw-fonts-cjk-000` and `excalidraw-fonts-cjk-001`,
two balanced storage shards of the original Xiaolai chunks. These are packaging
boundaries, not language subsets. Cargo features are additive: another dependency
can enable CJK again. Disabling it removes the shards from the active build graph,
not necessarily from the lockfile or a `cargo fetch` download.

## Testing and contributing

```sh
cargo nextest run --locked --workspace --exclude xtask --lib --bins --profile ci-fast  # unit feedback
cargo nextest run --locked --workspace                    # includes parity tests
dagger --engine cloud check                              # canonical CI
```

Use Rust 1.97.1 for the full suite. Renderer/CLI tests use checked-in reference
evidence and need no browser or container runtime. Workspace `xtask` tests also
need Node and pinned qpdf for
evidence replay; Dagger installs them and runs the packaging and browser checks too.
A successful Dagger run is green CI; GitHub Actions is an optional caller.

For fresh PDF harness and representative-intake work, use the current Rust
commands in the [PDF harness guide](tools/pdf-harness/README.md) and
[representative-intake guide](tools/representative-intake/README.md). Python
remains in CI/release tooling, legacy representative bindings and approval
recording, and selected research/audit tools; see the
[Python inventory and CI handoff](tools/python-inventory.md). Archived commands
for deleted diagnostic and requalification drivers are historical research.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the test tiers and expected-output
review, [development operations](docs/development.md) for detailed commands,
and the [release policy](docs/release-0.1.0.md) for the 0.1.0 scope.

## License

Project code is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE). Bundled assets have their own licenses, collected
in the [asset license bundle](crates/excalidraw-fonts/LICENSE-BUNDLE.txt).
Exports carry the attribution required by the bundled resources they use.
