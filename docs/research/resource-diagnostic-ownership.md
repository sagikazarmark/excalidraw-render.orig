# Resource diagnostic ownership completion (#85)

The coordinate implementation is committed at `aafed1d`. This follow-up completes
the remaining five resource diagnostic codes in all three formats, using the
public scene/frame facade and complete report comparisons in
`tests/fast/resource_diagnostic_contract.rs`.

## Primary owners

| Code | Consuming test |
| --- | --- |
| `resource.coordinate-range-exceeded` | `coordinate_contract_covers_components_points_recovery_and_frames` |
| `resource.asset-bytes-exceeded` | `asset_byte_contract_covers_admission_authority_recovery_and_frames` |
| `resource.asset-work-exceeded` | `asset_work_contract_covers_intrinsics_nested_recovery_and_frames` |
| `resource.element-complexity-replaced` | `element_complexity_contract_covers_geometry_text_recovery_and_frames` |
| `resource.element-work-exceeded` | `element_work_contract_covers_generation_shaping_projection_and_frames` |

Each owner executes SVG, PNG and PDF. Reports compare every machine field,
including severity, effects, format, element ID, source pointer, complete detail
shape, configured limit, measurement kind/value, recovery, nested paths and recipe
identity where applicable. Messages must be nonempty; prose is not a semantic
pin. Fresh sessions must produce byte-identical artifacts and reports. Assertions
compare the entire ordered report, rather than filtering out secondary warnings.

Rejection cases also have independent artifact oracles. Authored empty text
boxes retain the expected canvas footprint for omitted geometry/glyphs; a
generic unsupported element supplies the specified shell without exercising the
resource crossing; missing-file scenes supply ordinary image recovery. Nested
coordinate/work placeholders are constructed from the frozen recipe manifest,
with surrounding transforms, opacity, clipping and a visible sibling. Omission
references remove only the rejected nested href. Comparisons cover complete PNG
pixels, the canonical embedded SVG subtree, and PDF drawing streams; resource
identities and mandatory-notice metadata are checked separately where applicable.

## Coverage assessment

### Coordinates

The owner combines the existing source-component, point, non-outline, image
authority, frame-subtree and effect tests with exact derived-value cases:

- source edges and translated bounds at N−1/N/N+1;
- Catmull-Rom controls computed independently from the point sequence;
- crop expansion and nonfinite arithmetic, including nested `at_least` details;
- two nested slots, exact node/attribute paths, repeated identity, omitted or
  independently admitted 668-unit placeholder recipes;
- authoritative scene files and supplements, saved/error statuses and deletion
  silence;
- source order despite opposite lexical IDs, fatal diagnostic prefixes, and
  direct/prepared frame success and typed viewport failure.

The implementation and clipping invariant remain documented in
[coordinate-admission.md](coordinate-admission.md).

### Asset bytes

The owner checks exact valid decoded payload lengths at N−1/N/N+1, source and
supplemental locations, escaped IDs, repeated referring elements, saved/error
recovery, zero-extent references, unreferenced/deleted silence, aggregate byte
precedence, diagnostic prefixes, and independent direct/prepared frame outcomes.

**Nested byte reachability:** nested data payloads are embedded inside their
authoritative SVG parent's admitted payload. Base64, percent encoding and XML
entity spelling cannot make that decoded child larger than its containing source
payload. Both use the same occurrence-byte ceiling. A local byte cap that would
reject the child therefore rejects the parent before nested discovery. The owner
tests child-size and parent-size N−1/N/N+1 values with positive and zero-sized
slots: rejection is attributed to the parent; an admitted parent has no nested
byte warning. This is a reasoned branch proof, not a fabricated nested report or
an ownership exclusion.

### Asset work

Validation, subsequent raster normalization and SVG sanitizer work have
independently calculated thresholds using payload octets, selected payload
passes, decoded pixels and XML record multiplicity. The owner checks top-level
raster/SVG boundaries and nested raster/SVG boundaries. A dense, short SVG child
crosses its sanitizer work limit while its larger but token-sparse parent fits.
The nested recovery envelope is tested immediately below and at 8,192 work units,
with and without a usable viewport. Exact effects distinguish raster loss from
potential SVG semantics. Source/supplemental order, repeated references, complete
report prefixes and frame scope/replay are included.

### Element complexity

Both fields sharing this code are owned: `max_geometry_segments_per_element`
and `max_text_scalars_per_element`. Cases cover a 16-command rectangle, a
zero-width four-command line, the nine-command compatibility box, multi-byte
Unicode scalar counts, and shaped glyph outlines. The glyph count comes from
the pinned Excalifont face through the font outline API, independently of the
renderer and diagnostic under test.

The assertions distinguish shell replacement, complete omission, fallback-label
scalar omission and post-shaping glyph omission. They include nonreciprocal text
and empty-content effects, aggregate geometry/text precedence, complexity-before-
work precedence, source ordering, diagnostic prefixes, and direct/prepared frames.

### Element work

Semantic crossings and format-projection charges are tested separately. The
rectangle's semantic work is 128 base units plus 16 commands × 32 = 640. PNG
additionally charges 16 samples over its 12×12 conservative stroked coverage
box, and a 10×10 placeholder 100×16 projection samples, but to total work only:
the element work boundary is the same in every format, and no PNG raster sample
crossing omits an element the admitted canvas contains.

Text tests cover pre-shaping work and complete glyph work. The pinned face's
commands and control-point bounds determine the expected PNG coverage at the
reference family's baseline; expectations are not learned from renderer reports.
That coverage and the one-pixel image's 24 projection, composite and mask
samples are read back as the element's own total-work events through the
public total-work ceiling. Compatibility fallbacks, empty text/geometry
effects, source order, report prefixes and frame replay are also covered.

## Ledger and execution

The repository's `record-test-owner` command recorded these 15 scoped owners;
`refresh-test-owner-source` refreshed existing consuming-test hashes after the
reviewed test edits. There are now **807 assigned/excluded obligations and 121
explicit gaps**, including **83/122** diagnostic/format assignments. All **30**
resource diagnostic/format obligations now have proposed primary owners. The
global ledger remains an unapproved draft; these assignments are not a release
approval.

Native and browser-WASM packaged-facade checks passed for the committed renderer.
Full inventory execution, approved native replay, and the still-failing #178
cross-target equality result are recorded in
[resource-recovery-target-evidence.md](resource-recovery-target-evidence.md).

## Verification and review

- All **43** resource-contract tests passed after strengthening the report and
  artifact oracles.
- `cargo nextest run --locked --workspace --no-fail-fast` passed **1,040** tests;
  eight existing ignored tests remained skipped.
- Workspace formatting, Clippy with warnings denied, and documentation tests
  passed under Rust 1.97.1.
- All 37 registered conformance cases and the catalog, inventory, ownership,
  font-subset and viewer evidence verification gates passed.
- Standards and Spec reviews initially requested complete reports for the
  clipping/cycle and fallback/missing-image coordinate scenarios, plus independent
  artifact expectations for recovery. Those changes were made and both closure
  reviews confirmed no remaining findings. The cycle fixture disables generated
  names so its good frame genuinely survives under the coordinate ceiling.
