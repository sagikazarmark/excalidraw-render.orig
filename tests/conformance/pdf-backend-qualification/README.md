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

The approved rectangle, Excalifont, and frame-export focused cases run through
both canonical PDF viewers. The package-consumer risk set pins selectable text,
source-image, native nested-SVG, and isolated-fallback artifact and report bytes
on all four native targets and browser-hosted `wasm32-unknown-unknown`.

Run the backend qualification gates after installing the pinned PDF harness:

```sh
cargo test --locked -p excalidraw-render --test pdf_backend_qualification
cargo test --locked -p excalidraw-render --test pdf_backend_qualification -- --ignored
cargo test --locked --release -p excalidraw-render --test rectangle_pdf_render \
  approved_rectangle_passes_both_canonical_pdf_visual_gates -- --ignored --exact
cargo test --locked --release -p excalidraw-render --test excalifont_text_render \
  approved_excalifont_text_passes_both_canonical_pdf_gates -- --ignored --exact
cargo test --locked --release -p excalidraw-render --test frame_conformance \
  frame_pdfs_pass_both_canonical_pdf_visual_gates -- --ignored --exact
```

## Re-pinning the ignored gates

The three `*-structure.json` expectations carry two digests that the readable
field checks do not cover: `operation_sha256` hashes every content operator with
its operands, and `inventory_sha256` hashes the structural inventory, including
the XMP metadata stream, embedded font bytes, `ToUnicode` streams, image bytes,
clips, and `ActualText` mappings. Renderer work that moves any of those bytes
moves the digests. The non-ignored tests in the same file pin the artifact bytes
and the expectation file bytes, so re-pinning an artifact digest there keeps
them green while the digests inside the expectation silently go stale. Re-run
the ignored gates whenever a PDF artifact digest in
`crates/excalidraw-render/tests/pdf_backend_qualification.rs` changes.

To re-pin after such a change, first attribute the drift to the commits that
moved the artifact bytes, or accept the current output on review; a diff that
touches any field other than the two digests means a readable field check
failed and must be treated as a defect rather than re-pinned. Then:

1. Run the ignored gates. The structural inspector writes a
   `excalidraw-render/pdf-structure-report/v1` report to
   `target/<prefix>-<pid>-structure.json`, where `<prefix>` is
   `pdf-backend-qualification`, `pdf-fallback-qualification`, or
   `pdf-nested-native-qualification`. The observed digests are the `actual`
   values of the `operation-sequence-sha256` and `structural-inventory-sha256`
   entries in `checks[]`. Copy them from the report; never derive them by hand.
2. Write the observed values into `operation_sha256` and `inventory_sha256` of
   the matching expectation file, keeping the existing single-line layout and
   trailing LF.
3. Update the pinned sha256 of each edited expectation file in
   `pdf_backend_qualification.rs` (`PROJECT_STRUCTURE`, `FALLBACK_STRUCTURE`,
   `NESTED_NATIVE_STRUCTURE`) to `sha256sum` of the new file bytes.

The isolated-fallback visual gate compares the fallback PDF rendered by PDFium
and PDF.js against the official-SVG authority under the tolerance exception in
`tests/conformance/cases/focused/pdf-isolated-fallback/pdf-visual-exception.json`.
That record is a no-margin ceiling: `max_changed_pixels` and
`minimum_ssim_bits` are the exact observed values, so any movement of the
fallback PDF bytes fails the gate even when the divergence shrinks. Run the gate
with `--nocapture`; on failure it prints the observed `changed_pixels` and
`ssim` for the viewer that failed. The printed `Score(...)` decimal is the
shortest round-tripping rendering of the binary64 score, so its `f64::to_bits`
(for example `struct.pack(">d", score).hex()` in Python) is the exact
`minimum_ssim_bits`. Because PDF.js runs first, also run
`bash tools/pdf-harness/run-pdfium.sh render target/pdf-fallback-viewers-<pid>.pdf <output.rgba> 40 30`
on the PDF the test wrote and confirm both viewers emit the same canonical RGBA
before re-approving; if they differ, record both measurements on the issue
instead of choosing a floor. Re-approval sets `minimum_ssim_bits` to the
observed binary64 bits, `max_changed_pixels` to the observed count,
`approved_issue` to the issue carrying the decision, and `reason` to the
diagnosed cause, then updates the pinned sha256 of the exception file in
`pdf_backend_qualification.rs`.

The literals that move together are therefore: the expectation file digests and
their sha256 pins in the test, and the exception record fields and its sha256
pin in the test. The scene, official-SVG, official-RGBA, and evidence-generator
pins in the same test never move for a re-pin; they change only through a
reviewed reference-profile change.

The frame PDF visual gate compares every frame-export target rendered by both
viewers against its `official-svg.rgba` and records its tolerance exceptions in
`tests/conformance/cases/focused/frame-export/pdf-tolerance-exceptions.json`,
one record per `frame_ordinal` and `viewer_profile`, pinned by sha256 in
`frame_conformance.rs` (`PDF_TOLERANCE_EXCEPTIONS_SHA256`). The gate renders
every target in both viewers before judging, so one failing run prints every
divergence with its `changed_pixels` and exact `ssim_bits`, ready to transcribe;
a target whose record it holds must still fail the canonical budget, or the
gate reports the record as stale. Re-measuring follows the fallback flow above,
except that both viewers' values are already printed and each viewer keeps its
own record.

Reproduce the isolated-fallback official-SVG authority in two fresh canonical
Chromium processes after installing the reference-oracle dependencies:

```sh
bash tools/reference-oracle/run-pdf-fallback-evidence.sh
```
