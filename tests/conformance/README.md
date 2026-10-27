# Conformance corpus

**Current acceptance policy:** [ADR-0019](../../docs/adr/0019-review-expectations-in-git-without-approval-hashes.md)
uses Git/PR review and reproducible tests for every evidence family. Approval
hashes, reviewer metadata and issue-comment recording commands described in older
sections below are historical; approval commands now report their retirement.
Input/output and tool-integrity digests, tolerance scopes and completeness checks
remain enforced. Producer-source identities are historical observations.

This directory contains versioned scene documents and reference
evidence used to judge format-specific parity. Ordinary verification is offline:

```sh
cargo run --locked -p xtask -- conformance verify focused.rectangle-outline.v1
```

Reference evidence is generated only in the pinned development reference
profile. Explicit generation produces a diff for review; reproduction writes no
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
reference wrapper's default export; a further variant may vary dark mode,
selects exactly one sizing mode (`exportScale` or `maxWidthOrHeight`), and may
drop the scene background (`exportBackground: false`), change the export
padding (`exportPadding`) or embed the scene (`exportEmbedScene: true`); its
evidence nests under `evidence/<variant-id>/`. Both parity harnesses build the
renderer request from the variant's options — dark mode, the sizing mode, the
scene background, the padding and the embedded canonicalized original scene
document, each the request control the reference option stands for — and fail
closed on any other deviation, so a variant cannot be compared against
evidence generated under settings the harness did not reproduce. A scaled
variant's official SVG multiplies only the root `width` and `height` while the
`viewBox` stays logical; a maximum-dimension export leaves the SVG untouched
and downscales only the PNG. The verifier holds both rasters to those rules:
the SVG rounds `viewBox` times the scale, the PNG truncates it. A variant
without the scene background has no opaque surface to hold to and must show a
transparent pixel instead, and its official SVG omits the one background
`<rect>` the registered `svg_element_counts` include for the default export.
A variant that embeds the scene must carry the reference's payload and nothing
else may: the `tEXt` chunk keyed `application/vnd.excalidraw+json` before the
PNG's `IEND`, and inside the SVG's `<metadata>` the `payload-type`,
`payload-version:2`, `payload-start` and `payload-end` comments around the
encoded scene text.

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
divergence moves in either direction. A defect the SVG harness observes on its
geometry dimensions — the root extent or the path data, which carry no
changed-pixel or SSIM measurement — is held in that harness's
`KNOWN_GEOMETRY_DEFECTS` at the exact SHA-256 of the renderer's own geometry
(the root `viewBox`, `width` and `height`, then every path's data in document
order), and the divergence from the official export must persist, so the entry
moves with a fix or a regression and cannot outlive the defect; a scene-extent
defect the direct-PNG harness observes as a raster of another size than the
evidence is held likewise in `KNOWN_EXTENT_DEFECTS` at the rendered
dimensions (#170, #171, #172).

A tolerance exception records a maintainer's judgment that one variant's
observed difference from the official evidence has a non-defect cause. It names
the variant, format, viewer, and region it governs, sets its ceilings to the
exact observed changed-pixel count and binary64 SSIM score with no margin,
documents the cause, and links the issue that carries the decision. The
generator carries registered exceptions into the manifest for Git review;
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
requires a coordinated schema and evidence update. The official evidence for both cases
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

That blindness has a limit. For an image with both `roundness` and `crop`, the
official exporter declares the corner `<clipPath>` at the crop-shifted group
origin instead of the element's rectangle, so the clip usvg honours is itself
misplaced: under one `resvg` the official document shows only the image's
top-left quadrant with one rounded corner, while the renderer clips at the
element rectangle as the official canvas does (`focused.rounded-image-crop.v1`,
#163). The raster dimension of `parity_registered_case_svg` sees that, and
neither of its other two lists fits it: it is not a renderer defect, and the
asset-bearing scene schema admits no tolerance exception for a compatibility
exception pin to stand for. It is held in that harness's third list,
`REFERENCE_AUTHORITY_DIVERGENCES`, as a reference authority divergence pin:
the exact `changed_pixels` and SSIM bits, the spec section that records the
divergence (8.2) and the issue that measured it, failing if the measurement
moves in either direction, plus the requirement that the same variant's direct
PNG passes the canonical budget against the official canvas raster — the proof
that the renderer, not the reference SVG, is right. A variant whose direct PNG
fails that comparison is not a reference authority divergence and the harness
refuses the pin. `KNOWN_DEFECTS` stays for renderer defects only.

The raster dimension has one more blind spot, and it is the harness's own
rasterizer's rather than the reference's (#168). `usvg 0.45.1` parses the
`direction` attribute but never reads it: it resolves `text-anchor` as if every
text were left-to-right (`end` puts the text's right edge at `x`), runs UAX #9
at a left-to-right base level, and lays out only the first UAX #9 paragraph of
a `<text>`; `usvg 0.47` and `0.48.1` do the same. The official exporter writes every
right-to-left element as `<text direction="rtl" text-anchor="end">` with `x` at
0, width/2 or width for `textAlign` left, center or right, which browsers place
at the element's left edge, centre or, overflowing, right edge (the last a
reference authority divergence spec 8.3 records; the renderer follows the
canvas). Under `usvg` a left-aligned right-to-left element of the official
document lands off the canvas and a right-to-left line's digits and neutrals
take left-to-right positions, so the raster dimension cannot judge right-to-left
alignment; the SVG document parity of the extent and path dimensions and the
direct PNG against the official canvas raster are the gates for it. A variant
whose official document this rasterizer cannot lay out as browsers do is held
in the SVG harness's fourth list, `RASTERIZER_LIMITATIONS`, as a rasterizer
limitation pin: the exact `changed_pixels` and SSIM bits, the issue that
recorded the limitation and what it is, failing if the measurement moves in
either direction, plus the requirement that the same variant's direct PNG is
where its record says it is against the official canvas raster (inside the
canonical budget or its approved tolerance exception) — the proof that the
renderer, judged where it can be judged, agrees with the reference
(`focused.bidi-alignment.v1`). Nothing in that list awaits a renderer fix and
nothing in it excepts a difference; a rasterizer bump that lifts the limitation
moves the entry, which then goes. Neither of the other three lists fits it: the
official SVG is right and only this rasterizer misreads it, so it is no
reference authority divergence, and the measurement is not the one the
manifest's compatibility exception describes.

One filler has no case here. The reference's `DotFiller` draws each dot's
centre from the unseeded `Math.random()` (spec 8.1), so two fresh browser
processes never export the same bytes and the generator's own repeatability
gate (`assertRepeatable`) rejects a dotted scene; evidence round A's
`focused.rough-fillers-dots.v1` could not be generated and was withdrawn. The
dots fill's evidence is the JS-derived unit vectors in
`crates/excalidraw-render/src/render/tests.rs` and the fast test over the
withdrawn scene, `crates/excalidraw-render/tests/fixtures/rough-fillers-dots.excalidraw`,
both pinned against the npm Rough.js 4.6.4 through
`tools/rough-filler-vectors/derive.mjs` (#137).

`xtask` resolves every case through this file, so adding a scene case is a
registry entry plus a case directory rather than a new generator script or a new
constant. `conformance-case.mjs` reads the same file and refuses any entry whose
kind is not `scene`; the frame and typography cases keep their own standalone
generators, which carry their manifest inputs themselves. Neither imports
anything `conformance-case.mjs` owns. Generator hashes describe the capture run
and do not gate source-only maintenance. The one module all three share,
`canonical-json.mjs`, is bound
by the reference profile whose digest every provenance pins.

Verify every registered case at once with:

```sh
cargo run --locked -p xtask -- conformance verify-all
```

The first typography case uses a separate pinned runner and generator,
`typography-conformance-case.mjs`, so changes needed for font-bearing evidence
are isolated from scene generation. The generator resolves the case
directory and runner through the registry, derives the bound `assets` record
from the official font the reference renderer fetched while producing the
evidence, and writes the single `expected-report.json` the typography schema
pins:

```sh
cargo run --locked -p xtask -- conformance generate focused.excalifont-text.v1
cargo run --locked -p xtask -- conformance reproduce focused.excalifont-text.v1
```

`focused.excalifont-text.v1` binds the official basic-Latin Excalifont WOFF2
identity. Changes to the generated SFNT bytes or evidence need review and fresh
qualification; source-only generator edits do not require a metadata refresh.

The checked manifest is canonical JSON followed by one LF. Its adjacent digest
hashes the canonical bytes without that LF. File descriptors bind referenced
inputs, reports, artifacts and raw rasters. Historical approval objects are read
as metadata rather than revalidated as acceptance decisions.

Producer descriptors record which harnesses and generators produced a capture.
Their shapes and required membership are checked where applicable, while source
edits are qualified by fresh reproduction rather than hash-refresh ceremonies.
Parser locks, schemas, comparator and expected evidence retain live byte checks.
Current structural and fixture callers use `xtask pdf-harness`; the PDFium worker
uses Rust and a separately pinned native engine. See
[`tools/pdf-harness/README.md`](../../tools/pdf-harness/README.md) for installation
and reproduction.

## Visual comparator

`comparator-profiles/canonical-visual-v1.json` freezes the format-neutral
RGBA8 projection, Pixelmatch, SSIM, crop, and visual-budget protocol. Its
adjacent digest binds the profile. The profile in turn binds what can change a
measurement: the unpublished Rust implementation, adversarial vectors, and
imported Pixelmatch 7.2.0 fixtures. Verify all of them offline with:

```sh
cargo test --locked -p excalidraw-render-compare
```

The comparator's dependency closure,
`comparator-profiles/canonical-visual-v1.closure.json`, lists the `Cargo.lock`
packages reachable from `excalidraw-render-compare`: name, exact version,
source, and checksum. It is committed and reviewed in Git, and the profile
test recomputes it from the live lock and names any package that differs. No
digest binds it (ADR-0005, amended for #212), so when a comparator dependency
moves, the closure file is the only thing that changes. Refresh it with:

```sh
EXCALIDRAW_REFRESH_COMPARATOR_CLOSURE=1 cargo test --locked -p excalidraw-render-compare --test profile -- --ignored materialize
```

Without the variable, `materialize` only stages the closure under Cargo's
scratch directory.

The profile itself moves only when the comparator's sources, protocol, limits,
vectors or fixtures change, and that change gets its own review. Its digest is
bound by live evidence, and each rebinding moves that evidence's own digest in
turn:
- the SVG and PDF viewer qualifications, and their sidecars (digests of the
  manifest without its trailing LF);
- the rectangle PDF renderer expectations;
- the oracle references under `tests/oracle/` and the test sources that pin
  them;
- `coverage/pdf-judgments.json`, and the `pdf-judgments` record digest in
  `coverage/ownership.json`.

Follow each moved digest until nothing live changes. Historical records stay
as they were: `proposals/`, `candidates/`, `coverage/*-application-v*/`,
`docs/research/`, `coverage/font-packaging-reconciliation-2.json`, and the
`source` digests of test-owned rows in `coverage/ownership.json`. A digest
refresh never rewrites them. A proposal or candidate that no check, test or
script reads is deleted instead of kept (ADR-0022); kept records may still name
deleted ones, which resolve against Git history.

Kept evidence records the comparator profile, viewer profiles and viewer
adapters it was measured with. Those records are historical (ADR-0019): the
candidate gate checks their shape, not today's files, and recomputes every
judgment from the stored rasters with the live comparator. A comparator or
viewer move therefore needs no successor or archived preimage; one that changes
a judgment fails that recomputation, and the new judgments are reviewed like
any other change.

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

## SVG font subset qualification

`svg-font-subset-viewer-qualification/revision-1.json` is the dual-viewer
qualification of every SVG font subset the renderer can derive (issue #99):
the corpus `tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-qualification-corpus.json`
classifies each of the 231 sources — the 230 official WOFF2 chunks and the
Noto Emoji fallback face (#150) — into the eight qualification classes, and
every successful cell's subset is rendered beside its complete source in
pinned Chromium and the controlled pure-Rust viewer, one semantic execution
per distinct subset identity. Both viewers admit only WOFF2, so the source
role of an sfnt source is the subsetter's own WOFF2 encoding of the
manifested face; the corpus records that encoding's digest as
`viewerSourceWoff2Sha256` beside the sfnt's `sourceSha256`, the pure-Rust
viewer renders from the sfnt itself, and the corpus test checks that the
encoding reconstructs the face's outlines, advances and tables.

Evidence verification is offline and read-only: it checks the accepted corpus,
profiles, complete execution inventory and retained results from both viewers.
The corpus test regenerates subsets with today's implementation and compares
their exact bytes and qualification behavior with the accepted corpus. It is
part of the release-mode qualification set and runs only with
`EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1`:

```sh
cargo run --locked -p xtask -- conformance verify-svg-font-subsets
cargo run --locked -p xtask -- conformance verify-svg-font-subset-release
EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1 cargo test --locked --release -p excalidraw-render --lib \
  complete_font_subset_qualification_corpus_matches_generated_bytes_and_traces
```

Regenerating the corpus stages it under `target/tmp` and prints the `cp`
commands that refresh the committed copies; the dual-viewer run then writes
the viewer inputs, both reports and the manifest for review:

```sh
cargo test --locked --release -p excalidraw-render --lib \
  render::font_subset_qualification::materialize_complete_font_subset_qualification_corpus \
  -- --ignored --exact --nocapture
bash tools/svg-font-subset-qualification.sh generate
bash tools/svg-font-subset-qualification.sh reproduce
```

`just viewer-evidence font-subset generate` acquires what the script needs
and runs it; the `restage.yaml` workflow does so on Linux x86-64 and uploads
the evidence directory.

The manifest's `implementation` list records historical producer provenance:
the subsetter, the qualification module that produced the corpus and viewer
inputs, the two viewer drivers, the report runner, the materializer and the
harness script. Its paths and digests describe the capture, but
verification does not require those source files to exist or match today's
checkout. A source-only refactor therefore needs no manifest refresh. Generation
still records the source identities actually used.

After subsetter changes, run the corpus test above. After changing viewer adapters
or input generation, use `bash tools/svg-font-subset-qualification.sh reproduce`
to regenerate viewer inputs and compare fresh reports byte-for-byte with the
retained reports. Changed subset bytes, reference inputs, viewer profiles or
results still fail their checks and need review; implementation hashes are not
a substitute for those checks.

The release evidence beside it
(`svg-font-subset-release-evidence/revision-1.json`) binds the approved
viewer qualification to the supported-target measurement the packaged
public facade workflow produces (`svg_font_subset_targets.rs`), and returns
to `pending_ci` whenever the corpus moves until a fresh aggregate is
recorded.

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

The candidate expectation manifest binds the two viewer profiles, the
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
nextest profile selects that binary by name, so `dagger api call evidence pdf`
runs them without listing them, and a change that moves a structural digest or a
viewer raster fails visibly rather than leaving the expectation files stale
(#109). The adapter contract (`harness_pdf_viewers::adapters`) pins the viewer
adapters' own behavior: a page is admitted when each of its 96 DPI dimensions
is within one pixel of the supplied canvas and is then placed at exact scale
from the canvas origin, never stretched to fit (#119).

A PDF viewer gate's tolerance exceptions live beside the case:
the consuming test pins the file's sha256 and the record
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
