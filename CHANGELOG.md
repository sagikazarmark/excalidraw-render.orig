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

### Resource limits

- Every render's work is bounded: `max_total_work_units` defaults to
  4,294,967,296 and accepts up to 17,179,869,184, and it covers Rough
  (hand-drawn) generation as well as rasterization.
- PNG draws elements of any size its raster surface admits, as SVG and PDF
  do; their raster work counts against the total work limit.
- Filter effects in nested SVG images are charged before they are rasterized,
  in PNG and in PDF raster fallback: each primitive's work at its device size
  (including octaves, kernel size and morphology radius) counts against the
  total work limit, and its result surface against the total rasterized pixels.

### Compatibility

- Targets Excalidraw 0.18.1, with
  [known limitations](https://github.com/sagikazarmark/excalidraw-render/blob/main/docs/known-limitations.md).
- The font crates name that release in their version's build metadata
  (`0.1.0+excalidraw.0.18.1`).
- Requires Rust 1.92 or newer. Linux x86-64 (glibc) is release-qualified;
  other native platforms are unqualified and browser WASM is experimental.
- Dependencies take caret requirements so the renderer resolves beside other
  crates; the gates test the versions in the repository `Cargo.lock`.
  Rendering and diagnostics may change in later 0.x releases: pin the version
  and keep your lockfile when exact output bytes matter.
- Exhaustive conformance and exact native/WASM equality are follow-up goals;
  this release does not claim their completion.
