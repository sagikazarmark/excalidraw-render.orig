# Diagnostic/format ownership audit
#85 — public-facade diagnostic assertions

Reviewed all 122 current diagnostic/format obligations. This round proposes owners for 15 and retains 107 explicit gaps. The global ledger now has 134 proposed owners and 794 gaps.

The new `fast/diagnostic_contract.rs` tests author expected machine-readable diagnostic fields independently of the production registry. They compare the complete ordered diagnostic array, exact format/severity/effects/details/recovery/owner/pointer, nonempty messages, and byte-identical serialized reports and artifacts across fresh calls. Message prose is not pinned as a semantic requirement. Image recovery tests cover both saved and error source statuses.

Timestamp assertions cover both element `updated` and referenced-file `created`, including file-source order that disagrees with lexical and reference order, null file ownership, and unreferenced-file silence.

Existing emoji and missing-glyph tests compare full registered ordinary-text reports, occurrence counts and order in every format. They remain partial evidence: frame/generated labels and silence branches need complete assertions before assigning diagnostic ownership. Review similarly retained malformed-element and image obligations as gaps despite the new passing tests.

## Per-code disposition

| Code | Formats | Status | Owner / remaining work |
| --- | --- | --- | --- |
| `asset.malformed-file-record` | pdf, png, svg | gap | New tests cover null records, multiplicity and saved/error recipes. Missing-dataURL pointers, disagreeing file/reference order, escaped IDs and supplemental authority scope remain incomplete. |
| `asset.media-type-mismatch` | pdf, png, svg | gap | Existing static PNG/JPEG tests assert precedence and code counts. Complete claimed/validated MIME, authority/file-ID ordering, repair effects and no-conflict silence assertions are still needed across formats. |
| `asset.supplement-conflict` | pdf, png, svg | gap | Existing static PNG/JPEG tests assert precedence and code counts. Complete claimed/validated MIME, authority/file-ID ordering, repair effects and no-conflict silence assertions are still needed across formats. |
| `color.invalid` | pdf, png, svg | owned | invalid_colors_substitute_in_member_order_and_valid_colors_are_silent |
| `compatibility.known-element-replaced` | pdf, png, svg | gap | Current accepted element families render directly; prove reachability or request an evidence-backed exclusion. Existing no-known-fallback tests are not proof of global unreachability. |
| `compatibility.malformed-element` | pdf, png, svg | gap | New tests cover bounded placeholders, unbounded omissions and deleted silence. Conditional semantic effects for lost recoverable source semantics remain unproven. |
| `compatibility.unsupported-element-type` | pdf, png, svg | owned | unknown_elements_cover_bounded_placeholders_and_unbounded_omissions |
| `compatibility.unsupported-property` | pdf, png, svg | owned | unknown_properties_report_each_source_subtree_and_ignore_opaque_metadata |
| `compatibility.unsupported-property-value` | pdf, png, svg | gap | Field-dependent repaired/ignored/placeholder/omitted/substituted branches and conditional effects span many tests. Enumerate every reachable branch per format before selecting one complete owner. |
| `font.liberation-sans-substituted` | pdf, png, svg | gap | New diagnostic_contract test covers user-text multiplicity and empty/direct-font silence in all formats. Frame names and generated/embeddable labels, including recipe identity, still require a combined exact-report owner. |
| `font.missing-glyph` | pdf, png, svg | gap | Ordinary-text report tests cover scalar count/order. Generated labels/frame names and default-ignorable/unaffected silence need exact coverage. |
| `font.noto-emoji-substituted` | pdf, png, svg | gap | Ordinary-text reports are covered. Generated/embeddable and frame-name occurrences, unaffected/empty silence and embeddable recipe selection need exact coverage and registry reconciliation. |
| `image.animation-flattened` | pdf, png | gap | APNG/GIF/WebP tests cover flattened default frames, duplicate references and omitted occurrences; consolidate full severity/effects/details/file identity and static/one-frame/unreferenced silence across accepted animated codecs. |
| `image.decode-failed` | pdf, png, svg | gap | Each accepted decoder has malformed-payload tests. Need an all-decoder exact-format report matrix preserving validated media type, file deduplication, status-selected recipe and silence before assigning. |
| `image.forbidden-resource` | pdf, png, svg | gap | Existing nested_svg_asset_render tests cover sanitization and reference restrictions. Full nested_source chains, safe-viewport/omission branches and repeated occurrence ordering need a complete exact-field owner. |
| `image.missing-file` | pdf, png, svg | gap | New tests cover referring owners, saved/error PNG recipes and deleted silence. Missing authority alternatives, escaped IDs and valid referenced-image silence need complete coverage. |
| `image.unsafe-svg-content-removed` | pdf, png, svg | gap | Existing nested_svg_asset_render tests cover sanitization and reference restrictions. Full nested_source chains, safe-viewport/omission branches and repeated occurrence ordering need a complete exact-field owner. |
| `image.unsupported-format` | pdf, png, svg | gap | New tests cover unsupported payloads, per-file multiplicity and saved/error recipes. Disagreeing file/reference order, escaped IDs and supplemental authority scope remain incomplete. |
| `link.unsafe-omitted` | pdf, svg | gap | New exact-report tests cover linked shapes, safe-link silence and PNG silence. Generated-label participation remains unproven by this owner. |
| `metadata.diagnostic-summary-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.manifest-embedding-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.manifest-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.provenance-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.scene-compression-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.scene-embedding-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.scene-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `normalization.duplicate-element-id` | pdf, png, svg | gap | Existing normalization tests cover ID repair/rekey behavior, but generated IDs and relation-dependent effects require complete all-format report assertions and exact multiplicity/silence ownership. |
| `normalization.missing-element-id` | pdf, png, svg | gap | Existing normalization tests cover ID repair/rekey behavior, but generated IDs and relation-dependent effects require complete all-format report assertions and exact multiplicity/silence ownership. |
| `normalization.missing-timestamp` | pdf, png, svg | owned | missing_timestamps_are_repaired_in_source_order_and_valid_or_deleted_are_silent |
| `normalization.relationship-cycle` | pdf, png, svg | gap | Stronger two-cycle public-facade probe emits each closing-edge diagnostic twice (b/frameId and d/frameId). Existing relationships test uses find and misses duplicates. Diagnose before assigning; exact effect rules also require review. |
| `normalization.zero-rough-seed` | pdf, png, svg | owned | zero_rough_seeds_are_repaired_once_per_participating_shape |
| `pdf.raster-fallback` | pdf | gap | Existing PDF fallback tests cover isolated closure and forbid behavior; consolidate exact affected IDs, bounds, reason/family details and source order for all fallback families. |
| `resource.asset-bytes-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.asset-work-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.coordinate-range-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.element-complexity-replaced` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.element-work-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.image-dimensions-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.image-pixels-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.nested-asset-depth-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.nested-svg-complexity-exceeded` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |
| `resource.optional-metadata-omitted` | pdf, png, svg | gap | Existing resource-limit tests cover selected ceilings and recoveries. Need complete field/configured/measurement/value-kind checks and top-level/nested bounded/unbounded branches with exact per-format occurrence order; do not infer full ownership from one limit crossing. |

## Cycle diagnostic finding

A focused probe using two independent frameId cycles observed **two identical diagnostics per closing edge**, not one. The existing `relationships::canonical_cycle_closing_edge_is_removed_with_its_source_pointer` test uses `.find()` and cannot reject duplicates. All three relationship-cycle obligations remain gaps; no expected duplicate behavior was approved or installed as a passing regression.

Reproduction scene (render through each SVG/PNG/PDF default request):

```json
{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {
      "id": "a",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 40,
      "height": 40,
      "seed": 1,
      "updated": 1,
      "frameId": "b"
    },
    {
      "id": "b",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 40,
      "height": 40,
      "seed": 1,
      "updated": 1,
      "frameId": "a"
    },
    {
      "id": "c",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 40,
      "height": 40,
      "seed": 1,
      "updated": 1,
      "frameId": "d"
    },
    {
      "id": "d",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 40,
      "height": 40,
      "seed": 1,
      "updated": 1,
      "frameId": "c"
    }
  ],
  "appState": {},
  "files": {}
}
```

Observed SVG order: `/elements/1/frameId` twice, then `/elements/3/frameId` twice; each is `normalization.relationship-cycle`, warning, effects `[input_repair, semantic]`, compatibility recovery `removed`, no recipe. Investigate repeated cycle admission/emission and reconcile conditional effects before assigning ownership. The initial exact-report probe stopped at SVG on failure; PNG/PDF duplication has not yet been independently asserted.

## Remaining work

- Complete generated-ID/rekey and relationship-cycle contracts.
- Enumerate unsupported-property-value recovery branches and known-element fallback reachability.
- Complete authority/media/decoder/nested-resource and animation matrices.
- Cover frame/generated font substitutions and every PDF fallback detail family.
- Add exact metadata failure injection owners and complete resource-limit recovery branches.

The ledger remains unapproved and incomplete. These mappings do not close #85 or waive upstream-blocked #178.

## Verification

On the working tree based on `c51a52e`:

- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 11 passed.
- `cargo test --locked -p xtask ownership`: six matching unit tests passed.
- `cargo test --locked -p xtask coverage`: 13 matching unit tests passed.
- `cargo test --locked -p xtask --test coverage`: three integration tests passed, including rejection of draft release closure.
- `cargo run --locked -p xtask -- conformance verify-ownership`: 928 assignments, 134 assigned/excluded, 794 explicit gaps; approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks passed.
- `dagger check excalidraw-render:test`: fast, render and doctest checks passed.
- `git diff --check`: passed.

Exactly the 122 diagnostic assignments changed in the ledger. Its other assignments and top-level approval/authority fields retain their previous values.
