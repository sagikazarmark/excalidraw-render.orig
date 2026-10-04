# Development and evidence operations

Expectation updates use Git/PR review and reproducible tests (ADR-0019).
Approval-subject and approval-recording commands in older workflow examples are
retired. Tool/download checksums and actual input/output comparisons still apply.

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

Every dependency is pinned to an exact version (`=x.y.z`), because artifact
bytes are the product and any transitive bump can change them. For a project
that depends on the library this has consequences. Cargo resolves one version
per semver-compatible range, so within each such range the pins decide which
version of a shared crate (`serde`, `png`, `resvg`, ...) your whole build uses:
a requirement of yours in the same range that the pinned version cannot satisfy
fails to resolve, while one in a different range builds a second copy alongside
it. A patch release of a pinned crate only reaches you through an
`excalidraw-render` release that moves the pin and re-verifies the artifact
bytes. The workspace and its published crates also declare `rust-version`, the
toolchain the release gates build with; older compilers are not exercised.

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

`--list` is a dry run: it reports the ordinal, kind, name, and derived basename
of every frame the same selection would export, and writes nothing. An empty
selection succeeds, with a `note:` line in human mode. `--output` and `--format`
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
and pinned qpdf; Dagger installs these automatically. See
[`tools/reference-oracle/README.md`](../tools/reference-oracle/README.md) for the
opt-in profile verification workflow.

The PDF harness uses a separately installed native PDFium library with Rust
bindings. On Linux x86_64, run `devenv shell pdfium-install` (or
`bash tools/pdf-harness/install.sh --pdfium-only`) to install the checksum-pinned
upstream binary and build its Rust worker. This needs no Python or Docker.
The complete PDFium/PDF.js/qpdf setup is documented in
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

Maintainers can reproduce the checked font files from an explicit local
`@excalidraw/excalidraw` `dist/prod` directory. The importer verifies every
source byte against the approved reference asset manifest and never downloads
or generates production data during a build:

```sh
cargo run --locked -p xtask -- catalog import <local-dist-prod>
```

The catalog also records every official font chunk's reviewed decode, the sfnt
the pinned `wuff 0.2.8` decode of its exact bytes produces and the only bytes
PDF may embed for it (spec 8.6). `catalog verify` re-derives all 230 decodes
and compares the actual decoded bytes; `import` and `materialize` regenerate the
record. Review the generated diff and verify it:

```sh
cargo run --locked -p xtask -- catalog verify
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

`excalidraw-fonts::fonts` presents the bundled fonts as typed `FamilyId`s and
opaque `FaceId`s. `FamilyId::faces()` returns an immutable slice in upstream
declaration order, generated by `xtask catalog` from the existing asset and face
declarations. A face exposes declared Unicode ranges, source identity and sizes,
container format, and availability. Noto Emoji is a complete sfnt face; official
fonts are WOFF2 chunks. Metadata remains enumerable with CJK payloads disabled.

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
bytes with the checked native expectations.

```sh
cargo package --locked --workspace --exclude xtask --exclude excalidraw-render-compare --no-verify
bash tests/package-consumer/check-packaged-cli.sh x86_64-unknown-linux-gnu
(cd tests/package-consumer && cargo clean && cargo test --locked --release --test public_facade)
```

Run the consumer from its own directory so its `.cargo/config.toml` (target
directory, WASM test runner) applies, and clean it first: `cargo package`
stamps the unpacked sources with a fixed mtime, so a consumer build left over
from an earlier package never notices that the crate changed.

The target workflow executes this consumer on Linux and in the pinned
browser-hosted `wasm32-unknown-unknown` harness (ADR-0006).

## Tests

Start with [CONTRIBUTING.md](../CONTRIBUTING.md) for test commands, interpreting
failures, generating references and changing expectations. The details below
explain the tiers and release-only checks behind that workflow.

The suite is tiered, and `.config/nextest.toml` gives each tier a
[cargo-nextest](https://nexte.st) profile that selects it by binary name and
kind, so neither a workflow nor a developer names individual tests. The
renderer's integration binaries under `crates/excalidraw-render/tests` are named
by tier:

| binary | tier | profile |
|---|---|---|
| `fast` (one module per former binary) and every other crate's targets | every unit, component and full-pipeline test | `ci-fast` |
| `parity_registered_case_svg`, `parity_registered_case_png`, `parity_frame_export` | parity against official evidence: the registry-driven scene harnesses and the frame-export case | `ci-render` |
| `limits_scene_admission` | fixed parser ceilings that build 64 MiB and million-entry scenes | `ci-render` |
| `harness_pdf_viewers`, `harness_svg_viewers` | `#[ignore]`d gates that need the pinned PDFium, PDF.js, qpdf and Chromium harnesses | `harness` |

```sh
cargo nextest run --workspace                      # a developer's run: ci-fast and ci-render together
cargo nextest run --workspace --profile ci-fast    # the push gate's fast leg
cargo nextest run --workspace --profile ci-render  # the push gate's render leg
cargo nextest run -p excalidraw-render --profile harness --run-ignored only
cargo test --workspace --doc                       # nextest runs no doc tests
```

`cargo test --workspace` still runs every non-ignored test in one go, without
the tiers. The pinned nextest release is installed from
`tests/package-consumer/tool-manifest.json` with
`bash tests/package-consumer/install-cargo-nextest.sh <download-directory>`.

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
The `Packaged public facade` workflow's native leg and the
`packaging:qualification` Dagger check run the complete renderer unit target in
`--release` with the feature that lifts their `#[ignore]`, without a name
filter, so a renamed test cannot drop out silently:

```sh
cargo test --locked --release -p excalidraw-render --features release-qualification --lib
```

Both tests are `#[ignore]`d unless `release-qualification` is enabled, so a
plain `cargo test` or `cargo nextest run` leaves them to the release-mode run
without naming them.

## Canonical CI and evidence generation

Dagger owns CI for both supported targets: Linux x86-64 and browser WASM.
Run the push gates locally or on any Dagger runner:

```sh
dagger check
```

The devenv shell carries the pinned Dagger CLI (`dagger.enable` in
`devenv.nix`); the engine needs a working container runtime and nothing else.
`dagger check --list` names every check, and a selector runs a subset:

```sh
dagger check --list
dagger check 'lint:*'
dagger check test:fast
```

`lint` checks formatting and clippy; `test` runs the fast/render tiers and
doctests; `conformance` verifies catalog and approved evidence; `packaging`
executes the native qualification and packaged CLI/facade; `wasm` executes
the browser tests and packaged facade in Chromium **149.0.7827.55**.
Longer scheduled operations are explicit calls:

```sh
dagger api call nightly font-subset-fuzz
dagger api call nightly frame-batch-property --cases 64
dagger api call nightly scene-fuzz --cases 512
dagger api call evidence pdf
```

`.github/workflows/dagger.yaml` is generated from `dagger.toml`. The
`dagger-gha:generate` check fails when the committed file drifts from the
configuration; regenerate it with:

```sh
dagger generate dagger-gha:generate
```

`dagger.lock` pins module revisions and base image resolution. The package-consumer
tool manifest pins Rust, nextest, Chromium, ChromeDriver, and the WASM runner;
the reference/viewer harnesses retain their separately pinned images and tools.
GitHub workflows only invoke Dagger and handle scheduling/artifact upload.

See [the Dagger operation guide](ci.md) for evidence reproduction,
generation, the two-target aggregate, and the pending identity refresh.
