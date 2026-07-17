# Deterministic reference-oracle options

Research date: 2026-07-17

Ticket: [#13, Evaluate deterministic reference-oracle options](https://github.com/sagikazarmark/excalidraw-render.orig/issues/13)

Map: Specify perceptually equivalent Excalidraw static exports

## Executive answer

Use a repository-owned, development-only Playwright wrapper around the exact
`@excalidraw/excalidraw` package as the reference oracle. Run it in one
digest-pinned `linux/amd64` Playwright container, serve the package's own font
and image assets locally, and disable network access while rendering. Generate
both formats through the public package APIs:

- `exportToSvg()` for the official SVG structure;
- `exportToBlob({ mimeType: "image/png" })` for the official browser-canvas
  PNG, rather than rasterizing the SVG with a second renderer.

The initial experimentally verified pin set is:

- `@excalidraw/excalidraw` `0.18.1`;
- `playwright` `1.61.0`;
- Playwright's Ubuntu 24.04 `linux/amd64` image manifest
  `sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9`
  (the multi-platform tag's index digest is
  `sha256:57b65fdc9ceabe0ef613124c7bbe2babcf9362c4d85e382fe3b03604e84b428a`);
- the container's Chromium `149.0.7827.55` and Node `24.16.0`.

The version numbers are not special compatibility promises. They are a
known-working baseline. Preserve them in an exact package lock and image digest,
then upgrade them only through an explicit evidence-regeneration review.

`excalidraw-image` `0.2.0` is reproducible enough to be a useful secondary
diagnostic and a convenient local generator. It should not be the primary PNG
oracle: it runs a modified host environment around Excalidraw's SVG path and
rasterizes SVG to PNG with `resvg`, not Chromium canvas. Its own fidelity
documentation says PNG text spacing is approximate and that browser/SSIM and
PNG-golden gates are not implemented. In the experiment below its text PNG
differed from the official browser PNG at 1.9655% of pixels under pixelmatch's
`0.1` threshold, even though its repeated outputs were byte-stable.

Check the generated PNGs, SVGs, and provenance sidecars into the repository.
Normal tests should read those files using Rust only. Do not run the oracle in
the normal test path. Test the Rust renderer's required same-build byte
stability separately by rendering the same input twice and comparing its own
bytes; byte equality with the official oracle is neither required nor useful
for perceptual parity.

This design does not change the production boundary. The shipped renderer can
remain pure Rust, WASM-safe, network-free, and independent of Node, JS/V8,
browsers, Docker, and OS graphics libraries.

## Scope and decision criteria

The oracle has two jobs:

1. Capture what a pinned official Excalidraw release emits for a canonical
   scene and explicit export settings.
2. Leave durable evidence that ordinary Rust tests can consume after the
   development-only oracle is gone.

It is not a second production implementation, and it is not the definition of
the Rust renderer's byte encoding. The decision therefore prioritizes:

- authority for both Excalidraw SVG and browser PNG behavior;
- exact version and asset pinning;
- reproducible regeneration in local development and CI;
- explicit font, locale, scale, and export settings;
- no render-time network access;
- reviewable checked-in evidence;
- isolation from production and normal-test dependencies.

## Comparison matrix

| Option | SVG and PNG authority | Pinning and upgrades | Runtime and OS dependencies | Fonts and assets | Determinism | License and redistribution | Decision |
|---|---|---|---|---|---|---|---|
| Official `@excalidraw/excalidraw` in a repository-owned Playwright wrapper | Direct public `exportToSvg` and `exportToBlob` calls. PNG uses the browser canvas path that users receive. Highest authority. | Exact npm versions and integrity in a lockfile; exact Playwright version; exact OS/architecture image digest. Package, browser, and wrapper can be upgraded independently and reviewed. | Development only: OCI/Docker host; inside the image, Ubuntu 24.04, Node, Playwright, Chromium and browser shared libraries; React peer packages; a bundler for the browser entry. No runtime dependency in the Rust product or ordinary tests. | Package WOFF2 files are served from `node_modules`; external requests are rejected. Binary scene files must be supplied in `files`. OS-local fallbacks remain a policy issue. | Repeatable in the tested pinned image. Playwright warns that host OS, browser, hardware, settings, power and headless mode can affect pixels, so one architecture/image must be canonical. Run twice before accepting evidence. | Excalidraw is MIT; Playwright is Apache-2.0; Node, Chromium, Ubuntu and bundled fonts have their own notices. PNG does not embed font software; default SVG does embed WOFF2 subsets and needs font-license handling. | **Primary oracle.** Smallest authoritative wrapper, despite the heaviest development runtime. |
| `excalidraw-image` `0.2.0` prebuilt binary | Uses Excalidraw `0.18.1` for SVG generation, but with linkedom, canvas/font shims and custom text metrics. PNG is `resvg`/`tiny-skia`, not official browser canvas. | Pin release tag, target archive and SHA-256. `--version` reports the embedded Excalidraw and `deno_core` versions. Upgrading one CLI version may change Excalidraw, shims, fonts, V8 and PNG stack together. | Development runtime is one target-specific executable with embedded V8 and fonts. The tested Linux binary dynamically linked glibc, `libgcc_s`, and `libm`. It writes a V8 snapshot cache and spawns a warmer unless disabled. Source builds additionally need Rust, Node/npm, Deno for parity tests, a bundler and native build tooling. | Prebuilt releases enable `cjk-full` and bundle fonts; default `cargo install` is Latin-only. No system fonts or render-time network. Emoji, Helvetica, embeddables and CJK shard behavior have documented caveats. | Two SVG and two PNG fixtures repeated byte-for-byte in the experiment. Project tests gate JS-host versus embedded-V8 SVG bytes, not official browser PNG pixels. `--no-snapshot-cache` removes cache side effects. | Project is MIT and bundles Excalidraw plus third-party code/fonts. The root MIT license does not erase the separate font and transitive licenses. | **Secondary diagnostic only.** Excellent convenience and triangulation, but not an independent authoritative PNG oracle. |
| Published `@excalidraw/utils` | Exposes export helpers, but its stable/public version stream is not tied to Excalidraw `0.18.x`. The current monorepo source is not what the old stable package version proves. | Stable `0.1.2` was published in 2022; npm's current `latest` points to `0.1.3-test32`, published in 2025. There is no `0.18.1`-aligned release to pin. | Browser-oriented JS/UMD dependencies; still needs browser APIs for canvas and fonts. | Old bundle owns an old dependency set and cannot establish current font/schema behavior. | Pinning an old tarball is reproducible but answers the wrong versioned behavior. | MIT. | **Reject.** A misleading version seam, not a current oracle. |
| Official package in Node with DOM and `node-canvas` or similar shims | Can call portions of the official code, but PNG comes from Cairo/Pango rather than Chromium and SVG needs a sufficiently complete DOM shim. This creates another renderer to validate. | Must pin Node, Excalidraw, DOM shim, canvas package, native libraries, fonts and bundler. Upgrades have many coupled surfaces. | Node plus native `node-canvas`; source builds require Cairo and Pango, with optional libpng/libjpeg/giflib/librsvg and platform toolchains. Prebuild support is platform-specific and Linux is glibc-only. | Fonts must be registered manually. System Pango/fontconfig state can leak into output. | Possible in a fully pinned container, but less authoritative and no lighter operationally than using the real browser image. | `node-canvas` is MIT; native libraries and fonts retain separate licenses. | **Reject.** More custom surface and lower authority than Playwright. |

## Verified behavior from primary sources

### Official Excalidraw package

`@excalidraw/excalidraw` `0.18.1` is the current stable package and corresponds
to commit `a2ec2889babf7d2295469c6d90ebe77fae57df84`. The release is a security
patch for the `0.18.x` line. The exact package manifest identifies a browser
ES module, React/React DOM peer ranges, a single runtime package-root export,
and MIT licensing. Its wildcard package exports expose types, not a supported
runtime-only static-export submodule. A consumer should therefore bundle the
package root rather than depend on internal source paths.

Primary sources:

- [Excalidraw v0.18.1 release](https://github.com/excalidraw/excalidraw/releases/tag/v0.18.1)
- [Exact package manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json)
- [Exact npm metadata and tarball integrity](https://registry.npmjs.org/@excalidraw/excalidraw/0.18.1)
- [Repository MIT license at the pinned commit](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/LICENSE)

The public wrappers restore elements and app state before export.
`exportToBlob` delegates to `exportToCanvas` and then browser
`HTMLCanvasElement.toBlob`; `exportToSvg` delegates to the static SVG renderer.
The lower-level canvas path loads element fonts, initializes images, and draws
through Rough.js and the browser 2D canvas. The SVG path uses the same scene
logic but emits DOM/SVG nodes and optionally inlines subsetted font faces.

Primary sources:

- [Public export wrappers, including `exportToBlob` and `exportToSvg`](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/export.ts#L24-L194)
- [Canvas export implementation](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L153-L255)
- [SVG export implementation and font inlining](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L265-L479)
- [Official export API documentation at the pinned commit](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/dev-docs/docs/%40excalidraw/excalidraw/api/utils/export.mdx)
- [Official SVG/canvas export tests and snapshots](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/export.test.ts)

Fonts are a load-bearing input. The package README says it defaults to a CDN
and instructs self-hosters to copy `dist/prod/fonts` and set
`window.EXCALIDRAW_ASSET_PATH`. The implementation constructs a package-version
CDN fallback and fetches WOFF2 data before subsetting. A network-free oracle
must override that path and serve the exact package files locally; merely
waiting for `document.fonts.ready` is not enough if the URLs are uncontrolled.

Primary sources:

- [Official self-hosted-font instructions](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/README.md#L17-L25)
- [Font URL fallback, fetch and subsetting implementation](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts#L11-L169)
- [Emoji is explicitly an OS-local font](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Emoji/index.ts)
- [Segoe UI Emoji is marked local fallback metadata](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/FontMetadata.ts#L123-L133)

The repository contains an `index-node.ts` example using `node-canvas`, but it
is an internal source example, not the package's exported Node entry. It
registers only Virgil and Cascadia and demonstrates canvas/PNG, not a complete
current SVG-and-PNG oracle. It is evidence that an adapter is possible, not
that the published package is Node-native.

- [Pinned internal Node canvas example](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/index-node.ts)

### `excalidraw-image`

`excalidraw-image` `0.2.0` pins `@excalidraw/excalidraw` `0.18.1`, embeds a
`deno_core`/V8 host, ships fonts in Rust font crates, and uses native
`resvg`/`usvg`/`tiny-skia` for PNG. Its release provides binaries for Linux
x86_64/aarch64, macOS x86_64/aarch64 and Windows x86_64. Prebuilt binaries
enable full CJK assets; source installation without features is Latin-only.

Its fidelity document is unusually clear about the relevant limit: it does not
claim pixel identity with a browser. It replaces browser text measurement with
fontkit, has OS-local font and emoji exceptions, and says PNG horizontal glyph
advance can differ because `resvg` and Chromium use different shaping
backends. Browser/SSIM PNG comparison and committed PNG goldens are listed as
known follow-ups.

The existing determinism gates prove different things from this ticket's
oracle:

- checked-in SVG goldens compare the project's own bundled JS output;
- the parity test compares Deno-hosted and embedded-V8 SVG bytes;
- the browser-font test consumes a checked-in Chromium metrics baseline and
  does not launch a browser;
- PNG integration tests check valid output and size thresholds, not pixels.

Primary sources:

- [`excalidraw-image` v0.2.0 release and target archives](https://github.com/rickardp/excalidraw-image/releases/tag/v0.2.0)
- [Pinned JS dependency manifest](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/package.json)
- [CLI crate dependencies and feature tiers](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/crates/excalidraw-image/Cargo.toml)
- [Architecture, build requirements, cache and release targets](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/README.md)
- [Documented fidelity and divergence contract](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/docs/fidelity.md)
- [Native PNG raster implementation](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/crates/excalidraw-image/src/raster.rs)
- [Embedded V8 host and deterministic crypto/cache handling](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/crates/excalidraw-image/src/engine.rs)
- [Deno-versus-Rust SVG parity gate](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/crates/excalidraw-image/tests/parity.rs)
- [PNG smoke tests and explicit lack of pixel comparison](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/crates/excalidraw-image/tests/png.rs)
- [Cross-platform build/test workflow](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/.github/workflows/test.yml)
- [MIT license](https://github.com/rickardp/excalidraw-image/blob/9cafd553d979d66879a13863181be152ef5c92bb/LICENSE)

### Browser and container controls

Playwright ties every package version to specific browser revisions and says a
browser install may need to change on each Playwright update. Its Docker image
contains browser binaries and OS dependencies but not the Playwright npm
package. The project recommends pinning image versions, matching package and
image versions, and using one CI worker for stability. It also warns that
screenshots vary with OS, browser, settings, hardware, power source and
headless mode, and recommends generating and testing baselines in the same
environment.

The image is Ubuntu/glibc based; Alpine/musl is not supported for all bundled
browsers. These facts make an image digest plus explicit `linux/amd64` a better
pin than `ubuntu-latest` plus a browser download cache.

Primary sources at the tested Playwright tag:

- [Browser-version and OS-dependency contract](https://github.com/microsoft/playwright/blob/v1.61.0/docs/src/browsers.md)
- [Docker image contents, platform and pinning guidance](https://github.com/microsoft/playwright/blob/v1.61.0/docs/src/docker.md)
- [Exact Ubuntu Noble image build definition](https://github.com/microsoft/playwright/blob/v1.61.0/utils/docker/Dockerfile.noble)
- [Visual-comparison determinism warning and checked-in snapshots](https://github.com/microsoft/playwright/blob/v1.61.0/docs/src/test-snapshots-js.md)
- [CI container and single-worker guidance](https://github.com/microsoft/playwright/blob/v1.61.0/docs/src/ci.md)
- [Playwright Apache-2.0 license](https://github.com/microsoft/playwright/blob/v1.61.0/LICENSE)

### Published `@excalidraw/utils`

The separately published utility package is not a version-aligned substitute
for the main package. Its last non-test release, `0.1.2`, was published in
March 2022. As retrieved on the research date, npm's `latest` dist-tag pointed
to `0.1.3-test32`, a package published in April 2025 from another Excalidraw
commit. Neither package identifies itself with Excalidraw `0.18.1`. Pinning one
would make installation repeatable without making its behavior the requested
reference version.

- [Exact `@excalidraw/utils` 0.1.2 npm metadata](https://registry.npmjs.org/@excalidraw%2Futils/0.1.2)
- [Exact `@excalidraw/utils` 0.1.3-test32 npm metadata](https://registry.npmjs.org/@excalidraw%2Futils/0.1.3-test32)
- [Utility package source present at Excalidraw 0.18.1](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/utils/package.json)

### Rejected Node canvas host

`node-canvas` implements Canvas with Cairo, not Chromium. Its documented
prebuilt targets are macOS x86_64/aarch64, Linux x86_64 glibc, and Windows
x86_64. Other targets compile locally and require Cairo and Pango plus optional
format libraries. That expands the pinning surface and changes the renderer
whose pixels are being treated as truth.

- [`node-canvas` v3.2.0 platform, native dependency and font documentation](https://github.com/Automattic/node-canvas/blob/9bcf3631b41c422ad832118186ee9f02bbde2810/Readme.md)

## Operational experiments

These experiments were disposable and lived outside this repository. They
verify feasibility and repeatability for a narrow fixture sample; they are not
proof for every element, font or host.

### Environment and commands

The official-package harness used exact `@excalidraw/excalidraw` `0.18.1`,
React/React DOM `19.2.3`, Playwright `1.61.0`, and the pinned Playwright
`linux/amd64` image above. A small browser bundle called both public export
APIs. A Playwright route served only files under the package's
`dist/prod/fonts`; every other browser request was aborted. The final text run
also used Docker `--network none` and produced the same hashes.

Representative invocation shape:

```sh
docker run --rm --init --ipc=host --network none \
  --platform linux/amd64 \
  mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9 \
  node run.mjs fixture.excalidraw expected.svg expected.png
```

The package's worker-based WOFF2 subset path could not be split by the
disposable Bun bundle, so official code logged that it was falling back to its
main-thread subset implementation. Outputs were stable, but a maintained
wrapper should use a bundler configuration that supports the package's worker
chunk or explicitly lock and test this fallback.

The `excalidraw-image` experiment downloaded the v0.2.0 Linux x86_64 release
archive and verified its published archive digest before invoking:

```sh
excalidraw-image fixture.excalidraw -o expected.svg --no-snapshot-cache
excalidraw-image fixture.excalidraw -o expected.png --no-snapshot-cache
```

The archive SHA-256 was
`6a86f5188b55aef092d9cedda786565bfdb399bb58e57a1bea26335ed8f19ac6`;
the extracted executable SHA-256 was
`6daba56e87553a8f67b1b30d256b8740ea50ab7e75892c817b52e137b45fc000`.
`--version` reported `excalidraw-image 0.2.0 (excalidraw 0.18.1, deno_core
0.399.0)`. The executable was approximately 87 MiB. `ldd` showed only the
Linux loader, glibc, `libgcc_s` and `libm`; Node, Deno and a browser were not
external runtime dependencies.

### Repeatability results

| Fixture and output | Official package run 1/run 2 SHA-256 | `excalidraw-image` run 1/run 2 SHA-256 | Observation |
|---|---|---|---|
| Basic geometry SVG | `4d740c3c8536f082b9c483d746f3127f688e7cce312111bf3811ccbb68dea17a` | `a578f79a249eedadd9b7f193ff0e2314e6696ee1154c90daa1186e3e9892e4f0` | Each implementation repeated exactly. The CLI output also exactly matched its checked-in SVG golden. Cross-implementation bytes differed mainly in DOM serialization and attribute order. |
| Basic geometry PNG | `b519ccb3764854eed5fd672aea19673c7a8bc66b5e1d92070a715e32a816dc4f` | `41ccbdc263b0196a7a0edd63b98839b94d7b5d69e5a60235c08f533f8d5b231b` | Each repeated exactly. Decoded pixels had `0 / 58,800` pixelmatch differences at threshold `0.1`; PNG encodings differed. |
| Wrapped text SVG | `47392273523a5324f20dbacb18ff425d8d5a9770c75fa8f608d9549e7149c242` | `ea7a13c1de8efe6ad92e0a9483b00fba66c0c28227ce809139f0bcfea7d77611` | Each repeated exactly. The CLI output exactly matched its checked-in golden, but browser/linkedom serialization differed. |
| Wrapped text PNG | `53e719b824b806f30e706a33b70a5fd7ac88717bbc867c3283995405c58c513a` | `9985f7c16aa73dbbe20576a7b8e4b560870d84c380d92896f9eb03b2ebc668ce` | Each repeated exactly. `4,104 / 208,800` pixels differed at threshold `0.1` (`1.9655%`), consistent with different text raster/shaping backends. |

The official text outputs generated with Docker network access disabled had
the same SVG and PNG hashes as the earlier runs. This verifies that locally
served package fonts are sufficient for these fixtures.

### What the experiment establishes

- Both candidates can produce same-environment byte-stable evidence for the
  tested inputs.
- Official SVG bytes and linkedom-serialized SVG bytes need not match even
  when their rendered geometry does; byte identity is the wrong cross-oracle
  criterion.
- PNG byte differences can be encoder-only for geometry, but browser versus
  `resvg` text can create real pixel differences.
- A pinned, offline browser oracle is operationally feasible in local Docker
  and Linux CI.

### What the experiment does not establish

- stability on another architecture, kernel, CPU/GPU, browser headless mode,
  or container release;
- fidelity for CJK, emoji, bidirectional text, images, frames, crops,
  freedraw, bindings, embeddables or every Excalidraw element;
- that either candidate remains stable after an upstream or toolchain upgrade;
- a final perceptual metric or acceptance threshold for the Rust renderer.

## Recommended reproducibility design

### Evidence layout

Use an explicit, versioned evidence layout when implementation begins. One
possible shape is:

```text
tests/oracle/fixtures/<case>.excalidraw
tests/oracle/expected/<case>.svg
tests/oracle/expected/<case>.png
tests/oracle/expected/<case>.oracle.json
tools/reference-oracle/package.json
tools/reference-oracle/package-lock.json
tools/reference-oracle/...
```

The sidecar should record at least:

- fixture SHA-256;
- output SHA-256 and dimensions;
- `@excalidraw/excalidraw` version and npm integrity;
- React, React DOM, bundler and Playwright versions;
- canonical OCI image repository, `linux/amd64` manifest digest, browser
  version, Node version and wrapper revision;
- export options, scale, padding and background;
- whether scene metadata and fonts were embedded;
- a deterministic digest of all locally served font/assets used;
- generation timestamp as sidecar provenance only, never embedded into the
  compared image;
- command used to regenerate.

Do not derive expected artifact names from a floating `latest` version. Keep
the provenance content-addressed even if filenames remain readable.

### Generator contract

The wrapper should do the following for every fixture:

1. Validate that each element carries canonical `id`, `index`, `seed`,
   `version`, `versionNonce`, `updated` and other schema fields instead of
   asking `restore()` to invent values.
2. Reject remote image URLs. Supply embedded images through the scene's
   `files` map.
3. Set every export input explicitly: background, background color, dark mode,
   scene embedding, export scale, padding, frame selection and files.
4. Use a fresh browser context with fixed locale `en-US`, timezone `UTC`,
   device scale factor `1`, color scheme and one worker.
5. Point `window.EXCALIDRAW_ASSET_PATH` at a local origin backed only by the
   exact package assets. Reject all other requests, and run the container with
   `--network none` after dependencies are acquired.
6. Await font loads and all export promises. Fail on font fetch errors,
   console errors, missing glyphs or unexpected fallback families.
7. Call `exportToSvg` and `exportToBlob` independently. Do not generate the
   reference PNG by rasterizing the reference SVG.
8. Run the full generation twice in fresh browser processes. Require identical
   hashes before allowing an update.
9. Write to a temporary tree first. Show artifact and provenance diffs, and
   replace checked-in evidence only through an explicit update command.

Default SVG export embeds only the glyph subsets it uses, producing
self-contained evidence. If redistributing embedded font subsets is not
acceptable after license review, use the public `skipInliningFonts: true`
option, make PNG the canonical visual evidence, and treat SVG as structural
diagnostic evidence. Do not silently depend on host-installed fonts to render a
checked-in SVG.

### Test split

Normal Rust tests:

- consume checked-in PNG evidence with the chosen perceptual comparator;
- use checked-in SVG for structural diagnostics or a separately specified SVG
  comparison, not byte equality with official browser serialization;
- run with no Node, JS/V8, browser, Docker or network;
- render each Rust output twice and require byte equality for this renderer's
  same-build stability contract.

Oracle update/verification job:

- is development-only and opt-in or separately gated;
- runs on Linux x86_64 in the exact image digest with one worker;
- regenerates twice and checks hashes;
- compares against checked-in evidence and uploads visual diffs;
- never updates goldens automatically on an ordinary test run;
- is the only CI job that needs npm and browser/container dependencies.

This separation lets users build, run and test the product from a normal Rust
toolchain while preserving an auditable path back to the reference renderer.

### Upgrade mechanics

Treat the official package and browser as separate controlled variables:

1. Upgrade `@excalidraw/excalidraw` alone where possible. Update its exact
   lockfile integrity, inspect release notes, export source, schema restoration,
   bundled font inventory and licenses, then regenerate all evidence.
2. Upgrade Playwright/browser/Ubuntu separately where possible. Update the npm
   package and matching per-platform image digest together; record the new
   browser and Node versions.
3. Run old and new oracles over the complete fixture corpus. Produce image
   diffs and classify changes as scene semantics, fonts, browser rasterization,
   PNG encoding or wrapper behavior.
4. Require two-run byte stability in the new environment before review.
5. Review changed PNGs perceptually. Never accept an oracle upgrade by merely
   replacing every golden.
6. Keep the old provenance sidecars in git history. If a migration must be
   staged, store old and new evidence side by side temporarily.

`excalidraw-image` upgrades should be evaluated separately. Pin its archive
digest and always use prebuilt `cjk-full` artifacts if it is used for
triangulation. Run with `--no-snapshot-cache` or
`EXCALIDRAW_IMAGE_NO_SNAPSHOT_CACHE=1`. Compare its decoded pixels to the
official browser evidence, but never promote its output merely because it is
more byte-stable or easier to invoke.

## Dependency and OS notes for later README use

### Production and normal development

- Building and running `excalidraw-render` requires only the documented Rust
  toolchain and Rust dependencies.
- The production executable/library must not invoke Node, V8, a browser,
  Docker, OS graphics libraries or the network.
- Ordinary tests consume committed oracle artifacts and do not regenerate
  them. They must remain runnable without JS tooling or network access.
- WASM builds do not include or call the oracle.

### Regenerating official evidence

- Recommended host requirement: Docker or another OCI runtime capable of
  running a `linux/amd64` image. On ARM hosts, explicit amd64 emulation is part
  of the canonical environment and will be slower.
- Canonical image: pin the per-platform digest, not a floating tag or only the
  multi-platform index digest.
- The image contains Ubuntu 24.04 browser libraries and Chromium, but the
  Playwright npm package is installed separately from the exact lockfile.
- The wrapper also needs the exact Excalidraw package, React peer packages and
  a pinned browser bundler. These are development dependencies only.
- Dependency/image acquisition normally needs registry access. Actual evidence
  generation must run with the network disabled. A prebuilt internal oracle
  image can make regeneration fully offline after it is cached.
- A non-container fallback needs a supported Node release, the exact
  Playwright package/browser revision, and Playwright's OS dependencies. It is
  convenient for debugging but not canonical for accepting goldens.
- Expect a large one-time browser image download and a browser process during
  generation. Neither cost applies to normal tests.

### Using `excalidraw-image` as a diagnostic

- Prefer the exact GitHub release archive for the current target and verify its
  SHA-256. Do not use an unpinned Homebrew formula or `cargo install` result as
  golden provenance.
- Linux release binaries require glibc, `libgcc_s`, `libm` and the ELF loader;
  they are not musl-static despite being self-contained with respect to JS,
  browser and font assets.
- Prebuilt releases bundle `cjk-full`; default Cargo builds do not.
- Disable the startup snapshot cache for hermetic invocations. Otherwise the
  tool writes per-user cache files and starts a background warmer process.
- A source build needs Rust 1.75+, Node/npm and build tooling; the project's
  complete parity test path also uses Deno 2.x. Its CI currently uses Node 22,
  Deno 2.x and Rust stable across Linux, macOS and Windows.

## Licensing

This is an engineering inventory, not legal advice.

### Tooling

- Excalidraw code is MIT at the pinned release.
- `excalidraw-image` is MIT, but incorporates Excalidraw, V8/Rust crates,
  raster libraries and font assets with their own licenses.
- Playwright is Apache-2.0.
- [Node 24.16.0](https://github.com/nodejs/node/blob/v24.16.0/LICENSE) is MIT
  with bundled third-party notices.
- [Chromium 149.0.7827.55](https://chromium.googlesource.com/chromium/src/+/refs/tags/149.0.7827.55/LICENSE)
  is BSD-style at the top level and contains a large third-party license
  inventory.
- The Playwright image also distributes Ubuntu packages under their respective
  licenses.
- `node-canvas` is MIT; Cairo, Pango and optional native libraries retain their
  own terms.

Development-only execution avoids shipping these runtimes with the Rust
renderer, but any cached/re-hosted OCI image or prebuilt tool archive must keep
the notices required by its distribution.

### Generated evidence and fonts

The Excalidraw package's MIT license does not uniformly license its bundled
fonts. The pinned source explicitly records, for example:

- [Excalifont under SIL OFL 1.1](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Excalifont/index.ts)
- [Xiaolai under SIL OFL 1.1, with documented modifications](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Xiaolai/index.ts)
- [Comic Shanns under MIT](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ComicShanns/index.ts)

A PNG contains rendered glyph pixels, not the WOFF2 software. A default
Excalidraw SVG contains base64-encoded subsetted WOFF2 font software. Before
checking such SVGs into a distributed crate/repository, preserve the relevant
font copyright/license notices in a nearby notice file or choose
`skipInliningFonts: true` after legal review. The OFL permits embedding but has
notice and derivative-font conditions; other families must be inventoried too.

Record artifact provenance regardless of whether a notice is legally required.
It makes later package/font upgrades and removals auditable.

## Recommendation and caveats

Adopt the official package plus a thin Playwright wrapper as the sole authority
for checked-in reference PNGs and SVGs. Keep the wrapper small: scene loading,
local asset routing, explicit export options, output writing and provenance.
Do not copy Excalidraw internals into the wrapper and do not introduce a
Node-canvas abstraction.

Keep `excalidraw-image` optional. It is valuable for fast manual generation,
for identifying whether a discrepancy is in scene geometry or rasterization,
and for exercising another host around the same upstream SVG logic. It is not
independent enough to break ties with the official browser and not equivalent
enough in text PNGs to define perceptual parity.

The main caveats are:

- The canonical oracle is "official Excalidraw in pinned Linux Chromium," not
  every browser/OS combination.
- The official renderer itself is not promised byte-stable across upgrades.
- Browser pixels require a pinned execution environment even when package and
  fonts are fixed.
- Local/system fallback fonts, especially emoji and Helvetica, can escape the
  package asset pin unless fixture and fallback policy forbids or controls
  them.
- Default SVGs embed separately licensed font subsets.
- The fixture corpus and perceptual threshold still determine how useful the
  evidence is.

## Unknowns and newly sharp follow-up decisions

1. **Canonical platform:** confirm that one Linux x86_64 Chromium baseline is
   the product definition. If Windows/macOS-native typography is also a target,
   one universal byte/pixel golden is impossible; use platform-specific
   evidence or define an explicit fallback-font policy.
2. **System-font scope:** decide whether emoji, Helvetica and unknown local
   fonts are unsupported, normalized to pinned substitutes, or represented by
   separate platform baselines. Excalidraw intentionally treats Segoe UI Emoji
   as local, while the tested Ubuntu image provides Noto Color Emoji instead.
3. **SVG font policy:** choose default inlined-font SVGs plus notices, or
   `skipInliningFonts: true` structural SVGs with PNG as the visual authority.
4. **Bundler worker path:** select and pin a bundler configuration that emits
   Excalidraw's font-subsetting worker correctly, then verify whether its output
   matches the tested main-thread fallback.
5. **Fixture completeness:** add representative text scripts, images, frames,
   crops, arrows/bindings, freedraw, transparency, dark mode and scale cases.
   Embeddables should remain disabled unless a deterministic offline contract
   is designed.
6. **Perceptual metric:** specify the Rust-side comparator, color handling,
   alpha semantics, dimensional tolerance and case-specific thresholds. The
   oracle choice does not settle these.
7. **Container supply chain:** decide whether CI pulls the digest from MCR or
   mirrors an approved image internally; mirroring adds notice and update
   responsibilities.
8. **Cross-machine confirmation:** before accepting the first committed
   evidence, run the exact amd64 digest and lockfile on at least one developer
   machine and CI and confirm identical hashes. The current experiment proves
   independent container-process repeatability on one host only.

The first three are specification decisions. The remaining items can be
implementation acceptance criteria for the future oracle/evidence ticket.
