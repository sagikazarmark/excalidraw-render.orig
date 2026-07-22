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

The first typography case uses a separate pinned runner so changes needed for
font-bearing evidence cannot invalidate the approved rectangle generator:

```sh
cargo run --locked -p xtask -- conformance generate focused.excalifont-text.v1
cargo run --locked -p xtask -- conformance reproduce focused.excalifont-text.v1
cargo run --locked -p xtask -- conformance approval-subject focused.excalifont-text.v1
```

`focused.excalifont-text.v1` binds the official basic-Latin Excalifont WOFF2
identity and remains unapproved until a maintainer records approval for its
printed subject digest. Policy approval for deterministic predecoding does not
constitute approval of either the generated SFNT bytes or this evidence case.

The checked manifest is canonical JSON followed by one LF. Its adjacent digest
hashes the canonical bytes without that LF. The approval subject excludes only
the four-field approval object and binds the remaining manifest plus every
referenced scene, report, provenance, artifact, and raw-raster byte string.

## Visual comparator

`comparator-profiles/canonical-visual-v1.json` freezes the format-neutral
RGBA8 projection, Pixelmatch, SSIM, crop, and visual-budget protocol. Its
adjacent digest binds the profile; the profile in turn binds the unpublished
Rust implementation, workspace lockfile, adversarial vectors, and imported
Pixelmatch 7.2.0 fixtures. Verify all of them offline with:

```sh
cargo test --locked -p excalidraw-render-compare
```

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
