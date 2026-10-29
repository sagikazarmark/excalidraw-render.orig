# Representative intake (#84)

Representative intake is available through `cargo run --locked -p xtask --
representative`. It prepares immutable review bundles and native observations.
The confirmed test seam is the tool's command interface.

Use the Rust commands below for preparation, capture, evidence and case
verification. The Python implementation and standalone comparator have been
retired. Representative cases are accepted through normal PR review; no separate
issue-comment approval or approval-recording command is required. Historical
non-null approvals remain validated.

## Velox plan

`velox-plan.json` proposes six whole-scene executions: light/dark × SVG/PNG/PDF,
default padding and scale, background included, and all optional metadata on.
The original 53-element board is retained byte for byte, with its MIT license and
pinned upstream provenance. Its 23 technical text labels, null links, empty asset
map and grid-only app state need no proposed redaction. Review sanitization, the
proposed whole-region class and source-element crop anchors with the case PR.
Pixel crop rectangles must be chosen against reference output; the tool does not
invent them from native output.

```sh
cargo run --locked -p xtask -- representative prepare \
  tests/public-scenes/velox-design tools/representative-intake/velox-plan.json \
  tests/conformance/intake/velox-design-v1
cargo run --locked -p xtask -- representative verify \
  tests/conformance/intake/velox-design-v1
```

Preparation refuses an existing destination. Verification recomputes the full
manifest from its bound inputs, checks fixed local filenames, upstream commit
and digest, complete format/variant inventory and valid source-element anchors.
Two sanitization operations are implemented. `reviewed-no-change` retains the
upstream bytes. `reviewed-index-repair` is for exports that predate fractional
indices, which the pinned reference oracle requires. It inserts
`"index":"<key>",` after each element's opening brace, in array order, with the
keys Excalidraw's restore assigns (`fractional-indexing` 3.2.0
`generateNKeysBetween(null, null, n)`; `a0`, `a1`, …), and refuses a scene in
which any element already has an index. Every other byte, number spelling
included, is the upstream file. Verification removes exactly those members and
requires the pinned upstream digest, so only the repaired scene is retained.
Redaction or any other rewrite needs its own explicit reviewed transformation
contract, never an implicit rewrite.

## Native observations

Build the renderer separately, then capture twice in fresh CLI processes, with
reversed variant/format order on the second pass:

```sh
cargo build --locked -p excalidraw-render-cli
cargo run --locked -p xtask -- representative capture \
  tests/conformance/intake/velox-design-v1 target/debug/excalidraw-render \
  /tmp/velox-native-observations
cargo run --locked -p xtask -- representative verify-capture \
  tests/conformance/intake/velox-design-v1 /tmp/velox-native-observations
```

Capture binds the exact executable, tool, intake manifest, complete returned
artifacts and JSON reports. It verifies the report's artifact digest/length,
schema/identity/format envelope and diagnostic field types/effect order, requires
success and exact repeat bytes, and publishes its completion marker
last. Rendering uses a private copy of the verified scene; capture rejects a
changed source bundle or executable. A render timeout/failure leaves no capture
directory. `--timeout` sets the per-process ceiling (default 180 seconds).
On Unix, renderer wrappers run in an isolated process group, which is terminated
on completion, failure or timeout before scratch cleanup. Windows uses best-effort
`taskkill /T`; descendant cleanup after a wrapper has already exited is not
guaranteed there. Use the renderer executable directly on Windows.
`verify-capture`
checks stored integrity; it does not re-execute the renderer or authenticate a
human approval. Diagnostic detail objects remain opaque observed data; the tool
does not duplicate the revision-specific diagnostic registry or claim complete
report conformance. Independent expected reports remain a promotion requirement.
The binary digest is an observation identity, not release-build
or source provenance. Local debug builds are useful here and cannot substitute
for the pinned release/native/WASM qualification required for promotion.

Rust preparation retains the v1 intake bundle format and verifies the existing
Python-authored bundles byte-for-byte. Rust capture writes
`excalidraw-render/representative-intake-capture/v2`: its `tool` records
`implementation: cargo-xtask-representative`, the package version, and an
executable descriptor. This is historical producer information, so rebuilding
xtask does not invalidate stored observations. During capture the renderer and
xtask binaries and the source bundle must remain unchanged.

Rust `verify-capture` accepts both v2 and legacy Python v1 captures. A v1 `tool`
descriptor is validated as historical producer identity, without reading the
deleted `intake.py` source. Actual retained files, reports, requests and intake
bindings are still checked. Verification checks integrity, not proof of execution
or authorship, and captures remain observations awaiting review.

Every result stays `pending-review` or `observed-unapproved`, has null approval,
and exposes no eligible conformance executions. Captures are observations,
**never generated expected reports or visual judgments**. The pending list in
the historical intake manifest records reference/viewer evidence, independent
semantics and notices, resource outcomes, canonical crops, approval and
integration. New cases complete acceptance through normal code review and
registry integration as described below.

## Verification

The seven `xtask/tests/representative_*` suites run under `just test`. The
real-renderer tests in `representative_intake` and `representative_case`,
which capture and verify all six Velox executions twice, need a freshly built
CLI and are `#[ignore]`d in the ordinary tiers; run them from the devenv shell:

```sh
cargo build --locked -p excalidraw-render-cli -p xtask
REPRESENTATIVE_RENDERER="$PWD/target/debug/excalidraw-render" \
  cargo test --locked -p xtask --test representative_intake --test representative_case -- --include-ignored
target/debug/xtask representative verify tests/conformance/intake/velox-design-v1
target/debug/xtask representative capture tests/conformance/intake/velox-design-v1 target/debug/excalidraw-render target/velox-intake-capture
target/debug/xtask representative verify-capture tests/conformance/intake/velox-design-v1 target/velox-intake-capture
target/debug/xtask representative case verify-registry --renderer target/debug/excalidraw-render
```

Numeric tests are active Rust command-boundary checks with literal expectations.
The process-boundary observer delegates to the real CLI, records invocation
order and injects post-render drift/failure/delay to verify refusal and cleanup.
No tests call private helpers or substitute fake renderer output. Optional
pinned-font and Docker viewer-harness tests stay ignored.

## Reference and viewer proposal

`reference.sh` runs the unchanged official export entry point in the profile-pinned
offline Playwright container, twice in fresh browsers and reversed variant order.
It first verifies the intake bundle through Cargo's `representative verify`, then
verifies the reference profile before collection. Rust `representative evidence`
consumes that collection and an intact native capture, derives font resources
through `representative viewer-assets` (below), executes two fresh processes for
each SVG/PDF viewer, and compares in-process with `excalidraw-render-compare`.
It supports canonical whole/crop budgets only.

```sh
bash tools/representative-intake/reference.sh \
  tests/conformance/intake/velox-design-v1 target/velox-reference
cargo build --locked --release -p excalidraw-render-cli
cargo run --locked -p xtask -- representative capture \
  tests/conformance/intake/velox-design-v1 target/release/excalidraw-render target/velox-native
cargo run --locked --release -p xtask -- representative evidence collect \
  tests/conformance/intake/velox-design-v1 target/velox-reference target/velox-native target/velox-viewers
# Stored file/source/input bindings and actual canonical judgments are rechecked:
cargo run --locked --release -p xtask -- representative evidence verify \
  tests/conformance/intake/velox-design-v1 target/velox-reference target/velox-native target/velox-viewers
```

Use Rust 1.97.1 and the Node/browser environment in the
[reference-oracle setup](../reference-oracle/README.md), and install the PDF
harness using `tools/pdf-harness/install.sh`. Collection writes new directories
only. Failed collections have no terminal `viewers.json`. Compressed RGBA files
retain exact canonical samples without resizing, alignment or color fitting.

The current authored adapter supports **unrotated source crop anchors**, embedded
WOFF2 faces from distinct font families, and observed substitutes for undeclared
official families (see below). Multiple faces of one family are rejected.
Unsupported inputs fail explicitly; it is not yet a
general representative admission adapter.
`regions.json` records source-bounds-plus-four-pixel-context crop proposals.

Velox has fractional SVG dimensions. `svg-viewer.mjs` is a proposed, source-bound
extension of the existing Chromium adapter: it truncates only the screenshot
extent at the origin, audits actual text nodes, accepts absent optional WOFF2
format hints, and admits only LF's `&#10;` numeric entity (decoded during policy
checks). Rust viewer output is clipped at the right/bottom edge onto the same
integer grid; its SVG geometry and transforms are unchanged. These adapter
choices require qualification/review before becoming canonical authority.

Current evidence tests are listed under [Rust viewer evidence](#rust-viewer-evidence).
Completed typography, paint-isolation and stroke investigations are historical
research. Their diagnostic drivers have been deleted; archived results and
research prose remain, and Git history has the drivers.

### Viewer font resources

`representative viewer-assets` derives each SVG viewer's inputs and asset
declarations. The retained Velox resources test reproduction of the Stage-4
adapter's single-face output (`stage4-viewer-assets.mjs` plus
`xtask conformance candidate-prepare-assets`), with a v2 plan encoding. It
additionally declares each supported embedded face, so native SVGs may use more
than one family.

The official exporter does not embed every family it names: frame names use
`Helvetica`. The canonical Chromium viewer rejects ambient platform fonts, so an
undeclared official family is admitted only through an observed substitute.
`font-resolution.mjs` loads the official SVG in the pinned reference browser and
records the platform font CDP reports for each text node, alongside
`fc-match`. Collection requires both to agree and the file to be a pinned
reference-profile system font, retains those exact bytes, wraps them as WOFF2
with the renderer's encoder settings (`viewer-fonts`, which pins its own
`wuff`, `rustybuzz` and `ttf2woff2` as bound evidence), and injects one `@font-face` for the
requested family into `official-substituted.svg`. Chromium renders that file;
the Rust viewer inputs derive from it. In the pinned container, `Helvetica`
resolves to Liberation Sans (`LiberationSans-Regular.ttf`). This substitution is
a proposed adapter choice that needs review before it becomes canonical.

```sh
cargo run --locked --release -p xtask -- representative viewer-assets verify <viewer-variant-directory>
```

Verification requires the complete producer inventory, validates both viewer
receipts against retained raw raster bytes, re-derives Rust origin clipping,
and matches each report file to its canonical recomputation. The opt-in Rust
`representative_evidence` tests exercise command-boundary tampering checks and
fresh collection with the pinned viewers.

## Compact retention

Large scenes produce tens of megabytes of rasters, fonts and derived SVGs per
collection. `representative compact retain` verifies a full Rust v2 collection, keeps only its JSON
records and writes `compact.json`: a digest inventory of every file in the
reference, native and viewer collections, plus receipt signatures with process
receipts and executable/tool identity removed. `verify` checks the retained
records offline. `check` requires a fresh full
collection at the recorded `target/` paths to reproduce every other digest.

```sh
cargo run --locked --release -p xtask -- representative compact retain <bundle> <reference> <native> <viewers> <new-proposal-dir>
cargo run --locked -p xtask -- representative compact verify <proposal-dir>
cargo run --locked --release -p xtask -- representative compact check <proposal-dir>
```

New compact records use v2. Offline `verify` also accepts the six retained v1
records, including historical prose alongside their retained directories. It
checks JSON integrity and receipt bindings without requiring discarded rasters.
`check` requires v2 and a full collection at the recorded roots; legacy v1 fresh
checks are retired. Fresh v2 checks allow only observer identities and the three
volatile browser process fields to differ. Pixel artifacts, reports, derived
resources, profiles and nonvolatile receipts must still match. The standalone
Python compact script and its tests have been removed.

## Registered cases

Create a new case for review from an authored spec:

```sh
cargo run --locked -p xtask -- representative case materialize <new-case-directory> \
  --intake <bundle> --reference <reference-directory> \
  --evidence-record <compact.json> --spec <spec.json>
cargo run --locked -p xtask -- representative case subject <case-directory>
```

The spec supplies `case_id`, `formats`, `semantic_expectations`,
`expected_reports`, `exceptions`, and optionally `resource_outcomes`.
Materialization validates expectations and inputs before publishing, retains the
reference bytes, refuses existing destinations, and writes `approval: null`.
It does not register the case. Review the authored expectations, reference and
viewer evidence, sanitization, resources and comparisons through a normal PR;
after review, add the case to
`tests/conformance/cases/representative/registry.json` through the PR. No separate
issue-comment approval or `record-approval` ceremony is needed.
`--evidence-record` is a provenance binding, not an assertion that the referenced
observations prove the expectations correct. `case subject` remains useful as a
diagnostic hash of case contents; it is not an acceptance gate.

Read-only case verification is available in Rust:

```sh
cargo run --locked -p xtask -- representative case verify \
  tests/conformance/cases/representative/excali-local-slides
cargo run --locked -p xtask -- representative case verify-registry
cargo run --locked -p xtask -- representative case verify-registry \
  --renderer target/debug/excalidraw-render
```

Offline verification checks retained file hashes, the intake bundle, variants,
formats and comparison/exception declarations. Both single-case and registry
verification accept `approval: null`; registry identities must match and be
unique. Coverage's independent-case validation also accepts null approval.
Historical approval fields are opaque metadata. All evidence families use
Git/PR review and reproducible checks rather than approval hashes (ADR-0019).
`--renderer` uses Rust v2 captures, checks repeated artifact/report bytes,
expected diagnostics, dimensions, selectable text, links, theme/embedding flags
and mandatory notices, and replays declared resource failures twice. Each failed
resource render must leave no artifact and match its expected report fields.

Offline and native-only verification do not claim visual comparison success.
Add `--viewers` alongside `--renderer` to collect fresh viewer evidence in Rust,
verify it and judge every registered comparison. Use a release-built xtask for
the raster comparisons:

```sh
cargo run --locked --release -p xtask -- representative case verify-registry \
  --renderer target/debug/excalidraw-render --viewers
```

The required pinned harnesses are installed with `tools/pdf-harness/install.sh`
and the reference-oracle setup documented above. Collection invokes the existing
JavaScript SVG/PDF.js adapters, Rust SVG viewer and Rust font codec helper.
PDFium uses the isolated safe-Rust worker. The current viewer workflow and its
Rust tests do not invoke Python.

Rust can judge an existing raster collection separately:

```sh
cargo run --locked -p xtask -- representative case judge <case-directory> <viewer-collection>
cargo run --locked -p xtask -- representative controls <viewer-collection/light> \
  --comparison pdf-pdfium --tiles 64 --output <new-report.json>
```

`case judge` validates the case, requires all registered variant/comparison
entries in `viewers.json`, and recomputes candidate and identical-input control
comparisons directly from each variant's retained rasters and `regions.json`.
Stored result flags and measurements are not trusted. Each region passes by its
canonical budget, then its format-specific control envelope, then a matching
exception ceiling. Unused exceptions, missing/duplicate comparisons, invalid
crops, malformed rasters, non-finite SSIM ceilings and exceeded limits fail.
This measures the supplied collection; it does not authenticate its producer,
re-derive crop anchors or rerun viewers. Use evidence verification or case
`--viewers` for those additional consistency checks.

`controls` emits JSON observations, including every control pair's changed-pixel
count and SSIM, the worst-case envelope, and `within`/`beyond` for failing regions.
Repeat `--comparison` to select several; the default is all five comparisons.
`--output` is create-only. Tiles must be at least 11 pixels per side; edge strips
smaller than 11 pixels merge into the preceding tile so no pixels are discarded.
A valid out-of-budget controls report exits successfully; `case judge` enforces
the case decision. Both commands use the existing Rust comparator in-process.

```sh
cargo test --locked -p xtask --test representative_judge
```

### Rust viewer evidence

```sh
cargo run --locked --release -p xtask -- representative evidence collect \
  <bundle> <reference> <native-capture> <new-output>
cargo run --locked --release -p xtask -- representative evidence verify \
  <bundle> <reference> <native-capture> <output>
cargo run --locked --release -p xtask -- representative viewer-assets collect <variant-directory>
cargo run --locked --release -p xtask -- representative viewer-assets verify <variant-directory>
```

Harness input/output paths must be inside the repository. Collection is
create-only, removes incomplete output on a returned failure, and writes a
`representative-intake-viewers/v2` record. It accepts both Python v1 and Rust v2
native captures. Rust evidence verification requires a v2 collection. Existing
retained evidence is not rewritten; offline compact verification continues to
read historical v1 records and their retained files as described above.

Verification re-derives assets and source-anchored crops, checks reference options
against the intake plan, checks every copied reference/native byte, both raw
viewer repeats, receipt identities, declared custom fonts, Rust origin clipping,
and freshly recomputed comparison results. It also checks the complete file,
viewer and judgment inventories. Producer sources are historical provenance;
reference profiles, viewer profiles and actual inputs remain live bindings.
Observations cannot grant approval or prove authorship.

Viewer preparation supports multiple distinct font families. Multiple faces of
the same family are explicitly rejected until the Rust viewer's runtime font
resolver can honor family and Unicode-range selection without fallback ambiguity.
Undeclared official families require a pinned-browser/fontconfig agreement and
the exact profile-pinned substitute file. Reference option checks and anchored
crop derivation reject malformed or rotated geometry rather than fitting crops
to candidate differences.

Internal command timeouts remove tracked Docker containers as well as host
process groups. External termination of xtask (for example SIGKILL) cannot run
cleanup; inspect leftover Docker containers and incomplete `target/` directories
after an interrupted harness run. Full registry collection is an opt-in check
(about 26 minutes on the development machine):

```sh
REPRESENTATIVE_RENDERER="$PWD/target/debug/excalidraw-render" \
  cargo test --locked --release -p xtask --test representative_evidence -- --include-ignored --test-threads=1
cargo test --locked -p xtask --test representative_viewer_assets
```

```sh
cargo test --locked -p xtask --test representative_case
REPRESENTATIVE_RENDERER="$PWD/target/debug/excalidraw-render" \
  cargo test --locked -p xtask --test representative_case -- --include-ignored
```

The ignored case tests exercise fresh native renders, semantics and resource
failures. There are no Python parity tests in the Rust representative suite.
