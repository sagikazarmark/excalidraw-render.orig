# Conformance corpus

This directory contains immutable, approved scene documents and reference
evidence used to judge format-specific parity. Ordinary verification is offline:

```sh
cargo run --locked -p xtask -- conformance verify focused.rectangle-outline.v1
```

Reference evidence is generated only in the pinned development reference
profile. Generation refuses to replace an approved case; reproduction writes no
corpus files:

```sh
cargo run --locked -p xtask -- conformance generate focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance reproduce focused.rectangle-outline.v1
```

## Case registry

`cases/registry.json` is the checked list of registered conformance cases. Each
entry carries the case identity, directory, kind (`scene`, `typography`, or
`frame`), approving issue, and runner script; `scene` entries additionally carry
the manifest inputs the reference-oracle generator needs — schema, layer,
coverage, feature tags, purpose, licence, sanitization, determinism, semantic
expectations, and any tolerance exceptions.

A scene case declares one or more export variants. The default variant is the
reference wrapper's default export; a further variant may vary dark mode and
selects exactly one sizing mode (`exportScale` or `maxWidthOrHeight`), and its
evidence nests under `evidence/<variant-id>/`. Both parity harnesses build the
renderer request from the variant's options and fail closed on any other
deviation, so a variant cannot be compared against evidence generated under
settings the harness did not reproduce. A scaled variant's official SVG
multiplies only the root `width` and `height` while the `viewBox` stays logical;
a maximum-dimension export leaves the SVG untouched and downscales only the PNG.
The verifier holds both rasters to those rules: the SVG rounds `viewBox` times
the scale, the PNG truncates it.

Three registry-driven harnesses compare the renderer against every registered
scene case and variant, each isolating one variable:

- `registered_case_svg_path_parity` compares SVG path data and the root extent
  as text: geometry with no rasterizer involved.
- `registered_case_svg_raster_parity` rasterizes the renderer's SVG and the
  official SVG through the same in-process `resvg`, each with a font database
  built from its own embedded fonts, and compares under the pure-Rust budget:
  the SVG document with the rasterizer held constant. It is document parity,
  not a canonical viewer gate; the pinned canonical SVG viewers are a separate
  piece of work and neither admits text-bearing SVG today.
- `registered_case_png_parity` compares the renderer's PNG against the official
  browser raster under the canonical budget: the whole pipeline, rasterizer
  included, which is where tolerance exceptions apply.

A variant whose divergence is a diagnosed defect awaiting a decision is not
excepted: it is listed in the harness that observes it as a known defect naming
its issue, with its measurement pinned exactly so the entry fails if the
divergence moves in either direction.

A tolerance exception records a maintainer's judgment that one variant's
observed difference from the official evidence has a non-defect cause. It names
the variant, format, viewer, and region it governs, sets its ceilings to the
exact observed changed-pixel count and binary64 SSIM score with no margin,
documents the cause, and links the issue that carries the decision. The
generator carries registered exceptions into the manifest, so the case approval
subject binds them and any change to a record invalidates that approval;
`registered_case_png_parity` then holds an excepted variant to those ceilings
and fails if the variant starts passing the canonical budget, so a record cannot
outlive the difference it describes. Today only whole-artifact direct-PNG
exceptions with no viewer are admitted.

`xtask` resolves every case through this file, so adding a scene case is a
registry entry plus a case directory rather than a new generator script or a new
constant. `conformance-case.mjs` reads the same file and refuses any entry whose
kind is not `scene`; the frame and typography cases keep their own standalone
generators, which carry their manifest inputs themselves. Neither imports
anything `conformance-case.mjs` owns: every scene provenance pins that
generator's bytes, so a change made for one of them must not invalidate the
scene approvals. The one module all three share, `canonical-json.mjs`, is bound
by the reference profile whose digest every provenance pins.

Verify every registered case at once with:

```sh
cargo run --locked -p xtask -- conformance verify-all
```

The first typography case uses a separate pinned runner and generator,
`typography-conformance-case.mjs`, so changes needed for font-bearing evidence
cannot invalidate the approved scene generator. The generator resolves the case
directory and runner through the registry, derives the bound `assets` record
from the official font the reference renderer fetched while producing the
evidence, and writes the single `expected-report.json` the typography schema
pins:

```sh
cargo run --locked -p xtask -- conformance generate focused.excalifont-text.v1
cargo run --locked -p xtask -- conformance reproduce focused.excalifont-text.v1
cargo run --locked -p xtask -- conformance approval-subject focused.excalifont-text.v1
```

`focused.excalifont-text.v1` binds the official basic-Latin Excalifont WOFF2
identity and is approved on issue #58 for its printed subject digest; any change
to its generator bytes moves the provenance and needs a fresh approval there.
Policy approval for deterministic predecoding does not constitute approval of
either the generated SFNT bytes or this evidence case.

The checked manifest is canonical JSON followed by one LF. Its adjacent digest
hashes the canonical bytes without that LF. The approval subject excludes only
the four-field approval object and binds the remaining manifest plus every
referenced scene, report, provenance, artifact, and raw-raster byte string.

## Visual comparator

`comparator-profiles/canonical-visual-v1.json` freezes the format-neutral
RGBA8 projection, Pixelmatch, SSIM, crop, and visual-budget protocol. Its
adjacent digest binds the profile; the profile in turn binds the unpublished
Rust implementation, the comparator's own dependency closure, adversarial
vectors, and imported Pixelmatch 7.2.0 fixtures. Verify all of them offline
with:

```sh
cargo test --locked -p excalidraw-render-compare
```

The closure, `comparator-profiles/canonical-visual-v1.closure.json`, is the
set of `Cargo.lock` packages reachable from `excalidraw-render-compare` — name,
exact version, source, and checksum — rather than the whole workspace lock, so
a renderer dependency change moves the profile only when it also moves a
package the comparator builds against (ADR-0005). The profile test recomputes
the closure from the live lock and names any package that differs. When a
comparator dependency legitimately moves, refresh the committed closure with:

```sh
cargo test --locked -p excalidraw-render-compare --test profile -- --ignored materialize
```

then repin `closure.sha256` in the profile, the profile's adjacent digest, and
the three artifacts that bind it (the SVG and PDF viewer qualifications and the
rectangle PDF renderer expectations), which start a fresh approval round.

## Canonical SVG viewers

The canonical Chromium and pure-Rust SVG viewer profiles are under
`viewer-profiles/`. Their first qualification uses the approved rectangle SVG
as both the reference input and a controlled candidate input, rendered in
independent fresh processes. This proves viewer determinism, isolation, raster
canonicalization, and comparator wiring before a renderer-owned candidate SVG
exists. Renderer conformance uses distinct candidate bytes.

Ordinary verification and comparison are offline:

```sh
cargo run --locked -p xtask -- conformance verify-svg-viewers
cargo test --locked -p excalidraw-render-compare --test svg_viewer_qualification
```

The development-only harness qualification and read-only reproduction commands
are:

```sh
bash tools/reference-oracle/run-svg-viewer.sh test
CARGO_TARGET_DIR=target/svg-viewer-rust cargo test --locked \
  --manifest-path tools/svg-viewer-rust/Cargo.toml --test viewer_cli
bash tools/svg-viewer-qualification.sh reproduce
```

Run the renderer-owned focused rectangle candidate through both canonical SVG
viewers with:

```sh
cargo test --locked -p excalidraw-render --test rectangle_svg_render \
  approved_rectangle_passes_both_canonical_svg_visual_gates -- \
  --ignored --exact --nocapture
```

The first case declares no text faces or image assets. Both viewers fail closed
on undeclared text and image payloads, local files, network resources, and
undeclared data URLs. Cases containing text or images supply a closed declaration
that binds embedded source identities to reviewed sfnt faces or canonical
in-memory image bytes; ambient fallback is never permitted.

## Canonical PDF harnesses

The PDF qualification pins PDFium, PDF.js, and qpdf independently. PDFium and
PDF.js render one page at exactly 96 DPI into canonical RGBA8, then use the
approved comparator against canonical Chromium rendering of official SVG.
qpdf and the owned inspector separately report and gate page boxes, ordered
vector and text operations, fonts and mappings, annotations, clips, masks,
transparency, image XObjects, metadata, attachments, output intent, ICC profile,
and parser warnings. Exact operation-sequence and structural-inventory digests
prevent extra or missing objects from passing count-only checks; Form XObjects
and soft-mask forms are traversed recursively.

Ordinary verification and comparison are offline:

```sh
cargo run --locked -p xtask -- conformance verify-pdf-viewers
cargo test --locked -p excalidraw-render-compare --test pdf_viewer_qualification
```

The first valid PDF is a project-authored calibration control, not renderer PDF
evidence. It uses PDF vectors only, retains the rough cubic path for structural
extraction, and uses pixel-grid rectangle fills to qualify both viewer adapters
against the approved raster without making PDFium antialiasing a hidden
tolerance exception. Separate controls prove visual failure with structural
success, visual success with an undeclared Image XObject, and hard failure on
qpdf parser-recovery warnings.

Additional hard-failure controls cover a catalog JavaScript `OpenAction` and an
unused image nested inside an unused Form. Image soft-mask bytes are separately
inventoried and hashed, and complete catalog/page/annotation action graphs are
classified so disabled viewer annotations cannot hide active content.

A separate structural-coverage control exercises recursive Form and soft-mask
content, embedded-font and complete ToUnicode digests, line-level ActualText,
URI annotations, clipping, transparency groups, a declared source image, XMP,
the embedded-scene carrier, and output intent without pretending to be official
PDF evidence.

Run the renderer-owned focused rectangle candidate through both PDF viewers and
the structural inspector with:

```sh
cargo test --locked --release -p excalidraw-render \
  --test rectangle_pdf_render \
  approved_rectangle_passes_both_canonical_pdf_visual_gates -- \
  --ignored --exact --nocapture
```

The candidate expectation manifest approval-binds the two viewer profiles, the
comparator and structure profiles, the direct 31-operation structure, and the
narrow PDFium antialiasing exception approved on issue #44. The exception keeps
the global comparator unchanged and requires zero Pixelmatch changed pixels;
PDF.js remains subject to the ordinary canonical visual gate.

Install the pinned development-only tools once, then reproduce without network:

```sh
bash tools/pdf-harness/install.sh
bash tools/pdf-viewer-qualification.sh reproduce
```

The `#[ignore]`d gates that need this harness (`pdf_backend_qualification`,
`rectangle_pdf_render`, `excalifont_text_render`, `pdf_viewer_adapters`) run in
CI on every push to `main` and nightly through
`.github/workflows/pdf-harness-gates.yml`, so a change that moves a structural
digest or a viewer raster fails visibly rather than leaving the expectation
files stale (#109). `pdf_viewer_adapters` pins the viewer adapters' own
contract: a page is admitted when each of its 96 DPI dimensions is within one
pixel of the supplied canvas and is then placed at exact scale from the canvas
origin, never stretched to fit (#119).
