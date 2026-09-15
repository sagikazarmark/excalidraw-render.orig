# Derived coordinate admission

This audit follows the coordinate checkpoint in
[`nested-resource-limit-amendment.md`](nested-resource-limit-amendment.md#coordinate-admission-checkpoint-ownership-still-open).
The governing contract is renderer spec §13: admitted source components alone
do not admit their derived geometry.

## Invariant and boundaries

For the request's coordinate ceiling `L`, an admitted semantic coordinate is
finite and has absolute value at most `L`. `render/coordinates.rs` owns this
predicate. It rejects NaN and infinity explicitly; neither a comparison with
NaN nor failure to construct `FiniteValue` can turn rejection into admission.
Finite crossings report their actual magnitude. Unrepresentable results report
`at_least: L.next_up()`, which is finite because the hard maximum is `1e12`.

The boundary checks are deliberately independent:

| Producer → consumer | Admission |
| --- | --- |
| Normalized element → family resolution | Source components and points, translated/rotated boxes, stroke dimensions, text line-box dimensions, image crop/projection dimensions |
| Frame → target selection | Source checks plus all rotated AABB edges and its width/height; runs for direct and prepared targets before selection |
| Rough generator → retained command | Every move/line endpoint and cubic control, including fills and decorations; coordinate failure preserves the generated-segment observation |
| Fill scan → subsequent scan/generation | Rotated scan points, actual scan spacing, active intersections, and returned endpoints |
| Freehand construction → path generation | Stroke size, stroke points and outline points; generated quadratics and background fills are checked before layout |
| Generated geometry → layout/placement | Local paths, translation intermediates, both reference rotation-centre conventions, and resulting element bounds |
| Text/generated label → scene bounds | Shape once, check source-space glyph positions, baselines and all quadratic/cubic controls before allowing the source to determine the export origin |
| Resolved image asset → scene bounds | Intrinsic source rectangle, normalized crop expansion, flipped content placement; a destination clip does not waive these checks |
| Scene origin → graph | Recheck translations, composed affine translations, transformed controls, masks, frame paths and generated-label placements |
| Sanitized SVG doubles → viewer | Lengths, viewBoxes, point lists, allocation-free path-token admission, relative and reflected controls, arc radii and corrected radii, ancestor transform composition |
| Viewer tree → asset publication | Expanded path controls, local/absolute bounds, f32 mapping, and subroots; object-bounding-box clips also apply the clip's separate transform |
| Graph → backend | Exhaustive element-family check covering paths, text, image projections and source rectangles, frame/corner clips and label masks |

Rejected ordinary text is a quarantined accounting record, not paintable
geometry. Its glyph counts and geometry observation still reach the ordinary
paint-ordered text accounting path and aggregate-fatal preflight. It is removed
before backend planning. Other already-produced rejected geometry carries its
resource observation in the same paint-ordered accounting stream.

## Why source-value admission is insufficient

The regressions exercise independent counterexamples:

- `x=75`, `width=50`, limit100: translated edge125.
- Origins −90 and +90, each with width10: export translation180.
- A centred 80×80 frame rotated π/4: every corner fits limit100 but its
  viewport dimension is approximately113.137.
- An admitted curved line produces a control outside its endpoint box; an
  admitted filled rectangle exceeds the ceiling only in scanline space.
- A 10-unit image with a 1/100 crop produces a 1000-unit content rectangle.
  A subnormal crop can produce an unrepresentable result.
- A small authored text box does not bound its later multiline glyph positions.
- SVG relative points, ancestor translations and object-bounding-box clipping
  can cross the ceiling while their individual operands fit.

Checking each producer's result before its next consumer closes these paths;
checking only final visible bounds would miss oversized controls, cropped image
content and clipping subtrees.

## Clipping argument

For a clipping inside/outside transition, the signed distances have opposite
signs. Therefore `t = d0 / (d0 - d1)` lies in `[0,1]`, and an intersection is a
convex combination of two admitted endpoints. `line_intersection` clamps both
the computed parameter and final endpoint rounding to that closed interval.
Consequently every Sutherland–Hodgman pass preserves the input coordinate
envelope, including near-parallel edges. Tests repeat clipping at the hard
coordinate maximum and with near-parallel transitions.

Squared distances, polynomial coefficients, scan slopes, angles and
dimensionless interpolation parameters are arithmetic, not additional scene
coordinates. Their coordinate-valued results cross the gates above. Backend
device/page sizing remains subject to the separate raster, SVG-dimension and
PDF-page limits in §13.

## Recovery closure

- A coordinate-rejected outline cannot influence scene bounds. Other existing
  local-limit layout behavior is retained.
- A placeholder is admitted independently. A rejected nested SVG placeholder
  is omitted within its slot, rather than making the parent asset fail.
- Layout recovery only removes occurrences and recomputes the origin. It
  terminates after at most the original occurrence count of successful removal
  rounds. Content rejected by shaping or intrinsic image admission is removed
  before it can displace an otherwise valid sibling.
- Frame rejection closes over active repaired clipping dependencies, including
  the placeholder-frame lookup used for fallback-only records. Disabled
  clipping does not create such dependencies.
- Omitted bound text leaves no arrow mask. Placeholder diagnostics are
  reconciled to omission, including removal of stale recipe IDs.
- The CJK subset trigger is frozen before local recovery, and discarded shaped
  text still participates in aggregate-fatal precedence.

## Executable evidence

`tests/fast/coordinate_recovery.rs` exercises the public SVG, PNG and PDF
facades, survivor artifact equality, finite/nonfinite measurements, source and
derived crossings, nested clipping, and direct/prepared frame outcomes.
`render/frame_batch_property.rs` additionally compares failure/release traces;
`render/resolve_tests.rs` covers the pre-recovery CJK trigger; the geometry unit
test checks the clipping-envelope invariant independently of the admission
predicate.

## Verification record

Validated with the repository's Rust 1.97.1 toolchain:

- The complete default workspace test set: 1,036 cases. The full run passed
  1,032; an obsolete source-overflow expectation and three stale ownership
  source-binding failures were corrected and all four reruns passed.
- After the final SVG filter-region guard: all 12 coordinate-recovery tests,
  31 nested-SVG integration tests and 18 backend-plan tests passed.
- Workspace formatting, Clippy with warnings denied, documentation tests and
  `wasm32-unknown-unknown` type-checking passed.
- All 37 registered conformance cases and all eight catalog, inventory,
  ownership and viewer/font-evidence verification gates passed. The existing
  draft ownership source digests were refreshed with the repository tool.
- Standards and Spec closure reviews confirmed that their identified blockers
  were addressed. Browser execution was not part of this verification run;
  the WASM check above is compilation, not cross-target artifact evidence.
