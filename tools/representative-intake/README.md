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
Only `reviewed-no-change` sanitization is implemented: a later case requiring
redaction needs an explicit reviewed transformation contract, not an implicit
rewrite. A second scene can use the same plan schema and command path.

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
collection and an intact native capture, derives font resources through the
existing Stage-4 asset adapter, executes two fresh processes for each SVG/PDF
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

The current authored adapter supports **unrotated source crop anchors** and the
existing Stage-4 **single covering embedded WOFF2 face** contract. Unsupported
inputs fail explicitly; it is not yet a general representative admission adapter.
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

Verification requires the complete producer inventory, validates both viewer
receipts against retained raw raster bytes, re-derives Rust origin clipping,
and matches each report file to its canonical recomputation. Use
`REPRESENTATIVE_EVIDENCE_TESTS=1` for command-boundary tampering checks.
`--repeat-unapproved` re-executes an existing unapproved collection, refuses
changed original files and requires every existing artifact/judgment byte to
remain identical before replacing process receipts. This is for producer
verification changes, never for changing a crop, budget or failed measurement.
