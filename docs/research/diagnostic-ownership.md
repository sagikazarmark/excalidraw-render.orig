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
| `normalization.relationship-cycle` | pdf, png, svg | gap | Duplicate repairs and unconditional effects are fixed. Tests cover scene/frame consumers and format-dependent effects; mixed-cycle reciprocal bindings and fallback participation still need a complete exact-report owner. |
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

A focused probe using two independent frameId cycles observed **two identical diagnostics per closing edge**, not one. The original `relationships::canonical_cycle_closing_edge_is_removed_with_its_source_pointer` test used `.find()` and could not reject duplicates. The follow-up replaces that assertion with an exact count and fixes the duplication as described below. All three relationship-cycle obligations remain gaps pending conditional-effects coverage.

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

Original SVG order: `/elements/1/frameId` twice, then `/elements/3/frameId` twice; each is `normalization.relationship-cycle`, warning, effects `[input_repair, semantic]`, compatibility recovery `removed`, no recipe. The initial exact-report probe stopped at SVG on failure.

### Duplicate-emission fix

The minimal two-element cycle reproduced the same failure. A targeted probe at cycle rejection identified two graph projections of one source relationship: `frameId` produces both `Frame` and `Clip`; `containerId` similarly produces `Binding` and `Mask`. Each rejected projection appended a removed-cycle record, so report assembly correctly emitted the duplicated records it received.

`relationships::resolve` now sorts removed-cycle records by source owner/position/pointer and consolidates identical source pointers after traversal. Graph-edge admission/removal and traversal accounting remain intact. Distinct source properties on the same owner remain separate repairs.

The public-facade regression covers the minimal cycle, the original two independent cycles, self-cycles on two distinct properties of a text owner, acyclic/deleted silence, exact ordered machine-readable reports and repeated artifact/report equality in SVG, PNG and PDF. A separate test verifies per-frame scoping and standalone/batch replay equality. Exact cardinality assertions at the facade would have caught the original defect; `.find()` only proved existence.

**Effects gap at the duplicate-fix checkpoint:** `relationship_cycle_diagnostic` still supplied `[input_repair, semantic]` unconditionally. The subsequent channel-aware fix below replaces that behavior. Counts remain 134 proposed owners and 794 gaps until the remaining branches have a complete owner.

Follow-up verification on the working tree based on `97c8fa1`:

- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract::relationship_cycles_report_each_closing_edge_once -- --exact`: failed before the fix with two identical SVG diagnostics, passed after the fix across all three formats.
- `cargo test --locked -p excalidraw-render --test fast relationships::`: three passed.
- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 12 passed.
- `cargo run --locked -p xtask -- conformance verify-ownership`: passed with 134 assigned/excluded and 794 gaps.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: ten checks passed.
- `dagger check excalidraw-render:test`: fast, render and doctest checks passed.
- `git diff --check`: passed; temporary cycle-rejection instrumentation removed.

## Remaining work

### Channel-aware cycle effects

The follow-up based on `bfedb0b` preserves source/target indices and a source-consumer category in each removed-cycle record. Selection occurs during target-scoped semantic resolution, where normalized participation, persisted frame controls, request controls and output format are available. Graph traversal still removes each projection and emits one diagnostic per repaired source pointer.

| Consumer | Visual effect selected when | Semantic effect |
| --- | --- | --- |
| Frame membership | Source participates and scene clipping is enabled, or the target is a frame export | SVG/PDF when visual is selected |
| Text container | Source has nonempty normalized text, or its binding can select a normalized target in a frame export | SVG/PDF when visual is selected |
| Reverse bound-element reference | Nonempty target text retains a container relationship to the source, or the reference can select a normalized target in a frame export | SVG/PDF when visual is selected |
| Endpoint binding | The reference can select a normalized target in a frame export; whole-scene geometry uses stored coordinates | SVG/PDF when visual is selected |

Every repair retains `input_repair`. PNG carries no text/link/vector output semantics, so its affected-output branch adds only `visual`. Disabled scene clipping (persisted or requested), empty unbound text, and unconsumed whole-scene bindings are input-only. Frame selection can exclude a target because its binding was removed; pre-scope normalized participation therefore comes from the relationship graph, rather than only the filtered element list.

The minimal regression disables scene frame rendering and asserts input-only effects for a self-referencing `frameId`. It failed before the change with an extra `semantic` effect. Additional tests cover clip-off controls, persisted controls, both endpoint binding fields, both reverse-binding encodings, empty/nonempty text, per-frame scoping, and standalone/batch equality for a rejected binding to a nonselected target. Existing exact cycle reports now expect visual effects and the SVG/PDF semantic branch.

The original constructor accepted neither consumer information nor selected-channel flags; it could not express the registry's conditional contract. Carrying the source relationship's consumer category to the target-aware diagnostic call fixes that loss of information. Remaining mixed-cycle reciprocal-binding and fallback-participation branches stay explicit ownership gaps.

Verification for this follow-up:

- The disabled-frame minimal test failed before the fix with an extra `semantic` effect and passed afterward.
- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 14 passed.
- `cargo test --locked -p excalidraw-render --test fast relationships::`: four passed, including the nonselected-target frame regression.
- `cargo run --locked -p xtask -- conformance verify-ownership`: passed; 134 assigned/excluded, 794 gaps, approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks passed.
- `dagger check excalidraw-render:test`: all three checks passed.
- `git diff --check`: passed.

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
