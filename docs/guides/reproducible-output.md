# Reproducible output

Rendering is deterministic: the same renderer release, input, options and
dependency versions produce the same bytes, every time and on every supported
host. This guide explains what that covers and how to keep it when you
upgrade.

## What is deterministic

- **Artifacts.** SVG, PNG and PDF bytes depend only on the scene, the request
  and the code. Nothing reads the clock, the environment, installed fonts or a
  random source: PDFs carry no creation date, and their file identifier is
  derived from their content. Embedded provenance (`--embed-provenance`)
  records the renderer version and the Excalidraw release, not a timestamp.
- **Reports.** The compatibility report, including the order of its
  diagnostics, is deterministic too, as are resource-limit failures: a scene
  that crosses a limit crosses it on every machine.
- **Hosts.** Output does not depend on the operating system or CPU. The
  project's test suite renders every test scene on Linux x86-64 and macOS
  arm64 to the same bytes. Rendering math goes through a portable
  implementation instead of the platform's math library for that reason.
- **Browser WebAssembly** is the exception: four PNG test cases differ from
  native by one channel value in one or two pixels
  ([#178](https://github.com/sagikazarmark/excalidraw-render/issues/178)).

Linux x86-64 (glibc) is the release-qualified platform; other native platforms
build and run and, by the property above, produce the same bytes, but each
release is checked on Linux.

## What can change the bytes

- **A new renderer release.** In 0.x, any release may intentionally change
  rendering and diagnostics as it moves closer to Excalidraw. The project's
  rule is that patch releases do not change the output of its test scenes;
  changes that do wait for a minor release, and the release notes say so.
- **A dependency update.** The library's dependencies use ordinary semver
  ranges so it can coexist with other crates. Decoders, the rasterizer, the
  PDF writer, font shaping and compression can change output in a patch
  release of their own. Each renderer release is tested with the versions in
  its repository `Cargo.lock`.
- **A different request.** Scale, padding, color mode, metadata options and
  resource limits all affect the output, as do supplemental assets.
- **Cargo features** do not: turning off `png`, `pdf` or `cjk` never changes
  the bytes of the formats that remain (it can only make a render fail).

## Pin it

When exact bytes matter (content-addressed caches, golden tests,
reproducible documentation builds), pin the renderer exactly and commit your
lockfile:

```toml
[dependencies]
excalidraw-render = "=0.1.0"
```

Install the CLI with `--locked`, which uses the dependency versions the
release was tested with:

```sh
cargo install --locked excalidraw-render-cli --version 0.1.0
```

or use the prebuilt archive of a GitHub release, whose checksum is published
with it.

## Upgrade deliberately

Treat an upgrade of the renderer, or a lockfile update that touches its
dependencies, like any other change to your rendered output: re-render, diff
the bytes, and review the images that changed. The compatibility report helps
here too, since a new warning or a disappeared one usually explains a visual
change. Tools that compare rendered files can key on the artifact's SHA-256,
which the JSON report of the CLI includes.

`excalidraw-render --version --verbose` names the exact compatibility revision
and Excalidraw profile a build implements; record it alongside rendered
artifacts if you need to know later what produced them.
