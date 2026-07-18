Good question — and it's very doable. Let me check the current state of the ecosystem so you're not reinventing anything that already exists.Yes, and the hardest part is already solved for you. Let me break down what's actually involved, because the format looks simpler than it renders.

**The format itself is the easy part.** A `.excalidraw` file is just JSON: a top-level object with an `elements` array plus `appState` and `files` (for embedded images). Each element is a flat record — `type` (`rectangle`, `ellipse`, `diamond`, `arrow`, `line`, `freedraw`, `text`, `image`, `frame`), plus `x`, `y`, `width`, `height`, `angle`, `strokeColor`, `backgroundColor`, `fillStyle` (`hachure` / `cross-hatch` / `solid` / `zigzag`), `strokeWidth`, `strokeStyle`, `roughness`, `roundness`, and crucially a per-element `seed`. Linear elements carry a `points` array; text elements carry `text`, `fontSize`, `fontFamily`, `textAlign`. `serde` handles all of this in an afternoon.

**The look is the whole game, and it comes from Rough.js.** Excalidraw's hand-drawn aesthetic isn't decorative styling — every shape is run through [Rough.js](https://github.com/rough-stuff/rough), which generates randomized point offsets, hachure fills via scan-line rasterization, and multi-stroke outlines. Reproduce that faithfully or your output won't look like Excalidraw at all. The good news: there's a mature Rust port, [`roughr`](https://docs.rs/roughr) (part of `orhanbalci/rough-rs`), which ports the drawing primitives and — importantly — supports the seed parameter. Because Excalidraw stores the seed per element, feeding the same seed into `roughr` gives you deterministic, pixel-identical re-renders, which matters a lot for a docs/content pipeline where you don't want diagrams jittering between builds.

So the architecture is roughly:

1. Parse JSON → typed structs (`serde`).
2. For each element, translate its geometry + style into `roughr` primitive calls with the stored seed.
3. Emit through a backend. `roughr` doesn't draw itself — it produces drawables you feed to `tiny-skia` (raster → PNG), `raqote`, or `piet`. Alternatively, generate SVG path data and assemble an SVG document, then optionally run it through `resvg`/`tiny-skia` for PNG/PDF. Given your Typst doc-generation stack, the SVG-then-`resvg` route is probably the natural fit — same rendering primitives, and SVG is a nice intermediate to keep around.

**The two things that will actually eat your time:**

Fonts. Excalidraw uses custom fonts — Excalifont (the hand-drawn one, formerly Virgil), plus a normal and a code font. Rough.js only handles shapes, not text, so you own text layout entirely: shaping, wrapping, alignment, vertical centering inside bound containers, and the `containerId` binding that ties a label to its shape. Reach for `cosmic-text` (or `swash` + `fontdb`) and bundle the actual font files. Getting text metrics to match Excalidraw's wrapping is the fiddliest part of the whole project.

Freedraw. The pencil/freehand strokes don't go through Rough.js at all — they use Steve Ruiz's `perfect-freehand` algorithm (pressure-sensitive variable-width polygons). You'll need to port that separately; it's small and well-documented, so not a big lift, but it's a distinct code path.

**On prior art:** worth knowing that basically every existing "Excalidraw renderer" — the `excalidraw-render` MCP server, Kroki, the mdBook preprocessors — cheats by spinning up a headless Chromium and using the real JS library. I didn't find a pure-native Rust renderer that reproduces the aesthetic from scratch, which means (a) there's a genuine gap here if you want a browserless, single-binary renderer, and (b) nobody's done the tedious font-matching work for you, so budget for it.

If your goal is a content-pipeline tool that turns `.excalidraw` files into SVG/PNG without a browser, this is very much within reach — `roughr` de-risks the scary part, and the rest is parsing plus the font grind. Do you have a specific output target in mind (SVG for embedding, or raster/PDF for the Typst pipeline)? That changes which backend I'd steer you toward.




SVG, without question — and the nice part is it's not just the simplest, it's a superset that hands you PNG and PDF almost for free. Let me confirm the current state of the conversion crates so I give you accurate versions.SVG first is the clear call, and here's the payoff: all three targets run through the same `usvg` core, so once your SVG is correct, PNG and PDF are thin wrappers rather than separate rendering paths.

The stack lines up almost too neatly for you specifically — `resvg`, `usvg`, and `svg2pdf` are all from the Typst team's lineage and share the same `usvg` parse/simplification layer (current versions: `resvg`/`usvg` 0.47, released Feb 2026; `svg2pdf` on usvg 0.45). It's literally the same `usvg` Typst uses internally, so this drops right into the world you already work in. The flow is:

1. Your code: `.excalidraw` JSON → `roughr` primitives → an **SVG string**.
2. Parse it once: `usvg::Tree::from_str(&svg, &options)`.
3. Fan out from that one tree — `resvg::render(...)` into a `tiny_skia::Pixmap` for PNG, or `svg2pdf::to_pdf(&tree, ...)` for PDF. Notably, svg2pdf produces **true vector PDF** (no rasterization except for filtered groups), so your diagrams stay crisp at any zoom.

Skeleton of the whole back half:

```rust
// 1. You produce this from the excalidraw elements + roughr
let svg: String = render_excalidraw_to_svg(&doc)?;

// 2. Parse once — load Excalifont into the fontdb FIRST (see below)
let mut options = usvg::Options::default();
options.fontdb_mut().load_font_data(EXCALIFONT_TTF.to_vec());
let tree = usvg::Tree::from_str(&svg, &options)?;

// 3a. PNG
let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
pixmap.encode_png()?; // -> Vec<u8>

// 3b. PDF
let pdf: Vec<u8> = svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(),
                                   svg2pdf::PageOptions::default())?;
```

(Treat that as the shape, not copy-paste — the 0.4x APIs drift between minor versions, so check the exact signatures against the docs for whatever version you pin.)

**The one decision to make consciously up front: text-to-path.** `usvg` converts `<text>` elements to vector paths at parse time, using whatever fonts are in its `fontdb`. This is actually a gift — it means if you load Excalifont into the fontdb before parsing, your text renders *identically* across SVG-rasterized-to-PNG and PDF, and you sidestep the whole "does the consumer have the font" problem. The only thing you give up is selectable/copyable text in the PDF; if you ever want that back, `vl-convert-pdf` layers embedded text on top of svg2pdf, but that's a later refinement, not a first-target concern.

For the actual MVP, I'd scope it hard: parse the JSON, handle **only** `rectangle`, `ellipse`, `diamond`, `line`, and `arrow` through `roughr` → SVG, get the shapes and hachure fills looking right, and render to PNG. Defer text, freedraw, images, and containers entirely. Shapes-to-PNG is the milestone that proves the `roughr` → `usvg` → raster pipeline end to end; once that's solid, text and the other element types are additive, and PDF is just swapping the final call. Want me to sketch the element-to-`roughr` mapping for that first batch of shapes?



This is a real project now, so let me build you a proper prompt. First, let me pull up the prompt-engineering skill so this is structured well.Here's the prompt, built for GPT-5.x and scoped so it doesn't wander into the deferred features.

```
You are a senior Rust engineer who ships small, correct, dependency-lean binaries. Build a complete, compiling MVP in one pass. Output the full project as a file tree followed by every file's complete contents. Do not stub, do not leave TODOs in core paths, do not explain your reasoning outside code comments.

## Context (locked decisions — do not revisit)
- Goal: a PURE-RUST renderer for the Excalidraw `.excalidraw` format. NO JavaScript, NO V8/deno_core, NO Node, NO headless browser, NO network calls. Single self-contained binary.
- The hand-drawn look comes from Rough.js. Use the Rust port `roughr` (crate from the `rough-rs` workspace) to generate sketchy primitives — do NOT reimplement the rough algorithm.
- SVG is the pivot format: generate an SVG string yourself, then reuse it for the other targets. PNG = rasterize the SVG with `resvg`/`tiny-skia`; PDF = convert the SVG with `svg2pdf`. All three share the `usvg` core.
- This was deliberately chosen over embedding Excalidraw's real JS for byte-perfect fidelity. Small binary + no JS is the priority; approximate visual parity is acceptable.

## Task
Build a CLI crate `excalidraw-render` that reads a `.excalidraw` JSON file and writes SVG, PNG, or PDF.

CLI contract:
`excalidraw-render <input.excalidraw> -o <output.{svg|png|pdf}> [--scale <f32=1.0>] [--padding <f32=20.0>]`
- Output format is inferred from the `-o` extension. `--scale` affects PNG only. Exit 0 on success, non-zero with a clear stderr message on failure.

## Stack (pin these; verify exact APIs against docs.rs before using)
- `serde` + `serde_json` — parse the format
- `roughr` — sketchy primitives (works on `euclid` Point2D; add `euclid` if needed)
- `usvg` + `resvg` + `tiny-skia` (0.47.x) — SVG→PNG
- `svg2pdf` (targets usvg 0.45.x — keep usvg versions compatible between resvg and svg2pdf; if they conflict, pick the versions that share one usvg and say so in a comment)
- `clap` (derive) — CLI; `anyhow` — errors

## Data model (serde)
- Deserialize the top-level object: `type`, `version`, `elements: Vec<Element>`, `appState` (read `viewBackgroundColor`), `files` (ignore for MVP).
- Elements have heterogeneous fields. Deserialize each into ONE struct with `#[serde(rename_all = "camelCase")]` and `#[serde(default)]` on optional fields. Keep the raw `type` as a String.
- Common fields: id, type, x, y, width, height, angle (radians), strokeColor, backgroundColor, fillStyle ("hachure"|"cross-hatch"|"solid"|"zigzag"), strokeWidth, strokeStyle ("solid"|"dashed"|"dotted"), roughness, opacity (0–100), seed (i64), roundness (nullable), isDeleted (bool), points (Vec<[f64;2]>, present on linear elements, relative to element x/y).
- CRITICAL ROBUSTNESS: real files contain element types you will NOT render (text, freedraw, image, frame, embeddable, etc.). Skip any element whose `type` is not one of the five supported types, and skip `isDeleted == true`. Never panic or error on an unknown type — just ignore it. The program must succeed on a mixed file, rendering only the supported subset.

## Supported element types (MVP — ONLY these five)
- rectangle, ellipse, diamond, line, arrow.
Mapping to roughr:
- rectangle → rough rectangle at (x,y,w,h).
- ellipse → rough ellipse; note roughr/Excalidraw center vs top-left conventions and convert correctly.
- diamond → rough polygon through the 4 edge midpoints of the bounding box.
- line → rough linear path through `points` (offset by element x,y).
- arrow → rough linear path through `points`, PLUS a simple arrowhead: two short strokes at the final point, ~20–25px long, ~30° off the incoming segment. A basic arrowhead is fine; do not model Excalidraw's arrowhead variants.

## Style mapping (element → roughr Options)
- stroke = strokeColor; stroke_width = strokeWidth.
- fill = backgroundColor ONLY if it is set and not "transparent"; otherwise no fill.
- fill_style = map hachure/cross-hatch/solid (treat zigzag as hachure).
- roughness = element.roughness (default ~1.0).
- seed = element.seed — MUST be passed through so re-renders are byte-stable. If seed is absent, derive a stable u64 from the element id (hash) — never random.

## SVG assembly (you own this)
- Compute the bounding box over all rendered elements; add `--padding` on each side; set the SVG `width`/`height`/`viewBox` from it.
- Emit a background `<rect>` filling the canvas using appState.viewBackgroundColor (skip if "transparent"/absent).
- For each element: convert the roughr drawable's op sets into SVG `<path d="…">` — roughr ops are Move/LineTo/BCurveTo → SVG `M`/`L`/`C`. Emit fill op-sets and stroke op-sets as separate paths (hachure fills are stroked paths colored with the fill color).
- Apply per-element: `stroke`, `stroke-width`, `fill`, `opacity` (element.opacity/100), and `stroke-dasharray` for dashed/dotted strokeStyle.
- Apply element rotation: wrap the element's paths in `<g transform="rotate(angle_deg cx cy)">` where (cx,cy) is the element center and angle is converted from radians to degrees.

## Backends
- SVG: write the assembled string to the output path.
- PNG: `usvg::Tree::from_str(&svg, &usvg::Options::default())`, render with `resvg` into a `tiny_skia::Pixmap` sized `ceil(w*scale) × ceil(h*scale)` with the scale transform applied, then `encode_png()`.
- PDF: `svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default())` from the same parsed tree.
- No text is rendered in the MVP, so fonts/fontdb are a non-issue — do not load system fonts.

## Deliverables
- `Cargo.toml`, `src/main.rs`, and whatever modules you split out (e.g. `model.rs`, `render_svg.rs`, `backends.rs`).
- One test fixture `tests/fixtures/basic-shapes.excalidraw` containing at least one of each supported shape PLUS one unsupported element (e.g. a `text` element) to prove it is skipped cleanly.
- An integration test that renders the fixture to SVG, PNG, and PDF into a temp dir and asserts each output is non-empty and, for PNG, decodes to the expected pixel dimensions.
- A short README section: build, run, and the three example commands.

## Non-goals — DO NOT build these (hard scope lock)
- No text, freedraw, image, frame, or embeddable rendering. No text layout, no font loading, no perfect-freehand.
- No editable/round-trip `.excalidraw.svg`/`.png` embedding or extraction.
- No JS/V8/Node/browser anything. No web/network access.
- No extra element types, no plugin system, no config files, no abstractions "for later." Build exactly the five shapes and three outputs above and nothing more.

## Done when
- `cargo build --release` compiles with no errors and no warnings you can reasonably avoid.
- `cargo test` passes.
- Running the three example commands on the fixture produces a valid `.svg`, `.png`, and `.pdf`, and the run does not fail even though the fixture contains an unsupported element.
- Rendering the same input twice produces byte-identical SVG (determinism via seed).

If any pinned crate API differs from what you expect, adapt to the real current API and note the change in a one-line code comment rather than guessing silently.
```

🎯 Target: GPT-5.x — 💡 Front-loaded the locked architecture and a hard scope lock so it builds the five-shape pure-Rust core in one pass instead of drifting into text/freedraw or re-adding a JS engine.

One setup note: paste this into a fresh GPT session (no prior context needed — the decisions are baked in). If it stops partway through a large file dump, reply "continue" and it'll resume the file tree.

One thing worth flagging before you run it: the single riskiest spot for a clean one-shot is `roughr`'s exact op-set → SVG-path API, since that's the least standardized part and its signatures have shifted across versions. If GPT stumbles anywhere, it'll be there — so if the build breaks, that's the first place I'd look, and I can help you nail down the current API.
