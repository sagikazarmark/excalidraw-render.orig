# Pure-Rust SVG viewer

This nested, unpublished test workspace pins the canonical pure-Rust SVG viewer
separately from production renderer dependencies. It uses `resvg` and `usvg`
0.47.0 with only reviewed text and raster-image features, explicit rendering
options, no resource directory, no system or memory-mapped fonts, and custom
resolvers that admit only catalog-bound embedded resources. WOFF2 identities map
to reviewed owned sfnt faces; raster identities map to reviewed canonical PNG;
sanitized nested SVG remains SVG.

Run its process-level contract tests with:

```sh
CARGO_TARGET_DIR=target/svg-viewer-rust cargo test --locked \
  --manifest-path tools/svg-viewer-rust/Cargo.toml --test viewer_cli
```

Render one repository SVG to tightly packed straight-alpha sRGB RGBA8 with:

```sh
CARGO_TARGET_DIR=target/svg-viewer-rust cargo run --locked --release \
  --manifest-path tools/svg-viewer-rust/Cargo.toml -- \
  render input.svg output.rgba [assets.json]
```

The checked profile is
`tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json`. Text and
image support is enabled only through a closed per-case declaration; preload and
ambient fallback remain forbidden.
