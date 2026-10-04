# Side-by-side demo

A static page showing this library's SVG and PNG renders beside the official
Excalidraw 0.18.1 exports of the same scenes. [SPEC.md](SPEC.md) is the
specification it was built against.

```sh
cargo run --locked -p xtask -- demo build [--out DIR]
```

The default output is `target/demo/`; open `target/demo/index.html` directly
(`file://`) or serve the directory statically. The page fetches nothing from
the network. A rebuild replaces the directory, and the command refuses to
replace a non-empty directory it did not build.

Prerequisites: a Rust toolchain. Unlike the native conformance capture, the
demo records no build identity, so it does not require pinned Rust 1.97.1 on
Linux x86_64; it builds on any host. Renders can differ slightly between hosts
and toolchains.

## What it does

- `examples.json` lists the examples in page order, grouped under headings.
  Each entry names a conformance case directory (`source`), a title, a one-line
  feature caption and its variants: `light`, `dark` (a scene export) or `frame`
  (every target of a frame-export case). The page is generated from this file;
  do not edit the HTML by hand.
- Library renders come from the release CLI, built with `--locked` exactly as
  the native conformance capture builds it. Each variant is exported with the
  flags that reproduce its official export options (the focused manifest's
  variant options, or the representative `reference/reference.json` row):
  padding, scale or maximum dimension, background, dark mode, scene embedding
  and frame rendering. An option with no CLI equivalent fails the build rather
  than being dropped. The view background comes from the scene, so the build
  checks it equals the official `viewBackgroundColor`. Frame examples use
  `excalidraw-render frames` and are matched to the official targets by
  ordinal, kind and name.
- Official SVG and PNG are copied from the committed evidence under
  `tests/conformance/cases/`; nothing is regenerated and nothing under
  `tests/` is written.
- Diagnostics are each render's `--report-format json` report, kept next to
  the artifacts and shown collapsed on the page.

- PDF is not part of the demo: Excalidraw has no PDF export, so there is no
  official PDF to sit beside this library's.

## Determinism

Two builds produce byte-identical trees:

```sh
cargo run --locked -p xtask -- demo build --out /tmp/demo-a
cargo run --locked -p xtask -- demo build --out /tmp/demo-a2
diff -r /tmp/demo-a /tmp/demo-a2
```

The header names the checked-out commit, and adds "with uncommitted changes"
when tracked files differ from it.

## Why the output is not committed

The demo is about 19 MB of artifacts derived entirely from committed inputs,
and one command regenerates it byte for byte. Committing it would double-store
evidence that is already in the repository (ADR-0022 pruned unread evidence for
the same reason), and every renderer change would churn it.

## Example selection

Every requested example has usable official evidence and options the CLI
reproduces, so none was dropped or substituted. Representative scenes show
both the light and the dark reference. A library PNG at 2x is not included: the
official evidence has only the case's own scales, so it would have nothing to
sit beside.
