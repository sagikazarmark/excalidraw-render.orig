# Development and evidence operations

This repository contains a pure-Rust rendering library and native CLI targeting
compatibility revision 1 of the static-renderer design. The
[0.1.0 release policy](release-0.1.0.md) defines the initial public scope.

The approved implementation target is the
[Excalidraw Static Renderer Specification](renderer-spec-v1.md): a
self-contained pure-Rust renderer for scene and frame SVG, PNG, and vector-first PDF
with explicit compatibility diagnostics and no production browser or network
dependency.

The performance and canonical WASM footprint protocols are specified but not yet
activated; they live in [the roadmap](roadmap.md) and gate nothing today.

## Build

```sh
cargo build --release -p excalidraw-render-cli
```

The workspace separates the supported `excalidraw-render` Rust library from the
native `excalidraw-render-cli` package. They share one lockstep version, and the
CLI is a thin adapter over the public library facade.

The three font packages version separately: their versions carry the Excalidraw
release their fonts come from as semver build metadata, e.g.
`0.1.0+excalidraw.0.18.1`. The reference oracle's npm lockfile
(`tools/reference-oracle/package-lock.json`) is the source of that release;
`cargo run -p xtask -- catalog materialize` writes it into the manifests and
`catalog verify` rejects drift. Cargo and crates.io ignore build metadata when
comparing versions, so a new Excalidraw release still needs a version bump, and
dependency requirements name only the part before the `+`.

The library's dependencies take caret requirements. Cargo resolves one version
per semver-compatible range, so an exact `=x.y.z` requirement on a crate the
ecosystem shares (`png`, `libm`, `tiny-skia`, ...) would make the renderer
unresolvable beside any other crate that needs a later patch of it. Crates
whose code computes artifact bytes (decoders, shaping, subsetting,
rasterization, PDF writing, compression, number formatting) can still change
output in a patch release, so `Cargo.lock` fixes every version the gates test,
and moving one of them is a deliberate, reviewed lock update: run the evidence
checks, not only the unit tests. Applications that need a release's exact
bytes commit their own lockfile. The CLI, xtask and dev-only requirements stay
exact; they bind nothing downstream, and `cargo install --locked` uses the
published lock. `serde_json` is a test-only dependency of the library.

The published crates declare `rust-version = "1.92"`, the oldest compiler they
support; `just msrv` compiles the renderer and CLI with it. The gates otherwise
build with the toolchain `devenv.nix` pins (1.97.1).

## Run

```text
excalidraw-render [OPTIONS] <INPUT> --output <OUTPUT>
excalidraw-render frames [OPTIONS] <INPUT> --output <DIRECTORY> --format <svg|png|pdf>
excalidraw-render frames [OPTIONS] <INPUT> --list
```

For scene export, `INPUT` and `OUTPUT` may be `-` for standard input and artifact
standard output. Frame batches accept standard input but require an output
directory. The scene output format is inferred from a recognized path extension
or selected with `--format`; the default padding is 10 logical pixels. Run
`excalidraw-render --help` and `excalidraw-render help frames` for the complete
request and reporting controls. `--max-dimension` applies to PNG and PDF only;
SVG output ignores it with a warning.
For large scenes, prefer `--max-dimension <value>` for PNG and PDF output;
the default scale of 1 preserves the full logical dimensions and may exceed the
raster-surface or PDF page-size limits.

The `frames` command writes one zero-padding artifact per selected live ordinary
or magic frame. It uses readable frame-name basenames, falls back to the stable
source-order name `frame-N`, and resolves unsafe or duplicate names
deterministically. `--name-regex` filters nonempty source frame names. Batch
publication is stage-all by default; `--allow-partial` publishes independent
successes while retaining a nonzero exit status for any failed frame.

`--list` is a dry run: it prints the ordinal, kind, name, and derived basename
of every frame the same selection would export to standard output (or to
`--report-output`), and writes nothing else. An empty selection succeeds, with
a `note:` line on stderr in human mode. `--output` and `--format`
are required only outside `--list`: the output is a directory, so there is no
extension to infer the format from.

The human report (the default, on stderr) prints one line per distinct
diagnostic, `warning: MESSAGE [CODE] (N occurrences; elements ...)`, and hides
informational ones unless `-v, --verbose` is given. A failure prints
`error: MESSAGE [CODE]` with the path and operating-system reason where one
applies, followed by `hint:` lines suggesting a fix. Use `--report-format json`
for every diagnostic occurrence in a stable, machine-readable form.

```sh
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.svg
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.png --scale 2
target/release/excalidraw-render crates/excalidraw-render-cli/tests/fixtures/basic-shapes.excalidraw -o basic.pdf
target/release/excalidraw-render frames drawing.excalidraw -o frames --format png --name-regex 'Slide'
target/release/excalidraw-render frames drawing.excalidraw --list --report-format json
```

## Reference renderer

The pinned official Excalidraw browser renderer is development-only. Normal Rust
builds and renderer/CLI tests do not need Node, a browser, Docker, or network
access. The full workspace's `xtask` evidence tests additionally invoke Node
and pinned qpdf; the devenv shell provides both. See
[`tools/reference-oracle/README.md`](../tools/reference-oracle/README.md) for the
opt-in profile verification workflow.

The PDF harness uses a native PDFium library with Rust bindings. The devenv
shell supplies the pinned upstream PDFium build and qpdf for the host and links
them under `target/pdf-harness/`; `bash tools/pdf-harness/install.sh` then
builds the Rust worker and installs PDF.js, which runs in Docker. No Python is
needed. The complete PDFium/PDF.js/qpdf setup is documented in
[`tools/pdf-harness/README.md`](../tools/pdf-harness/README.md).

`cargo run --locked -p xtask -- demo build` writes a static side-by-side page of
this library's renders beside the committed official exports to `target/demo/`;
see [`tools/demo/README.md`](../tools/demo/README.md).

## Maintenance commands

The `xtask` command grammar lives in `xtask/src/cli.rs`. Run
`cargo run --locked -p xtask -- <catalog|conformance|representative|pdf-harness|demo> --help`
for the commands and arguments of each family. Help exits 0; malformed
invocations and failed commands exit 1.

## Revision catalog

Compatibility revision 1 incorporates its approved manifests, license bundle,
and official rendering-font bytes into a sealed catalog. Ordinary verification
is offline and read-only:

```sh
cargo run --locked -p xtask -- catalog verify
```

The catalog also records every official font chunk's reviewed decode, the sfnt
the pinned `wuff 0.2.9` decode of its exact bytes produces and the only bytes
PDF may embed for it (spec 8.6). `catalog verify` re-derives all 230 decodes
and compares the actual decoded bytes; `import` and `materialize` regenerate the
record.

Maintainers can reproduce the checked font files from an explicit local
`@excalidraw/excalidraw` `dist/prod` directory. The importer verifies every
source byte against the approved reference asset manifest and never downloads
or generates production data during a build:

```sh
cargo run --locked -p xtask -- catalog import <local-dist-prod>
```

## Font catalog and loading

Original WOFF2 files live under each font crate's `fonts/<family>/` directory.
The generated Rust embeds each file with `include_bytes!`; there is no packed
font blob or runtime segment reconstruction. The optional Xiaolai crates expose
generated file indexes, while the base crate embeds its fonts directly.
`catalog verify` checks exact source bytes, file inventory, package membership,
and generated indexes. The frozen JSON catalog retains historical segment
coordinates as provenance so its identity and downstream evidence stay stable;
runtime loading does not use those coordinates.

The `excalidraw-render-fonts` package (`crates/excalidraw-fonts`, imported
under the dependency key `excalidraw-fonts`) presents the bundled fonts in its
`fonts` module as typed `FamilyId`s and opaque `FaceId`s. `FamilyId::faces()`
returns an immutable slice in upstream declaration order, generated by
`xtask catalog` from the existing asset and face declarations. A face exposes
declared Unicode ranges, source identity and sizes, container format, and
availability. Noto Emoji is a complete sfnt face; official fonts are WOFF2
chunks. Metadata remains enumerable with CJK payloads disabled.

`crates/excalidraw-render/src/fonts.rs` owns verified source-program loading and
a preparation-local `FontLoader`. It is also the renderer crate's sole interface
to the external font package: production and unit-test modules import through
`crate::fonts`. Explicit crate-private exports provide font metadata and subset
policy; `fonts::inspection` exposes raw catalog inspection only for tests and
qualification. Identity-based notice and PDF chunk-digest lookup live behind
this interface as well.

Its independent source and decoded caches each retain at most 16 MiB, evict in
insertion order, and bypass larger entries.
`load_sfnt` accepts only the exact reviewed program, using the checked-in
Excalifont Latin decode where available. SVG uses verified original containers
through its own session loader and retains the qualified subsetter's decode path.

Excalidraw source-family metrics, first-occurrence primary-face selection,
fallback probes, and per-output font demand remain renderer policy. Cache hits
do not alter semantic resource charges. Glyph coverage comes from the decoded
font's cmap; a declared Unicode range alone does not establish coverage. Nested
SVG viewers use the same verified loading functions with their existing isolated
Excalifont/Liberation font database.

## Artifact byte stability

`tests/package-consumer` depends on Cargo's unpacked package output rather than
the workspace crate. It renders the approved rectangle through the public Rust
facade to SVG, PNG, and PDF twice and compares complete artifact and report
bytes with the checked native expectations. `just package` packages the
workspace, `just package-cli` checks the packaged CLI and `just package-facade`
runs the consumer:

```sh
cargo package --locked --workspace --exclude xtask --exclude excalidraw-render-compare --no-verify
bash tests/package-consumer/check-packaged-cli.sh x86_64-unknown-linux-gnu
(cd tests/package-consumer && cargo clean && cargo test --locked --release --test public_facade)
```

Run the consumer from its own directory so its `.cargo/config.toml` (target
directory, WASM test runner) applies, and clean it first: `cargo package`
stamps the unpacked sources with a fixed mtime, so a consumer build left over
from an earlier package never notices that the crate changed.

`just wasm-packaging` runs the same consumer in browser WASM (ADR-0024).

## Tests

`just test` runs the fast tier and `just ci` every per-change gate
([CI and evidence operations](ci.md)). The details below explain the tiers and
release-only checks behind that workflow.

The suite is tiered, and `.config/nextest.toml` gives each tier a
[cargo-nextest](https://nexte.st) profile that selects it by binary name and
kind, so neither a workflow nor a developer names individual tests. The
renderer's integration binaries under `crates/excalidraw-render/tests` are named
by tier:

| binary | tier | profile |
|---|---|---|
| `fast` (one module per former binary) and every other crate's targets | every unit, component and full-pipeline test | `ci-fast` |
| `parity_registered_case_svg`, `parity_registered_case_png`, `parity_frame_export` | parity against official evidence: the registry-driven scene harnesses and the frame-export case | `ci-render` |
| `limits_scene_admission`, `limits_scene_scaling` | fixed parser ceilings that build 64 MiB and million-entry scenes, and scenes large enough that a quadratic per-element lookup would exceed a time ceiling | `ci-render` |
| `harness_pdf_viewers`, `harness_svg_viewers` | `#[ignore]`d gates that need the pinned PDFium, PDF.js, qpdf and Chromium harnesses | `harness` |

```sh
cargo nextest run --workspace                      # a developer's run: ci-fast and ci-render together
cargo nextest run --workspace --profile ci-fast    # the fast tier (`just test`)
cargo nextest run --workspace --profile ci-render  # the render tier (`just test-render`)
cargo nextest run -p excalidraw-render --profile harness --run-ignored only
cargo test --workspace --doc                       # nextest runs no doc tests
```

`cargo test --workspace` still runs every non-ignored test in one go, without
the tiers. The devenv shell provides nextest.

The two registry parity harnesses render each registered variant once per
format per run: the SVG harness judges one render on path geometry, root extent
and the rasterized document, the PNG harness one render against the direct
official raster, and each spreads its variants over the available cores. The
comparator, not the renderer, is what their time goes to.

The root `Cargo.toml` optimizes the `test` profile (`opt-level = 2`, keeping
debug assertions, overflow checks, and debuginfo), because brotli quality-11
encoding, WOFF2 decoding, SSIM, and resvg rasterization dominate the suite and
run an order of magnitude slower unoptimized. Dependencies are optimized under
the `dev` profile as well, so `cargo run -p xtask` verification shares the
compiled external dependencies with `cargo test`.

Two corpus-driven tests form the release-mode qualification set and are not
part of the push gate: the SVG font subset qualification corpus
(`render::font_subset_qualification::complete_font_subset_qualification_corpus_matches_generated_bytes_and_traces`)
and the frame batch property test
(`render::frame_batch_property::frame_batch_rendering_matches_standalone_around_resource_limits`).
`just qualification x86_64-unknown-linux-gnu`, which the `Release evidence`
workflow runs, runs the complete renderer unit target in
`--release` with the environment variable that admits them, without a name
filter, so a renamed test cannot drop out silently:

```sh
EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1 cargo test --locked --release -p excalidraw-render --lib
```

Both tests are `#[ignore]`d unless `EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1`,
so a plain `cargo test` or `cargo nextest run` leaves them to the release-mode
run without naming them. The renderer's build script turns the variable into the
`release_qualification` cfg, so changing it rebuilds only the renderer; it is
not a Cargo feature, so it stays out of the published interface. Any value other
than unset, empty, `0` or `1` fails the build. The build script is not
packaged: the published crate has no build script, so dependents never run it.

## CI and evidence generation

The gates are recipes in the [justfile](../justfile), run in the devenv shell
(ADR-0024). The shell pins every tool they use through `devenv.lock`, so a
laptop and CI run the same commands with the same tools:

```sh
just              # list the recipes
just ci           # every per-change gate
just test         # one of them
just release      # the release checks, on the supported targets
```

The reference oracle and the viewer harnesses run digest-pinned Docker images,
from the host's Docker through `just pdf-harness`, `just viewer-evidence` and
`just candidate-successor`, or through Dagger (`dagger api call evidence svg`)
for hosts without a suitable Docker; the devenv shell carries the Dagger CLI.
GitHub workflows run the recipes in the devenv shell and handle scheduling and
artifact upload.

See [CI operations](ci.md) for the gate inventory, the release checks, the
nightly expansions, evidence reproduction and generation, and the two-target
aggregate.
