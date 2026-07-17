# Asset licensing and distribution for perceptually equivalent Excalidraw exports

Research ticket: [excalidraw-render.orig#4](https://github.com/sagikazarmark/excalidraw-render.orig/issues/4)

Destination map: [excalidraw-render.orig#1](https://github.com/sagikazarmark/excalidraw-render.orig/issues/1)

Researched: 2026-07-17

## Executive answer

The self-contained native and WASM distributions can bundle the official open font files and the small static-export icon paths. The redistributable font assets audited here are licensed under the SIL Open Font License 1.1 (OFL-1.1), except Comic Shanns, which is MIT-licensed. The two Font Awesome placeholder icons are CC BY 4.0. The rendering code most likely to be ported is MIT-licensed, subject to retaining each applicable copyright and permission notice. None of these licenses requires a runtime OS package, a network request, or distribution of application source code.

For the provisional Excalidraw v0.18.1 baseline, a complete browser-rendering font bundle contains 230 WOFF2 files in eight families, totaling 13,025,924 bytes (about 12.42 MiB): Excalifont, Nunito, Lilita One, Comic Shanns, Virgil, Cascadia Code, Liberation Sans, and the 209-file Xiaolai fallback. Assistant's four WOFF2 files are editor UI assets and are not needed by static scene export. The package also contains build-time TTF assets for deterministic server rendering, notably Noto Emoji and Liberation Sans variants.

Two official browser fallbacks cannot be made self-contained by copying official Excalidraw assets:

- `Helvetica` is deliberately local-only and resolves to Helvetica on macOS or Arial on Windows.
- `Segoe UI Emoji` is deliberately local-only and is the fallback for every scene font.

The renderer must therefore make a specification decision. The recommended deterministic policy is to follow Excalidraw's server-export strategy: use the bundled OFL-licensed Liberation Sans and Noto Emoji assets instead of Helvetica/Arial and Segoe UI Emoji, and define the pinned server-style output as authoritative for those glyphs. This is self-contained and WASM-safe, but it cannot be described as pixel identity with every OS-dependent browser export.

The release should embed assets with `include_bytes!` or an equivalent generated Rust byte table, expose no system-font discovery path in the default renderer, and perform font parsing, shaping, WOFF2 decoding, rasterization, and optional subsetting in pure Rust. Both the native and WASM packages must carry a human-readable third-party notice bundle. Exact upstream WOFF2 files should be preserved rather than pre-converted where practical; this minimizes both parity risk and OFL Reserved Font Name risk.

Do not copy Excalidraw's embedded HarfBuzz and WOFF2 WebAssembly binaries into production. They optimize SVG font size, not appearance. Full official WOFF2 faces or existing official Unicode-range chunks can be embedded without subsetting. A later pure-Rust subsetter may be used after its own dependency and OFL name-preservation audit.

This report records license evidence and engineering implications, not legal advice. Questions explicitly marked for counsel or maintainer decision should not be treated as legal conclusions.

## Scope and evidence baseline

The destination has not yet selected its reference renderer in ticket #9. This audit therefore uses the latest official release available on the research date, [Excalidraw v0.18.1 at commit `a2ec2889babf7d2295469c6d90ebe77fae57df84`](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84), as a provisional concrete inventory. The official package metadata pins `roughjs` 4.6.4, `perfect-freehand` 1.2.0, and `points-on-curve` 1.0.1 and declares the Excalidraw package MIT-licensed ([package metadata](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json)). The repository's own code license is MIT ([LICENSE](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/LICENSE)).

Ticket #9 must re-run the following mechanical checks against its chosen tag:

- Diff `packages/excalidraw/fonts`, `scripts/woff2/assets`, the font IDs and fallbacks in `constants.ts`, and font registration in `fonts/Fonts.ts`.
- Diff external rendering dependencies in `packages/excalidraw/package.json`.
- Search static Canvas/SVG renderers for newly embedded image, icon, shader, pattern, or WASM data.
- Compare each changed asset's embedded metadata and upstream license, rather than assuming v0.18.1 terms still apply.

## Official font behavior

The official family IDs are `Virgil=1`, `Helvetica=2`, `Cascadia=3`, `Excalifont=5`, `Nunito=6`, `Lilita One=7`, `Comic Shanns=8`, and `Liberation Sans=9`. ID 4 is intentionally unused. Xiaolai is fallback ID 100 and Segoe UI Emoji is fallback ID 1000. Excalifont falls back to Xiaolai and then Segoe UI Emoji; every other family falls back to Segoe UI Emoji ([constants and fallback selection](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts#L119-L156)). Excalifont is the default family ([default constants](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts#L179-L182)).

Static SVG export gathers the families used by text elements, adds Xiaolai when Excalifont text contains CJK, ignores local-only faces, subsets the selected WOFF2 faces, and writes data-URL `@font-face` rules into the SVG ([font selection and declarations](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts), [SVG export call site](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L410-L426), [font data loading and subsetting](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts)). Frame labels are synthesized as Helvetica text during export ([frame-label export](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L78-L112)).

### Asset inventory

| Family or asset | Official files at v0.18.1 | Static-export role | License evidence | Distribution conclusion |
|---|---|---|---|---|
| Excalifont, ID 5 | 7 WOFF2 chunks, 64,768 bytes; exact imports and Unicode ranges are in the [family manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Excalifont/index.ts) | Default hand-drawn text | The same pinned manifest embeds the copyright, trademark, and complete OFL-1.1 text | Bundle all chunks. Preserve bytes and name; include the embedded notice and OFL text. |
| Nunito, ID 6 | 5 WOFF2 chunks, 57,564 bytes: Latin, Latin Extended, Cyrillic, Cyrillic Extended, and Vietnamese, enumerated in the [family manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Nunito/index.ts) | Normal font-picker text, registered at weight 500 | [Google Fonts OFL at immutable commit](https://github.com/google/fonts/blob/a5bd0ea86b2576f86672aab557a6024d272187a5/ofl/nunito/OFL.txt) and [family metadata](https://github.com/google/fonts/blob/a5bd0ea86b2576f86672aab557a6024d272187a5/ofl/nunito/METADATA.pb) | Bundle all chunks under OFL-1.1. |
| Lilita One, ID 7 | 2 WOFF2 chunks, 12,092 bytes: Latin and Latin Extended, enumerated in the [family manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Lilita/index.ts) | Heading font-picker text | [Google Fonts OFL at immutable commit](https://github.com/google/fonts/blob/90abd17b4f97671435798b6147b698aa9087612f/ofl/lilitaone/OFL.txt) | Bundle under OFL-1.1; `Lilita` is an explicit Reserved Font Name. |
| Comic Shanns, ID 8 | 4 WOFF2 chunks, 31,452 bytes, plus `ComicShanns-Regular.sfd`; exact files and notices are in the [family directory](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ComicShanns) and [manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ComicShanns/index.ts) | Code font-picker text | The pinned manifest embeds the full MIT notice and all recorded copyright holders; [PR #8641](https://github.com/excalidraw/excalidraw/pull/8641) records the upstream and Excalidraw modifications | Bundle WOFF2 chunks under MIT and reproduce the complete multi-holder notice. No source offer is stated. The SFD is not a runtime requirement. |
| Virgil, ID 1 | `Virgil-Regular.woff2`, 56,156 bytes ([file and registration](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Virgil)) | Deprecated but required for old scene documents | [Virgil OFL at immutable commit](https://github.com/excalidraw/virgil/blob/3d4c829dc85b455d4646631ac52bc1517df04217/LICENSE.md) | Bundle for backward compatibility under OFL-1.1; `Virgil` is an explicit Reserved Font Name. |
| Cascadia, ID 3 | `CascadiaCode-Regular.woff2`, 65,732 bytes ([file and registration](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Cascadia)) | Deprecated but required for old scene documents | [Cascadia Code v2407.24 OFL](https://github.com/microsoft/cascadia-code/blob/56bcca3f2c1e4cb19458954f0e2bb4635960df91/LICENSE) | Bundle under OFL-1.1; `Cascadia Code` is an explicit Reserved Font Name. |
| Helvetica, ID 2 | No font file; registration is `local:` ([registration](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Helvetica/index.ts)) | Deprecated scene font and generated frame labels | Excalidraw says it uses Helvetica on macOS and Arial on Windows and deliberately does not inline local fonts ([registration policy](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts)) | Do not copy Helvetica or Arial. No redistribution grant for either is present in the audited sources. Pin Liberation Sans as the self-contained deterministic substitute, and document the resulting browser-platform parity limit. |
| Liberation Sans, ID 9 | `LiberationSans-Regular.woff2`, 70,668 bytes; server build also holds `LiberationSans-Regular.ttf` and a 2048-units-per-em variant ([browser family](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Liberation), [server assets](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/scripts/woff2/assets)) | Official server-side replacement/fallback for Helvetica-like coverage | [Liberation Fonts 2.1.5 OFL](https://github.com/liberationfonts/liberation-fonts/blob/4b0192046158094654e865245832c66d2104219e/LICENSE); [Excalidraw server merge change](https://github.com/excalidraw/excalidraw/pull/9052) | Bundle under OFL-1.1. `Liberation`, `Arimo`, `Tinos`, and `Cousine` are explicit Reserved Font Names in the notice. Prefer exact Excalidraw WOFF2 unless the server TTF merge behavior is selected as the oracle. |
| Xiaolai, fallback ID 100 | 209 WOFF2 chunks, 12,667,492 bytes, all enumerated with Unicode ranges in the [family manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Xiaolai/index.ts); the modified source TTF is [also pinned](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/scripts/woff2/assets/Xiaolai-Regular.ttf) | CJK hand-drawn fallback for Excalifont | The family manifest records OFL-1.1 metadata and Excalidraw's modifications. The corresponding Xiaolai 3.11 upstream license is [OFL-1.1 at immutable commit](https://github.com/lxgw/kose-font/blob/21828241081eb0ecc1b4cb40bd9a1b253c09c488/License.txt). [PR #8530](https://github.com/excalidraw/excalidraw/pull/8530) records removal of non-CJK glyphs and metric changes. | Bundle all exact chunks for arbitrary local scenes. OFL permits modification and embedding, and the audited license text does not designate a Reserved Font Name. Keep the Xiaolai copyright, modification record, and OFL text. |
| Segoe UI Emoji, fallback ID 1000 | No font file; registration is `local:` ([registration](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Emoji/index.ts)) | Browser fallback for all text families | No redistribution grant or official binary exists in the audited Excalidraw assets | Do not copy Segoe UI Emoji. Use the official server's Noto Emoji fallback and specify that emoji appearance follows the server-style baseline, not each host OS. |
| Noto Emoji server fallback | `NotoEmoji-Regular.ttf` and a 2048-units-per-em variant in [server assets](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/scripts/woff2/assets); the pinned regular file is 862,680 bytes and identifies itself as Noto Emoji 3.003 | Merged into generated server-side family TTFs ([build plugin](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/scripts/woff2/woff2-esbuild-plugins.js)) | [Noto Emoji OFL and copyright](https://github.com/google/fonts/blob/a73b9ab0a5df191bcfed817159a903911ea7958a/ofl/notoemoji/OFL.txt); the exact Excalidraw binary's name table also states OFL-1.1 | May be bundled under OFL-1.1. Exact provenance of Excalidraw's static 3.003 build is not documented in a sidecar, so preserve its embedded metadata and the cited Noto OFL notice. |
| Assistant UI family | 4 WOFF2 files, 81,144 bytes ([files and CSS](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Assistant)) | Editor UI only; the font ID formerly associated with Assistant is intentionally unused | [Assistant OFL at immutable commit](https://github.com/google/fonts/blob/89c9db01508963eb8b48a171c8baf2ef750c5bd9/ofl/assistant/OFL.txt) | Do not ship in the renderer. If a future UI package uses it, OFL-1.1 permits bundling; retain the Assistant and Source notices and the Reserved Font Name `Source`. |

The byte counts above come from the immutable v0.18.1 Git tree. Public-root copies of Virgil, Cascadia, and Assistant are application deployment duplicates and are not additional render families. Bundle one canonical copy of each required face.

### Exact manageable file names

The non-CJK rendering files are:

- Excalifont: `Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2`, `Excalifont-Regular-be310b9bcd4f1a43f571c46df7809174.woff2`, `Excalifont-Regular-b9dcf9d2e50a1eaf42fc664b50a3fd0d.woff2`, `Excalifont-Regular-41b173a47b57366892116a575a43e2b6.woff2`, `Excalifont-Regular-3f2c5db56cc93c5a6873b1361d730c16.woff2`, `Excalifont-Regular-349fac6ca4700ffec595a7150a0d1e1d.woff2`, and `Excalifont-Regular-623ccf21b21ef6b3a0d87738f77eb071.woff2`.
- Nunito: `Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTQ3j6zbXWjgeg.woff2`, `Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTo3j6zbXWjgevT5.woff2`, `Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTA3j6zbXWjgevT5.woff2`, `Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTk3j6zbXWjgevT5.woff2`, and `Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTs3j6zbXWjgevT5.woff2`.
- Lilita One: `Lilita-Regular-i7dPIFZ9Zz-WBtRtedDbYEF8RXi4EwQ.woff2` and `Lilita-Regular-i7dPIFZ9Zz-WBtRtedDbYE98RXi4EwSsbg.woff2`.
- Comic Shanns: `ComicShanns-Regular-279a7b317d12eb88de06167bd672b4b4.woff2`, `ComicShanns-Regular-fcb0fc02dcbee4c9846b3e2508668039.woff2`, `ComicShanns-Regular-dc6a8806fa96795d7b3be5026f989a17.woff2`, and `ComicShanns-Regular-6e066e8de2ac57ea9283adb9c24d7f0c.woff2`.
- Virgil: `Virgil-Regular.woff2`.
- Cascadia: `CascadiaCode-Regular.woff2`.
- Liberation Sans: `LiberationSans-Regular.woff2`.

Listing 209 Xiaolai names a second time would create a less reliable duplicate manifest. Its pinned `index.ts`, linked in the inventory, is the exact ordered file and Unicode-range manifest and should be machine-checked when vendoring.

## Other static-export assets

### Image and error placeholder icons

Canvas/PNG rendering embeds a Font Awesome `image` icon path for an unavailable image and combines that path with the Font Awesome `ban` icon for an errored image ([Excalidraw source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts#L346-L381)). The paths match Font Awesome Free 5.15.4's [`solid/image.svg`](https://github.com/FortAwesome/Font-Awesome/blob/7d3d774145ac38663f6d1effc6def0334b68ab7e/svgs/solid/image.svg) and [`solid/ban.svg`](https://github.com/FortAwesome/Font-Awesome/blob/7d3d774145ac38663f6d1effc6def0334b68ab7e/svgs/solid/ban.svg). Font Awesome's pinned [license](https://github.com/FortAwesome/Font-Awesome/blob/7d3d774145ac38663f6d1effc6def0334b68ab7e/LICENSE.txt) applies CC BY 4.0 to icons and says attribution is required.

Engineering implication: the tiny path constants may be embedded in native and WASM code, but the distribution notice must identify Font Awesome Free 5.15.4, link CC BY 4.0, and identify that the error placeholder combines/transforms the original icons. Because a rendered PNG/SVG/PDF can contain the icon without accompanying application files, maintainers or counsel must decide whether attribution metadata must also be written into outputs whenever a placeholder is rendered. A nonvisual SVG comment, PNG text chunk, and PDF metadata entry would preserve appearance while making attribution travel with the output.

### Patterns, arrowheads, and geometry

There are no hachure bitmap textures, brushes, or arrowhead image assets. Rough fills, strokes, polygons, and arrowheads are generated from code and scene parameters ([shape generation](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts)). They create code-license notice obligations if ported, not asset-license obligations.

### Scene-provided images and external content

Image elements reference binary files supplied in the scene and SVG export writes the supplied data URL into an `<image>` element ([static SVG image path](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts#L408-L458)). Those files are user inputs, not Excalidraw-distributed assets; their rights remain the caller's responsibility. No stock image, favicon, Open Graph image, example image, or editor icon is needed to render a local scene.

The production renderer must not fetch iframe previews, external images, CDN fonts, or other URL content. Offline embeddable behavior should be generated geometry/text or a caller-supplied asset through the public injection point. This introduces no bundled third-party asset unless the later specification selects one.

### Official font-subsetting WASM

Official SVG export embeds generated JavaScript arrays for `hb-subset.wasm` and `woff2.wasm` and uses them only to decompress, subset, and recompress embedded fonts ([subset pipeline](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-shared.chunk.ts), [pinned binary sources](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84/scripts/wasm)). Excalidraw [PR #8384](https://github.com/excalidraw/excalidraw/pull/8384) describes this as an output-size optimization of up to 95%, not a rendering change.

These binaries are not required for perceptual parity. Their provenance is also broader than a single MIT line:

- `harfbuzzjs` 0.3.6 declares MIT in [package metadata](https://github.com/harfbuzz/harfbuzzjs/blob/54e9968f3e641534eb4a2480b2c118696974a522/package.json), while its pinned [LICENSE](https://github.com/harfbuzz/harfbuzzjs/blob/54e9968f3e641534eb4a2480b2c118696974a522/LICENSE) says two libc/allocator components are Apache-licensed and the rest is MIT. Its HarfBuzz submodule is pinned to `e079dd20304bacd8d753eee2ceacbaa1a3978a34`; HarfBuzz's [COPYING file](https://github.com/harfbuzz/harfbuzz/blob/e079dd20304bacd8d753eee2ceacbaa1a3978a34/COPYING) uses the Old MIT notice and warns that some subdirectories have their own notices.
- Google's WOFF2 reference implementation is MIT-licensed ([v1.0.2 LICENSE](https://github.com/google/woff2/blob/1bccf208bca986e53a647dfe4811322adb06ecf8/LICENSE)); its pinned [README](https://github.com/google/woff2/blob/1bccf208bca986e53a647dfe4811322adb06ecf8/README.md) also identifies the included Brotli reference code as MIT-licensed.

The exact WOFF2-WASM build input commit is not recorded beside Excalidraw's binary. Therefore copying either official binary would require a separate binary-provenance/SBOM reconstruction. The pure-Rust destination should instead omit subsetting initially or select audited pure-Rust font codecs. No nested WASM, Emscripten glue, worker, JS engine, or network loader is needed in production.

## Rendering code likely to be ported

| Code | Pinned use/evidence | License and notice | Engineering implication |
|---|---|---|---|
| Excalidraw static renderer and geometry | v0.18.1 at `a2ec2889...`; [root LICENSE](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/LICENSE) | MIT, copyright 2020 Excalidraw | A source port or substantial copied portion must retain the Excalidraw copyright and MIT permission notice. The license does not require publishing Rust source. |
| Rough.js 4.6.4 | Version pinned by Excalidraw; its immutable [source package metadata](https://github.com/pshihn/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/package.json) and npm metadata identify source commit `7944f1cb95da6efadff7c15c9dc96ae0abc51199`; [source LICENSE](https://github.com/pshihn/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/LICENSE) | MIT, copyright 2019 Preet Shihn | Porting seeded stroke/fill algorithms is permitted with the notice. Preserve deterministic behavior separately; the license imposes no runtime dependency. |
| Rough.js transitive geometry | Rough.js 4.6.4's pinned [package metadata](https://github.com/pshihn/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/package.json) declares `hachure-fill` 0.5.2, `path-data-parser` 0.1.0, `points-on-curve` 0.2.0, and `points-on-path` 0.2.1. Authoritative licenses: [hachure-fill](https://github.com/pshihn/hachure-fill/blob/2083be6f7e6c814dc6a8c2c88a7823827566504e/LICENSE), [path-data-parser](https://github.com/pshihn/path-data-parser/blob/93d3fa8448c50d1b6443b06b8c71202ee0967e25/LICENSE), [points-on-curve 0.2](https://github.com/pshihn/bezier-points/blob/48241476eb86307294f54913ccfb60a3e7338c56/LICENSE), [points-on-path](https://github.com/pshihn/points-on-path/blob/7693ef0c930466b7408ee7082497e0d735f9c646/LICENSE) | All MIT, with separate Preet Shihn/Preet notices | Include every notice for code actually ported. Do not assume the top-level Rough.js notice covers separately packaged dependencies. |
| perfect-freehand 1.2.0 | Excalidraw calls `getStroke` for pressure-sensitive freedraw; [v1.2.0 source](https://github.com/steveruizok/perfect-freehand/tree/fa0b754d49bb60813c24cbd7fdcd5cda40f560f3) and [LICENSE](https://github.com/steveruizok/perfect-freehand/blob/fa0b754d49bb60813c24cbd7fdcd5cda40f560f3/LICENSE) | MIT, copyright 2021 Stephen Ruiz Ltd | A direct pure-Rust port is permitted with the notice and no source-publication requirement. |
| points-on-curve 1.0.1 | Excalidraw directly imports simplification/Bezier helpers; the pinned [package metadata](https://github.com/pshihn/bezier-points/blob/00d0a7de7dc4b6e4aab70d670afa21c4a6c6b77a/package.json) identifies version 1.0.1; [LICENSE](https://github.com/pshihn/bezier-points/blob/00d0a7de7dc4b6e4aab70d670afa21c4a6c6b77a/LICENSE) | MIT, copyright 2020 Preet Shihn | Include its notice if ported even if another version is already present through Rough.js. |
| Current Rust `roughr` 0.12.0 | Current manifest pins it; [crate source tag](https://github.com/orhanbalci/rough-rs/tree/b1c2d96c944da4e74275aa09892be14e2d54445a/roughr), [README](https://github.com/orhanbalci/rough-rs/blob/b1c2d96c944da4e74275aa09892be14e2d54445a/roughr/README.md), and [LICENSE](https://github.com/orhanbalci/rough-rs/blob/b1c2d96c944da4e74275aa09892be14e2d54445a/LICENSE) | MIT, copyright 2022 Orhan Balci; README describes it as a Rust port of Rough.js | Include both the `roughr` and original Rough.js notices as the conservative distribution policy. The crate's own package does not answer whether its Rough.js-derived portions need the original notice; maintainers or counsel should confirm. |

MIT notices may be consolidated into one `THIRD_PARTY_NOTICES.md`, but the copyright and complete permission/disclaimer text for each applicable work must remain. A binary-only native or WASM release still constitutes distribution of the copied software; putting notices only in the source repository is insufficient for a standalone artifact.

## Font license obligations

The authoritative OFL texts linked above grant use, study, copy, merge, embed, modify, redistribute, and sale of modified and unmodified font software. They impose these engineering-relevant conditions:

1. Do not sell a font or one of its components by itself.
2. A bundled original or modified font copy must contain its copyright notice and the OFL text in a standalone file, readable header, or easily viewable metadata.
3. A modified font must not use an explicitly Reserved Font Name without permission from the relevant copyright holder.
4. Do not use a copyright holder's or author's name to promote or endorse a modified font except to acknowledge contributions or with permission.
5. Distribute the font software itself under OFL-1.1, not solely under the application's MIT/Apache license. This does not impose OFL on a document made with the font.

OFL-1.1 contains no source-offer clause. It permits embedding and distinguishes documents made with a font from the font software itself. Thus raster PNG output does not become OFL-licensed merely because it depicts glyphs, and the surrounding Rust application does not become OFL-licensed merely because it bundles a font. An SVG or PDF that contains extractable font bytes sits at the practical boundary between "document" and redistributed font software. The text includes a document exception, but also requires notices with redistributed font copies. Maintainers or counsel should decide whether generated SVG/PDF files need embedded OFL metadata; including it is the conservative option and has no perceptual effect.

### Name and provenance cautions

- Virgil, Cascadia Code, Lilita, and Liberation have explicit Reserved Font Names. Exact upstream bytes may retain their original names. Any build-time conversion, merge, glyph removal, subsetting, or metric rewrite may be a "Modified Version" under OFL's definition and can trigger the Reserved Font Name condition.
- Excalifont's embedded metadata says "Excalifont is a trademark of Excalidraw," identifies Virgil and Excalifont designers, and licenses the font under OFL-1.1. It does not state an OFL Reserved Font Name. Trademark and OFL name restrictions are different questions. Redistribute exact official bytes and do not market a modified font as official. Obtain maintainer or counsel review before modifying or renaming it.
- Excalidraw's Xiaolai is already a documented modified build. Its pinned manifest identifies the changes and OFL metadata. Preserve that modification record; do not replace it with an unmodified upstream Xiaolai file because metrics and coverage would differ.
- Comic Shanns' pinned manifest records more copyright holders than the original Comic Shanns repository because Excalidraw adopted Comic Shanns Mono and additional glyph changes. Copy the full notice from the pinned manifest, not only the first upstream notice.
- Noto Emoji 3.003's exact static-build provenance is not documented next to the binary. Its own embedded name table identifies Google, version 3.003, and OFL-1.1. Preserve that metadata and the cited Google Fonts OFL notice; do not strip the name table during conversion.

## Native and WASM distribution design

| Distribution/output | Asset strategy | Required notices | Runtime/OS dependencies |
|---|---|---|---|
| Rust source crate | Include exact official WOFF2/TTF bytes in the crate archive under an `assets/` tree. Keep license files under `LICENSES/` and map every byte file in `THIRD_PARTY_NOTICES.md`. | OFL notices per font family; Comic Shanns and ported-code MIT notices; Font Awesome CC BY attribution if paths are included | Rust toolchain at build time only. No system fonts, Fontconfig, FreeType, HarfBuzz, Node, browser, or network. Chosen crates must be separately audited for pure-Rust/WASM support. |
| Native CLI/library binary | Compile required bytes into the executable or linked library. Provide notices beside the executable and through a stable `--licenses`/API access path so single-file packaging does not hide them. | Same as source crate | No required OS packages or font installation. No network. |
| `wasm32` core | Compile bytes into WASM data segments. Include notices in the containing package/archive and expose a Rust/WASM-readable notice string or file; a custom WASM section alone may not be easily viewable by users. | Same as native | WebAssembly host only. No JS package, DOM, Canvas, system fonts, filesystem, worker, nested WASM, or fetch requirement. |
| SVG output | Embed only the exact faces/chunks needed by the scene as data URLs. Initially skip further subsetting, or use an audited pure-Rust subsetter after the Reserved Font Name decision. Preserve text as text if required by the later output contract. | Maintainer/counsel decision on carrying OFL notices in generated documents. Add Font Awesome attribution metadata when a placeholder appears. | Output is self-contained; viewers need no installed font or network. |
| PNG output | Rasterize with bundled fonts and path assets. Do not attach font binaries. | No OFL notice is needed merely for rendered glyph pixels. Font Awesome attribution should be placed in a PNG text chunk if maintainers/counsel require output-traveling attribution. | None beyond the pure-Rust renderer. |
| PDF output | Embed exact full fonts or subsets, or outline only where the PDF policy permits. Full/subset embedding preserves selectable text better than outlines. | Maintainer/counsel decision on OFL notice metadata; Font Awesome attribution metadata when used | No system fonts or network. |

### Upstream dependencies not inherited by production

The official browser package uses `FontFace`, `document.fonts`, `fetch`, CDN fallback URLs, Web Workers, and nested font-processing WASM ([font registration](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts), [asset URLs and fetch](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts), [subset dispatch](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-main.ts)). The official server font build uses Node file/process APIs, `wawoff2`, `fonteditor-core`, an installed `fonttools` executable, and `pyftmerge` ([server build plugin](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/scripts/woff2/woff2-esbuild-plugins.js)). These are facts about the upstream reference implementations, not dependencies to inherit.

The proposed production dependency list is therefore explicit: no required OS package; no Fontconfig, FreeType, system HarfBuzz, Node, Python, FontTools, browser DOM/Canvas, Web Worker, JavaScript package, network, or installed font. Native execution needs only the compiled Rust artifact and input bytes. WASM execution needs only a conforming WebAssembly host and input/output memory integration. Development may use the pinned official exporter as an oracle under a separately documented toolchain, but generated expectations must not become a production runtime dependency.

The simplest initial implementation is to index every official WOFF2 face by family and Unicode range, choose matching existing chunks for a scene, decode them with a pure-Rust codec for shaping/rasterization, and copy the same exact chunks into SVG output. This avoids generating modified font files in the release and preserves the official glyph data. It costs output size but not perceptual parity. Xiaolai dominates artifact size; feature-gating it would make default distributions incomplete for arbitrary CJK scenes and should only be allowed as an explicitly reduced-coverage build, not the parity build.

The default renderer must never fall back silently to installed fonts. If a codepoint is absent from the bundled deterministic chain, return a compatibility diagnostic and render the specified deterministic missing-glyph behavior. An asset-injection API may allow callers to supply additional fonts or scene files, but injected assets cannot be required for the official baseline.

## Required notice package

Before release, generate and verify this layout for every native archive, crate archive, and WASM package:

```text
THIRD_PARTY_NOTICES.md
LICENSES/
  excalidraw-MIT.txt
  roughjs-MIT.txt
  roughjs-transitive-MIT.txt
  perfect-freehand-MIT.txt
  points-on-curve-MIT.txt
  roughr-MIT.txt                 # only while roughr is shipped
  font-awesome-CC-BY-4.0.txt     # only while placeholder paths are shipped
  fonts/
    Excalifont-OFL.txt
    Nunito-OFL.txt
    Lilita-One-OFL.txt
    Virgil-OFL.txt
    Cascadia-Code-OFL.txt
    Liberation-Sans-OFL.txt
    Xiaolai-OFL.txt
    Noto-Emoji-OFL.txt
    Comic-Shanns-MIT.txt
```

`THIRD_PARTY_NOTICES.md` should record the exact Excalidraw reference tag/commit, every vendored file path and cryptographic checksum, upstream source commit, license path, copyright notice, Reserved Font Name, local modifications, and whether the item is present in native, WASM, SVG, PNG, or PDF artifacts. The application `LICENSE` must say that bundled third-party assets remain under their listed licenses rather than implying they are all `MIT OR Apache-2.0`.

Release CI should fail if:

- a vendored asset is not in the notice manifest;
- a hash differs from the pinned upstream/Excalidraw asset without an explicit modification record;
- a required license file is absent from a crate, native archive, or WASM package;
- production code contains an HTTP(S) font/asset fallback or system-font lookup;
- the parity build omits one of the official baseline families or Unicode-range chunks;
- a copied rendering module lacks its corresponding MIT notice.

## Unresolved decisions

These are newly sharp specification decisions, not missing research facts:

1. **Reference fallback renderer:** choose Excalidraw's deterministic server-style Liberation Sans + Noto Emoji chain for `Helvetica`, frame labels, missing alphabetic glyphs, and emoji, or accept a documented different open fallback. The renderer cannot simultaneously be self-contained and reproduce each OS's Helvetica/Arial/Segoe rendering.
2. **Emoji appearance:** decide whether the server-style Noto Emoji 3.003 fallback is authoritative. It will not match every Segoe UI Emoji or other browser/platform emoji renderer.
3. **Generated SVG/PDF font notices:** decide with maintainers or counsel whether OFL notices must travel inside every output containing embedded font bytes. The conservative engineering choice is to include metadata/comments.
4. **OFL Reserved Font Names and subsetting:** decide whether to ship full exact chunks in SVG/PDF initially or obtain counsel/holder guidance before preserving Reserved Font Names in generated subsets. Exact unmodified release assets avoid this issue for application distribution.
5. **Font Awesome output attribution:** either carry CC BY attribution in every output that renders the placeholder, or replace the placeholder with independently created project geometry and accept/measure the parity delta.
6. **Excalifont provenance/name:** ask Excalidraw maintainers to confirm redistribution/modification guidance for the Excalifont trademark and its Virgil-derived provenance. Until then, redistribute only exact official files and include both Excalifont's embedded OFL notice and Virgil's notice in the bundle.
7. **`roughr` notice chain:** confirm whether the existing Rust port retains all original Rough.js notices. Until confirmed, ship both notices or replace it with a directly audited port pinned to Rough.js 4.6.4.
8. **Baseline re-audit:** once ticket #9 chooses a release, regenerate the inventory and hashes. This report's conclusions are concrete for v0.18.1, not a blanket statement about future Excalidraw releases.

## Recommended specification language

The implementation-ready specification can adopt this language:

> The parity distribution vendors the exact rendering font assets from the pinned Excalidraw release, including deprecated scene families and all official Unicode-range fallback faces. It uses no installed fonts and performs no asset fetch. `Helvetica`/Arial and `Segoe UI Emoji`, which the official browser renderer treats as local-only, are deterministically replaced by the pinned OFL-licensed Liberation Sans and Noto Emoji assets used by the official server-export pipeline; this server-style fallback behavior is the compatibility baseline. Native and WASM artifacts include complete, easily viewable OFL, MIT, and CC BY notices. Font bytes remain under their original licenses. Production does not include Excalidraw's HarfBuzz/WOFF2 WASM helpers; font processing is pure Rust. Generated outputs are self-contained, and the final policy for embedded-font and Font Awesome attribution metadata is resolved before release.
