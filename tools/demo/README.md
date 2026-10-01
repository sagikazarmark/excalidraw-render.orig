# Side-by-side demo

A static page showing this library's SVG, PNG and PDF renders beside the
official Excalidraw 0.18.1 exports of the same scenes.

```sh
cargo run --locked -p xtask -- demo build [--out DIR]
```

The default output is `target/demo/`; open `target/demo/index.html` directly
(`file://`) or serve the directory statically. The page fetches nothing from
the network. A rebuild replaces the directory, and the command refuses to
replace a non-empty directory it did not build.

Prerequisites: the pinned Rust 1.97.1 toolchain, Docker, and the reference
oracle's npm install (`tools/reference-oracle/run-container.sh install`, the
only step that uses the network).

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

## Official PDFs

Excalidraw has no PDF export. `print-official-pdf.mjs` prints each committed
official SVG with Chromium's `page.pdf()` (page size = the SVG's intrinsic
width and height in CSS pixels, zero margins, `printBackground`, first page
only), in the reference oracle's pinned Playwright image with `--network none`
and the checkout mounted read-only. It uses the oracle's launch arguments and
browser context, so text falls back to the same 48 system fonts as the oracle;
fonts the official SVG embeds are used as embedded. The page labels these
files "Official SVG printed by Chromium".

Skia writes the wall-clock time into `/CreationDate` and `/ModDate`; the script
overwrites both with `D:20000101000000+00'00'` (same length, so cross-reference
offsets stay valid). Nothing else in the PDFs varies between runs, so they are
part of the determinism check.

## Determinism

Two builds produce byte-identical trees, PDFs included:

```sh
cargo run --locked -p xtask -- demo build --out /tmp/demo-a
cargo run --locked -p xtask -- demo build --out /tmp/demo-a2
diff -r /tmp/demo-a /tmp/demo-a2
```

The header names the checked-out commit, and adds "with uncommitted changes"
when tracked files differ from it.

## Why the output is not committed

The demo is about 35 MB of artifacts derived entirely from committed inputs,
and one command regenerates it byte for byte. Committing it would double-store
evidence that is already in the repository (ADR-0022 pruned unread evidence for
the same reason), and every renderer change would churn it.

## Example selection

Every requested example has usable official evidence and options the CLI
reproduces, so none was dropped or substituted. Representative scenes show
both the light and the dark reference. A library PNG at 2x is not included: the
official evidence has only the case's own scales, so it would have nothing to
sit beside.
