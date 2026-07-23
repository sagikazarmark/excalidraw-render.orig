# PDF backend qualification

Compatibility revision 1 uses the mandated direct-writer path for project-owned
PDF content. The controlled-fork candidate was rejected because exposing the
complete semantic operation matrix while moving page construction, selectable
text, annotations, output intent, metadata, source images, and fallback planning
to the outer writer would replace rather than adapt `svg2pdf`.

The selected boundary is therefore:

- Project-owned vectors, text, colors, transparency, source images, annotations,
  clipping, metadata primitives, and page structure are written directly from
  `PdfPlan` with `pdf-writer 0.12.0` object identities where applicable.
- Exactly pinned `svg2pdf 0.13.0` receives only resolved, sanitized nested-SVG
  trees and returns bounded Form chunks. The project renumbers and canonicalizes
  every returned object before assembly.
- Project-owned fallback preflight remains authoritative. Unsupported nested-SVG
  effects are admitted only as isolated closures; ordinary content is never
  broadened into raster fallback.

`project-operations.excalidraw` and its structural expectation gate the complete
native matrix through qpdf. Every image is explicitly classified as source
content. `isolated-fallback-structure.json` separately classifies the exact
fallback image and soft mask, preventing either from being mistaken for source
content or whole-page rasterization.

The approved rectangle and Excalifont focused cases run through both canonical
PDF viewers. The package-consumer risk set pins selectable text, source-image,
native nested-SVG, and isolated-fallback artifact and report bytes on all four
native targets and browser-hosted `wasm32-unknown-unknown`.

Run the backend qualification gates after installing the pinned PDF harness:

```sh
cargo test --locked -p excalidraw-render --test pdf_backend_qualification
cargo test --locked -p excalidraw-render --test pdf_backend_qualification -- --ignored
cargo test --locked --release -p excalidraw-render --test rectangle_pdf_render \
  approved_rectangle_passes_both_canonical_pdf_visual_gates -- --ignored --exact
cargo test --locked --release -p excalidraw-render --test excalifont_text_render \
  approved_excalifont_text_passes_both_canonical_pdf_gates -- --ignored --exact
```

Reproduce the isolated-fallback official-SVG authority in two fresh canonical
Chromium processes after installing the reference-oracle dependencies:

```sh
bash tools/reference-oracle/run-pdf-fallback-evidence.sh
```
