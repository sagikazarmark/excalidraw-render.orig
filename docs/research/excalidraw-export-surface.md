# Official Excalidraw static-export surface

Research for [excalidraw-render.orig#6](https://github.com/sagikazarmark/excalidraw-render.orig/issues/6), prepared 2026-07-17.

## Executive answer

### Observed facts

There are two strong, materially different official baselines and one conditional legacy baseline:

| Candidate | Immutable identity | Why it is viable | Principal compatibility cost |
| --- | --- | --- | --- |
| `v0.18.1` | tag commit [`a2ec2889babf7d2295469c6d90ebe77fae57df84`](https://github.com/excalidraw/excalidraw/commit/a2ec2889babf7d2295469c6d90ebe77fae57df84), official [GitHub release](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.1), official [npm package metadata](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.18.1) | Current patched `0.18.x` release; its public package exports the official restore and SVG/canvas/blob export paths. | Largest surface: image crops, iframe and magic-frame elements, elbow arrows, expanded arrowheads, fractional ordering, metric-based text, new font families, CJK fallback, font loading, and SVG font subsetting. |
| `v0.17.6` | tag commit [`0dbd2a39319d41fda37b2945dea0dcbd58d6a564`](https://github.com/excalidraw/excalidraw/commit/0dbd2a39319d41fda37b2945dea0dcbd58d6a564), official [tag](https://github.com/excalidraw/excalidraw/tree/v0.17.6), official [npm package metadata](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.17.6) whose `gitHead` is the same commit | Last `0.17.x` release and a coherent pre-`0.18` surface. It already has frames, bound text, images, embeddables, links, SVG/PNG scene embedding, and explicit frame-export tests. | SVG font declarations reference external assets instead of embedding them; fewer fonts and arrowheads; baseline-based text; no crop, elbow, iframe, magic-frame, or fractional-index behavior. |
| `v0.16.4` | tag commit [`1863da44053dcf95aa4be0891065e8a580002de6`](https://github.com/excalidraw/excalidraw/commit/1863da44053dcf95aa4be0891065e8a580002de6), official [tag](https://github.com/excalidraw/excalidraw/tree/v0.16.4), official [npm package metadata](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.16.4) whose `gitHead` is the same commit | Maintained `0.16.x` patch release and the smallest candidate considered here. | It does not materially reduce the element schema relative to `v0.17.6`, but whole-scene export omits frame outlines, names, and clipping; separate-frame detection depends on scene/array heuristics rather than an explicit API; it has only three font IDs and older RoughJS/direct curve dependencies. |

`v0.18.0` is not a useful independent rendering candidate. It is the feature release that introduced the major `0.18` surface ([official release notes](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.0)), but the `v0.18.0` and `v0.18.1` blobs for `element/types.ts`, `scene/export.ts`, `renderer/staticSvgScene.ts`, and `packages/utils/export.ts` are identical. The immutable [`817d8c5...a2ec288` comparison](https://github.com/excalidraw/excalidraw/compare/817d8c553c3389650f8b4503984a6d4a5d2f0c11...a2ec2889babf7d2295469c6d90ebe77fae57df84) changes package metadata, a Mermaid dependency and two related application files, not static rendering. Therefore `v0.18.1` subsumes `v0.18.0` as a pinned reference for this question.

For a reproducible package oracle, pin the tarball as well as the source commit. The npm integrity values are `sha512-6i5Gt7IDTOH//qa0Z315Ly5iVRhjWpu2whrlQFqkuwrkKUWgRsMk0P5qdE7bpyDpai7jeLeWYkyj1eVAfni1lw==` for `0.18.1`, `sha512-fyCl+zG/Z5yhHDh5Fq2ZGmphcrALmuOdtITm8gN4d8w4ntnaopTXcTfnAAaU3VleDC6LhTkoLOTG6P5kgREiIg==` for `0.17.6`, and `sha512-x56YTb5jmHAJ9SP2R81ywU28Y+QlOgjmCYHVMgHKPhh1hwKzimt+Z+iz/Rf2x1JpQOJRYbfeoxiGPQNhnYwGWQ==` for `0.16.4`. Unlike the two older records, the `0.18.1` npm record has no `gitHead`; its release commit and npm integrity are therefore two separate pins rather than a metadata-proven identity assertion. Sources: the versioned npm records linked in the candidate table.

Rendering-library versions are part of each baseline, not implementation details:

| Candidate | RoughJS | `perfect-freehand` | Direct `points-on-curve` | Other material helpers |
| --- | --- | --- | --- | --- |
| `v0.18.1` | `4.6.4` | `1.2.0` | `1.0.1` | `fractional-indexing 3.2.0`, `@braintree/sanitize-url 6.0.2`, `pako 2.0.3` |
| `v0.17.6` | `4.6.4` | `1.2.0` | `1.0.1` | `@braintree/sanitize-url 6.0.2`, `pako 1.0.11` |
| `v0.16.4` | `4.5.2` | `1.2.0` | `0.2.0` | `@braintree/sanitize-url 6.0.2`, `pako 1.0.11` |

The exact manifests are [`v0.18.1 packages/excalidraw/package.json`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json), [`v0.17.6 package.json`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/package.json), and [`v0.16.4 package.json`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/package.json). `v0.16.4` can therefore produce different rough paths and simplified freedraw fills even where its element declaration matches `v0.17.6`.

All three candidates serialize scene envelope `version: 2`; that number is the Excalidraw data-format version, not the application or npm release. A renderer cannot infer `0.16`, `0.17`, or `0.18` from the document version. It must detect concrete element types, properties, font IDs, and enum values. The constant remains `VERSIONS.excalidraw = 2` in both [`v0.17.6`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/constants.ts) and [`v0.18.1`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts).

The official package has static SVG and canvas/blob APIs, with PNG produced by canvas `toBlob()`. It has no PDF export API. Vector-first PDF is necessarily a project-defined compatibility output rather than byte or structure parity with an official Excalidraw PDF. The complete public export API is visible in [`v0.18.1 packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts) and [`v0.17.6 src/packages/utils.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/packages/utils.ts).

### Recommendations, not a baseline decision

- Treat `v0.18.1` and `v0.17.6` as the real decision pair. `v0.18.1` is the modern target; `v0.17.6` is a defensible reduced target with a clear diagnostic boundary for `0.18` features.
- Keep `v0.16.4` only as a deliberate legacy option. Its element type definition is byte-identical to `v0.17.6`, while `v0.17.6` has substantially sharper frame and export behavior. The main reduction is behavior and font/default breadth, not a simpler document model. See the immutable [`v0.16.4...v0.17.6` comparison](https://github.com/excalidraw/excalidraw/compare/1863da44053dcf95aa4be0891065e8a580002de6...0dbd2a39319d41fda37b2945dea0dcbd58d6a564).
- Define parity against a fixed official harness, including browser engine/version, device-pixel ratio, export options, locally served pinned assets, and network policy. The official output is not fully environment-independent. Ticket #13 has already selected and experimentally pinned a repository-owned Playwright wrapper around the official package; use its immutable [reference-oracle report](https://github.com/sagikazarmark/excalidraw-render.orig/blob/287f33596eb43fc8d855cd64e4bfb2aa0350b4cb/docs/research/reference-oracle-options.md) rather than reopening the oracle choice.
- For the required network-free product, adopt the official default static fallback for embeddables: draw the placeholder and retain the hyperlink. Do not enable live iframe export.
- Preserve SVG anchors as PDF link annotations where the PDF library permits it. This is an extension of official SVG semantics, not an official PDF behavior.

This report intentionally does not choose the baseline.

## Candidate baseline evidence

### `v0.18.1`: modern surface

`v0.18.1` is an official non-prerelease security patch published on 2026-04-21. Its release says it backports a Mermaid XSS mitigation to `0.18.x`; the release commit changes only package metadata, two Mermaid-related application files, and the lockfile. The renderer-facing files are unchanged from `v0.18.0`, so the extensive `v0.18.0` release notes describe the static-export additions while `v0.18.1` supplies the safer immutable pin. Sources: [release](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.1), [release commit](https://github.com/excalidraw/excalidraw/commit/a2ec2889babf7d2295469c6d90ebe77fae57df84), [`v0.18.0` release notes](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.0), and [immutable version comparison](https://github.com/excalidraw/excalidraw/compare/817d8c553c3389650f8b4503984a6d4a5d2f0c11...a2ec2889babf7d2295469c6d90ebe77fae57df84).

The `0.18` release notes explicitly call out text wrapping, more fonts, CJK font support, SVG font subsetting, elbow arrows, image cropping, element linking, fractional indexing, canvas font loading, and frame clipping fixes. Those features are all present in the pinned schema and implementation: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`fonts/Fonts.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts), [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts), and [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).

This candidate is viable if the intended compatibility claim includes contemporary Excalidraw files. It is not a small target: a renderer must either implement the full `0.18` surface below or issue explicit feature-level diagnostics while providing best-effort output.

### `v0.17.6`: reduced pre-`0.18` surface

The official npm metadata pins `0.17.6` to `gitHead` `0dbd2a39319d41fda37b2945dea0dcbd58d6a564`, matching the official tag commit. The package exposes the restore, canvas, blob, and SVG APIs from that tree. Sources: [npm metadata](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.17.6), [tag commit](https://github.com/excalidraw/excalidraw/commit/0dbd2a39319d41fda37b2945dea0dcbd58d6a564), and [package export wrapper](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/packages/utils.ts).

Its schema supports rectangle, diamond, ellipse, text, line, arrow, freedraw, image, frame, and embeddable elements, plus the internal legacy selection type. It has four arrowhead values and image scale but no crop. It has font IDs Virgil 1, Helvetica 2, Cascadia 3, and Assistant 4. Sources: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts) and [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/constants.ts).

`v0.17.6` is not missing core static-export semantics. Its pinned tests cover dimensions/background/dark mode/padding/scale, embedded scenes, links, whole-scene frames, and single-frame export; transformed-image export and embedded PNG/SVG round trips are also tested. Sources: [`tests/scene/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/scene/export.test.ts) and [`tests/export.test.tsx`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/export.test.tsx).

Its main self-containedness problem is observable in the export implementation: SVG emits `@font-face` URLs rooted at `window.EXCALIDRAW_ASSET_PATH` or an unpkg/excalidraw.com fallback, rather than embedding font bytes. Sources: [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/scene/export.ts) and the pinned [`smiley_embedded_v2.svg` fixture](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/fixtures/smiley_embedded_v2.svg). A pure-Rust implementation can still be network-free by bundling the exact assets and embedding or outlining text, but that output policy is a compatibility design choice.

### `v0.16.4`: conditional legacy surface

The official npm metadata pins `0.16.4` to `gitHead` `1863da44053dcf95aa4be0891065e8a580002de6`, matching its official tag. Sources: [npm metadata](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.16.4) and [tag commit](https://github.com/excalidraw/excalidraw/commit/1863da44053dcf95aa4be0891065e8a580002de6).

The `src/element/types.ts` Git blob is exactly the same in `v0.16.4` and `v0.17.6` (`e8d71cae5e2f390c0b3f18ea7420a40b84998805`). However, export behavior is not the same. `v0.16.4` does not draw frame outlines or names during static whole-scene SVG/canvas export and deliberately does not clip frame children for a whole-scene export. Its bounds and separate-frame behavior infer whether the input is the whole scene by looking up hidden `Scene` membership and by recognizing an array containing one frame, rather than accepting an explicit `exportingFrame`. Its separate-frame canvas/SVG clip is square. Sources: [`v0.16.4 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/scene/export.ts) and [`renderer/renderScene.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/renderer/renderScene.ts).

`v0.17.6` adds frame-name synthesis (Assistant text, with unnamed frames numbered `Frame 1`, `Frame 2`, and so on), an explicit frame-rendering configuration, clip paths for all frames, root-element bounds, explicit frame export arguments, static frame outlines, and extensive frame tests. It also changes default element style from hachure/1 px stroke to solid/2 px stroke and upgrades RoughJS/direct `points-on-curve`. Sources: the immutable [`v0.16.4...v0.17.6` comparison](https://github.com/excalidraw/excalidraw/compare/1863da44053dcf95aa4be0891065e8a580002de6...0dbd2a39319d41fda37b2945dea0dcbd58d6a564), [`v0.17.6 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/scene/export.ts), and [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/constants.ts).

Thus `v0.16.4` is technically pin-able but is a weaker general baseline than `v0.17.6`: it gives up modern, tested whole-scene frame semantics and uses older geometry dependencies without eliminating most element-level implementation work.

## Complete scene-document surface

This section inventories the current `v0.18.1` surface, then marks the older-candidate differences. It distinguishes persisted fields from fields that visibly affect static output.

### Envelope and persistence

A local exported scene has this envelope:

```text
{
  type: string,                 normally "excalidraw"
  version: number,              2 in every candidate here
  source: string,
  elements: ExcalidrawElement[],
  appState: exported subset,
  files: Record<FileId, BinaryFileData> | undefined
}
```

The imported type makes all top-level fields optional and allows `elements` and `appState` to be null. It also admits editor-oriented `scrollToContent` and `libraryItems`; the public static-export wrappers consume only `elements`, `appState`, and `files`. `type`, `version`, and `source` are serialization/provenance fields and do not select renderer behavior in those wrappers. The official package restores defaults before export. Sources: [`v0.18.1 data/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/types.ts), [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts), [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts), and [`v0.17.6 data/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/data/types.ts).

Official local serialization removes deleted elements, sets line/arrow `lastCommittedPoint` to null (but does not perform that clearing for freedraw), retains only files referenced by non-deleted image elements, pretty-prints JSON, and preserves only the exportable app-state subset. It does not strip ordinary current element properties. A subsequent public-wrapper `restore()` does null freedraw `lastCommittedPoint` before static rendering. Sources: [`v0.18.1 data/json.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/json.ts), [`element/index.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/index.ts), [`element/typeChecks.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/typeChecks.ts), [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts), and [`appState.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/appState.ts).

The public export wrappers expect non-deleted elements. There is an upstream skipped test noting that the wrapper no longer filters deleted elements itself. Saved local documents avoid this because serialization already removes them. A compatibility renderer should define deleted-element handling explicitly rather than reproducing this API precondition accidentally. Source: [`v0.18.1 packages/utils/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.test.ts).

### Common element fields

Every `v0.18.1` element carries the following base fields. The same list applies to `v0.17.6` and `v0.16.4` except that `index` does not exist there. Source for the complete declarations: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts) and [`v0.17.6 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts).

The exact `v0.18.1` type values are `selection`, `rectangle`, `diamond`, `ellipse`, `text`, `line`, `arrow`, `freedraw`, `image`, `frame`, `magicframe`, `iframe`, and `embeddable`. The older candidates omit `magicframe` and `iframe`. `selection` is declared but filtered by restore, so the persisted/renderable union is the remaining set.

| Category | Fields | Static-export significance |
| --- | --- | --- |
| Identity and kind | `id: string`, `type` | `type` selects the renderer. `id` keys bindings, frame membership, SVG masks/clip paths/image symbols, and test hooks. |
| Geometry | `x`, `y`, `width`, `height`, `angle` in radians | Placement, dimensions, center of rotation, and scene bounds. Restore normalizes negative dimensions. |
| Rough/style | `strokeColor`, `backgroundColor`, `fillStyle`, `strokeWidth`, `strokeStyle`, `roundness`, `roughness`, `opacity`, `seed` | Directly controls RoughJS paths/fills/dashes, element alpha, rounding, and seeded hand-drawn variation. |
| Order and grouping | `index: string | null` (`0.18` only), `groupIds: string[]`, array position | The element array is paint order. `0.18` repairs fractional indices to agree with array order; groups also influence frame clipping as a unit. |
| Containers/binding | `frameId: string | null`, `boundElements: { id, type: "arrow" | "text" }[] | null` | `frameId` selects frame clipping and inherited frame opacity. `boundElements` locates attached text/arrows and controls bound-text rendering/masking. |
| Lifecycle | `version`, `versionNonce`, `updated`, `isDeleted` | Primarily editing/reconciliation data. `isDeleted` suppresses rendering. Missing `updated` is filled with the current time during restore, so normalized in-memory data can be time-dependent even though the field is not drawn. |
| Semantics | `link`, `locked`, `customData?` | `link` becomes an SVG anchor. `locked` is editor-only. `customData` is preserved for forward compatibility but is not generally rendered; `iframe.customData.generationData` is a `0.18` subtype exception in the schema, not a distinct static rendering path. |

The exact style enums are `fillStyle: "hachure" | "cross-hatch" | "solid" | "zigzag"`, `strokeStyle: "solid" | "dashed" | "dotted"`, and `roundness: null | { type: 1 | 2 | 3, value?: number }`. Type 1 is legacy proportional radius, type 2 is proportional radius, and type 3 uses an adaptive fixed radius (default 32 px, reduced to 25 percent on small shapes); `value` overrides the adaptive fixed radius. `opacity` is interpreted as a percentage, while `strokeWidth` and `roughness` are numeric rather than closed schema enums. Sources: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), and [`shapes.tsx#getCornerRadius`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/shapes.tsx#L454-L475).

Color fields are unrestricted strings, not palette enums. SVG assigns them to SVG attributes and canvas assigns them to browser `fillStyle`/`strokeStyle`, so CSS color parsing and invalid-color behavior are delegated to the browser. For missing properties, restore defaults `v0.18.1` and `v0.17.6` to stroke `#1e1e1e`, background `transparent`, solid fill, 2 px solid stroke, roughness 1, opacity 100, and unlocked. `v0.16.4` instead defaults to hachure fill and 1 px stroke (the other listed defaults are the same). Sources: [`v0.18.1 constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), [`colors.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/colors.ts), [`v0.17.6 constants.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/constants.ts), and [`v0.16.4 constants.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/constants.ts).

The official RoughJS options are also part of perceptual parity. They include `seed`, size-adjusted roughness, exact dash arrays, disabled multi-stroke for dashed/dotted strokes, a 0.5 px width addition for non-solid strokes, `fillWeight = strokeWidth / 2`, `hachureGap = strokeWidth * 4`, and vertex preservation rules. Source: [`v0.18.1 scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts).

### Element-specific fields and rendering

#### Rectangle, diamond, and ellipse

These types add no fields beyond the base. Rectangle and diamond honor `roundness`; ellipse uses RoughJS ellipse generation with curve fitting 1. All honor stroke/fill styles, opacity, rotation, links, frame opacity, and clipping. Source: [`v0.18.1 scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts) and [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).

#### Text

`v0.18.1` text adds:

```text
fontSize: number
fontFamily: 1 | 2 | 3 | 5 | 6 | 7 | 8 | 9
text: string
textAlign: "left" | "center" | "right"
verticalAlign: "top" | "middle" | "bottom"
containerId: string | null
originalText: string
autoResize: boolean
lineHeight: unitless number
```

The `v0.18.1` family mapping is Virgil 1, Helvetica 2, Cascadia 3, retired/reserved 4, Excalifont 5, Nunito 6, Lilita One 7, Comic Shanns 8, and Liberation Sans 9. Excalifont is the default. Text is emitted as SVG `<text>` nodes, one per newline-delimited line, with explicit family, size, fill, direction, text anchor, alphabetic dominant baseline, and a per-family vertical offset computed from pinned metrics. Canvas uses the corresponding canvas font and metric-derived vertical offset. Sources: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), [`fonts/FontMetadata.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), and [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts).

`originalText` is materially relevant even though `text` is what gets drawn: the SVG font-subsetting pipeline gathers codepoints from `originalText`. `autoResize` and `verticalAlign` mostly describe editor layout; the persisted `x/y/width/height/text/lineHeight` and container relationship determine final static placement. Source: [`fonts/Fonts.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts) and [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts).

`v0.17.6` and `v0.16.4` instead store `baseline: number`, have no `autoResize`, and support the older font IDs. Their SVG renderer uses `dominant-baseline="text-before-edge"`; their canvas renderer computes vertical placement from `height - baseline`. Sources: [`v0.17.6 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts) and [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/renderer/renderElement.ts).

Bound text whose referenced container exists is not painted at its array position. It is skipped in the normal pass and rendered immediately after its container; a dangling `containerId` falls back to ordinary text rendering under the public wrapper's default non-repairing restore. For arrow labels, position is recomputed from the arrow and a mask clears the line beneath the label. Shapes eligible as text containers are rectangle, diamond, ellipse, and arrow. Sources: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), and [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts).

#### Line and arrow

Both add:

```text
points: [number, number][]
lastCommittedPoint: [number, number] | null
startBinding/endBinding: {
  elementId: string,
  focus: number,
  gap: number,
  fixedPoint?: [number, number]   // elbow bindings
} | null
startArrowhead/endArrowhead: Arrowhead | null
```

Points are normalized so the first point is `[0, 0]`, with `x/y/width/height` repaired from points. A null `roundness` produces a straight polyline/polygon; non-null roundness produces a RoughJS curve. A line whose first and last points are within the loop threshold and whose background is non-transparent is filled and uses even-odd fill in SVG. Sources: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts), [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts), and [`shapes.tsx#isPathALoop`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/shapes.tsx#L477-L494).

`v0.18.1` arrowheads are:

```text
arrow, bar, dot (legacy), circle, circle_outline,
triangle, triangle_outline, diamond, diamond_outline,
crowfoot_one, crowfoot_many, crowfoot_one_or_many
```

Filled and outline circular/triangular/diamond heads have specialized fill, stroke, and roughness behavior; bar/arrow/crowfoot heads are line constructions. Outline heads fill with the canvas background color, so `viewBackgroundColor` affects them even when the export background is transparent. Source: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts) and [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts).

`v0.17.6` and `v0.16.4` support only `arrow`, `bar`, `dot`, and `triangle`. Source: [`v0.17.6 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts).

`v0.18.1` arrows additionally have `elbowed: boolean`. An elbow arrow can carry fixed-point bindings, `fixedSegments: { start: Point, end: Point, index: number }[] | null`, and `startIsSpecial`/`endIsSpecial: boolean | null`; its path is generated as orthogonal segments with rounded 16 px corners. Extreme points beyond `1e6` are rejected from shape generation. Restore contains optional repairs for invalid/extreme elbow data, but those repairs run only with `repairBindings: true`, which the public package export wrappers do not pass. Sources: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts), [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts), and [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts).

#### Freedraw

Freedraw adds `points: [number, number][]`, `pressures: number[]`, `simulatePressure: boolean`, and `lastCommittedPoint: [number, number] | null`. The visible stroke is generated with `perfect-freehand`: size `strokeWidth * 4.25`, thinning 0.6, smoothing 0.5, streamline 0.5, and sine easing. Real pressures are paired with points when `simulatePressure` is false. Closed paths can receive a simplified RoughJS background fill. SVG path decimals are trimmed to two places. Source: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts), and [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts).

#### Image and embedded files

`v0.18.1` image adds:

```text
fileId: string | null
status: "pending" | "saved" | "error"
scale: [number, number]
crop: { x, y, width, height, naturalWidth, naturalHeight } | null
```

`v0.17.6` and `v0.16.4` have the same fields except `crop`. `scale` applies horizontal/vertical mirroring. Rotation, opacity, roundness clipping, crop, frame clipping, and dark-mode image compensation all affect output. Sources: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts), and the [`crop export test`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/cropElement.test.tsx#L300-L344).

`files` maps IDs to:

```text
{
  id: FileId,
  mimeType: image/svg+xml | image/png | image/jpeg | image/gif |
            image/webp | image/bmp | image/x-icon | image/avif |
            image/jfif | application/octet-stream,
  dataURL: string,
  created: epoch milliseconds,
  lastRetrieved?: epoch milliseconds,
  version?: number
}
```

`v0.17.6` and `v0.16.4` use the same MIME set and fields except that `BinaryFileData.version` is absent. `created`, `lastRetrieved`, and `version` do not affect pixels unless scene embedding is enabled, when they become part of metadata bytes.

Valid official local files store Data URLs. The map key addressed by the element's `fileId` selects the record; the record's own `id` also participates in SVG symbol IDs. `mimeType` selects binary rejection and dark-mode compensation. Canvas rejects `application/octet-stream` for image-cache use. SVG writes `dataURL` directly into `<image href>` and reuses it via `<symbol>/<use>` by default; `reuseImages: false` creates per-element symbols. Cropped image symbols include a hash of uncropped dimensions in their ID. SVG rounds the image element's displayed width and height to integers before writing `<use>`, while canvas draws to the numeric element dimensions. Sources: [`v0.18.1 types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/types.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), [`element/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts), and [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).

The imported JSON type is not runtime enforcement. Export does not verify that `dataURL` starts with `data:`: canvas assigns it directly to `HTMLImageElement.src`, and SVG copies it to `href`. A malformed/untrusted scene can therefore request an external image. An embedded SVG image can also contain external references; upstream provides no renderer-level validation and leaves their handling to browser image security/decoding rules. The network-free renderer must reject external resolution and locally sandbox SVG content rather than copy this trust boundary. Animated GIF/WebP/AVIF data are not frozen by Excalidraw: SVG preserves the animated resource, while canvas draws whatever frame the browser decoder exposes when export runs. These cases need explicit deterministic policy. Sources: [`element/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), and the accepted MIME list in [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts).

The editor's ingestion constant is 4 MiB in `v0.18.1` and 2 MiB in both older candidates, but the static export path does not enforce that limit on already-supplied `files`. A renderer processing untrusted local documents therefore needs its own decoded/input/output resource limits rather than treating the editor constant as export protection. Sources: [`v0.18.1 constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), [`v0.17.6 constants.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/constants.ts), and [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts).

Missing/failed image behavior differs by format. Canvas draws a light-gray image placeholder and chooses a distinct error glyph when `status == "error"`; with a successfully decoded file, `status` does not suppress the image. SVG ignores `status` and emits nothing for an image whose `fileId` is null or absent from `files`. Source: [`v0.18.1 renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts) and [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).

#### Frame and magic frame

Frame adds `name: string | null`; `v0.18.1` magic frame has the same field. `v0.17.6` and `v0.16.4` support ordinary frames only. Source: [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts) and [`v0.17.6 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts).

Whole-scene static export defaults to `frameRendering = { enabled: true, clip: true, name: true, outline: true }`. A frame is drawn as a non-filled rounded 8 px rectangle with `#bbb` 2 px stroke. `v0.18.1` magic-frame canvas outlines use theme-specific green, while its SVG frame branch uses the ordinary frame stroke. Frame opacity multiplies child opacity. Frame names are synthesized as temporary Helvetica text above the frame, truncated to frame width; null names become `Frame` or `AI Frame`. Because Helvetica is local-only, whole-scene exports with frame names are platform-font-dependent even when the scene contains no user text. Sources: [`appState.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/appState.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts), [`fonts/FontMetadata.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts), and [`frame.ts#getFrameLikeTitle`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/frame.ts#L918-L925).

Whole-scene bounds are computed from root elements: frames, elements with no `frameId`, and elements whose referenced frame is missing. Normal frame children do not expand the scene bounds. The required storage order is children immediately before their frame; the official renderer relies on this for correct clipping and performance. Sources: [`frame.ts#getRootElements`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/frame.ts#L254-L272) and the official [`frames.mdx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/dev-docs/docs/codebase/frames.mdx).

SVG creates a rounded `<clipPath>` for every frame and wraps frame children in a group referencing the containing frame ID. Canvas applies a rounded clip according to frame membership/intersection and group logic. This means frame membership, group membership, ordering, rotation, and frame geometry are all compatibility inputs, not editor-only data. Sources: [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), and [`frame.ts#shouldApplyFrameClip`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/frame.ts#L858-L916).

Separate-frame export changes selection/filtering, forces zero padding, hides frame outline/name, and has format-specific clipping adjustments. It is well tested upstream but is outside this project's stated scope. The whole-scene behavior above remains in scope. Source: [`v0.18.1 tests/scene/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/export.test.ts#L190-L520).

#### Embeddable and iframe

`v0.17.6` and `v0.16.4` have `embeddable` with `validated: boolean | null`. `v0.18.1` removes `validated` from the element, adds `iframe`, and allows iframe `customData.generationData` with pending/done/error states. Sources: [`v0.17.6 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts), [`v0.18.1 element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), and the [`v0.18.0` breaking-change notes](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.0).

The default static behavior is offline-friendly. Canvas export passes an empty validation-status map, so iframe-like elements draw an exported placeholder rectangle and centered label. SVG defaults `renderEmbeddables` to false and emits the same placeholder/label plus a link rather than a live iframe. Empty embeddables say `Empty Web-Embed`; linked embeddables show the link; iframe elements say `IFrame element`. Sources: [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), and [`element/embeddable.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/embeddable.ts).

If `renderEmbeddables: true`, SVG may emit a `<foreignObject>` with an iframe whose URL is transformed for recognized providers. Twitter, Reddit, and gist document embeds still fall back to a link because `srcdoc` is not used in SVG. This mode is network-dependent and is incompatible with the destination's network-free requirement. Source: [`element/embeddable.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/embeddable.ts) and [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).

#### Selection and unknown element types

`selection` remains in the type union for legacy/internal use but restore filters it before rendering. Restore also filters unknown element types and invisibly small elements. For known element types, restore deliberately spreads the original object before normalized properties so unknown properties survive for forward compatibility. Sources: [`v0.18.1 data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts) and its [`restore tests`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/data/restore.test.ts).

The destination requirement is stricter than upstream here: silently filtering newer types is insufficient. A compatible renderer should diagnose each unsupported rendering-affecting type/property/value, retain all understood fields, and produce a declared best-effort placeholder or omission.

### Restore-time compatibility behavior

The public export wrappers call `restore()` before low-level export. Reproducing only the final type declarations is therefore insufficient for parity on local files. The pinned restore implementation performs these material migrations/repairs: [source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts).

- Defaults missing common style/geometry/version/binding fields; generates missing IDs; normalizes dimensions and links; converts legacy `strokeSharpness` to a roundness type and `boundElementIds` to `boundElements`.
- Preserves unknown properties on known element types.
- Converts legacy text `font` strings; defaults alignments, text, `originalText`, `autoResize`, and line height; marks empty text deleted.
- Converts legacy `draw` to `line`; synthesizes two points when points are missing/invalid; normalizes point origins; recalculates linear dimensions; defaults arrow end head to `arrow` when undefined.
- Nulls `lastCommittedPoint` for line, arrow, and freedraw.
- Defaults image status/scale/crop and frame names.
- Always regenerates duplicate IDs. Missing-frame membership, text/container relationships, cross-element bindings, text-dimension refresh, and malformed elbow-arrow repairs run only when `repairBindings`/`refreshDimensions` options are enabled.
- In `0.18`, assigns/repairs fractional indices in array order without reordering the array. The fractional-index implementation documents array order as the cache and rendering boundary. Source: [`fractionalIndex.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fractionalIndex.ts).

The public `exportToSvg`/`exportToCanvas` wrappers call `restore(..., null, null)` without an element configuration, so they do not enable the optional relationship, dimension, or elbow repairs. Missing ID and timestamp repair and duplicate-ID replacement remain nondeterministic (`nanoid()` and current time). Valid official local files already contain these fields, so a parity harness should reject or separately classify malformed/defaulted fixtures rather than mix them into deterministic golden tests. Sources: [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts), [`random.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/random.ts), and [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts).

### App state that matters

There are two distinct app-state surfaces.

#### Persisted local-document app state

`v0.18.1` local export retains only `viewBackgroundColor`, `gridSize`, `gridStep`, and `gridModeEnabled`. `v0.17.6` and `v0.16.4` retain only `viewBackgroundColor` and `gridSize`. Grid values do not appear in static exports because export explicitly disables grid rendering; `viewBackgroundColor` remains visually relevant both as an optional painted background and as the fill used by outline arrowheads. Sources: [`v0.18.1 appState.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/appState.ts), [`v0.17.6 appState.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/appState.ts), and [`v0.18.1 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts).

#### Static-export options/defaults

The public utilities restore a full default app state, but these values control the static artifact: [source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts).

| Input | Official behavior |
| --- | --- |
| `exportBackground` | Paints `viewBackgroundColor` when true; transparent otherwise. Default app state is true. JPEG forces true, but JPEG is outside the requested PNG/SVG scope. |
| `viewBackgroundColor` | Background color and outline-arrowhead fill context. Default `#ffffff`. |
| `exportPadding` | Logical padding around whole-scene bounds. Default 10 px; zero only when requested or exporting a separate frame. |
| `exportScale` | SVG multiplies `width`/`height` but not `viewBox`. The low-level canvas exporter also uses it, but the public canvas/blob wrapper supplies its own canvas factory and does not use restored `appState.exportScale` for overall output dimensions by default; callers scale public canvas/blob output through `getDimensions`, or through the `maxWidthOrHeight` fallback described below. The app-state value still reaches internal renderer paths such as the temporary canvas for a labeled arrow. Default app state derives SVG/low-level scale from `devicePixelRatio` if DPR is 1, 2, or 3, otherwise 1. |
| `exportWithDarkMode` | Applies `invert(93%) hue-rotate(180deg)` to the exported root/canvas. Non-SVG bitmap images receive an additional inverse/rotation/saturation compensation. |
| `exportEmbedScene` | Adds a compressed scene payload to SVG metadata or a PNG `tEXt` chunk. |
| `frameRendering` | `enabled`, `clip`, `name`, and `outline`; all default true. Not persisted in local scene app state. |
| `maxWidthOrHeight` | Public canvas/blob only: caps the larger dimension and overrides `getDimensions`; when content is smaller, uses `appState?.exportScale ?? 1` from the caller's supplied partial app state. |
| `getDimensions` | Public canvas/blob only: custom output width/height and optional drawing scale. Width/height and scale are independent inputs, so inconsistent values can add whitespace or clip content. |
| `renderEmbeddables` | SVG only; defaults false. True permits network-backed iframe foreign objects. |
| `skipInliningFonts` | `0.18` SVG only; true emits no generated embedded font faces. |
| `reuseImages` | `0.18` SVG only; defaults true. |
| `mimeType`, `quality` | Blob only. PNG is default and ignores quality. |

The default `exportScale` dependence on DPR is defined in [`appState.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/appState.ts); public canvas sizing is in [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts), and labeled-arrow temporary sizing is in [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts). The dark-mode filters and padding constants are in [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts); the canvas filter/background behavior is in [`renderer/helpers.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/helpers.ts).

The package also exposes `exportToClipboard`, which delegates to the same SVG/blob paths or serialized JSON and adds no distinct static artifact semantics. Source: [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts).

## Complete static-output semantics

### Shared scene selection and paint order

Whole-scene size is the common bounds of root elements plus padding. Elements are painted in array order, except bound text is painted with its container and iframe-like elements are removed from the main pass and painted in a final top pass. Deleted elements are skipped by the render loops. Frames and frame children must follow the documented child-before-frame order. Sources: [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), and [`frames.mdx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/dev-docs/docs/codebase/frames.mdx).

The SVG and canvas renderers are separate implementations. They share shape generation but differ in text APIs, image failure behavior, masks/clipping details, and rasterization. Perceptual tests should compare each project output with the corresponding official format, not assume that rasterizing the official SVG defines official PNG behavior. Sources: [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), and [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts).

### SVG

The official `v0.18.1` SVG has:

- root `version="1.1"`, SVG namespace, logical `viewBox`, and scaled `width`/`height`;
- optional root dark-mode filter;
- `<!-- svg-source:excalidraw -->`, a `<metadata>` element, and `<defs>`;
- frame clip paths and generated font-face CSS in `<defs>`;
- optional background rectangle;
- RoughJS path groups with output precision capped at two decimals;
- real SVG `<text>` nodes, not glyph outlines;
- image Data URLs in reusable `<symbol>/<use>` nodes;
- masks for arrow labels and cropped images;
- `<a href>` wrappers for linked elements; and
- optional compressed embedded scene metadata.

Source: [`v0.18.1 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), and the pinned [`scene export snapshot`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/__snapshots__/export.test.ts.snap).

When `exportEmbedScene` is true, `v0.18.1` serializes the supplied original elements/app state/files, compresses/encodes the JSON, and places base64 between `payload-start`/`payload-end` comments inside `<metadata>` with MIME `application/vnd.excalidraw+json` and payload version 2. `v0.17.6` uses the earlier metadata-comment layout shown by its pinned fixture. Sources: [`v0.18.1 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`tests/export.test.tsx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/export.test.tsx), and [`v0.17.6 smiley_embedded_v2.svg`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/fixtures/smiley_embedded_v2.svg).

### PNG

PNG goes through the official canvas renderer and browser `canvas.toBlob("image/png")`. Links are not represented: export mode suppresses the editor's visible link icon and a raster PNG has no link structure. Embedded images are decoded through browser `HTMLImageElement`; fonts are loaded before `0.18` canvas export; output dimensions are determined by scale/capping options and canvas integer dimensions. Sources: [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts), [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), and [`element/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts).

When `exportEmbedScene` is true, the wrapper inserts a PNG `tEXt` chunk immediately before `IEND`. Its keyword is `application/vnd.excalidraw+json`; its text is the compressed/encoded serialized scene. Source: [`data/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/image.ts) and the round-trip tests in [`tests/export.test.tsx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/export.test.tsx).

### Links

Any element with a non-empty `link` is wrapped in an SVG `<a href>` after trimming, escaping quotes, and URL sanitization. The upstream link snapshot confirms this structure. Embeddable fallbacks additionally use `target="_blank"` and `rel="noopener noreferrer"`. Relative links and element-link host checks can depend on `window.location`, so the oracle origin is part of link semantics. Sources: [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`data/url.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/url.ts), [`element/elementLink.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/elementLink.ts), and [`tests/scene/__snapshots__/export.test.ts.snap`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/__snapshots__/export.test.ts.snap).

`v0.18` element links are ordinary absolute URLs with an `element=<id>` query parameter on the same host. Static SVG does not convert them to local SVG fragments; it preserves the URL. Sources: [`element/elementLink.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/elementLink.ts) and [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).

Recommendation for PDF: preserve sanitized external URLs as PDF link annotations over the element's transformed bounds. Preserve same-document/element URLs as URI annotations unless a separate, explicitly specified PDF-internal destination mapping is adopted. PNG cannot preserve links.

### Fonts and assets

`v0.17.6` SVG references Virgil, Cascadia, and Assistant assets by URL. Helvetica and emoji rendering depend on local platform fonts/fallbacks. Its package/application tree contains the WOFF2 files, and the export API allows an asset base path, but the emitted SVG is not self-contained by default. Sources: [`v0.17.6 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/scene/export.ts), [`public/fonts.css`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/public/fonts.css), and [`smiley_embedded_v2.svg`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/fixtures/smiley_embedded_v2.svg).

`v0.18.1` registers bundled Cascadia, Comic Shanns, Excalifont, Liberation Sans, Lilita One, Nunito, Virgil, and Xiaolai faces; Helvetica and Segoe UI Emoji are explicitly local-only. It uses per-family fixed metrics. Canvas attempts to load needed faces before export. SVG gathers unique `originalText` codepoints by family, adds Xiaolai for CJK, fetches relevant WOFF2 faces, subsets them using bundled HarfBuzz/WOFF2 machinery, and emits data-URL `@font-face` rules in stable source order. Per-face load/subset failures are caught and logged rather than failing the whole export; canvas can continue with browser fallback, and SVG font generation can fall back to an external asset URL. A reference run must therefore promote those console failures or unexpected external URLs to harness failures. Sources: [`fonts/Fonts.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts), [`fonts/FontMetadata.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts), [`fonts/ExcalidrawFontFace.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts), and [`tests/scene/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/export.test.ts).

The `v0.18.0` release notes say package fonts default to a CDN and instruct self-hosters to copy the package's `dist/prod/fonts` and set `EXCALIDRAW_ASSET_PATH`. The code also has an esm.sh fallback URL. Thus official SVG font inlining can perform network fetches at export time unless assets are locally served/cached; when fetch/subsetting succeeds, output is self-contained except for local-only Helvetica/emoji. Sources: [`v0.18.0` release notes](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.0) and [`fonts/ExcalidrawFontFace.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts).

For a network-free Rust implementation, exact pinned font bytes and metrics must be bundled or transformed at build time. Helvetica and Segoe UI Emoji need an explicit deterministic substitution or diagnostic because upstream intentionally delegates them to the host OS. `v0.18.1` already includes Liberation Sans as a server-side substitute for Helvetica, but using it changes glyph metrics/appearance unless that substitution is part of the declared compatibility profile. This affects generated frame names and embeddable placeholders as well as user text because both generators choose Helvetica. Sources: [`fonts/Fonts.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts), [`fonts/FontMetadata.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts), [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), and [`element/embeddable.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/embeddable.ts).

Asset redistribution has already been inventoried separately for provisional `v0.18.1`. The immutable [asset licensing and distribution report](https://github.com/sagikazarmark/excalidraw-render.orig/blob/0b44df6fc64ce71d192fb3b04d5841b0927f8290/docs/research/asset-licensing-and-distribution.md) identifies 230 rendering WOFF2 files across eight families, the local-only Helvetica/Segoe gaps, the official server-style Liberation Sans/Noto Emoji option, and remaining notice/subsetting decisions. Ticket #9 must re-run its mechanical hash/license diff if it chooses another baseline; this ticket does not duplicate or supersede that audit.

### PDF compatibility surface

Observed: no candidate exposes official PDF export. Source: [`v0.18.1 packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts) and [`v0.17.6 src/packages/utils.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/packages/utils.ts).

Recommendation: define vector-first PDF as a semantic projection of the pinned SVG contract:

- retain vector paths/fills/clipping/masks where the PDF backend supports them;
- retain text as text only when exact pinned font embedding and placement are available, otherwise use glyph outlines with a diagnostic;
- embed local image bytes;
- translate SVG anchors to URI annotations;
- flatten the official dark-mode filters to deterministic colors if PDF filter support differs; and
- rasterize only unsupported local subtrees, never the whole page by default.

This policy should have its own acceptance criteria; it cannot claim official PDF parity.

## Determinism and reproducibility

### Deterministic inputs and algorithms

- A non-zero element `seed` is explicitly documented as the random integer used so RoughJS shape generation does not differ across renders, and the official shape options pass it directly. RoughJS `4.6.4` and `4.5.2` fall back to `Math.random()` when the seed is zero, so `seed: 0` is a genuine nondeterministic edge case even though the Excalidraw schema accepts any number. Sources: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts), pinned RoughJS [`4.6.4 math.ts`](https://github.com/pshihn/rough/blob/7944f1cb95da6efadff7c15c9dc96ae0abc51199/src/math.ts), and [`4.5.2 math.ts`](https://github.com/pshihn/rough/blob/e2c1551a87c623338d0a76316bb86abf37bc3beb/src/math.ts).
- SVG RoughJS drawables force at most two decimal places; freedraw path serialization separately trims to two decimals. Sources: [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), and [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts).
- `0.18` fractional-index repair uses deterministic keys between neighboring indices and preserves input array order; ties during explicit index ordering are broken by element ID. Source: [`fractionalIndex.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fractionalIndex.ts).
- Font-face async work is collected by assigned source-order indices before CSS is returned, rather than completion order. Sources: [`fonts/Fonts.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts) and [`utils.ts#PromisePool`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/utils.ts#L1187-L1234).

### Environment-dependent or nondeterministic behavior

- Default app-state/SVG and low-level canvas scale depends on browser `devicePixelRatio`; the public canvas/blob overall-dimension caveat is described above. Source: [`appState.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/appState.ts) and [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts).
- PNG pixels and bytes depend on the browser canvas implementation, font rasterizer, image decoder, antialiasing, and PNG encoder because the official API delegates to DOM canvas and `toBlob()`. Sources: [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts), and [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts).
- `v0.17.6` SVG display depends on whether external font URLs resolve. `v0.18.1` can fetch/subset fonts during export, and local-only Helvetica/Segoe UI Emoji remain platform-dependent. Sources: [`v0.17.6 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/scene/export.ts), [`v0.18.1 fonts/ExcalidrawFontFace.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts), and [`fonts/FontMetadata.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts).
- Generated `v0.18.1` frame labels and embeddable placeholders use local-only Helvetica, so they inherit platform font metrics and rasterization. `v0.17.6` frame labels instead use externally referenced Assistant. Sources: [`v0.18.1 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`element/embeddable.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/embeddable.ts), and [`v0.17.6 scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/scene/export.ts).
- Animated image resources are copied through to SVG without freezing and are decoded/drawn asynchronously for canvas, so observed content can depend on playback/decode timing. Non-Data-URL `dataURL` values can introduce network and response variability; nested SVG references are delegated to browser image security/decoder behavior unless the harness rejects them. Sources: [`element/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), and [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts).
- CSS color strings and SVG text are interpreted by the browser/viewer. Relative links, element-link host classification, default serialization `source`, and font asset bases depend on `window.location` and `window.EXCALIDRAW_ASSET_PATH`. Sources: [`renderer/helpers.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/helpers.ts), [`data/url.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/url.ts), [`element/elementLink.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/elementLink.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts), and [`fonts/ExcalidrawFontFace.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts).
- Missing IDs and timestamps are synthesized from random/current-time sources during restore. Duplicate IDs are regenerated. Source: [`random.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/random.ts) and [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts).
- SVG text remains text, so final viewing can depend on the consumer's SVG text shaping and fallback fonts even when primary faces are embedded. Source: [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts).
- Embedded-scene JSON includes `source`, file timestamps, and all preserved current element metadata. Those values can change artifact bytes without changing pixels. Source: [`data/json.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/json.ts) and [`data/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/types.ts).

### Recommended reproducibility contract

- Pin release and commit, official package integrity, all rendering-helper versions, the already-selected Playwright image/browser profile, DPR 1, locale, timezone, origin, OS font set, and every export option.
- Serve all pinned official font assets locally to the reference harness and block network access. Record font-load failures as invalid reference runs.
- Use valid complete scene documents with stable IDs, non-zero seeds, timestamps, indices, dimensions, and locally validated files; test malformed/defaulted and animated/external-resource documents separately.
- Compare normalized SVG structure and rasterized perceptual output. Do not require byte-identical SVG where XML serialization or embedded metadata is nonvisual.
- Compare PNG perceptually and by dimensions/alpha/background semantics, not by compressed bytes.
- Test repeated Rust renders for byte determinism even though the browser reference is only perceptually deterministic. Deterministic production output is a useful stronger guarantee.

## Compatibility diagnostics implied by the candidates

Because scene `version` cannot identify the producing release, diagnostics should be feature-based.

For a `v0.17.6` baseline, rendering-loss diagnostics are required for `iframe`, `magicframe`, image `crop`, arrow `elbowed`/`fixedSegments`/special-end flags/fixed-point bindings, arrowhead values outside the older four, font IDs 5-9, missing old text `baseline`, and newer fallback requirements. The renderer can then render the understood remainder. Evidence for the complete schema delta is the immutable [`v0.17.6...v0.18.1` comparison](https://github.com/excalidraw/excalidraw/compare/0dbd2a39319d41fda37b2945dea0dcbd58d6a564...a2ec2889babf7d2295469c6d90ebe77fae57df84), especially the pinned [`v0.17.6 element types`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts), [`v0.18.1 element types`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), and constants files.

Other `0.18` markers do not by themselves imply static rendering loss. `autoResize`, `index`, app-state `gridStep`/`gridModeEnabled`, and `customData.generationData` are schema/editor/normalization inputs whose visible result is already represented by persisted geometry, text, and array order or is deliberately ignored by static export. They should be accepted and preserved where applicable, not turned into noisy compatibility warnings unless malformed values actually change the chosen renderer's behavior.

For any baseline, diagnose:

- unknown element types and enum values;
- unsupported/missing font family or glyph coverage;
- missing/invalid image file records, unsupported binary MIME, image decode failure, external/nested resource requests, or animated-image policy;
- malformed relationships (`frameId`, `containerId`, bindings, duplicate IDs);
- non-finite/extreme geometry and malformed points/crops;
- zero RoughJS seeds where deterministic output is required;
- requested live external embed content, which the network-free renderer must replace with the official placeholder/link fallback;
- unsupported SVG/PDF effects that require subtree rasterization; and
- embedded-scene metadata that cannot be retained exactly.

The upstream restore behavior and tests provide primary evidence for unknown-type filtering, unknown-property retention, normalization, and malformed-data repairs: [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts) and [`tests/data/restore.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/data/restore.test.ts).

## Useful upstream fixtures and tests

### Highest-value reference sources

| Reference source | What to reuse |
| --- | --- |
| [`v0.18.1 tests/scene/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/export.test.ts) and [snapshot](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/__snapshots__/export.test.ts.snap) | Default SVG structure; CJK and external font inlining; background; dark mode; padding; scale; scene metadata; links; whole-scene and separate-frame behavior. |
| [`v0.17.6 tests/scene/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/scene/export.test.ts) and [snapshot](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/tests/scene/__snapshots__/export.test.ts.snap) | Direct comparison for the pre-`0.18` SVG/font/frame contract. |
| [`v0.18.1 tests/export.test.tsx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/export.test.tsx) | Embedded PNG/SVG v1/v2 round trips, emoji payloads, transformed image reuse, and a structural image-export snapshot. |
| [`v0.18.1 tests/fixtures/svg-image-exporting-reference.svg`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/svg-image-exporting-reference.svg) | Visual reference for four combinations of image rotation and horizontal/vertical flip. |
| [`v0.18.1 tests/cropElement.test.tsx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/cropElement.test.tsx) | Crop persistence and exported SVG/canvas aspect-ratio checks. |
| [`v0.18.1 tests/fixtures/smiley_embedded_v2.svg`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/smiley_embedded_v2.svg), [`smiley_embedded_v2.png`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/smiley_embedded_v2.png), [`test_embedded_v1.svg`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/test_embedded_v1.svg), and [`test_embedded_v1.png`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/test_embedded_v1.png) | Compatibility fixtures for embedded-scene metadata generations. |
| [`v0.18.1 tests/data/restore.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/data/restore.test.ts) and [snapshot](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/data/__snapshots__/restore.test.ts.snap) | Default/migration behavior, unknown-type filtering, line normalization, font fallback, IDs, and binding repair. |
| [`v0.18.1 element/elbowArrow.test.tsx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/elbowArrow.test.tsx) | Elbow route and fixed-segment edge cases for `0.18`. |
| [`v0.18.1 frame.test.tsx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/frame.test.tsx) and [`frames.mdx`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/dev-docs/docs/codebase/frames.mdx) | Frame membership and required ordering invariants. |
| [`v0.18.1 tests/fixtures/diagramFixture.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/diagramFixture.ts) and [`elementFixture.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/elementFixture.ts) | Small stable element/envelope constructors used by official package export tests. |
| [Project reference-oracle report](https://github.com/sagikazarmark/excalidraw-render.orig/blob/287f33596eb43fc8d855cd64e4bfb2aa0350b4cb/docs/research/reference-oracle-options.md) | Already-validated harness pin and evidence workflow: official `exportToSvg` plus direct browser-canvas PNG, local assets, blocked network, explicit options, two fresh-process runs, and provenance sidecars. |

### Important gaps in upstream coverage

The pinned repository has no comprehensive checked-in `.excalidraw` scene corpus for static export. The main diagram fixture has only diamond, ellipse, and rectangle. Export tests structurally snapshot SVG but canvas tests mostly assert dimensions; repository test searches reveal no pixel-screenshot matcher for static export. Sources: [`diagramFixture.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/fixtures/diagramFixture.ts), [`packages/utils/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.test.ts), and [`tests/scene/export.test.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/export.test.ts).

Therefore the project needs its own generated, valid, pinned fixture matrix. At minimum it should cross:

- every element type and each candidate-only feature;
- all fill/stroke/roughness/roundness/opacity combinations with stable non-zero seeds, plus a separate zero-seed diagnostic case;
- official palette values, valid CSS color syntaxes accepted by the canonical browser, transparency, and invalid color strings;
- straight/rounded/closed lines, every arrowhead, bindings, and bound text;
- pressure and simulated-pressure freedraw;
- every supported font, multiline/RTL/CJK/emoji, wrapping, alignments, and fallback failures;
- every image MIME, alpha, crop, fractional dimensions, rotation, mirroring, rounded corners, reuse, missing/error files, dark mode, animation, malformed external `dataURL`, and nested SVG external references;
- ordinary/magic frames, labels, opacity, rotation, children crossing each edge, grouped children, malformed order, and missing frame references;
- external and element links, sanitized-dangerous links, and offline embeddables;
- transparent/colored backgrounds, padding/scales, embedded-scene on/off; and
- repeated exports under a fixed harness to expose nondeterminism.

Generate these scenes through the pinned official package's element constructors where possible, then freeze the resulting local documents and official SVG/PNG artifacts. This avoids hand-authoring invalid combinations while retaining auditable JSON inputs. The official helper used by upstream tests is [`tests/helpers/api.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/helpers/api.ts).

## Unknowns and follow-up decisions

1. **Baseline release.** Choose between the broad modern `v0.18.1` contract and the smaller `v0.17.6` contract; retain `v0.16.4` only if legacy output itself is a goal. This report does not decide.
2. **Baseline pin set and upgrade edge.** Along with the release commit, pin npm integrity and the complete rendering dependency graph. If `v0.16.4` remains a candidate, its RoughJS/direct curve versions must be treated as intentional output behavior. Ticket #9 should diff schema, export files, dependency versions, and font/asset hashes before any future upgrade.
3. **Canonical oracle profile.** Ticket #13 already selected the official package in pinned Linux/amd64 Chromium and produced a known-working Playwright/container/browser pin. Confirm that profile as the product's one browser reference, set all export options explicitly, and keep native macOS/Windows typography outside the universal golden rather than reopening the oracle implementation choice.
4. **Public scale contract.** Decide whether compatibility follows the public package wrappers or the lower-level scene exporter. They differ: public canvas/blob output does not use restored `appState.exportScale` for overall dimensions unless `getDimensions`/`maxWidthOrHeight` supplies scaling, while public SVG applies it; internal labeled-arrow canvas behavior still receives the app-state value.
5. **Text and fallback policy.** Decide whether self-contained SVG/PDF preserves selectable text and adopt a deterministic policy for local-only Helvetica and Segoe UI Emoji. The completed [asset audit](https://github.com/sagikazarmark/excalidraw-render.orig/blob/0b44df6fc64ce71d192fb3b04d5841b0927f8290/docs/research/asset-licensing-and-distribution.md) recommends the official server-style Liberation Sans/Noto Emoji route and records the remaining output-notice/subsetting decisions; the baseline ticket must re-audit hashes if it chooses another release.
6. **Generated-label typography.** Apply the fallback decision explicitly to frame names and embeddable placeholders, not only scene text. In `v0.18.1` both use local-only Helvetica, so otherwise even a geometry-only framed scene remains platform-dependent.
7. **Animated and external image policy.** Define which frame of animated GIF/WebP/AVIF is authoritative, reject non-Data-URL file records, and specify sandboxing/diagnostics for external references inside embedded SVG. Copying the upstream browser trust boundary would violate the network-free and untrusted-input requirements.
8. **Color grammar.** Decide whether to implement the canonical browser's full CSS color grammar or a documented subset. Element/background color fields are arbitrary strings, and invalid canvas color assignment does not have a simple schema-level fallback.
9. **SVG metadata.** Decide whether `exportEmbedScene` is required. It is useful round-trip semantics but enlarges output and introduces nonvisual byte variability.
10. **PDF link mapping.** Decide whether `?element=<id>` URLs remain URI annotations or become internal PDF destinations. Upstream SVG preserves the URI and supplies no PDF precedent.
11. **Best-effort placeholders.** Specify a stable visual placeholder and diagnostic schema for unknown element types, unsupported newer enum values, missing files/fonts, zero seeds, and forbidden live embeds. Do not warn for harmless newer editor metadata such as `index` or grid settings when static output is fully understood.
12. **Parity tolerance.** Define per-format perceptual metrics and thresholds. Rough geometry can be structurally compared, but browser text/rasterization requires tolerant image comparison.
13. **Cross-format differences.** Decide whether known official SVG/PNG divergences, including missing-image behavior and integer-rounded SVG image dimensions, must be copied exactly or normalized under a documented compatibility exception.
14. **Empty/malformed scenes.** Specify bounds/output and diagnostic behavior for empty scenes, non-finite geometry, duplicate IDs, invalid relationships, and optional restore repairs. The public export wrappers do not enable relationship/elbow repair and some defaulting is nondeterministic.

## Operational handoff

- [Ticket #9, baseline and upgrades](https://github.com/sagikazarmark/excalidraw-render.orig/issues/9): decide `v0.18.1` versus `v0.17.6`, record source commit plus npm integrity/dependencies, and keep `v0.16.4` only for an explicit legacy promise.
- [Ticket #3, compatibility contract](https://github.com/sagikazarmark/excalidraw-render.orig/issues/3): settle public-versus-low-level scale semantics, fallback typography, generated labels, animated/external images, color grammar, metadata, diagnostics, and whether to preserve official SVG/PNG divergences.
- [Ticket #7, robustness](https://github.com/sagikazarmark/excalidraw-render.orig/issues/7): set limits independently of the editor's unenforced file-size constant and define rejection/sandbox behavior for malformed files, non-finite geometry, external references, and optional restore repairs.
- [Ticket #12, PDF](https://github.com/sagikazarmark/excalidraw-render.orig/issues/12): treat PDF as a specified projection, including text/font fallback, URI annotations, filter flattening, and subtree rasterization; no official PDF oracle exists.
- [Ticket #15, conformance](https://github.com/sagikazarmark/excalidraw-render.orig/issues/15): build the generated fixture matrix above on the already-selected pinned Playwright oracle, set format-specific tolerances, and keep same-build Rust byte determinism separate from browser parity.

## Primary-source index

- Official releases: [`v0.18.1`](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.1), [`v0.18.0`](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.0).
- Official version package metadata: [`0.18.1`](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.18.1), [`0.17.6`](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.17.6), [`0.16.4`](https://registry.npmjs.org/@excalidraw%2Fexcalidraw/0.16.4).
- Pinned commits: [`v0.18.1`](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84), [`v0.17.6`](https://github.com/excalidraw/excalidraw/tree/0dbd2a39319d41fda37b2945dea0dcbd58d6a564), [`v0.16.4`](https://github.com/excalidraw/excalidraw/tree/1863da44053dcf95aa4be0891065e8a580002de6).
- Pinned dependency manifests: [`v0.18.1`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json), [`v0.17.6`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/package.json), [`v0.16.4`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/package.json), RoughJS [`4.6.4`](https://github.com/pshihn/rough/tree/7944f1cb95da6efadff7c15c9dc96ae0abc51199) and [`4.5.2`](https://github.com/pshihn/rough/tree/e2c1551a87c623338d0a76316bb86abf37bc3beb).
- `v0.18.1` schema/API: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts), [`types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/types.ts), [`data/types.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/types.ts), [`packages/utils/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts).
- `v0.18.1` restore/persistence: [`data/restore.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts), [`data/json.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/json.ts), [`appState.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/appState.ts).
- `v0.18.1` renderers: [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts), [`scene/Shape.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/Shape.ts), [`renderer/staticSvgScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticSvgScene.ts), [`renderer/staticScene.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/staticScene.ts), [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/renderer/renderElement.ts).
- `v0.18.1` frames/embeds/fonts/files: [`frame.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/frame.ts), [`element/embeddable.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/embeddable.ts), [`fonts/Fonts.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts), [`element/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/image.ts), [`data/image.ts`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/image.ts).
- `v0.17.6` comparison sources: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/element/types.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/constants.ts), [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/scene/export.ts), [`renderer/renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/0dbd2a39319d41fda37b2945dea0dcbd58d6a564/src/renderer/renderElement.ts).
- `v0.16.4` comparison sources: [`element/types.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/element/types.ts), [`constants.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/constants.ts), [`scene/export.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/scene/export.ts), [`renderer/renderScene.ts`](https://github.com/excalidraw/excalidraw/blob/1863da44053dcf95aa4be0891065e8a580002de6/src/renderer/renderScene.ts).
- Coordinating project research: immutable [reference-oracle report](https://github.com/sagikazarmark/excalidraw-render.orig/blob/287f33596eb43fc8d855cd64e4bfb2aa0350b4cb/docs/research/reference-oracle-options.md) and [asset licensing report](https://github.com/sagikazarmark/excalidraw-render.orig/blob/0b44df6fc64ce71d192fb3b04d5841b0927f8290/docs/research/asset-licensing-and-distribution.md).
