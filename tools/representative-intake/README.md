# Representative intake (#84)

This standalone development tool prepares an immutable review bundle without
changing the existing approval-bound Stage-4 adapter. Python 3 and its standard
library are sufficient. The confirmed test seam is the tool's command interface.

## Velox plan

`velox-plan.json` proposes six whole-scene executions: light/dark × SVG/PNG/PDF,
default padding and scale, background included, and all optional metadata on.
The original 53-element board is retained byte for byte, with its MIT license and
pinned upstream provenance. Its 23 technical text labels, null links, empty asset
map and grid-only app state need no proposed redaction. This is an agent review
record, not a maintainer sanitization sign-off. The proposed whole-region class
and source-element crop anchors also await review. Pixel crop rectangles must be
chosen against reference output; the tool does not invent them from native output.

```sh
python3 -B tools/representative-intake/intake.py prepare \
  tests/public-scenes/velox-design tools/representative-intake/velox-plan.json \
  tests/conformance/intake/velox-design-v1
python3 -B tools/representative-intake/intake.py verify \
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
python3 -B tools/representative-intake/intake.py capture \
  tests/conformance/intake/velox-design-v1 target/debug/excalidraw-render \
  /tmp/velox-native-observations
python3 -B tools/representative-intake/intake.py verify-capture \
  tests/conformance/intake/velox-design-v1 /tmp/velox-native-observations
```

Capture binds the exact executable, tool, intake manifest, complete returned
artifacts and JSON reports. It verifies the report's artifact digest/length,
schema/identity/format envelope and diagnostic field types/effect order, requires
success and exact repeat bytes, and publishes its completion marker
last. Rendering uses a private copy of the verified scene; capture rejects a
changed source bundle or executable. A render timeout/failure leaves no capture
directory. `--timeout` sets the per-process ceiling (default 180 seconds).
`verify-capture`
checks stored integrity; it does not re-execute the renderer or authenticate a
human approval. Diagnostic detail objects remain opaque observed data; the tool
does not duplicate the revision-specific diagnostic registry or claim complete
report conformance. Independent expected reports remain a promotion requirement.
The binary digest is an observation identity, not release-build
or source provenance. Local debug builds are useful here and cannot substitute
for the pinned release/native/WASM qualification required for promotion.

Every result stays `pending-review` or `observed-unapproved`, has null approval,
and exposes no eligible conformance executions. Captures are observations,
**never generated expected reports or visual judgments**. The pending list in
the intake manifest records reference/viewer evidence, independent semantics and
notices, resource outcomes, canonical crops, exact approval and integration.

## Verification

```sh
REPRESENTATIVE_RENDERER="$PWD/target/debug/excalidraw-render" \
  python3 -B -m unittest discover -s tools/representative-intake -p test_intake.py
```

The real-renderer capture tests are skipped without `REPRESENTATIVE_RENDERER`;
the intake integrity tests still run. CI supplies a freshly built renderer.
The process-boundary observer delegates to the real CLI, records invocation
order and injects post-render drift/failure/delay to verify refusal and cleanup.
No tests call private helpers or substitute fake renderer output. CI also
captures and verifies all six Velox executions, each twice.

## Reference and viewer proposal

`reference.sh` runs the unchanged official export entry point in the profile-pinned
offline Playwright container, twice in fresh browsers and reversed variant order.
It verifies the reference profile before collection. `evidence.py` consumes that
collection and an intact native capture, derives font resources through
`viewer_assets.py` (below), executes two fresh processes for each SVG/PDF
viewer, and compares with the actual `excalidraw-render-compare` library. The
standalone comparator has its own exact lockfile; no approved comparator source
or workspace manifest changes. It supports canonical whole/crop budgets only.

```sh
bash tools/representative-intake/reference.sh \
  tests/conformance/intake/velox-design-v1 target/velox-reference
python3 -B tools/representative-intake/intake.py capture \
  tests/conformance/intake/velox-design-v1 target/release/excalidraw-render target/velox-native
python3 -B tools/representative-intake/evidence.py \
  tests/conformance/intake/velox-design-v1 target/velox-reference target/velox-native target/velox-viewers
# Stored file/source/input bindings and actual canonical judgments are rechecked:
python3 -B tools/representative-intake/evidence.py \
  tests/conformance/intake/velox-design-v1 target/velox-reference target/velox-native target/velox-viewers --verify
```

Use Rust 1.97.1, Node and browser versions pinned by the oracle, and install the
PDF harness using `tools/pdf-harness/install.sh`. Collection writes new directories
only. Failed collections have no terminal `viewers.json`. Compressed RGBA files
retain exact canonical samples without resizing, alignment or color fitting.

The current authored adapter supports **unrotated source crop anchors**, any
number of embedded WOFF2 faces, and observed substitutes for undeclared official
families (see below). Unsupported inputs fail explicitly; it is not yet a
general representative admission adapter.
`regions.json` records source-bounds-plus-four-pixel-context crop proposals.

Velox has fractional SVG dimensions. `svg-viewer.mjs` is a proposed, source-bound
extension of the existing Chromium adapter: it truncates only the screenshot
extent at the origin, audits actual text nodes, accepts absent optional WOFF2
format hints, and admits only LF's `&#10;` numeric entity (decoded during policy
checks). Rust viewer output is clipped at the right/bottom edge onto the same
integer grid; its SVG geometry and transforms are unchanged. These adapter
choices require qualification/review before becoming canonical authority.

```sh
REPRESENTATIVE_BROWSER_TESTS=1 \
REPRESENTATIVE_COMPARE="$PWD/target/intake-compare/debug/representative-intake-compare" \
python3 -B -m unittest discover -s tools/representative-intake -p test_evidence.py
```

The browser controls exercise fractional dimensions, fixed-position paint,
LF admission and rejection of other numeric entities/external resources.
`probe.py` localizes raw PNG channel differences. `diagnose.py` compares an
expanded typography crop as a diagnostic only; it never rewrites original
judgments, budgets or approvals.

### Viewer font resources

`viewer_assets.py` derives each SVG viewer's inputs and asset declarations. For
single-face SVGs it reproduces the Stage-4 adapter (`stage4-viewer-assets.mjs`
plus `xtask conformance candidate-prepare-assets`) byte for byte; the retained
Velox resources are the regression test. It additionally declares every
embedded face, so native SVGs may use more than one family.

The official exporter does not embed every family it names: frame names use
`Helvetica`. The canonical Chromium viewer rejects ambient platform fonts, so an
undeclared official family is admitted only through an observed substitute.
`font-resolution.mjs` loads the official SVG in the pinned reference browser and
records the platform font CDP reports for each text node, alongside
`fc-match`. Collection requires both to agree and the file to be a pinned
reference-profile system font, retains those exact bytes, wraps them as WOFF2
with the renderer's encoder settings (`viewer-fonts`, which uses the renderer's
pinned `wuff`, `rustybuzz` and `ttf2woff2`), and injects one `@font-face` for the
requested family into `official-substituted.svg`. Chromium renders that file;
the Rust viewer inputs derive from it. In the pinned container, `Helvetica`
resolves to Liberation Sans (`LiberationSans-Regular.ttf`). This substitution is
a proposed adapter choice that needs review before it becomes canonical.

```sh
python3 -B tools/representative-intake/viewer_assets.py verify <viewer-variant-directory>
```

Verification requires the complete producer inventory, validates both viewer
receipts against retained raw raster bytes, re-derives Rust origin clipping,
and matches each report file to its canonical recomputation. Use
`REPRESENTATIVE_EVIDENCE_TESTS=1` for command-boundary tampering checks.
`--repeat-unapproved` re-executes an existing unapproved collection, refuses
changed original files and requires every existing artifact/judgment byte to
remain identical before replacing process receipts. This is for producer
verification changes, never for changing a crop, budget or failed measurement.

## Compact retention

Large scenes produce tens of megabytes of rasters, fonts and derived SVGs per
collection. `compact.py retain` verifies a full collection, keeps only its JSON
records and writes `compact.json`: a digest inventory of every file in the
reference, native and viewer collections, plus receipt signatures with process
receipts and executable/tool identity removed. `verify` checks the retained
records offline and reports stale producers. `check` requires a fresh full
collection at the recorded `target/` paths to reproduce every other digest.

```sh
python3 -B tools/representative-intake/compact.py retain <bundle> <reference> <native> <viewers> <new-proposal-dir>
python3 -B tools/representative-intake/compact.py verify <proposal-dir>
python3 -B tools/representative-intake/compact.py check <proposal-dir>
```

`pdf_thin_lines.py` renders hand-authored single-stroke PDFs in both pinned PDF
viewers and records the coverage each paints; it is an independent control for
sub-pixel stroke comparisons, not a judgment.

## Registered cases

`case.py` promotes an intake bundle and its collection into a registered case
under `tests/conformance/cases/representative/` (see that directory's README).
`materialize` retains the pinned reference collection and writes the case
record from a spec (formats, independently authored semantic expectations and
expected reports, exact-ceiling exceptions). `verify` checks retained bytes and
the approval subject; `--renderer` adds fresh-process native renders, reports,
dimensions and semantics; `--viewers` adds every pinned viewer and requires
each registered comparison to pass its budget or a matching exception, with
unused exceptions rejected. `judge` applies a case's budgets and exceptions to
an existing collection. CI runs `verify-registry --renderer` on every change.
