# Changelog

## 0.1.0 — Unreleased

Initial release candidate of the pure-Rust Excalidraw rendering library and CLI.

### Features

- Whole-scene and per-frame SVG, PNG, and vector-first PDF exports.
- Bundled rendering fonts and assets; no browser or network needed at runtime.
- Explicit compatibility diagnostics, resource limits, and deterministic output
  in the qualified environment.
- CLI frame listing, frame-name filtering, sizing, and JSON reporting.

### Compatibility

- Targets Excalidraw 0.18.1, with [known limitations](docs/known-limitations.md).
- Linux x86-64 GNU is release-qualified. Browser WASM is experimental.
- Rendering and diagnostics may change in later 0.x releases. Consumers needing
  historical bytes should pin the renderer version and dependency lockfile.
- Exhaustive conformance and exact native/WASM equality are follow-up goals;
  this release does not claim their completion.

Publication date, release commit, package checksums, and the Dagger verification
trace will be recorded when the prepared candidate is actually released.
