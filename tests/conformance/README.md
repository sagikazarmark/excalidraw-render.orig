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

Two registry-driven harnesses in `crates/excalidraw-render/tests` compare the
renderer against every registered scene case and variant, rendering each
variant once per format and judging that one render on dimensions that each
isolate one variable:

- `parity_registered_case_svg` compares SVG path data and the root extent as
  text, geometry with no rasterizer involved, and then rasterizes the
  renderer's SVG and the official SVG through the same in-process `resvg`,
  each with a font database built from its own embedded fonts, and compares
  under the pure-Rust budget: the SVG document with the rasterizer held
  constant. The raster dimension is document parity, not a canonical viewer
  gate; the pinned canonical SVG viewers are a separate piece of work and
  neither admits text-bearing SVG today.
- `parity_registered_case_png` compares the renderer's PNG against the official
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
`parity_registered_case_png` then holds an excepted variant to those ceilings
and fails if the variant starts passing the canonical budget, so a record cannot
outlive the difference it describes. Today only whole-artifact direct-PNG
exceptions with no viewer are admitted.

A compatibility exception, such as spec 8.3's replacement glyph for a scalar
no bundled face maps or its Noto Emoji substitution for the local-only emoji
face, is therefore excepted on the direct PNG and observed
just as well by the raster dimension of `parity_registered_case_svg`, whose scope the manifest
cannot except. That harness records a compatibility exception pin: the same
exact measurement and issue a known defect carries, plus the requirement that
the manifest record a whole direct-PNG tolerance exception for the same
variant, so the pin stands for an approved judgment rather than suppressing an
unrecorded difference. Admitting `format: svg` exceptions in the manifest
would move the scene case schema every scene manifest pins by digest, so that
is a batch re-approval round of its own. The official evidence for both cases
is what the pinned reference profile's container draws for a scalar the
official package has no glyph for — its fontconfig fallback — since the
profile's environment-dependent inputs are part of its identity; the case
`purpose` and exception `reason` name the face that supplied it.

Not every difference between the two official artifacts is one the renderer
preserves. The official 0.18.1 exporter sets frame clipping, and the
rounded-image corner clip, through `setAttributeNS` with the SVG namespace, so
its SVG carries `<g xmlns:nsN="http://www.w3.org/2000/svg"
nsN:clip-path="url(#id)">` rather than a plain `clip-path` attribute. Chromium
does not treat the namespaced attribute as the `clip-path` presentation
attribute, so the official SVG renders unclipped in a browser while the
official canvas PNG clips. `focused.frame-clip-mask-sharing.v1` shows it:
`evidence/official.svg` reads `ns1:clip-path="url(#clip-frame)"`, and the
approved `official-svg.rgba` (the Chromium raster) and `official-png.rgba` (the
canvas) differ by 6,491 pixels (1.74%) inside the overflowing rectangle. This
is a reference authority divergence (spec 8.2 and 8.4), not a known defect or
an exception: the renderer emits a real `clip-path` and follows the canvas in
every format, so its SVG agrees with the official PNG rather than with the
official SVG's browser appearance. The raster dimension of
`parity_registered_case_svg` cannot observe this, because usvg accepts an
attribute in the SVG namespace as the property it names
(`usvg-0.45.1/src/parser/svgtree/parse.rs:234-236`) and therefore clips the
official SVG as well; the pure-Rust canonical viewer parses with the same rule
and is blind the same way. Only a canonical Chromium raster of the official SVG
shows the difference, so a gate whose authority is that raster — the canonical
Chromium SVG viewer once it admits frame scenes, or a scene-export PDF gate
(see Canonical PDF harnesses) — sees the renderer clip what the official SVG
does not.

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

An approval subject pins the code that *produces* its evidence — the harness
scripts, fixture generators, materializers and comparator invocations that
wrote the bytes under approval, and the module that re-derives and records the
manifest — and never the code that only *consumes* it: not the tests that check
the evidence against the manifest, and not the CLI wiring that dispatches to a
verifier. A consumer can change without a byte of evidence changing, so pinning
it spends a human re-approval round on nothing, and it also keeps the pinned
file out of the ordinary formatting and lint gates (#141). Under this rule the
PDF viewer qualification pins `tools/pdf-viewer-qualification.sh`,
`tools/pdf-harness/generate-fixtures.py`, the comparator's `compare_pdf_rgba`
example and `xtask/src/pdf_viewers.rs`, and no longer `xtask/src/main.rs`,
`xtask/tests/pdf_viewer_verify.rs` or the comparator crate's
`pdf_viewer_qualification` test. A manifest whose `implementation` list still
names a consumer drops it in the next round that re-approves that manifest
anyway (the ride-along rule in `docs/cleanup-plan.md`), never in a round of its
own.

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
cargo test --locked -p excalidraw-render --test harness_svg_viewers \
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
  --test harness_pdf_viewers \
  rectangle_outline::approved_rectangle_passes_both_canonical_pdf_visual_gates -- \
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

Every `#[ignore]`d gate that needs this harness lives in the
`harness_pdf_viewers` test binary of `crates/excalidraw-render` (the backend
qualification's structural gates, the rectangle-outline, Excalifont text and
frame-export viewer gates, and the viewer adapter contract), and the `harness`
nextest profile selects that binary by name, so the gates run in CI on every
push to `main` and nightly through `.github/workflows/pdf-harness-gates.yml`
without being listed there, and a change that moves a structural digest or a
viewer raster fails visibly rather than leaving the expectation files stale
(#109). The adapter contract (`harness_pdf_viewers::adapters`) pins the viewer
adapters' own behavior: a page is admitted when each of its 96 DPI dimensions
is within one pixel of the supplied canvas and is then placed at exact scale
from the canvas origin, never stretched to fit (#119).

A PDF viewer gate's tolerance exceptions live beside the case, outside the case
approval subject: the consuming test pins the file's sha256 and the record
names the issue that carries the decision. Each record is one
`excalidraw-render/pdf-compatibility-exception/v1` judgment for one viewer
profile, with `max_changed_pixels` and `minimum_ssim_bits` set to that viewer's
exact observed values, so two viewers that differ get two records rather than a
shared floor. The rectangle, Excalifont, and isolated-fallback cases each carry
one record as `pdf-visual-exception.json`; the frame case records them per
frame target in `pdf-tolerance-exceptions.json`, keyed by `frame_ordinal`, the
frame analogue of a scene variant. The frame gate in `harness_pdf_viewers`
holds an excepted target
and viewer to those ceilings, fails when the target returns inside the
canonical budget so the record cannot outlive the difference, and rejects a
record naming a target or viewer the case does not export. The rotated frame's
records (#119) absorb the one partial row of a fractional 96 DPI page, which
Chromium's `<img>` raster of the official SVG rescales and no exact-scale
viewer render reproduces.

The PDF authority — official SVG rendered by canonical Chromium — carries the
frame-clipping caveat recorded under the case registry above. Chromium applies
no clip for the official SVG's namespaced `clip-path`, so for a scene with
frames the authority raster shows children overflowing their frame while the
renderer's PDF, which follows the canvas (spec 8.4 and 8.6), clips them. A
scene-export PDF gate judged against that raster would fail a correct PDF in
exactly the overflow, and would need the official PNG or a reviewed delta as
its authority there before it could be approved. The frame-export gate is
unaffected today: in every target it exports whose children are clipped, the
frame is unrotated and its clip coincides with the zero-padding page, so what
the clip removes lies off the page and both authorities agree on every pixel
the viewers render. A rotated target with overflowing children would not: its
page is the frame's AABB while its clip is the smaller rotated rectangle.
