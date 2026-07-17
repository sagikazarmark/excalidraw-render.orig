# Pure-Rust and WASM parity paths for Excalidraw static exports

Research ticket: [#2, "Survey pure-Rust and WASM parity paths"](https://github.com/sagikazarmark/excalidraw-render.orig/issues/2)
Wayfinder map: [#1, "Specify perceptually equivalent Excalidraw static exports"](https://github.com/sagikazarmark/excalidraw-render.orig/issues/1)
Research date: 2026-07-17

## Scope and evidence baseline

The eventual reference release is deliberately still a decision for map ticket #9. This survey uses Excalidraw `v0.18.1`, commit [`a2ec2889babf7d2295469c6d90ebe77fae57df84`](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84), as the latest official release available on the research date. Every upstream port must ultimately be pinned to whichever Excalidraw release ticket #9 selects, not silently to `v0.18.1` or a crate's latest version.

The component versions examined in depth are the versions in the current MVP (`roughr 0.12.0`, `usvg/resvg 0.45.1`, `svg2pdf 0.13.0`, and `tiny-skia 0.11.4`) plus directly relevant alternatives. No application code or manifest was changed. Source-only `wasm32-unknown-unknown` build probes were performed in temporary upstream checkouts and are recorded below.

Material conclusions were rechecked against the pinned upstream source on the research date. Source behavior is cited to immutable commits where it controls a decision; versioned docs.rs pages are used for published crate API, feature, and license claims. The five WASM component probes below were also rerun with Rust/Cargo `1.97.1` during the final audit and reproduced the recorded results.

“Parity” below means perceptual equivalence for static SVG and PNG, not SVG DOM identity or pixel identity, plus a vector-first PDF with useful semantics. It does not excuse visibly different seeded rough geometry, line routing, text placement, clipping, missing images, or missing content. Byte identity is discussed separately under determinism.

## Executive answer

There is a viable pure-Rust, network-free, native-and-WASM route for nearly all of the official static renderer, but it is not obtained by assembling generic graphics crates alone.

The credible fidelity boundary is:

1. Directly port the pinned upstream algorithms that define Excalidraw's appearance: Rough.js, Excalidraw's rough-option and element-shape synthesis, linear/arrowhead/bound-text geometry, frame/export preparation, and Perfect Freehand plus Excalidraw's stroke-to-SVG conversion. Preserve JavaScript number and PRNG behavior and validate every port against the pinned JavaScript functions.
2. Use mature Rust components below that boundary: `usvg`/`resvg`/`tiny-skia` for a controlled static SVG subset and rasterization, `rustybuzz` plus `fontdb`/`ttf-parser` for shaping bundled fonts, `image` and format-specific decoders for common raster assets, and `svg2pdf`/`pdf-writer`/`subsetter` for vector PDF and selectable text.
3. Carry nonvisual semantics, especially normalized links and diagnostics, beside the visual tree. `usvg` intentionally does not support SVG `<a>`, and `svg2pdf 0.13.0` emits no PDF annotations. A raw SVG-to-PDF conversion therefore cannot meet the link requirement.

The current `roughr` dependency is not a parity implementation. Rough.js 4.6.4 uses a specific signed-32-bit `Math.imul(48271, seed)` sequence and defaults `bowing` to `1`; `roughr 0.12.0` uses `StdRng`, defaults `bowing` to `2`, and its unconfigured `rand/getrandom` path does not compile for `wasm32-unknown-unknown` without a JavaScript entropy feature. Those differences change every seeded rough shape. Repairing a fork is possible, but a clean, pinned, line-by-line port with cross-language vectors is easier to make auditable.

An SVG pivot is viable for canonical SVG and is a strong candidate for PNG, provided fonts and unsupported image formats are supplied out of band and external resource resolution is disabled. It is not, by itself, viable for all required PDF behavior:

- `usvg` discards anchors because it supports only static visual SVG.
- `svg2pdf` has no link-annotation path.
- `svg2pdf` rasterizes filtered groups; Excalidraw dark export places a filter on the SVG root, which can collapse a whole PDF page to a raster image.
- The official Canvas and SVG renderers are separate compositors and have observable edge-case differences in text rasterization, image sampling, and frame clipping.
- `svg2pdf 0.13.0` serializes some `HashMap` values directly, so same-build PDF byte stability is not established.

A hybrid “SVG visual pivot plus semantic sidecar and controlled PDF page writer” could close these gaps. A backend-neutral typed display list rendered independently to SVG/PNG/PDF could also close them. This research does not choose between those architectures.

Two parity gaps are not currently covered by a mature permissively licensed component:

- Excalidraw `v0.18.1` accepts AVIF, but `image 0.25.5` decodes AVIF only through the C `libdav1d` feature. The pure-Rust `zenavif` decoder is AGPL/commercial, while permissive `oxideav-avif 0.0.10` ships a local `Av1Decoder` stub that always returns `Unsupported` ([source](https://github.com/OxideAV/oxideav-avif/blob/1fd75128fc2e0e4ee67326633e5264ece5712494/src/av1_stub.rs#L114-L137)). AVIF can be preserved in SVG, but deterministic PNG/PDF decode remains a baseline or exception decision.
- Official emoji fallback is a local OS font (`Segoe UI Emoji`), so the official output is itself platform-dependent. A self-contained renderer must freeze a distributable emoji fallback or explicitly diagnose unsupported glyphs; it cannot be both OS-independent and match every host's official browser output.

## What the official exporters actually do

Excalidraw does not render one SVG and use it for every official output.

The Canvas export loads element fonts, loads image `dataURL`s into browser `HTMLImageElement`s, creates a Rough.js Canvas adapter, and calls the Canvas static-scene renderer. PNG export is a serialization of that canvas. See [`exportToCanvas`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L153-L255), [`updateImageCache`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts#L14-L81), and the Canvas element switch in [`renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts#L383-L523).

SVG export independently creates an SVG DOM, frame clip paths, inlined subset font declarations, a background, and a Rough.js SVG adapter, then emits each element through `renderSceneToSvg`. See [`exportToSvg`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L265-L479) and the complete [SVG element switch](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L79-L744).

The two paths share generated Rough.js `Drawable`s, element bounds, and much geometry, but not their final painter. Excalidraw has no official PDF exporter, so PDF acceptance must be derived from the static visual contract and separately defined semantic requirements.

`v0.18.1` pins `roughjs 4.6.4` and `perfect-freehand 1.2.0` in the package manifest ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json#L83-L90)). The exact Rough.js 4.6.4 source commit is [`7944f1cb95da6efadff7c15c9dc96ae0abc51199`](https://github.com/rough-stuff/rough/tree/7944f1cb95da6efadff7c15c9dc96ae0abc51199), whose [`package.json`](https://github.com/rough-stuff/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/package.json#L1-L4) identifies 4.6.4. Perfect Freehand 1.2.0 is tag commit [`fa0b754d49bb60813c24cbd7fdcd5cda40f560f3`](https://github.com/steveruizok/perfect-freehand/tree/fa0b754d49bb60813c24cbd7fdcd5cda40f560f3).

## Feature-by-feature matrix

| Rendering surface | Official behavior that matters | Most credible Rust path | Readiness | Principal parity gap or required evidence |
|---|---|---|---|---|
| Rectangles, diamonds, ellipses | Excalidraw constructs exact paths/options, including roundness, adjusted roughness, dash widths, fill settings, seed, transforms, and opacity; Rough.js creates all outline/fill operations | Direct port of pinned Rough.js generator/renderer/fillers plus pinned Excalidraw `Shape.ts`; generic path types only below the port | Credible after conformance port | `roughr` is algorithmically different; JS `f64`, `Math.imul`, iteration, and rounding behavior need golden vectors |
| Rough fills | `solid`, `hachure`, `cross-hatch`, `zigzag`, `dots`, `dashed`, and `zigzag-line` are Rough.js algorithms with shared seeded state | Include Rough.js 4.6.4 fillers and its pinned `hachure-fill`, `points-on-curve`, `points-on-path`, and path parser behavior in the port | Credible after conformance port | Merely supporting names is insufficient; operation order and PRNG consumption determine appearance |
| Lines and arrows | Saved local points choose linear path, curve, polygon, or rounded elbow path; arrowheads are generated from the first Rough.js curve's Bézier operations | Port Excalidraw shape synthesis, elbow path, arrowhead, bounds, and curve helpers; use `kurbo`/`euclid` only as non-semantic primitives | Credible after focused port | Generic spline/arrow crates will not match; degenerate curves, all arrowhead kinds, loops, and short terminal segments need fixtures |
| Bindings and bound text | Static rendering consumes saved points and repaired references; it does not reroute normal bindings. It computes bound-arrow text position and masks/clears the arrow behind text | Port the render-time subset of restore/repair, `LinearElementEditor`, bounds, text-container lookup, and mask geometry | Credible, smaller than editor binding system | Decide whether malformed/stale documents are repaired exactly like the public export wrapper; porting the full interactive binding engine is unnecessary unless the compatibility contract says otherwise |
| Freedraw | Perfect Freehand 1.2.0 creates an outline with Excalidraw-specific options; Excalidraw serializes midpoint quadratics and rounds path numbers to two decimals; closed strokes can also receive a rough background fill | Direct pinned port of Perfect Freehand and Excalidraw's wrapper/path serializer | High-confidence port candidate | Existing Rust ports are recent and lack upstream snapshot equivalence; use official Perfect Freehand snapshots and cross-language outputs |
| Text line placement | Export uses already stored `text` line breaks, stored width/alignment, `fontSize * lineHeight`, family metadata vertical offset, direction, and rotation | Port placement directly; do not introduce an independent paragraph layout engine for valid scene documents | High for valid current documents | Legacy/programmatic restore and optional dimension refresh can invoke browser-style measurement/wrapping; decide contract for those documents |
| Text shaping and glyphs | SVG keeps `<text>` and embeds subset WOFF2. Canvas delegates shaping/rasterization/fallback to the browser | Bundle exact font assets; use `rustybuzz`/`fontdb`/`ttf-parser` with predecoded sfnt fonts for PNG/PDF; retain `<text>` plus embedded WOFF2 for SVG | Credible for pinned fonts | Browser hinting/antialiasing can differ; CJK and emoji fallback must be frozen; WOFF2 is not directly loadable by `fontdb` |
| Raster images | Browser supports PNG, JPEG/JFIF, GIF, WebP, BMP, ICO, AVIF; crop, flip, rotation, opacity, round clip, and dark-image filter are applied | Port image geometry; decode common formats with `image`/format crates into bounded RGBA; feed canonical PNG/pixels to the visual backend | Good except AVIF and animation policy | AVIF has no mature permissive pure-Rust decoder found; animated format frame timing and color management require decisions |
| Embedded SVG images | Browser treats local SVG data as an image; output SVG can preserve its data URL | Parse as untrusted static SVG with `usvg`, deny string/external resolvers, cap resources, and emit a sanitized static subtree or bounded raster fallback | Conditional | Unsupported SVG/CSS/`foreignObject` and external resources can differ from browsers; raw passthrough is not network-free or sufficiently controlled |
| Frames and clipping | Whole-scene export can add labels/outlines and clip members to rounded frame rectangles; frame opacity multiplies child opacity | Port export preparation and clip eligibility; SVG clip paths and `resvg`/`tiny-skia` masks cover the paint operation | Credible | Canvas and SVG clipping code are not structurally identical; overlapping/grouped/outside-frame edge cases need dual-oracle fixtures |
| Compositing and dark mode | Source-over alpha, masks, clips, root dark filter, and inverse/saturating image correction | `resvg`/`tiny-skia` supports required static visual operations; direct color/filter lowering may be needed for PDF | Credible with PDF exception | A root SVG filter makes `svg2pdf` rasterize the whole filtered group; color math and image resampling differ from browsers |
| Offline embeddables/iframes | Canvas export uses a placeholder because validation status is empty. SVG defaults `renderEmbeddables` to false and emits placeholder/label plus a link; opt-in live iframe uses `foreignObject` and a URL | Reproduce the default offline placeholder and retain a sanitized link in semantic metadata | High | Do not implement live `foreignObject`/iframe in a network-free renderer; document this as the intentional offline path |
| SVG output | Text and links remain semantic; paths/images/clips are self-contained; fonts are normally subset/inlined | Canonical typed serializer fed by ported display operations; embed full or deterministically selected WOFF2 assets; sanitize URL schemes and embedded SVG | High | Full fonts are larger than official subsets; exact DOM is out of scope; metadata embedding is a separate nonvisual compatibility decision |
| PNG output | Browser Canvas raster, including browser text and image decoders | Rasterize the controlled SVG/display tree with `resvg`/`tiny-skia`, then deterministic pure-Rust PNG encoding | Strong candidate, not exact Canvas clone | Antialiasing, font rasterization, image scaling, dark filter, and Canvas-vs-SVG frame edge cases establish the tolerance burden |
| PDF output | No official implementation; required output is vector-first with useful semantics | `svg2pdf` for vector visual content/selectable text plus `pdf-writer` page construction and link annotations, or a direct display-list PDF backend | Conditional | Plain conversion loses links; root filters rasterize; byte determinism and panic-free handling need changes; text extraction quality needs testing |

## Component evidence and fidelity assessment

### Rough geometry: port upstream; do not accept `roughr` output as parity

Excalidraw's [`generateRoughOptions`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts#L30-L121) modifies roughness by element size, changes dotted/dashed stroke width, explicitly fixes fill weight and gap, preserves vertices conditionally, and supplies the element seed. Its [`_generateElementShape`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts#L299-L538) selects rounded paths, polygons, curves, elbow paths, and arrowhead construction. These rules are part of the visual algorithm, not adapter configuration.

Rough.js 4.6.4 defaults and operation generation live in [`generator.ts`](https://github.com/rough-stuff/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/src/generator.ts) and [`renderer.ts`](https://github.com/rough-stuff/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/src/renderer.ts). Its PRNG is a compact, reproducible algorithm: [`Random.next`](https://github.com/rough-stuff/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/src/math.ts#L7-L21) uses `Math.imul(48271, seed)`, masks to 31 bits, and divides by `2**31`.

`roughr 0.12.0` describes itself as a Rough.js port, and it implements the same broad primitive and filler vocabulary. That is useful prior art, not fidelity evidence. At its release commit [`b1c2d96c944da4e74275aa09892be14e2d54445a`](https://github.com/orhanbalci/rough-rs/tree/b1c2d96c944da4e74275aa09892be14e2d54445a):

- [`Options::default` and `Options::random`](https://github.com/orhanbalci/rough-rs/blob/b1c2d96c944da4e74275aa09892be14e2d54445a/roughr/src/core.rs#L136-L187) use `StdRng`, seed `345`, and `bowing: 2.0`.
- Rough.js defaults to seed `0` and `bowing: 1` ([source](https://github.com/rough-stuff/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/src/generator.ts#L13-L35)).
- The Rust crate accepts `f32` for most options even when geometry is `f64`, introducing additional rounding points.
- Its manifest directly depends on `rand 0.8` ([source](https://github.com/orhanbalci/rough-rs/blob/b1c2d96c944da4e74275aa09892be14e2d54445a/roughr/Cargo.toml#L16-L24)). The WASM probe failed in `getrandom 0.2.17`; upstream documents that `wasm32-unknown-unknown` requires its JavaScript feature ([source](https://docs.rs/getrandom/0.2.17/getrandom/#webassembly-support)). Enabling browser entropy would compile it but would not fix fidelity and is unnecessary because Excalidraw supplies deterministic seeds.

The smallest credible implementation effort is either a new internal port or a tightly controlled fork that replaces the random/default/number semantics and then demonstrates operation-level equivalence. The port needs only the Rough.js paths Excalidraw calls, but that includes all current fill styles and transitive point/path algorithms. Each generated operation tuple should be compared before SVG serialization so backend antialiasing does not hide algorithm defects.

### Linear elements, bindings, arrowheads, and bounds

No generic Rust graph router or arrow library matches this surface. Excalidraw decides whether saved points become a polygon, linear path, smooth curve, or rounded elbow path, then derives arrowhead direction from Rough.js Bézier operations. [`getArrowheadPoints`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/bounds.ts#L543-L729) includes type-specific sizes/angles, short-segment scaling, circles, diamonds, bars, and crowfeet.

Bindings are less expansive for static rendering than the word suggests. Restore normalizes points and repairs references, while paint consumes saved points; it does not reroute ordinary arrows against target shapes. See the [line/arrow restore cases](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts#L295-L364). The required static subset is:

- Point normalization and legacy migration.
- Bound-element lookup and broken-reference policy.
- Rough curve generation and curve bounds.
- Arrowhead construction.
- Bound-arrow text midpoint and masking.
- Elbow path generation from saved points and validation of extreme/invalid points.

For example, [`getBoundTextElementPosition`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/linearElementEditor.ts#L1510-L1556) treats odd and even point counts differently and computes a curve midpoint for the central segment. This must be ported. A generic midpoint approximation will visibly move labels.

`kurbo`, `euclid`, or an internal typed point module can implement rotations, Bézier evaluation, and bounds below this layer. They must not replace the pinned decisions above.

### Freedraw: direct Perfect Freehand port

Excalidraw passes these exact options to Perfect Freehand: `size = strokeWidth * 4.25`, `thinning = 0.6`, `smoothing = 0.5`, `streamline = 0.5`, `easeOutSine`, actual/simulated pressure, and the committed-point flag. It then emits a closed midpoint-quadratic SVG path and truncates long decimals ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts#L1005-L1069)).

Perfect Freehand 1.2.0 is small, deterministic, dependency-free numerical code. Its top-level composition is explicit in [`getStroke.ts`](https://github.com/steveruizok/perfect-freehand/blob/fa0b754d49bb60813c24cbd7fdcd5cda40f560f3/packages/perfect-freehand/src/getStroke.ts#L20-L25), and its outline implementation and special `PI + 0.0001` behavior are in [`getStrokeOutlinePoints.ts`](https://github.com/steveruizok/perfect-freehand/blob/fa0b754d49bb60813c24cbd7fdcd5cda40f560f3/packages/perfect-freehand/src/getStrokeOutlinePoints.ts#L17-L83).

Two Rust ports were inspected:

- `perfect_freehand 0.1.1` is dependency-free and compiled for `wasm32-unknown-unknown`, but its repository contained no test files in the inspected revision.
- `freedraw 1.0.x` is also dependency-free and compiled for WASM. Its tests assert broad properties such as non-empty/closed output rather than equality with upstream snapshots ([source](https://github.com/ducflair/freedraw/blob/main/tests/get_stroke_test.rs)).

Both are useful references. Neither currently supplies stronger fidelity evidence than directly porting the pinned 1.2.0 source and importing the official upstream snapshot corpus. A direct port also keeps the provenance and future baseline diff obvious.

### Text, fonts, and layout

#### What must and need not be laid out

For valid current scene documents, static export does not reflow paragraphs. It splits `element.text` on newlines, uses the stored width for horizontal alignment, computes line advance as `fontSize * lineHeight`, and computes a family-specific baseline offset. The SVG implementation is visible at [`staticSvgScene.ts` lines 597-657](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L597-L657); Canvas performs the analogous `fillText` loop at [`renderElement.ts` lines 472-514](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts#L472-L514).

That makes a generic paragraph engine such as `cosmic-text` the wrong fidelity boundary for normal input: rewrapping could change line breaks and element bounds. `cosmic-text` is a capable pure-Rust shaping/layout/raster library ([documentation](https://docs.rs/cosmic-text/0.19.0/cosmic_text/)), but its wrapping and fallback policies are its own, not Excalidraw's Canvas measurement algorithm.

The port should preserve stored lines and directly port Excalidraw's baseline and alignment math. Legacy or programmatically incomplete scenes are a contract decision. Excalidraw restore can infer line height, and an opt-in `refreshDimensions` path invokes text measurement ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts#L490-L551)). If the Rust public API promises that refresh path, browser-compatible width measurement and wrapping become a separate algorithm port and conformance problem.

#### Shaping and rasterization

`rustybuzz` is a complete Rust port of HarfBuzz's shaping algorithm ([documentation](https://docs.rs/rustybuzz/0.20.1/rustybuzz/)). `usvg 0.45.1` uses `fontdb`, `rustybuzz`, Unicode bidi, script, and vertical-orientation crates under its `text` feature, explicitly described as text-to-path support in its [manifest](https://github.com/linebender/resvg/blob/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/usvg/Cargo.toml#L38-L55). This stack compiled for `wasm32-unknown-unknown` when filesystem font features were disabled. It is the strongest existing shaping path for raster/PDF because it shares HarfBuzz lineage with browsers and is already integrated into `resvg` and `svg2pdf`.

It is still not a browser clone. Hinting, glyph rasterization, fallback choice, bidi edge cases, emoji presentation, and antialiasing can differ. Perceptual acceptance needs corpus evidence, not an assumption based on HarfBuzz ancestry.

#### Font storage and WOFF2

Excalidraw ships WOFF2 faces and generates per-scene subset WOFF2 data URLs for SVG. Its [`Fonts.generateFontFaceDeclarations`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts#L162-L200) selects families and CJK fallback, and [`ExcalidrawFontFace`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts#L32-L87) fetches and subsets WOFF2. Upstream performs that subsetting with embedded HarfBuzz and WOFF2 WASM modules, not pure Rust ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-shared.chunk.ts#L1-L80)).

`fontdb` stores and CSS-matches TrueType/OpenType face data, but explicitly does not implement font fallback and has no font types beyond TrueType ([documentation](https://docs.rs/fontdb/0.23.0/fontdb/#non-goals)). `ttf-parser` is safe, allocation-free, `no_std`/WASM-compatible parsing for sfnt TrueType/OpenType/AAT data ([documentation](https://docs.rs/ttf-parser/0.25.1/ttf_parser/#features)); it does not make WOFF2 web containers directly usable as a font database face.

Viable asset handling options, without choosing one here, are:

| Option | SVG | PNG/PDF | Determinism and risk |
|---|---|---|---|
| Bundle official WOFF2 plus predecoded sfnt equivalents | Embed WOFF2 directly, whole or by deterministic shard selection | Load trusted sfnt bytes into `fontdb`; `svg2pdf` can subset sfnt for PDF | Simplest runtime, deterministic, no runtime WOFF2 parser; duplicates asset storage |
| Decode bundled WOFF2 at runtime | Reuse original bytes | Decode once, then load sfnt | `wuff 0.2.8` is a new pure-Rust WOFF/WOFF2 decoder ([docs](https://docs.rs/wuff/0.2.8/wuff/)); it needs compatibility, limits, fuzz, and WASM validation before trust |
| Pure-Rust webfont subsetting | Closest file-size behavior to official output | Not needed for PDF, where `subsetter` is specialized | General webfont subsetting remains higher risk; embedding full faces is perceptually equivalent and avoids making subsetting a launch blocker |

The predecoded-sfnt option does not require an OS package or runtime build tool if generated assets are reviewed and checked in. Licensing and exact asset set remain ticket #4's decision.

Excalidraw's font metadata exposes an intrinsic platform gap: `Segoe UI Emoji` is marked local and fallback, while Xiaolai is a shipped CJK fallback ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts#L114-L133)). The final specification must name a bundled emoji fallback or define a diagnostic/placeholder policy.

### Images, crop/flip, and image codecs

Image placement should be a direct port, not delegated to an SVG library's defaults. Official SVG rounds displayed width/height, computes uncropped dimensions and normalized crop offsets, uses a mask for crop, a clip path for rounded corners, a separate scale transform for mirroring, then translation/rotation and opacity ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L408-L565)).

Excalidraw `v0.18.1` accepts SVG, PNG, JPEG, GIF, WebP, BMP, ICO, AVIF, and JFIF MIME labels ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts#L204-L214)). The Rust landscape is:

| Format | Pure-Rust path | SVG-pivot handling | Gap |
|---|---|---|---|
| PNG | `png` or `image`; `tiny-skia` PNG path | Native in `resvg` | Enforce dimensions and allocation limits; preserve expected sRGB/alpha behavior |
| JPEG/JFIF | `zune-jpeg` or `image` | Native in `resvg`; treat JFIF MIME as JPEG by magic | Browser decoder/color differences and malformed-file policy |
| GIF | `gif`/`image` | Native in `resvg` with raster feature | Define deterministic frame selection; official browser timing may sample an animated frame |
| WebP | `image-webp`/`image` | Native in `resvg` with raster feature | Define animation policy and limits |
| BMP, ICO | `image` supports both ([format table](https://docs.rs/image/0.25.5/image/codecs/#supported-formats)) | Predecode to bounded RGBA or canonical PNG because `usvg` does not identify them | Verify alpha/mask and ICO subimage selection against browser oracle |
| Embedded SVG | `usvg` static parser plus `resvg` | Native only for supported sanitized static subset | Block external files; unsupported browser SVG features need fallback/diagnostics |
| AVIF | No mature permissive option established | Preserve raw data only in final SVG; current pivot cannot rasterize | `image 0.25.5` requires C `libdav1d`; `zenavif` is AGPL/commercial; `oxideav-avif 0.0.10` ships a pixel-decoder stub |

`image 0.25.5` describes native Rust implementations and exposes `Limits`, but only width and height are strict while allocation is a best-effort limit ([documentation](https://docs.rs/image/0.25.5/image/struct.Limits.html)). The renderer still needs its own preflight byte, pixel, frame, and aggregate-scene budgets.

AVIF evidence is especially sharp. The `image 0.25.5` format table says AVIF decoding requires the `avif-native` feature and C `libdav1d` ([source](https://docs.rs/image/0.25.5/image/codecs/#supported-formats)). `zenavif 0.1.6` claims a safe pure-Rust decoder but is licensed AGPL-3.0-only or commercially ([source](https://docs.rs/zenavif/0.1.6/zenavif/)). `oxideav-avif 0.0.10` is MIT, but its published manifest has no AV1 decoder dependency ([source](https://github.com/OxideAV/oxideav-avif/blob/1fd75128fc2e0e4ee67326633e5264ece5712494/Cargo.toml#L13-L36)) and the shipped pixel-decoder stub returns `Unsupported` for every packet ([source](https://github.com/OxideAV/oxideav-avif/blob/1fd75128fc2e0e4ee67326633e5264ece5712494/src/av1_stub.rs#L114-L137)). None is an implementation-ready public-library dependency under the stated constraints.

Color-managed source images are another bounded gap. `svg2pdf` explicitly writes raster images in DeviceRGB without color management ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/lib.rs#L47-L53)). A common-sRGB contract may be sufficient for perceptual parity, but ICC/P3/HDR input needs an explicit policy. `moxcms` is a pure-Rust ICC/CICP candidate ([documentation](https://docs.rs/moxcms/0.8.1/moxcms/)); it should be introduced only if the conformance corpus demonstrates a required color gap.

### Frames, clipping, opacity, and compositing

`resvg`/`tiny-skia` are a strong low-level match for the operations Excalidraw actually emits: Bézier fills/strokes, affine transforms, source-over alpha, images, masks, clip paths, and filters. `tiny-skia` is a pure-Rust Skia subset with explicit low-level transforms, clipping masks, styles, blend modes, and premultiplied pixmaps ([documentation](https://docs.rs/tiny-skia/0.11.4/tiny_skia/)). `usvg` resolves CSS, references, basic shapes, `use`, nested SVG, text, markers, filters, and recursive elements into a typed static tree, and forbids unsafe code ([source](https://github.com/linebender/resvg/blob/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/usvg/src/lib.rs#L4-L50)).

The geometry and eligibility decisions still belong in the Excalidraw port. Canvas's [`shouldApplyFrameClip`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/frame.ts#L858-L916) considers intersection, containment, groups, and frame membership. SVG wraps frame members with a `clip-path` based on the containing frame ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L58-L77)). The official paths therefore need edge-case comparison before one shared clip rule is declared perceptually equivalent.

Opacity is multiplicative between frame and child in both paths. Arrow-bound text uses a mask/clear region. Images use crop masks and round clips. Those operations are well covered by SVG and `resvg`; deeply nested or adversarial clips must be bounded.

Dark export is the important exception. Official SVG puts `filter="invert(93%) hue-rotate(180deg)"` on the root ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L325-L334)), and raster images receive a second inverse/hue/saturation correction ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts#L68-L94)). `svg2pdf` documents that filtered objects are rasterized ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/lib.rs#L1-L10)). A PDF path must lower this known filter to equivalent colors/image processing or accept whole-page rasterization, which conflicts with vector-first output.

### Offline embeddables

The network-free path is well defined and does not require a browser engine. Canvas export supplies an empty embed-validation map, so embeds render as placeholders. SVG export defaults `renderEmbeddables` to false; its embed branch draws a placeholder and label and creates an external link instead of a live frame. Only explicit opt-in creates `foreignObject`/iframe content ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L172-L272)).

The Rust renderer can faithfully implement the default offline appearance, carry the sanitized target as an SVG anchor/PDF annotation, and diagnose opt-in live rendering as unsupported. It should never fetch previews or iframe URLs. This is fidelity to the official offline/default path, not a fallback invented by the Rust renderer.

## SVG-pivot analysis

### Strengths

1. The official SVG painter already specifies the required vector decomposition for rough paths, freedraw, text lines, images, masks, frame clips, opacity, links, and transforms.
2. A canonical SVG makes SVG output direct and gives PNG/PDF a shared visual artifact, reducing drift among backends.
3. `usvg` simplifies the difficult static SVG subset into a typed tree and explicitly removes malformed and recursive content.
4. `resvg`/`tiny-skia` provides pure-Rust deterministic rasterization without OS graphics libraries.
5. `svg2pdf` preserves most paths, fills, clips, masks, images, and text as vector/selectable content, rasterizing only filtered groups.
6. The tested feature sets compile for `wasm32-unknown-unknown` without system-font or memory-map features.

### Fidelity and semantic losses

| Loss | Why it occurs | Consequence | Possible architecture response |
|---|---|---|---|
| SVG anchors disappear in PDF | `usvg` explicitly lists `a` as unsupported static SVG and treats links as visual groups ([source](https://github.com/linebender/resvg/blob/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/usvg/src/lib.rs#L39-L45)); `svg2pdf` has no annotation code | PDF links are lost | Carry normalized link and transformed hit geometry beside SVG; construct the page with `pdf-writer` link annotations |
| Root dark filter rasterizes PDF | `svg2pdf` rasterizes filtered objects/groups | Whole page may cease to be vector-first | Lower the known filter into vector colors and image transforms before PDF, or implement a PDF-specific paint path |
| Browser text differs | Official Canvas uses browser `fillText`; pivot uses Rustybuzz outlines/raster | Small glyph, hinting, fallback, and bidi differences | Freeze fonts/fallback, preserve stored lines, compare perceptually, allow targeted outline/raster fallback only if specified |
| Official Canvas and SVG edge cases differ | They are separate painters with different frame eligibility, image sampling, and browser APIs | A PNG rasterized from official-like SVG is not automatically the official PNG | Build a dual-oracle corpus; retain backend policy switches where differences exceed tolerance |
| Embedded WOFF2 CSS is not sufficient for `usvg` | Internal font layout uses an explicitly populated `fontdb`; `fontdb` consumes sfnt faces | Text may disappear or fall back during PNG/PDF | Load trusted predecoded font bytes out of band; use CSS data fonts only in final SVG |
| Unsupported image MIME types | `usvg`'s data resolver recognizes only JPEG, PNG, GIF, WebP, SVG/SVGZ ([source](https://github.com/linebender/resvg/blob/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/usvg/src/parser/image.rs#L48-L75)) | BMP/ICO/AVIF/JFIF labels can disappear | Predecode supported formats and inject canonical PNG/pixels; retain an AVIF decision |
| External local files are accessible by default | `usvg` default string resolver treats strings as file paths ([source](https://github.com/linebender/resvg/blob/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/usvg/src/parser/image.rs#L78-L113)) | Untrusted generated/embedded SVG could read local files | Always replace the string resolver with a deny resolver and admit only already-budgeted data assets |
| Reparse overhead and parser surface | Typed operations are serialized, parsed, normalized, and sometimes flattened | More memory, time, and denial-of-service surface, especially in WASM | Keep a typed display record as source of truth and make SVG parsing an adapter detail, or construct a controlled `usvg` tree if its API permits |
| PDF text and structure are partial | `svg2pdf` embeds selectable glyphs and ToUnicode mappings but does not produce a tagged document or links | Selection is useful, but reading order/accessibility need evidence | Specify required PDF semantics narrowly and test extraction; add annotations/structure only where promised |

### Verdict

An SVG visual pivot is viable for SVG and likely PNG. It is viable for PDF only as part of a hybrid that retains semantic side data and has a strategy for dark filters, fonts, deterministic resource ordering, and link annotations. “Generate SVG, parse it with default `usvg`, call `svg2pdf::to_pdf`” is not sufficient for all outputs.

## PDF component path

`svg2pdf 0.13.0` is pure Rust and its default feature set compiled for `wasm32-unknown-unknown` in this survey. It supports paths, gradients, patterns, clip paths, masks, transforms, text, raster images, and nested SVG; its public options default to actual embedded/selectable text ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/lib.rs#L133-L175)). Its text renderer writes subset fonts and per-glyph text operations ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/render/text.rs#L200-L378)). `subsetter` is deliberately specialized for TrueType/CFF PDF embedding and keeps copyright data ([documentation](https://docs.rs/subsetter/0.2.0/subsetter/)).

For useful links, `pdf-writer 0.12.0` exposes page annotation arrays, Link annotations with rectangles and quad points, and URI actions ([Annotation docs](https://docs.rs/pdf-writer/0.12.0/pdf_writer/writers/struct.Annotation.html), [Action docs](https://docs.rs/pdf-writer/0.12.0/pdf_writer/writers/struct.Action.html), [Page docs](https://docs.rs/pdf-writer/0.12.0/pdf_writer/writers/struct.Page.html)). `svg2pdf::to_chunk` can produce a visual Form XObject for a page assembled by `pdf-writer`, allowing link annotations to be added from the semantic sidecar without parsing generated PDF bytes.

This path remains conditional for four reasons:

1. It does not solve root-filter vector loss.
2. It does not automatically define link hit regions for rotated/irregular elements; the Excalidraw bounds port must provide transformed rectangles or quads.
3. `svg2pdf` has internal `unwrap`/`panic` sites in text rendering ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/render/text.rs#L242-L255)), image rendering ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/render/image.rs#L87-L113)), path conversion ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/render/path.rs#L54-L78)), and group bounds ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/render/group.rs#L78-L92)). Generated canonical SVG narrows reachability, but panic-free untrusted input requires fuzzing and likely a pinned patch/fork.
4. `Context::write_global_objects` iterates `HashMap::values_mut` for fonts ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/util/context.rs#L71-L86)), and `ResourceContainer::finish` serializes filtered `HashMap` values without sorting ([source](https://github.com/typst/svg2pdf/blob/ccab6d7a3081f512d412dfef456caed2caf3a698/src/util/resources.rs#L154-L170)). Same-build byte stability must not be assumed.

The direct alternative is to render the typed display record with `pdf-writer`, reuse Rustybuzz shaping and `subsetter`, and implement only the path/image/clip/opacity operations Excalidraw emits. That avoids SVG semantic loss and can make ordering explicit, but duplicates mature SVG-to-PDF paint handling. The architecture ticket should compare this cost against maintaining a small `svg2pdf` fork/adapter.

## Determinism

The pure-Rust constraint helps but does not by itself establish deterministic output.

Required controls are:

- Port Rough.js's exact 32-bit PRNG and PRNG-consumption order. Never substitute `rand`.
- Use `f64` for upstream JavaScript calculations unless a pinned source explicitly quantizes earlier.
- Canonically reject or normalize non-finite/overflowing geometry before bounds or allocation.
- Define numeric serialization and negative-zero behavior. Excalidraw rounds Rough.js SVG path values to at most two decimals ([constant](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts#L307)); freedraw has its own two-decimal regex.
- Assign internal SVG/PDF resource IDs in scene traversal order, not hash iteration order. Do not embed timestamps, random IDs, or host paths.
- Use a fixed PNG encoder configuration and omit time chunks.
- Freeze fonts and all fallback ordering. Never scan system fonts in production.
- Define first-frame behavior for animated raster inputs.
- Disable external resources and locale-dependent behavior.
- Patch or wrap `svg2pdf` resource serialization to sort deterministically, then test repeated fresh processes rather than repeated calls in one process.

Cross-platform byte identity is not currently a destination requirement, but Linux/macOS/Windows/WASM perceptual conformance is. Transcendental and raster edge behavior can still vary by target; the CI corpus must exercise all targets.

## Safety and untrusted inputs

The final renderer should parse the scene into a bounded model before any backend allocation. Component safety claims do not replace end-to-end budgets.

The required controls implied by this survey are:

| Surface | Required control |
|---|---|
| JSON | Maximum input bytes, element count, string length, point count, nesting depth, and asset aggregate; reject duplicate/invalid IDs according to a deterministic policy |
| Numbers | Reject non-finite, out-of-range, and overflow-producing dimensions/transforms before bounds; use checked pixel and byte arithmetic |
| Rough/freedraw | Limit points, generated operations, filler scan lines, and total path segments; validate zero-length and degenerate geometry |
| Raster assets | Validate MIME by magic, cap encoded bytes, dimensions, pixels, frames, decode allocations, and aggregate decoded bytes before allocation; do not trust `image::Limits::max_alloc` as strict |
| Embedded SVG | Cap bytes/nodes/depth/path segments/data URLs; deny all string/file/network resolvers; ignore scripts/events/`foreignObject`; bound nested SVG and filters |
| Fonts | Treat bundled fonts as trusted immutable assets; separately limit caller-injected font bytes and WOFF2 expansion; reject invalid faces without fallback to host fonts |
| Links | Reproduce upstream sanitization intent, but explicitly allow only chosen schemes for SVG/PDF (`https`, `http`, perhaps `mailto` and internal links); never emit JavaScript actions |
| PDF | Bound graphics-state nesting and resources; eliminate reachable panic/unwrap paths; fuzz canonical display records through the selected PDF backend |
| Output | Cap canvas/page dimensions, total output bytes, and filter intermediate surfaces; return structured diagnostics rather than silently exhausting memory |

Excalidraw itself sanitizes links before writing SVG anchors ([source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/url.ts#L1-L35)). The Rust port should match visible normalization where safe but make the accepted protocol set explicit for a public library.

`usvg` and `resvg` both forbid unsafe code in their top-level crates, and `usvg` detects recursive references. Their default image string resolver nevertheless permits local files, so default options are not acceptable for untrusted input. `ttf-parser` has unusually strong parser safety properties, including no unsafe code, no allocations, checked arithmetic, and recursion limits ([documentation](https://docs.rs/ttf-parser/0.25.1/ttf_parser/#safety)). These are good building blocks inside, not substitutes for, the limits ticket #7.

## WASM and platform evidence

The following source-only probes used Rust/Cargo `1.97.1`, target `wasm32-unknown-unknown`, and clean temporary target directories:

| Component | Probe | Result |
|---|---|---|
| `resvg/usvg 0.45.1` | `cargo check --target wasm32-unknown-unknown --no-default-features --features text,raster-images -p resvg` | Passed; includes `fontdb`, `rustybuzz`, PNG, JPEG, GIF, WebP, filters, and `tiny-skia`; excludes system-font and memmap features |
| `svg2pdf 0.13.0` | `cargo check --target wasm32-unknown-unknown -p svg2pdf` | Passed with default image/filter/text features |
| `perfect_freehand 0.1.1` | `cargo check --target wasm32-unknown-unknown` | Passed |
| `freedraw` repository head | `cargo check --target wasm32-unknown-unknown` | Passed |
| `roughr 0.12.0` | `cargo check --target wasm32-unknown-unknown -p roughr` | Failed because `getrandom 0.2.17` has no default `wasm32-unknown-unknown` backend |

Passing `cargo check` proves target compatibility of the selected feature graph, not browser runtime correctness or bounded memory behavior. Browser smoke tests must still instantiate the library, load bundled fonts, render a text/image/filter scene, and produce SVG/PNG/PDF bytes.

For native self-containment, do not enable `usvg/resvg` `system-fonts` or `memmap-fonts`; their [feature declarations](https://github.com/linebender/resvg/blob/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/resvg/Cargo.toml#L32-L44) separate those from text/raster support. Load bundled bytes into an in-memory `fontdb`. The same core can then compile on Linux, macOS, Windows, and WASM without Fontconfig, FreeType, Cairo, browser Canvas, or required OS packages.

`resvg 0.47.0` is newer than the MVP's `0.45.1`, but `svg2pdf 0.13.0` depends on the `usvg 0.45` API. The architecture decision should either pin a common tested generation, accept separate SVG parses for PNG and PDF, or update/fork `svg2pdf`; it should not assume `usvg::Tree` types are interchangeable across versions.

## Architecture implications, without selecting the architecture

The evidence establishes several boundaries that any final architecture must respect.

### A typed rendering record is needed somewhere

Even an SVG-pivot design needs data that SVG normalization discards:

- Element identity and diagnostics.
- Link target plus transformed hit geometry.
- Original text and font/glyph association.
- Asset provenance, decoded dimensions, and fallback status.
- Frame membership and effective opacity.
- Backend policy for dark filters and unsupported images.

This can be a full backend-neutral display list, or a narrower semantic sidecar attached to a canonical SVG. The final architecture must name it; reparsing final SVG cannot reconstruct all semantics.

### Ports should be isolated from backend crates

Rough.js/Perfect Freehand/Excalidraw ports should produce deterministic paths and placement records independent of SVG strings, `tiny-skia`, or PDF objects. This makes operation-level conformance possible and avoids tying an appearance algorithm to a replaceable backend.

### Fonts are an injected asset service

The same frozen font catalog should provide:

- WOFF2 data for semantic SVG text.
- sfnt bytes and fallback ordering for Rustybuzz/resvg.
- sfnt bytes and glyph data for PDF subsetting.
- family metrics for Excalidraw baseline placement.

System discovery must not be a hidden fallback.

### Images need normalization before the visual backend

A bounded asset layer should validate MIME/magic, decode or sanitize, select a deterministic animation frame, apply color policy, and expose either trusted encoded bytes plus dimensions or bounded premultiplied pixels. This closes the gap between Excalidraw's broad browser MIME surface and `usvg`'s narrower image resolver.

### PDF needs an explicit semantic stage

If `svg2pdf` remains, `to_chunk` plus a controlled `pdf-writer` page can add annotations and deterministic document structure. If a direct PDF backend is chosen, the same display record can emit paths/text/images/clips directly. In either case, PDF is not merely another filename extension on the raw SVG serializer.

## Risks and unknowns

| Risk or unknown | Why it matters | How to resolve |
|---|---|---|
| Reference release is not selected | Rough.js, fonts, arrow types, image MIME surface, and export quirks can change | Pin baseline in ticket #9, then regenerate this component lock and port scope |
| Rough direct-port size | Rough.js fillers and transitive point/path code are larger than basic shapes suggest | Build operation-level tracer prototypes for every Excalidraw option before estimating implementation |
| Browser-vs-resvg text | Same HarfBuzz lineage does not guarantee same raster, fallback, or metrics | Corpus across Latin ligatures, combining marks, Arabic/Hebrew, CJK, emoji, RTL mixing, rotation, and bound text |
| Official Canvas-vs-SVG disagreement | One pivot cannot exactly emulate two different painters | Render both official outputs and classify differences against perceptual tolerance; retain explicit backend rules where necessary |
| AVIF | Current baseline accepts it but no component fits all constraints | Decide baseline exception, defer with diagnostics, or fund/evaluate a permissive decoder |
| Animated images | Official browser sampling can depend on load/render timing | Probe official exporter repeatedly; specify first-frame or another deterministic static policy |
| Embedded SVG fidelity | Browser SVG image support exceeds `usvg` static subset | Curate safe fixtures, define supported static subset, and choose vector vs bounded raster fallback |
| Color management | PNG/PDF can differ for ICC/P3/HDR assets | Decide common-sRGB contract; prototype `moxcms` only if corpus demands it |
| SVG font size | Full WOFF2 embedding is larger than official subsetting | Measure representative Latin/CJK scenes; decide full fonts, shards, or audited pure-Rust subsetter |
| PDF extraction/reading order | Selectable per-glyph text may not copy/read ideally | Test with `pdftotext` and multiple viewers, including ligatures and RTL; define required semantics |
| `svg2pdf` determinism/panics | Same-build byte stability and panic-free untrusted operation are destination constraints | Fresh-process repeat tests, sorted-resource patch, fuzz generated trees, and remove reachable panic sites |
| WASM memory | SVG serialization plus parse tree plus pixmap can hold multiple scene copies | Measure peak memory; consider direct tree/display-list rendering and streaming output where possible |

## Suggested prototypes and decisions

These are decision-producing experiments, not implementation tickets for the renderer.

1. **Rough operation equivalence.** Feed Rough.js 4.6.4 and a minimal Rust port/fixed `roughr` fork the Cartesian product of primitive, fill style, stroke style, roughness, roundness path, seed edge cases, and degenerate dimensions. Compare operation types/counts/data before formatting. Decide new port versus repaired fork.
2. **Freedraw snapshot equivalence.** Import Perfect Freehand 1.2.0's official snapshots, add Excalidraw's exact option sets and SVG path formatter, and require point/path equivalence within a named numeric tolerance. Decide direct port packaging.
3. **Dual official-painter corpus.** Export the same scenes with official SVG and Canvas PNG, then rasterize official SVG through pinned `resvg`. Include frame/group edge cases, bound-arrow text masks, dark mode, opacity, crop/flip/round image, and all fills. Decide whether SVG-raster PNG is acceptable or where a Canvas-specific rule is required.
4. **Frozen-font text spike.** Predecode the candidate official WOFF2 assets to sfnt, load only those bytes into `fontdb`, and compare SVG-browser, official Canvas, `resvg`, and `svg2pdf` on Latin, CJK, RTL, combining marks, ligatures, emoji, multiline alignment, rotation, and bound text. Decide fallback catalog and any path/raster fallback.
5. **Image format matrix.** Probe every MIME label accepted by the selected baseline, including malformed MIME versus magic, alpha, ICC, crop, mirror, rounded corners, large dimensions, and animated inputs. Decide first-frame, color, AVIF, and unsupported-format diagnostics.
6. **Embedded-SVG safety/fidelity spike.** Test nested SVG, CSS, data images, external file/HTTP references, filters, fonts, `foreignObject`, recursion, and bombs with a deny-all `ImageHrefResolver`. Decide sanitized vector subset and raster fallback.
7. **PDF semantic spike.** Use `svg2pdf::to_chunk` and `pdf-writer` to build a page with sorted resources, link annotations/quads, selectable text, clips, opacity, and dark-mode lowering. Inspect vector objects, links, text extraction, viewer behavior, and repeat bytes. Compare effort with a direct display-list PDF prototype.
8. **Fresh-process determinism probe.** Render a multi-font, multi-image, clip/filter scene repeatedly on every target. Compare SVG/PNG/PDF bytes and visual hashes. Specifically verify `svg2pdf` resource ordering after any patch.
9. **Adversarial budget probe.** Fuzz scene restore, rough/freedraw generation, embedded SVG, image decoders, fonts, and PDF conversion under hard memory/operation limits on native and WASM. Feed results into ticket #7.

The newly sharp decisions for subsequent map tickets are:

- Whether the selected baseline requires AVIF raster parity or declares a diagnosed exception.
- Whether PNG parity is judged against official Canvas, official SVG rasterization, or both with feature-specific rules.
- Whether PDF uses a maintained `svg2pdf` adaptation with semantic sidecar or a direct display-list backend.
- Which exact bundled emoji/CJK fallback catalog replaces host-dependent fonts.
- Whether SVG embeds full WOFF2, deterministic source shards, or pure-Rust subsets.
- Which embedded-SVG subset, link schemes, animation frame, and color-space policies are safe and compatible.

## Bottom line for the map

Pure Rust and `wasm32` do not block perceptually equivalent Excalidraw static exports. The appearance-defining layer must be a set of pinned upstream algorithm ports, not `roughr` plus generic geometry. `resvg`/`tiny-skia`, Rustybuzz/fontdb, common pure-Rust image decoders, and `svg2pdf`/`pdf-writer` are credible lower layers. The remaining blockers are explicit and narrow enough for later decisions: AVIF, frozen fallback fonts, embedded-SVG policy, official Canvas-vs-SVG tolerances, dark-mode PDF lowering, PDF links/determinism, and untrusted-input limits.
