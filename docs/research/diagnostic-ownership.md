# Diagnostic/format ownership audit
#85 — public-facade diagnostic assertions

Reviewed all 122 current diagnostic/format obligations. Including PDF raster fallback, 98 now have proposed owners and 24 remain explicit gaps. The global ledger has 822 proposed owners and 106 gaps. The PDF assignment is assessed in [pdf-raster-fallback-ownership.md](pdf-raster-fallback-ownership.md); the six compatibility assignments in [compatibility-recovery-ownership.md](compatibility-recovery-ownership.md); the final 15 resource assignments in [resource-diagnostic-ownership.md](resource-diagnostic-ownership.md); and the eight nested SVG/link assignments in [svg-link-diagnostic-ownership.md](svg-link-diagnostic-ownership.md).

The new `fast/diagnostic_contract.rs` tests author expected machine-readable diagnostic fields independently of the production registry. They compare the complete ordered diagnostic array, exact format/severity/effects/details/recovery/owner/pointer, nonempty messages, and byte-identical serialized reports and artifacts across fresh calls. Message prose is not pinned as a semantic requirement. Image recovery tests cover both saved and error source statuses.

Timestamp assertions cover both element `updated` and referenced-file `created`, including file-source order that disagrees with lexical and reference order, null file ownership, and unreferenced-file silence.

Existing emoji and missing-glyph tests compare full registered ordinary-text reports, occurrence counts and order in every format. They remain partial evidence: frame/generated labels and silence branches need complete assertions before assigning diagnostic ownership. Review similarly retained malformed-element and image obligations as gaps despite the new passing tests.

## Per-code disposition

| Code | Formats | Status | Owner / remaining work |
| --- | --- | --- | --- |
| `asset.malformed-file-record` | pdf, png, svg | owned | malformed_file_contract_in_every_format — record/payload variants, authority/order, first-reference status, narrower-cause suppression and frame replay; see [assessment](image-diagnostic-ownership.md#final-ownership-assessment). |
| `asset.media-type-mismatch` | pdf, png, svg | owned | media_type_contract_in_every_format — exact seven-kind claims, strict-domain enrichment, source/supplemental ordering and equivalence, invalid/failed payload handling and frame replay; see [assessment](asset-claim-diagnostic-ownership.md#media-type-completion). |
| `asset.supplement-conflict` | pdf, png, svg | owned | supplement_conflicts_report_authority_once_in_every_format — exact authority/order/multiplicity, identical/different payloads, malformed scene precedence, silence and frame replay. |
| `color.invalid` | pdf, png, svg | owned | invalid_colors_substitute_in_member_order_and_valid_colors_are_silent |
| `compatibility.known-element-replaced` | pdf, png, svg | gap | Current dispatch is unreachable by the [control-flow proof](known-element-replacement-reachability.md), supported by exact narrower-cause guards. Explicit digest-bound #85 exclusion approval is still required. |
| `compatibility.malformed-element` | pdf, png, svg | owned | compatibility_recovery_contract::malformed_element_contract_covers_recovery_and_semantic_loss — complete reports, bounds/omission, safe-link loss including rejected frames, PNG silence, authored transform/opacity/clip artifacts, source preservation and frame replay. |
| `compatibility.unsupported-element-type` | pdf, png, svg | owned | unknown_elements_cover_bounded_placeholders_and_unbounded_omissions |
| `compatibility.unsupported-property` | pdf, png, svg | owned | unknown_properties_report_each_source_subtree_and_ignore_opaque_metadata |
| `compatibility.unsupported-property-value` | pdf, png, svg | owned | compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family — executable eight-module field matrices, authored manifest inventory, all currently bound recovery forms, conditional effects, source/component ordering, field-vs-element omission, mixed causes and fatal prefixes. |
| `font.liberation-sans-substituted` | pdf, png, svg | owned | font_diagnostics_cover_text_generated_labels_and_silence — exact surviving-text/fallback substitution, ordering and chrome/omission silence. See [assessment](font-diagnostic-ownership.md#final-ownership-assessment). |
| `font.missing-glyph` | pdf, png, svg | owned | font_diagnostics_cover_text_generated_labels_and_silence — text/frame/generated-label scalar reports, logical order, omission/truncation silence and default-ignorable boundaries. |
| `font.noto-emoji-substituted` | pdf, png, svg | owned | font_diagnostics_cover_text_generated_labels_and_silence — occurrence multiplicity, ordinary/embeddable recipes, frame scope, local-omission and zero-extent branches. |
| `image.animation-flattened` | pdf, png | owned | animation_flattening_contract_in_every_format — exact APNG/GIF/WebP occurrence reports, static/one-frame/SVG silence, resource omissions, frame replay and frame-zero PNG comparison. See [assessment](animation-diagnostic-ownership.md). |
| `image.decode-failed` | pdf, png, svg | owned | decode_failures_cover_every_codec_and_authority — exact seven-codec source/supplemental/nested reports, validated MIME, first-reference status, corruption variants, silence and frame replay. See [decode assessment](decode-diagnostic-ownership.md). |
| `image.forbidden-resource` | pdf, png, svg | owned | svg_link_diagnostic_contract::forbidden_resource_contract_in_every_format — exact root/nested reports, effective href precedence, bounded recipe/omission artifacts, target-local bounds, authority, ordering, notices and replay. |
| `image.missing-file` | pdf, png, svg | owned | missing_image_contract_in_every_format — exact per-reference reports, missing/null/empty and escaped IDs, saved/error recovery, valid/supplemental/deleted silence and frame scope/replay. |
| `image.unsafe-svg-content-removed` | pdf, png, svg | owned | svg_link_diagnostic_contract::unsafe_svg_contract_in_every_format — complete unsafe-boundary/effect/path reports, exact entity source locations, repeated identities, authority/frame scope, clean artifact oracles and source preservation. |
| `image.unsupported-format` | pdf, png, svg | owned | unsupported_image_contract_in_every_format — source/supplemental/nested reports, status and frame scope, ICC rejection/pass-through, nested paths and semantic preservation; see [assessment](image-diagnostic-ownership.md#final-ownership-assessment). |
| `link.unsafe-omitted` | pdf, svg | owned | svg_link_diagnostic_contract::unsafe_link_contract_in_semantic_formats — ordinary/fallback/generated-label participation, full reports, safe canonicalization and PNG silence, independent paint/no-navigation checks, source order, frame replay and preservation. |
| `metadata.diagnostic-summary-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.manifest-embedding-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.manifest-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.provenance-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.scene-compression-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.scene-embedding-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `metadata.scene-serialization-failed` | pdf, png, svg | gap | Production failure path needs deterministic injection through a supported test-only seam plus exact all-field format/operation/recovery assertions. Existing internal metadata tests cover selected failure stages but v1 ownership admits fast integration tests only. |
| `normalization.duplicate-element-id` | pdf, png, svg | owned | duplicate_identity_contract::identity_and_duplicate_relationship_contract_in_every_format — first-alias/rekey behavior, conditional effects, cycles, target silence, preservation and stage-appropriate failure reports; companion cold/prepared trace boundaries pass. See [completion](duplicate-identity-ownership.md#ownership-completion). |
| `normalization.missing-element-id` | pdf, png, svg | owned | admission_identity_contract::missing_and_invalid_ids_have_exact_recovery_and_target_participation — literal identity vectors, exact reports, all traversed kinds, target/deleted/ignored silence, source preservation and fresh-session/frame replay; see [assessment](admission-identity-ownership.md). |
| `normalization.missing-timestamp` | pdf, png, svg | owned | missing_timestamps_are_repaired_in_source_order_and_valid_or_deleted_are_silent |
| `normalization.relationship-cycle` | pdf, png, svg | owned | relationship_cycle_contract_in_every_format — exact scene/frame reports, mixed reciprocal cycles, fallback participation, channel effects, source ordering and repeatability. |
| `normalization.zero-rough-seed` | pdf, png, svg | owned | zero_rough_seeds_are_repaired_once_per_participating_shape |
| `pdf.raster-fallback` | pdf | owned | pdf_raster_fallback_contract::raster_fallback_contract_covers_families_occurrences_geometry_and_failures — exact family/occurrence reports, source/CSS paths, shared admitted/encoded grids, independent pixel oracles, repeating and transformed dependencies, frame replay, silence and complete-plan failures. See [assessment](pdf-raster-fallback-ownership.md). |
| `resource.asset-bytes-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::asset_byte_contract_covers_admission_authority_recovery_and_frames — exact byte boundaries, source/supplemental and reference ordering, recovery, aggregate/report precedence, frame replay and containing-parent proof for nested bytes. |
| `resource.asset-work-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::asset_work_contract_covers_intrinsics_nested_recovery_and_frames — validation/normalization/XML work boundaries, top-level and nested reports, recovery envelope, conditional effects, authority/reference order and frame replay. |
| `resource.coordinate-range-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::coordinate_contract_covers_components_points_recovery_and_frames — source/derived components, finite and at-least measurements, nested paths and recipes, clipped subtree omission, fatal direct/prepared viewports, conditional effects and report prefixes. |
| `resource.element-complexity-replaced` | pdf, png, svg | owned | resource_diagnostic_contract::element_complexity_contract_covers_geometry_text_recovery_and_frames — both geometry and Unicode-scalar fields, source-font glyph command oracle, fallback/label/glyph recovery, aggregate precedence and frame/source scope. |
| `resource.element-work-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::element_work_contract_covers_generation_shaping_projection_and_frames — semantic and PNG-projection boundaries, source-font glyph/coverage oracle, staged recovery, empty-content effects, order, prefixes and frame replay. |
| `resource.image-dimensions-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::image_dimension_contract_covers_codecs_recovery_and_scope — six-codec boundaries, width/height precedence, intrinsic attribution, independent recovery and frame/authority scope; see [assessment](nested-resource-limit-amendment.md#raster-dimension-and-pixel-ownership). |
| `resource.image-pixels-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::image_pixel_contract_covers_codecs_recovery_and_scope — six-codec boundaries, nested recovery, intrinsic attribution and authority/frame silence; see [assessment](nested-resource-limit-amendment.md#raster-dimension-and-pixel-ownership). |
| `resource.nested-asset-depth-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::nested_depth_contract_covers_owners_paths_frames_and_silence — exact depth/effect/path/owner reports, boundaries, depth-aware caches, stopped discovery, authority and frame silence, independent manifested placeholder/omission artifacts; see [assessment](nested-resource-limit-amendment.md#nested-depth-ownership). |
| `resource.nested-svg-complexity-exceeded` | pdf, png, svg | owned | resource_diagnostic_contract::nested_node_limits_cover_root_and_child_reports_and_silence — exact root/child node and text counts, boundaries, stopped discovery, authority/scope and independent recovery artifacts; see [assessment](nested-resource-limit-amendment.md#nested-node-ownership). |
| `resource.optional-metadata-omitted` | pdf, png, svg | owned | resource_diagnostic_contract::optional_metadata_contract_covers_components_limits_and_frames — component priority, output/work/scratch/retention boundaries, frozen summaries/notices, fatal prefixes and independent frame outcomes; see [assessment](nested-resource-limit-amendment.md#optional-metadata-ownership). |

## Cycle diagnostic finding

A focused probe using two independent frameId cycles observed **two identical diagnostics per closing edge**, not one. The original `relationships::canonical_cycle_closing_edge_is_removed_with_its_source_pointer` test used `.find()` and could not reject duplicates. The follow-ups replace that assertion with an exact count, fix duplication and channel effects, and consolidate ownership as described below.

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
| Frame membership | Source participates and scene clipping is enabled, the target is a frame export, or the parent frame supplies non-opaque inherited opacity | SVG/PDF when visual is selected |
| Text container | Source has nonempty normalized text, or its binding can select a normalized target in a frame export | SVG/PDF when visual is selected |
| Reverse bound-element reference | Reference can select a normalized target in a frame export; a retained reciprocal Mask cannot coexist with the path rejecting this reverse dependency | SVG/PDF when visual is selected |
| Endpoint binding | The reference can select a normalized target in a frame export; whole-scene geometry uses stored coordinates | SVG/PDF when visual is selected |

Every repair retains `input_repair`. PNG carries no text/link/vector output semantics, so its affected-output branch adds only `visual`. Disabled scene clipping (persisted or requested) without inherited opacity, empty unbound text, and unconsumed whole-scene bindings are input-only. Frame selection can exclude a target because its binding was removed; pre-scope normalized participation therefore comes from the relationship graph, rather than only the filtered element list.

The minimal regression disables scene frame rendering and asserts input-only effects for a self-referencing `frameId`. It failed before the change with an extra `semantic` effect. Additional tests cover clip-off controls, persisted controls, both endpoint binding fields, both reverse-binding encodings, empty/nonempty text, per-frame scoping, and standalone/batch equality for a rejected binding to a nonselected target. Existing exact cycle reports now expect visual effects and the SVG/PDF semantic branch.

The original constructor accepted neither consumer information nor selected-channel flags; it could not express the registry's conditional contract. Carrying the source relationship's consumer category to the target-aware diagnostic call fixes that loss of information. At that checkpoint, mixed-cycle reciprocal-binding and fallback-participation branches remained explicit ownership gaps; the next follow-up addresses them below.

Verification for this follow-up:

- The disabled-frame minimal test failed before the fix with an extra `semantic` effect and passed afterward.
- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 14 passed.
- `cargo test --locked -p excalidraw-render --test fast relationships::`: four passed, including the nonselected-target frame regression.
- `cargo run --locked -p xtask -- conformance verify-ownership`: passed; 134 assigned/excluded, 794 gaps, approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks passed.
- `dagger check excalidraw-render:test`: all three checks passed.
- `git diff --check`: passed.

### Consolidated cycle owner

The follow-up based on `c80616c` selects `diagnostic_contract::relationship_cycle_contract_in_every_format` as the proposed owner for all three cycle/format obligations. The unconditional test calls local scenario helpers in the same digest-bound file; its exact selector runs the full contract without relying on sibling tests.

It covers multiple and self-cycles, distinct properties on one owner, source order that disagrees with lexical IDs, acyclic/deleted silence, ordinary/magic frame scoping, both endpoint fields, both reverse-binding encodings, empty/nonempty text, persisted/requested clipping controls, mixed reciprocal cycles, valid reciprocal silence, bounded placeholders and unbounded omissions, and selected/nonselected frame sources. Scene and frame assertions compare complete ordered machine fields, nonempty messages, repeated serialized reports/artifacts, and standalone/batch equality.

Two regressions exposed additional defects:

- A retained placeholder's cycle diagnostic disappeared from frame exports because cycle filtering used only normalized participants. The scope now includes the same retained fallback owners as normalization diagnostics. The exact scene/frame test failed before this change and passes afterward; outside-viewport and unbounded omitted sources remain silent for that frame.
- A cycle removing a non-opaque parent frame's opacity reported input-only effects when clipping was disabled. Frame opacity is consumed independently of clipping controls; effect selection now includes it. The disabled-clipping opacity test failed before this change and passes afterward.

The mixed-cycle review also removed an unreachable reverse-binding effect branch: a retained reciprocal Mask has the same dependency direction as the reverse binding. Since the accepted graph is acyclic and only gains edges during traversal, that Mask cannot survive alongside the path that rejects the reverse binding. Without it, the target text is unbound and does not consume the rejected reverse reference. Valid reciprocal pairs are silent; opposing frame relationships exercise rejection of both encodings and the text-container edge with exact ordered reports. This is a graph invariant within the covered obligation, not an excluded obligation.

Current totals: 18 proposed diagnostic/format owners, 104 diagnostic gaps; 137 proposed owners and 791 gaps globally. Approval remains pending.

Verification for the consolidated owner:

- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract::relationship_cycle_contract_in_every_format -- --exact`: passed across all formats.
- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 12 passed after consolidating the cycle scenarios into one test.
- `cargo test --locked -p excalidraw-render --test fast relationships::`: four passed.
- `cargo run --locked -p xtask -- conformance verify-ownership`: 137 assigned/excluded, 791 explicit gaps; approval pending.
- Dagger conformance checks all passed. Clippy initially rejected an unnecessary cloned slice; after replacing it with `std::slice::from_ref`, `dagger check excalidraw-render:lint excalidraw-render:conformance:coverage-ownership` passed all three checks.
- `dagger check excalidraw-render:test`: fast, render and doctest checks passed on the final code.
- `git diff --check`: passed.

### Remaining diagnostic work

- Complete generated-ID/rekey contracts.
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
