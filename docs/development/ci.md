# CI and evidence operations

## The gates

Every gate is a recipe in the [justfile](../../justfile), run in the devenv shell
(ADR-0024). `devenv.lock` pins every tool the recipes use: the Rust toolchain,
nextest, qpdf 12.3.2 and PDFium build 7947. The base shell runs every
per-change gate and holds Rust, just, nextest, cargo-deny, qpdf and PDFium; the
`browser` profile (`devenv --profile browser shell`) adds the wasm-bindgen
runner and headless Firefox for the WASM recipes, the `release` profile extends
it with Node for the release checks (`just release`, whose candidate gate and
host-tool tests run Node on the host) and `just candidate-successor`. The
`maintainer` profile adds optional tools (cargo-audit, cargo-dist,
cargo-release, cargo-watch, wasm-pack) that no recipe or runbook step needs. A
laptop and CI run the same recipes with the same tools; GitHub Actions is a
caller, not the authority.

| Recipe | When | What it establishes |
| --- | --- | --- |
| `just ci` | Every change | Formatting, lints, the feature matrix and the dependency checks, the fast and render tiers (the golden bytes of every registered export target among them), the doc tests, the registered cases, the catalog and the viewer records, and the packaged consumers |
| `just msrv` | Every change | The published crates build on the workspace `rust-version`; run it as `devenv --option languages.rust.version:string 1.92.0 shell -- just msrv` |
| `just release` | Release | The release-tier verifiers and tests, native and browser-WASM qualification, the packaged consumers and the golden targets in WASM |
| `just nightly` | Scheduled | The bounded randomized expansions |
| `just pdf-harness`, `just svg-harness` | Scheduled, on demand | The PDF gates in PDFium, PDF.js and qpdf; the SVG gate in Chromium and the pure-Rust viewer |
| `just reference-profile verify`, `just reference-cases` | Scheduled weekly, on demand | The reference oracle still reproduces its profile and every registered case |

`just` lists every recipe. The per-change gates are `fmt`, `clippy`,
`features`, `deny` (advisories, licenses and sources of every dependency,
`deny.toml`), `msrv`, the `test`, `test-render`, `test-features` and
`test-doc` tiers, `conformance` and `packaging`. They run on any host the
devenv shell supports. CI (`.github/workflows/ci.yaml`) runs them as six
gates: `Lint` (fmt, deny, clippy), `Features` (the feature-matrix lints and
the reduced-feature builds), `Rust 1.92`, `Test` (one nextest archive, four
shards), `Conformance` (with the doc tests) and `Packaging`. Cargo's own
fingerprints decide what a recipe rebuilds; rust-cache keeps the dependencies
between runs, saved from `main`. Each job pulls the devenv shell from the
`rust-projects` Cachix cache and pushes the paths it builds.

### Test tiers

`.config/nextest.toml` gives each tier a profile that selects it by binary
name, so neither a workflow nor a developer names individual tests:

| binary | tier | what it judges |
|---|---|---|
| every binary not named below (`fast`, `asset_fuzz`, `render_fuzz`, `format_features`, and every other crate's targets) | `ci-fast` | every unit, component and full-pipeline test, and the fuzz smoke runs |
| `parity_registered_case_svg`, `parity_registered_case_png`, `parity_frame_export` | `ci-render` | parity against the official evidence of the registered cases |
| `golden_native` | `ci-render` | the golden bytes of every registered export target (below) |
| `limits_scene_admission`, `limits_scene_scaling` | `ci-render` | the fixed parser ceilings and the linear-time guards |
| `harness_pdf_viewers`, `harness_svg_viewers` | `harness` | `#[ignore]`d gates that need the pinned PDFium, PDF.js, qpdf and Chromium harnesses |

The xtask tests that build the qualified release CLI, and the ones that
judge the live coverage inventory and ownership ledger (which re-key on
every font-catalog or spec edit), are the release tier: `#[ignore]`d for the
fast tier and run by `just test-release`. So are the xtask tests that run host
tools (the `host-tools` nextest group): qpdf, bash and `realpath` for the PDF
inspector and harness wrappers, and Node for the WebP viewer boundary, the
candidate asset derivation and the ownership recorders. `just test-release`
runs from the `release` profile, which has Node; the base shell does not.

### The golden record

`tests/conformance/golden/native.json` holds the artifact and report bytes of
every registered case variant and frame target in each format, and of the
project-authored facade scenes in `golden/facade-scenes.json` under the default
request: 189 keys, with the bytes under `golden/blobs/`. A facade scene is a
project-authored scene with no official evidence; the fast-tier test that
exercises it reads the same file and asserts its behaviour, and the golden
record holds its bytes. The render-tier test `golden_native` derives the keys
from the registry and the facade list, renders each twice (forward and reverse,
a fresh renderer each time) and holds them to the record, so artifact byte
stability (ADR-0002) is a per-change gate. An intentional output change is
adopted with

```sh
just update-golden
```

on any host (ADR-0024: the bytes do not depend on the host; rendering the 168
registered keys on macOS arm64 reproduced every Linux-recorded blob) and
reviewed as the diff of the record and its blobs. Under wasm32 in Firefox (`just golden-wasm`)
the same test holds the browser build to the same record, with the four PNG
differences `golden/experimental-wasm.json` records for #178, which must all
appear exactly as recorded. The [release policy](../release-0.1.0.md) scopes them
as the experimental-WASM limitation; see
[the investigation record](../research/wasm-snapshot-mismatches.md).

### Conformance

`just conformance` verifies the 37 registered cases (manifest, registry,
evidence descriptors, schema), the sealed font catalog (including the 230
reviewed decodes) and the SVG and PDF viewer qualification records. The
release tier (`just conformance-release`) adds the representative candidate
replayed through a fresh release build, the coverage inventory and ownership
ledgers, and the font-subset viewer qualification; those bind evidence only
the release checks consume and re-key on every font-catalog change, so no
per-change gate carries them. `just conformance-release` runs on any host with
the pinned Rust; a change that moves the candidate's bytes needs a successor
(below).

## The release checks

```sh
devenv --profile release shell -- just release   # everything below
just conformance-release
just test-release
just qualification x86_64-unknown-linux-gnu    # the corpus and frame-batch tests, release mode
just qualification wasm32-unknown-unknown
just packaging                                  # the packaged crates and consumers
just wasm-packaging                             # the same consumers in browser WASM
just golden-wasm                                # the golden targets in browser WASM
```

The `Release checks` workflow (`release-checks.yaml`) runs `just release` on
demand from the commit to be released and uploads `target/release-evidence`;
its run URL goes in the release notes. Browser WASM runs in headless Firefox
through geckodriver on any host: the renderer makes no browser calls, and the
browser build cannot change artifact bytes (ADR-0024). `just qualification`
runs the renderer's unit target in `--release` with
`EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1`, which admits the font subset
qualification corpus (780 cells with their generated WOFF2 digests) and the
frame batch property; both targets assert the same committed pins, which is
the cross-target agreement.

## Release publication

The `Release` workflow (`release.yml`) is generated by
[dist](https://github.com/axodotdev/cargo-dist) from `dist-workspace.toml`;
regenerate it with `dist generate` rather than editing it, and Dependabot
leaves it alone for that reason. On a pull request it runs `dist plan`, which
fails when the committed workflow is stale. On a version tag it builds the
`excalidraw-render` binaries with the toolchain `rust-toolchain.toml` pins,
creates the GitHub release with the archives, checksums and shell installer,
and then calls `pages.yml`, which builds the side-by-side demo
([`tools/demo`](../../tools/demo/README.md)) from the tag and deploys it to
GitHub Pages. `pages.yml` also runs on demand to redeploy the demo.

## Scheduled runs

`nightly.yaml` runs `just nightly` (the five fuzz expansions in one job, with
`EXCALIDRAW_FUZZ_SEED` set to the run id so a night searches new cases and a
failure names the seed that replays it) and, in the same job,
`just nightly-render-fuzz-default-limits`, the browser-WASM checks, and the PDF
and SVG viewer harnesses on the runner's Docker. `oracle.yaml` runs weekly:
`just reference-profile verify` for the scene and frame profiles and
`just reference-cases reproduce`, which regenerates every registered case in
one container session and compares it with the committed evidence, so drift
in the oracle's closure is seen within days. A scheduled failure opens an
issue labelled `scheduled-failure`, or comments on the open one.

The expansions take a case count:

```sh
just nightly
just nightly-frame-batch-property 64
EXCALIDRAW_FUZZ_SEED=0x1234 just nightly-scene-fuzz 512
```

## What the evidence is

| Operation | What it establishes | Entry point |
| --- | --- | --- |
| Per-change gates | Formatting, lints, workspace tests, the golden bytes, the registered cases and packaged consumers | `just ci` |
| Release checks | The release-tier ledgers, native and WASM qualification, the golden targets in WASM | `just release` |
| PDF harness gates | PDFium/PDF.js rendering and qpdf structure tests | `just pdf-harness` |
| SVG harness gate | Chromium and pure-Rust rendering of the approved rectangle | `just svg-harness` |
| Reference profile | Pinned official Excalidraw wrapper, assets, environment and smoke test | `just reference-profile` (`update` as the first argument rewrites, `frame` as the second selects the frame profile) |
| Registered case | Expected reference SVG/PNG output for one scene, or for every registered case | `just reference-case focused.rectangle-outline.v1` (`generate` as the second argument generates); `just reference-cases` for all |
| Viewer qualification | Reproducibility and correctness of the tools used to judge rendered output | `just viewer-evidence svg` (also `pdf`, `font-subset`; `generate` as the second argument regenerates) |
| Font corpus | Per-cell generated WOFF2 byte pins and packaged corpus copy | `just font-corpus` |
| Golden record | The artifact and report bytes of the 189 export targets | `just update-golden` |
| Representative candidate | A numbered successor of the stage-4 representative case, collected from its frozen authoring | `just candidate-successor tests/conformance/candidates/<directory>` |
| Catalog / manifests | Derived catalog records and evidence bindings | `cargo run --locked -p xtask -- catalog materialize` |

The viewer scripts (`tools/svg-font-subset-qualification.sh`,
`tools/svg-viewer-qualification.sh`, `tools/pdf-viewer-qualification.sh`), the
reference oracle runners (`tools/reference-oracle/run-*.sh`) and the candidate
viewers run approval-pinned Docker images by digest, bind-mount the checkout
and render with `--network none`. The recipes acquire what the scripts need
first (the oracle's `npm ci`, the PDF harness, the pure-Rust viewer's crates)
and then run them verbatim against the host's Docker, so these recipes need
Docker. Acquisition can use the network. The images are `linux/amd64`; on an
ARM host Docker emulates them, which makes the browser runs slow. Under
emulation (Rosetta on Apple silicon) both reference profiles verify and the 25
cases without text reproduce, but Excalidraw's font-subsetting worker misses
its 1,000 ms deadline and logs a console error, which the oracle treats as a
failure, so the reference profile's smoke render and the 12 text-bearing
registered cases fail there: run `just reference-profile` and
`just reference-cases` on Linux x86-64, which is what the weekly workflow does
(measured 2026-11-02). The PDF viewer qualification reproduces under emulation
byte for byte. The `linux/arm64` build of
the same image is a different Chromium (Playwright's own 149.0.7827.0, not
Chrome for Testing 149.0.7827.55) and differs in a few blended and resampled
pixels, so it cannot stand in.

The reference profile binds the oracle's npm lock and the bundle built from it
(renderer spec 3.2), so Dependabot does not update `tools/reference-oracle`. A
dependency change there is a reviewed profile change: regenerate the profile
with `just reference-profile update` and carry the new digest through the
revision-1 records that bind it. No verifier, generator or test repeats the
digest of the scene profile, the frame profile, the asset manifest or the scene
surface: xtask (`revision::Reference`) and the oracle's scripts
(`scripts/revision.mjs`) read each from its `.sha256` sidecar after checking
the file still has it, the renderer's tests read the sidecar, and the renderer
reads the generated catalog. The scene-surface schema, which embeds the profile
digest and has no sidecar, is the one digest still pinned in xtask
(`SCENE_SURFACE_SCHEMA_DIGEST`). Besides that digest, `update` writing the
sidecar is the only hand edit; the rest is regenerated evidence
(`cargo run -p xtask -- catalog materialize`,
`just reference-cases generate --supersede`, `just update-golden`, and the
records that bind those). Reproduction compares the oracle's artifacts
and its run provenance; the generator and runner descriptors in that
provenance are historical, compared by path only.

Reproduction writes under `target/`; generation and updates write to the
checkout. The case generators refuse to overwrite a manifested case unless
given `--supersede`, which replaces it and keeps its approval block (ADR-0019
history), so `just reference-cases generate --supersede` regenerates the whole
registry in one container session for review in Git;
`just reference-case <id> generate --supersede` does one case. Against an
unmoved reference only each case's provenance moves: the fresh browser process
IDs and the producer digests (measured on `focused.rectangle-outline.v1`).
`just reference-case` reproduces into `target/reference-evidence`.
`just font-corpus` stages the corpus JSON and its digest sidecar in
`target/font-corpus`; they belong in `tests/oracle/compatibility-artifacts/`,
and the corpus JSON also at
`crates/excalidraw-render/tests/qualification/font-subset-corpus.json`.

## Coverage ledgers

The shared #85/#86 inventory is checked by `just conformance-release`
(`conformance verify-inventory`). See
[the inventory contract](../../tests/conformance/coverage/README.md).
Its consistency check does not grant release closure: `conformance verify-closure`
rejects the inventory until ownership and the golden record are complete.
The adjacent global ledger is checked by `conformance verify-ownership`; see
[ownership semantics](../../tests/conformance/coverage/OWNERSHIP.md). Its counts
describe assignments, not an approved coverage percentage. When a manifest,
the spec, the policy or a record moves, `conformance rebind-ownership-catalog`
rewrites the ledger's catalog digest and descriptors without changing an
assignment; `conformance inventory > tests/conformance/coverage/inventory.json`
and `conformance pdf-judgments-generate` regenerate the two derived records.

## Restaging

A change that moves approved evidence (a dependency that changes output bytes,
a subset profile token, a catalog descriptor) stales several records at once.
The golden record needs no workflow: `just update-golden` rewrites it on any
host. The Docker-bound records are regenerated by `restage.yaml` on demand,
which runs with `contents: read` and exports the results as artifacts; none of
its recipes edits a manifest or the registry. Adopt each in a commit, then run
the verifiers:

| Recipe | Writes | Adopt by |
| --- | --- | --- |
| `just update-golden` | `tests/conformance/golden` in place | reviewing the diff; `verify-pdf-judgments` |
| `just viewer-evidence font-subset generate` | the four files under `tests/conformance/svg-font-subset-viewer-qualification` (delete `revision-1.json` first; generation refuses to replace it) | `verify-svg-font-subsets` |
| `just candidate-successor tests/conformance/candidates/<dir>` | the successor directory through `candidate-materialize` | pointing `registry.json` at it, `record-representative-owner`, `verify-ownership`, `candidate-executions` |

The two restage recipes write into the checkout, so each runs in its own job.
The candidate recipe pre-builds the pure-Rust viewer and the PDFium worker
because the views script gives each viewer run 180 seconds. The workflow's
`viewer-checks` input instead reproduces the SVG, PDF and font-subset viewer
qualifications (`just viewer-evidence <kind> reproduce`) without writing
anything. After a change that moves only the subset profile, `just
font-subset-rebind` rebinds the font-subset qualification's subset fields to
fresh viewer inputs on any host, without a Docker regeneration. The [runbook](releasing.md)
merges byte-moving dependency bumps only between minor releases, so a release
restages once.

Codec and nested-SVG mutation fuzzing runs through `just nightly-asset-fuzz`,
and complete-render fuzzing through `just nightly-render-fuzz`; their PR smoke,
seed matrix, resource envelope and failure-replay procedure are documented in
[asset fuzzing](asset-fuzzing.md) and [render fuzzing](render-fuzzing.md).
