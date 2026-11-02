# External harnesses and the reference oracle

Scope: `tools/reference-oracle`, `harness_pdf_viewers`, `harness_svg_viewers`,
the viewer qualification scripts and evidence, `tools/svg-viewer-rust`,
`tools/pdf-harness`, the Docker and host-tool pins.

## 1. Reference oracle

A Vite-bundled browser wrapper around `@excalidraw/excalidraw@0.18.1`, driven
by Playwright in a digest-pinned Playwright image. `src/browser-entry.js:3`
imports `exportToBlob` and `exportToSvg`; SVG through `exportToSvg`
(`:112-117`), PNG through `exportToBlob` (`:118-130`), the official canvas
path. `scripts/conformance-case.mjs` loads one registry entry, launches
Chromium twice in fresh processes, asserts identical digests and distinct
PIDs, and writes `official.svg`, `official.png`, both `.rgba` rasters and
`provenance.json`. `run-conformance.sh:26-36` runs in `--network none
--platform linux/amd64` and does `npm run build && profile verify && node
conformance-case.mjs <mode> <id>`: every case run rebuilds the bundle and
re-verifies the profile (about 11–25 s per case under emulation). Three
generators exist, one per registry kind; there is no "all cases" command.

The profile (`tests/oracle/reference-profiles/revision-1.json`, canonical JSON,
identity in the sidecar) binds the npm integrity and source commit, the lock
SHA-256 and twelve named dependencies, 192 bundle files, nine wrapper sources,
the container digest, Chromium 149.0.7827.55, Node v24.16.0, npm 11.13.0,
fontconfig 2.15.0, 230 WOFF2 assets and 48 system fonts, and the browser
launch arguments (`profile.mjs`).

`just reference-profile verify` runs `profile.mjs verify` (asserts the
environment, rebuilds the manifest and profile and byte-compares them) and
`smoke.mjs` (renders a fixture twice in fresh Chromium, zero console errors,
identical hashes).

Platform problems (`docs/ci.md:101-108`): the images are `linux/amd64`; under
ARM emulation Excalidraw's font-subsetting worker misses its 1000 ms deadline,
so the smoke render and every text-bearing case fail (local run: 26 pass, 11
fail, ten of them text cases); the `linux/arm64` image is a different Chromium
and differs in blended pixels.

**Drift fixed by `86fcf1aa`.** Dependabot commit `96d5ff2a` (2026-10-14) bumped
`dompurify`, `mermaid`, `postcss` and `source-map-js` in the oracle lock, and
`659523ca` (2026-10-22) bumped `vite`; both changed the lock digest and the
bundle, so every `reference-profile` and `reference-case` run failed as
"stale". Nobody noticed for about 2.5 weeks because no CI job runs the oracle,
and `bc18be90` had just made the oracle lock historical in the offline
verifiers so PRs stopped failing. `86fcf1aa` reverted the lock, added a
Dependabot `ignore: "*"` for the directory, and fixed two generators still
requiring a pre-`d96cdddf` scene-surface digest — a second, independent latent
breakage. **The oracle is run in no workflow.**

## 2. Upstream tracking

What a move from 0.18.1 would regenerate: the reference and frame profiles;
hard-coded `PROFILE_DIGEST`/`ASSET_DIGEST`/`SURFACE_DIGEST` constants in about
13 source files (`conformance-case.mjs:47-49`,
`typography-conformance-case.mjs:94-96`, `scene-surface.mjs:19-21`,
`xtask/src/catalog.rs:10-30`, `conformance.rs`, `pdf_viewers.rs`,
`svg_viewers.rs`, `revision_1.rs`, `compatibility_identity.rs`,
`empty_scene_render.rs`); the scene-surface manifest; the font catalog (230
chunks, reviewed decodes, generated Rust, `+excalidraw.0.18.1` metadata); 37
cases of official evidence (489 files, 85 MB) through 37 separate invocations
whose generators refuse to overwrite approved manifests; the font-subset
viewer qualification; the SVG and PDF viewer controls; the representative
references; the candidate chain and native baseline; the spec §3 identity
table. The profile digest appears in 684 files and `0.18.1` in 667.

Documentation: spec §19 ("evaluated by release-note and source-diff review…
regenerate affected evidence, classify all changes, prove two-process
stability, activate atomically") and §3 (a post-publication bump is a new
revision coexisting with the retained one). Tooling: pieces exist but nothing
end to end; `revision-1` is a path and type name. Detection of an upstream
change is impossible by construction and nothing even checks the registry for
a new release.

The strategy helps in principle (everything derives from one closure; the
stability check and path-exact comparison would classify every change) and
hinders in practice: the digest is everywhere, there is no batch mode, the
oracle has no CI run, and text cases need Linux x86-64 so a bump cannot be
rehearsed on the maintainer's machine.

## 3. SVG viewer harness

`harness_svg_viewers.rs:17-78`, one ignored test: renders our rectangle SVG,
then Chromium through `run-svg-viewer.sh` in the pinned image and the
pure-Rust viewer through `cargo run --release --manifest-path
tools/svg-viewer-rust/Cargo.toml`, and asserts each raster byte-equal to the
official SVG's raster from the same viewer. It adds a real browser and a
closed-world pinned resvg 0.48.1 over the workspace resvg 0.49.0 in
`parity_registered_case_svg`, byte equality rather than a budget, and process
isolation — for one text-free case. Not in any workflow; no run record.

**#107/#109 risk is live again**: `f35a8e0c` (2026-09-05) created
`pdf-harness-gates.yml` on push and nightly because "expectations drifted
unnoticed (#107)"; `05a2e478` (2026-10-13) deleted it. `docs/ci.md:206-207`
now says the gates are not scheduled; `restage.yaml` `viewer-checks` defaults
to false. `harness_pdf_viewers/main.rs:5` still claims the gate "cannot drift
out of the pdf-harness-gates workflow".

## 4. PDF viewer harness

Nine ignored tests: `rectangle_outline.rs` (PDFium and PDF.js against the
Chromium official-SVG raster, qpdf structure), `excalifont_text.rs`,
`frame_export.rs` (seven frames), `backend_qualification.rs` (three structural
gates and the isolated fallback), `adapters.rs` (the 96/72 scale contract).
Not the other 33 scene cases.

Platform: `devenv.nix:7-42` fetches `pdfium-binaries` build 7947 for four
hosts and links it under `target/pdf-harness`; the worker asserts
`PDFIUM_VERSION = "152.0.7947.0"`; the library digest is recorded, not
asserted (acebefcf). qpdf 12.3.2 from nixpkgs-multiverse. PDF.js still runs
only in the Playwright image → Docker on any host.

**Drift**: `tools/pdf-harness/pdfjs/package.json` pins `pdfjs-dist 6.2.108`
(Dependabot `96d5ff2a`) while `viewer-profiles/canonical-pdf-pdfjs-v1.json`
and `xtask/src/pdf_viewers.rs:290-312` say 6.1.200 and bind a lockfile hash
that no longer matches. This passes because `historical_producer()` whitelists
the directory. Empirically fine (V29 ran 6.2.108 with identical rasters), but
the profile describes a viewer that is not the one running.

Value over the fast-tier PDF tests: the PDF parses and renders in two
independent engines; visual parity of text with the Chromium raster; qpdf
`--check` by an independent parser; the adapter contract. With Krilla pre-1.0
and byte-affecting on bump (ADR-0021), this is the only external check on the
PDF backend, and it is unscheduled.

## 5. Viewer qualification

Before a viewer judges, it must render a known control to the approved raster,
deterministically, and fail on negative controls, under a frozen profile:

| Script → evidence | Controls | Files / bytes |
|---|---|---|
| `svg-viewer-qualification.sh` → `svg-viewer-qualification/rectangle-outline/` | official SVG and a candidate control in Chromium and pure-Rust, ×2 | 11 / 396 KB |
| `pdf-viewer-qualification.sh` → `pdf-viewer-qualification/rectangle-outline/` | six PDFs (valid, visual-fail, structure-fail, warning, coverage, action); PDFium and PDF.js ×2; qpdf ×2 | 29 / 2.9 MB |
| `svg-font-subset-qualification.sh` → `svg-font-subset-viewer-qualification/` | 231 sources → 662 executions in two viewers, two samples each (about 2,648 Chromium processes) | 4 / 2.5 MB |

Regeneration: the SVG and PDF controls date from 2026-07-24 and were
requalified once each. The font-subset qualification was regenerated 17 times
since 2026-08-01, most recently 2026-10-27 and 2026-10-30 via `restage.yaml`,
both with every raster byte-identical: the manifest keys executions by
`semantic_subset_identity` (a hash of the subset bytes), so any profile token
or subsetter change rewrites 2.4 MB (a 5 MB diff) to restate that nothing
changed. Not proportionate; keying by glyph set and face would make a token
change a one-field edit.

## 6. Pure-Rust SVG viewer

`tools/svg-viewer-rust/` pins `resvg`/`usvg` `=0.48.1` with its own lock,
digest-verified by `verify-svg-viewers`, and its own `rust-toolchain.toml`.
The workspace is on 0.49.0. Two because the viewer is a judge with a frozen
identity qualified against retained rasters; the workspace resvg is the
renderer's PNG dependency. It adds a closed-world profile (no system fonts,
declaration-bound resources, WOFF2 → reviewed sfnt), process isolation and
byte-equality judging over the parity test's in-process SSIM budget, and
serves as the ADR-0014 control.

## 7. Docker and platform surface

Image digests: `mcr.microsoft.com/playwright@sha256:111dde95…` in about 100
files (33 `run-*.sh`, five `.mjs`, `webp-helper.Dockerfile`, two pdf-harness
scripts, two intake scripts, the font-subset script, four xtask sources, four
profiles, 37 provenance files, six oracle references, the spec, four fast
tests); `rust@sha256:1bcff4be…` once (`svg-font-subset-qualification.sh:9`, the
only place the toolchain still comes from Docker); a local webp helper image.

Host-tool literals: Node, npm, fontconfig, Chromium, Playwright come from the
image and are unaffected by devenv. rustc `1.97.1` is explicit in `devenv.nix`
and asserted in `svg-viewer-qualification.sh:18`, `svg_viewers.rs:136`,
`svg_font_subsets.rs:498`, `conformance_candidate_native.rs:206`,
`tools/svg-viewer-rust/rust-toolchain.toml`, `harness_svg_viewers.rs:18`;
editing it fails two verifiers until 2.4 MB of evidence is regenerated. qpdf
12.3.2 and PDFium 7947 are fixed in the nix expression. PDF.js 6.1.200 in the
profile versus 6.2.108 live. The `devenv update` commit `cecec8ca` changed only
`devenv.lock` and required no evidence edits, consistent with ADR-0024.

## 8. Demo and intake

`tools/demo` is dev tooling (a static side-by-side page; no test depends on
it). `tools/representative-intake` is test infrastructure for the
representative pipeline; its Docker legs are ignored tests.

## 9. Assessment

| Component | Unique catch | Cost | Actually run | Recommendation |
|---|---|---|---|---|
| Reference oracle | The only source of truth; profile drift; two-process stability | Docker; Linux x86-64 for text; ~17 s per case | Never in CI; broken Oct 14–Nov 1 | Keep as ground truth; schedule `reference-profile verify` and a reproduce of all cases on `ubuntu-latest`; build the bundle once |
| `harness_pdf_viewers` | Krilla output in PDFium and PDF.js; qpdf validity | Docker (PDF.js); minutes | Scheduled Sep 5–Oct 13, unscheduled since | Nightly |
| `harness_svg_viewers` | The rectangle byte-equal in Chromium and pinned resvg | Docker; a minute | Never in CI | Nightly with the PDF harness |
| SVG and PDF viewer qualification | Viewer determinism and negative controls | Docker; minutes | Reproduced 2026-10-27 | On viewer or image change |
| Font-subset viewer qualification | Subsets render as their sources in two viewers | Docker ×2; ~2 h Linux; 2.4 MB | 17 times | Release only; re-key by glyph set |
| Pure-Rust viewer | Closed-world judge; ADR-0014 control | Second lockfile | Requalified once | Keep; lag the workspace deliberately |
| `tools/representative-intake` | Intake and six-viewer collection | Docker, Linux, 180 s deadlines | V29 on 2026-10-30 | On demand |
| `tools/demo` | none | none | — | Dev tooling |

Making an Excalidraw bump one command and one review: a single recipe that in
one container session builds the bundle once, verifies the profile once,
iterates `registry.json` through the three generators, then updates the scene
surface, imports the catalog and regenerates the viewer controls; constants
read from the profile sidecar and `revision-N` as one configuration value;
`generate` refusing approved evidence by default with a `--supersede` flag;
the scheduled drift job; re-keying the font-subset qualification; resolving
the PDF.js identity mismatch.
