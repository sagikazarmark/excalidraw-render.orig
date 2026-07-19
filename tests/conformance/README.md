# Conformance corpus

This directory contains immutable, approved scene documents and reference
evidence used to judge format-specific parity. Ordinary verification is offline:

```sh
cargo run --locked -p xtask -- conformance verify focused.rectangle-outline.v1
```

Reference evidence is generated only in the pinned development reference
profile. Generation refuses to replace an approved case; reproduction writes no
corpus files:

```sh
cargo run --locked -p xtask -- conformance generate focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance reproduce focused.rectangle-outline.v1
```

The checked manifest is canonical JSON followed by one LF. Its adjacent digest
hashes the canonical bytes without that LF. The approval subject excludes only
the four-field approval object and binds the remaining manifest plus every
referenced scene, report, provenance, artifact, and raw-raster byte string.

## Visual comparator

`comparator-profiles/canonical-visual-v1.json` freezes the format-neutral
RGBA8 projection, Pixelmatch, SSIM, crop, and visual-budget protocol. Its
adjacent digest binds the profile; the profile in turn binds the unpublished
Rust implementation, workspace lockfile, adversarial vectors, and imported
Pixelmatch 7.2.0 fixtures. Verify all of them offline with:

```sh
cargo test --locked -p excalidraw-render-compare
```

## Canonical SVG viewers

The canonical Chromium and pure-Rust SVG viewer profiles are under
`viewer-profiles/`. Their first qualification uses the approved rectangle SVG
as both the reference input and a controlled candidate input, rendered in
independent fresh processes. This proves viewer determinism, isolation, raster
canonicalization, and comparator wiring before a renderer-owned candidate SVG
exists. Renderer conformance uses distinct candidate bytes.

Ordinary verification and comparison are offline:

```sh
cargo run --locked -p xtask -- conformance verify-svg-viewers
cargo test --locked -p excalidraw-render-compare --test svg_viewer_qualification
```

The development-only harness qualification and read-only reproduction commands
are:

```sh
bash tools/reference-oracle/run-svg-viewer.sh test
CARGO_TARGET_DIR=target/svg-viewer-rust cargo test --locked \
  --manifest-path tools/svg-viewer-rust/Cargo.toml --test viewer_cli
bash tools/svg-viewer-qualification.sh reproduce
```

The first case declares no text faces or image assets. Both viewers fail closed
on undeclared text and image payloads, local files, network resources, and
undeclared data URLs. Cases containing text or images supply a closed declaration
that binds embedded source identities to reviewed sfnt faces or canonical
in-memory image bytes; ambient fallback is never permitted.
