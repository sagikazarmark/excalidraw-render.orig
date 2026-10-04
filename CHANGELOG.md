# Changelog

## 0.1.0 — Unreleased

Initial release of the pure-Rust Excalidraw rendering library
(`excalidraw-render`) and command-line tool (`excalidraw-render-cli`).

### Features

- Whole-scene and per-frame SVG, PNG, and vector-first PDF exports.
- Bundled rendering fonts and assets; no browser or network needed at runtime.
  The default `cjk` feature bundles the Xiaolai CJK fallback.
- Compatibility reports describing substitutions, omissions, and fallbacks;
  resource limits for untrusted input; deterministic output.
- CLI frame listing, frame-name filtering, sizing, dark mode, embedded scene
  metadata, and JSON reporting. Human-readable errors name the path, the
  reason, and a fix; repeated diagnostics are grouped, and informational ones
  appear with `--verbose`.

### Compatibility

- Targets Excalidraw 0.18.1, with [known limitations](docs/known-limitations.md).
- The font crates name that release in their version's build metadata
  (`0.1.0+excalidraw.0.18.1`).
- Requires Rust 1.90 or newer. Linux x86-64 (glibc) is release-qualified;
  other native platforms are unqualified and browser WASM is experimental.
- Dependencies that compute output bytes are pinned exactly; others take caret
  requirements. Rendering and diagnostics may change in later 0.x releases:
  pin the version and keep your lockfile when exact output bytes matter.
- Exhaustive conformance and exact native/WASM equality are follow-up goals;
  this release does not claim their completion.

Publication date, release commit, package checksums, and the Dagger verification
trace will be recorded when the release is published.
