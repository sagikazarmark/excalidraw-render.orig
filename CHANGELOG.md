# Changelog

## 0.1.0 — Unreleased

First release of `excalidraw-render`, a pure-Rust library that renders
Excalidraw drawings to SVG, PNG and PDF, and of `excalidraw-render-cli`, its
command-line tool.

### Features

- Render a whole scene, or each frame on its own, to SVG, PNG, or vector-first
  PDF, matching Excalidraw 0.18.1's exporter as closely as possible.
- Fonts and other rendering assets are bundled: rendering needs no browser,
  Node.js, network access, or installed fonts. The default `cjk` feature adds Xiaolai, Excalifont's
  Chinese/Japanese/Korean fallback (about 12 MiB).
- PNG and PDF output are the default `png` and `pdf` features. SVG is always
  available, so an SVG-only build (WebAssembly in particular) can turn them
  off with `default-features = false` and skip the rasterizer and the PDF
  writer. The API is the same either way: a request for a format that is off
  fails to build with `request.format-unavailable`, and a feature never
  changes the bytes of the formats a build can produce.
- Every render returns a compatibility report listing what it substituted,
  omitted, or approximated, so differences are reported instead of silent.
- Output is deterministic: the same renderer release, input, options, and
  dependency versions produce the same bytes.
- Every render is bounded by configurable resource limits (input size, element
  and asset counts, text and geometry, decoded image pixels, raster surface,
  filter effects in embedded SVG images, total work, and output size). Limits
  are deterministic, so a scene crosses them the same way on every platform,
  and exceeding one fails the render with a `resource.*` error. The defaults
  and maximums are documented in the `ResourceLimits` API docs; they admit
  large drawings, so lower them when serving untrusted input.

### Command-line tool

- `excalidraw-render drawing.excalidraw -o drawing.svg` picks the format from
  the output extension (or `--format`) and reads and writes standard streams
  with `-`.
- `excalidraw-render frames` lists a scene's frames or exports each to its own
  file, with frame-name filtering.
- Sizing (`--scale`, `--max-dimension`), padding, dark mode, transparent
  background, frame clipping and decoration switches, and embedded scene data
  so Excalidraw can reopen an exported file.
- Errors name the file, the reason, and a fix. Repeated warnings are grouped,
  informational messages appear with `--verbose`, and `--report-format json`
  prints a machine-readable report.

### Compatibility

- Targets Excalidraw 0.18.1, with
  [known limitations](https://github.com/sagikazarmark/excalidraw-render/blob/main/docs/known-limitations.md).
- The bundled fonts ship in internal crates (`excalidraw-render-fonts`,
  `excalidraw-render-fonts-xiaolai`, `excalidraw-render-fonts-xiaolai-extended`)
  versioned `0.1.0+excalidraw.0.18.1` after the Excalidraw release they match.
  Depend on `excalidraw-render`, not on them directly.
- Requires Rust 1.92 or newer. Linux x86-64 (glibc) is release-qualified;
  other native platforms build and run but are not release-qualified, and
  browser WASM is experimental.
- Dependencies use ordinary semver ranges, so the renderer resolves alongside
  your other crates; the release is tested with the versions in the
  repository's `Cargo.lock`. Rendering and diagnostics may change in later 0.x
  releases: pin the version and keep your lockfile when exact output bytes
  matter.
- This release does not yet claim exhaustive conformance with Excalidraw or
  byte-identical output between native and WASM builds; both are follow-up
  goals.

<!--
  PENDING 0.1.0 ENTRIES: changes still landing from parallel pre-0.1 branches
  go here, then into the sections above (CLI message improvements, the PDF gradient fix, library API additions such as
  `Clone` on outcomes). Remove this comment before release.
-->
