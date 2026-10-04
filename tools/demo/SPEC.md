Build a repeatable, side-by-side demo of excalidraw-render against official Excalidraw 0.18.1.

  ## Deliverable
  A self-contained static demo under `demo/` (or `target/demo/` if you judge committing outputs wrong; justify the choice): one `index.html` plus the asset files it links to. For each
  example scene it shows:
  - the official SVG and this library's SVG side by side
  - the official PNG and this library's PNG side by side
  - download links for the source `.excalidraw`, the official PDF and this library's PDF
  - a one-line caption naming the features the example demonstrates, and this library's compatibility diagnostics for that scene (from `--report-format json`), shown collapsed

  ## Generation must be repeatable
  - One entry point regenerates everything from a clean checkout, e.g. `cargo run --locked -p xtask -- demo build [--out DIR]` (preferred, matching how this repo owns tooling) or
  `tools/demo/build.sh`. Document it in a short `tools/demo/README.md` (or a section of `docs/development.md`).
  - The example list lives in a checked-in data file (e.g. `tools/demo/examples.json`): id, source path, title, feature caption, variant (light/dark/frame). The HTML is generated from
  that file, not hand-edited.
  - Running it twice must produce byte-identical output. Verify this by building twice and diffing.
  - Library renders come from the workspace CLI/library built with `--locked` (`excalidraw-render <scene> -o x.svg|x.png|x.pdf`, plus `frames` for frame examples), using the same
  export options as the case's manifest variant (padding, scale, background, dark mode).
  - Official SVG/PNG come from the committed oracle evidence (`tests/conformance/cases/focused/<case>/evidence/official.{svg,png}`, `.../dark-mode/...`, frame-export
  `evidence/frame-N/...`, representative `reference/{light,dark}/official.{svg,png}`). Do NOT regenerate the evidence and do NOT modify anything under `tests/`; it is digest-pinned.
  - Official PDF: Excalidraw has no PDF export. Produce it by printing the committed official SVG with Chromium `page.pdf()` (page size = SVG intrinsic size, no margins,
  printBackground) inside the pinned Playwright image from `tools/reference-oracle/run-container.sh`, with `--network=none` and the oracle's fonts available, following that script's
  docker invocation pattern. Label it in the UI as "Official SVG printed by Chromium" so nobody mistakes it for an Excalidraw export. If Chromium PDFs turn out not to be byte-stable,
  pin what makes them stable (e.g. creation date and metadata) or document exactly what varies and exclude those files from the determinism check, and say so.

  ## Examples (cover as many features as possible)
  Use these, in this order, grouped under headings. Each one has official evidence already committed.
  Basics & rough style: focused rectangle-outline, ellipse-diamond-fills, rough-fillers-dashed-zigzag-line, cartoonist-roughness, roundness-records
  Lines & arrows: arrow-suite, cartoonist-arrowheads, rotated-rough-linear
  Freedraw: freedraw, freedraw-loop-fills
  Text & fonts: bound-text-containers, text-alignment-vertical, excalifont-text, font-families-virgil-lilita-comic, helvetica-text, cjk-propagation, emoji-fallback, bidi-text
  Images: image-codecs, rounded-image-crop
  Transforms & grouping: transform-opacity-groups, rotated-shape-bounds
  Frames, links, dark mode: frame-export (show every frame via the `frames` subcommand), embeddable-links, dark-mode-images (light and dark variants)
  Real-world scenes: representative velox-design, varmq-queue-diagram, excali-local-slides (light and dark where references exist)
  If any of these turns out to lack usable official evidence, or the CLI can't reproduce its export options, drop it, note why in the README, and pick the nearest substitute from
  `tests/conformance/cases/registry.json`. Don't invent new scenes.

  ## HTML requirements
  - Single static page, no build step, no CDN or network fetches; it works when opened via file:// and when served statically.
  - Show SVGs with `<img>` (sandboxed, so they get the same font handling as files) at their intrinsic size, scaling down to fit the column; PNGs at 1x, with optional 2x library PNG
  if cheap.
  - Light/neutral checkerboard behind transparent renders; sticky table of contents; works at phone width.
  - Brief header: library version, Excalidraw compatibility target 0.18.1, commit SHA, link to docs/known-limitations.md, and a sentence saying the renders are not claimed to be
  pixel-identical.

  ## Constraints
  - Follow AGENTS.md and the repo's conventions. Don't touch the conformance corpus, reference profiles or digests.
  - Local env: use rustup's 1.97.1 toolchain (`export PATH=$HOME/.cargo/bin:$PATH`) and build with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` (disk and RAM are tight).
  - Keep generated binary outputs out of git unless you decide committing them is right; if you commit them, keep the total size reasonable and say what it is.
  - Work on a new branch. Commit when done; don't push or open a PR.

  ## Done when
  1. The single command builds the demo from scratch, and a second run produces identical bytes (show the diff/hash check).
  2. Every listed example (or its documented substitute) appears with all four images and three download links, and every link resolves to an existing file.
  3. You've opened `index.html` in a headless browser (Playwright MCP), taken screenshots of the top, one text example, the frame example and the dark-mode example, and confirmed no
  image failed to load and the console is clean.
  4. `cargo clippy --locked --workspace --all-targets -- -D warnings` and the fast nextest profile pass if you added xtask code.
  5. Final report: the command, the output location and size, which examples were substituted or dropped and why, and how the official PDFs were produced.
