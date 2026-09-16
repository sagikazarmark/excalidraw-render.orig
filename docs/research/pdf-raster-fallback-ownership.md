# PDF raster-fallback ownership investigation (#85)

Base: `56ff59e836d1354b652456083ee5673a981b67ad`.

## Status: proposed PDF diagnostic owner recorded

The proposed exact selector is
`pdf_raster_fallback_contract::raster_fallback_contract_covers_families_occurrences_geometry_and_failures`.
The initial public-facade regressions exposed disagreements between the admitted
closure and the encoded PDF surface. The user authorized replacing that split
with a shared closure-plan adapter. The aggregate passes and independent review
findings are resolved. The PDF diagnostic now has a proposed owner: **822
assigned/excluded and 106 gaps** globally, with **98 of 122** diagnostic/format
entries assigned. The incomplete global ledger remains pending approval.

## Evidence added

The new fast module authors complete machine reports independently of the
production registry, including severity, effects, source owner, escaped pointer,
logical bounds, final dimensions, recovery, and every nested cause segment.
Fresh renderer sessions compare complete serialized reports and artifact bytes.

Passing focused coverage includes:

- Ten effect cases covering blur, convolution, morphology, displacement,
  turbulence, diffuse/specular lighting, color matrices, component transfer and
  filter blends. Each has exact success and Forbid failure reports and SVG/PNG
  silence. The PDF RGB/alpha planes are decoded and compared pixel by pixel
  against direct rasterization of the independently authored source SVG.
- Sibling occurrences, reused asset bytes, source order different from ID order,
  and dependent effects merged into one closure with all causes retained.
- Identical scene payloads under different file IDs, supplemental authority,
  expanded `use` occurrences and nested data-image cause paths.
- Native operation matrices and provably empty/identity filters, including
  independent clean-scene artifact equality for the no-op cases.
- Direct/prepared frame scope and replay, and mixed diagnostics from different
  source elements.

The aggregate also executes resource/unisolatable preflight assertions after the
scale matrix, including exact fatal categories/details, complete-plan warning
suppression, first-closure Forbid selection and the diagnostic-count failure prefix.

## Two narrow fixes

### Frame exports omitted a required warning

The frame PDF contained the raster image while its report was empty. Closure
planning called `Path::cubic_bounds` on a rectangular frame-export clip, whose
path has no cubic segments, and treated `None` as an empty clip. Scene frames
have rounded paths and did not expose this branch.

Planning now uses the clip's explicit frame width and height. The exact frame
regression failed before the change and passes after it, for both direct and
prepared rendering.

### Compact plan indices corrupted mixed diagnostic ordering

The PDF warning for an image sorted before an unsafe-link diagnostic from an
earlier rectangle. Closure planning used a compact PDF element index as a source
owner order, whereas other producers use the original scene owner order.

The PDF image variant now retains `ResolvedElement::source_order()`, and closure
reports use it. The independently ordered mixed report failed before the change
and passes after it.

## Original output/accounting violations

### Final scale

For a 20×10 filtered image exported at scale 0.5, the diagnostic reports a 10×5
surface, but the PDF embeds 20×10 RGB and alpha planes. `PdfPlan::svg_resources`
clamps its scalar raster scale to at least one. Removing that clamp makes this
example pass but does not solve independent axis rounding, subpixel placement,
or cropped output; the tentative clamp removal was reverted.

### Crop and surface limit

For a 20×10 filtered image cropped to its rightmost 10×10 pixels, a request with
`max_raster_surface_pixels = 100` succeeds and reports a 10×10 closure. Its PDF
still contains 20×10 RGB and alpha planes: **200 output pixels after admitting
100**. The test parses the actual image dictionaries and decoded streams; this
is not a substring match or an inferred backend allocation.

Minimal reproduction: an image with width/height 10, a 20×10 embedded SVG with
one full-region Gaussian-blurred rectangle, and crop
`{"x":10,"y":0,"width":10,"height":10,"naturalWidth":20,"naturalHeight":10}`.

Spec §8.6 requires clipped, device-grid-aligned closure output at one pixel per
final pixel (§8.6 lines 1547–1551 at the base); §13.2 charges every selected
raster surface's full area. A larger filter-input/intermediate workspace may be
necessary, but the larger surface here is the embedded fallback output itself.

The independent Spec review confirmed both as contract/accounting violations.
The initial Standards review found no new hard breach in the narrow fixes and
confirmed that the ledger correctly retained the gap at that checkpoint.

## Implemented shared-plan adapter

Previously the planner intersected and snapped bounds for admission and
diagnostics, then `svg2pdf::to_chunk` independently rasterized filtered group
layers. Scalar conversion options could not communicate the admitted device
origin, independent dimensions or input requirements.

`PdfRasterClosure` now carries the input/output grids and occurrence transform.
Preflight admits both grids when distinct; the raster adapter evaluates the full
input and copies the selected output pixels without resampling. The cropped blur
therefore rejects a 100-pixel ceiling because its input requires 200 pixels; at a
200-pixel ceiling it emits exactly the reported 10×10 output. The independently
rendered full blur, cropped in the test, proves that edge contributions survive.

The adapter preserves native ancestors and gives svg2pdf native trees containing
already-rendered closure PNGs. Pattern/mask definitions are expanded per reference
so differently transformed uses cannot share the wrong sampling grid. Their
consumer transforms participate in planning, and `feImage` inputs merge into the
enclosing closure. Tests cover an 8×8 and a 4×4 occurrence of one pattern, a scaled
mask, and an outer filter consuming another filtered subtree. Repeating texture
visibility follows its consumer, while the texture retains its complete tile
viewport even when the base tile is outside a crop. Positive/negative pattern
origins participate in both tile placement and sampling; the recorded dependency
bounds can lie outside the consumer's page because later repetitions remain visible.

Stylesheet filters previously bypassed effect discovery. The pinned `simplecss`
selector/cascade implementation now resolves them before private marker IDs are
introduced. PDF-only lowering preserves computed styles while recording original
node/rule/declaration cause paths and charging selector attempts. A `[id]` rule
regression checks that injected IDs cannot change appearance. The new direct
dependency is already present transitively; no external package version changed.
Inline-only filters use the same cascade path, including `!important` and
presentation-attribute overrides independent of source attribute order.

Normalized SVG serialization restores round-trip path/transform tokens beyond the
pinned writer's twelve-decimal precision ceiling and materializes anonymous
`feImage` inputs the writer cannot otherwise reference. A 1e-13 source rectangle
with a visible flood effect checks that normalization cannot erase the effect.
Owned buffers and the existing PNG encoder provide fallible raster/output
allocation. The serializer pairs existing nested-image data URLs with their
normalized source trees; lookup never serializes an additional child copy.
Adapted content streams are requested uncompressed and compressed by the owned
PDF stream writer, including tiling patterns, eliminating an allocating inflate
pass. Only XMP retains its permitted stored-stream exception.

See [ADR-0009](../adr/0009-share-pdf-raster-closure-planning-and-encoding.md) for
the chosen interface and qualification responsibilities.

## Reproduction commands

Use the pinned Rust 1.97.1 toolchain on `PATH`:

```sh
cargo test --locked -p excalidraw-render --test fast pdf_raster_fallback_contract::scaled_bounds_and_failure_preflight_are_exact -- --exact
cargo test --locked -p excalidraw-render --test fast pdf_raster_fallback_contract::cropped_closure_dimensions_match_admitted_grid -- --exact
```

Both regressions now pass. The crop test checks rejection of the insufficient
input ceiling and the exact output grid and pixels at an adequate ceiling.

## Verification checkpoint

- Initial investigation: six tests passed and the two grid regressions plus
  aggregate failed. Existing plan-only tests still passed, demonstrating why
  public-facade image-plane assertions were necessary.
- `cargo test --locked --workspace --no-fail-fast` with Rust 1.97.1 and
  `CARGO_INCREMENTAL=0`: **1,071 passed, 18 ignored**. The fast tier passed all
  513 tests in 898.05 seconds, including the pre-existing ownership aggregates.
- After the final pattern-origin and nested-image lookup fixes, all **16** PDF
  contract tests, **31** nested-SVG tests, **18** plan tests and **4** harness-free
  PDF qualification tests passed again. The complete workspace suite was not
  repeated after these narrow changes. Final contract assertions also cover
  multi-closure preflight and exact failure-report prefixes.
- All four existing harness-free PDF qualification tests pass. The isolated
  fallback remains byte-identical to its committed pin; no expectation or
  exception record was repinned.
- The exact-pinned qpdf structural gate and both PDF.js/PDFium visual gates pass.
  Both viewers produce RGBA SHA-256
  `4f09344fe43948db344a85f6da17821281d2f78d97edfe86291ff1aa34f155bd`.
- Workspace/all-target Clippy (`-D warnings`), library rustdoc
  (`RUSTDOCFLAGS="-D warnings"`), and the renderer's
  `wasm32-unknown-unknown` compilation check passed.
- Catalog verification, all **37** registered conformance cases, draft inventory
  verification and ownership verification passed. The inventory retains eight
  unresolved release milestones; the ownership ledger reports **822/106**.
- Standards review: no remaining finding in the completed changes. Spec review:
  the recorded blockers are resolved, with final narrow reviews confirming the
  last pattern-origin and allocation fixes. Review does not grant ledger approval.
- No new native/WASM execution equality evidence or exclusion approval is claimed.
  The separate #178 target-equality blocker remains at its existing checkpoint.
