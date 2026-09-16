# Known-element replacement reachability

## Revision-1 reachability conclusion

For the current revision-1 kind catalog and renderer dispatch, a successful
public render cannot emit `compatibility.known-element-replaced`. The generic
fallback remains a defensive branch for future supported kinds. Painting the
`renderer.known-element-unavailable.v1` box does **not** establish reachability
of this diagnostic: malformed point data selects that recipe under the narrower
`compatibility.unsupported-property-value` diagnostic.

This conclusion covers SVG, PNG and PDF, scene and frame targets. It is a
control-flow argument over the current closed kind set, supported by executable
boundary tests; a finite collection of absent-code assertions alone is not the
proof. The three ownership entries require explicit #85 exclusion approval.

## Surface-classification producer

`scene_surface::classify` emits the generic code only when a kind has a current
element policy, is not `static-export-irrelevant`, and
`SceneDocument::element_at(index)` returns `None`.

- Non-object records and absent/empty/non-string types select malformed-element
  recovery before this condition.
- Unrecognized nonempty type strings select unsupported-element-type recovery.
- `selection` is explicitly irrelevant and skipped.
- All twelve supported policy kinds have exact arms in `SceneDocument::element_at`:
  rectangle, diamond, ellipse, text, line, arrow, freedraw, image, frame,
  magicframe, iframe and embeddable. Their fields are represented as
  `SourceField::{Missing,Invalid,Value}`; a bad field does not make `element_at`
  return `None`.
- Legacy `draw` has its own recognized reader arm and explicit surface branch.

Thus the surface producer's conjunction is false for every current kind.

## Semantic-resolution producer

`RenderSession::resolve_scene` reaches the generic candidate only when
`!is_outline || !source.direct_outline_compatible`. It emits the diagnostic only
if no retained narrower replacement/omission already applies.

1. Frames and text take their dedicated branches before the generic candidate.
2. Rectangle, diamond, ellipse, iframe and embeddable are direct-compatible
   outlines by the closed match in normalization. No field can change that flag.
3. Linear and freehand elements are also direct-compatible. They cease to be
   outlines only when `geometry_available` is false. Every such path has a
   malformed-points cause:
   - linear arrays with an invalid participating point emit a precise point or
     component diagnostic before returning unavailable geometry;
   - freedraw invalid arrays emit the field diagnostic; invalid participating
     points emit the precise point/component diagnostic;
   - missing/empty/short inputs that restore to defaults or no-ops retain
     available geometry, so they do not enter the generic candidate.
   The narrower diagnostics carry `visual` and placeholder/omitted recovery,
    satisfying `diagnostic_replaces_source` for the precise points subtree.
    Zero-area boxes select omission/null recipe before diagnostic retention and
    still suppress the generic code. Binding-field omissions cannot suppress
    painting of a bounded point placeholder: they replace only the field.
4. Images resolve to an image or a typed image fallback/omission. Null file IDs
   and intrinsic raster limits return an `Outline` disposition directly.
   Remaining unsuccessful image resolutions have the source-owned missing,
   malformed, unsupported, decode, forbidden, or resource diagnostic. The generic
    candidate is suppressed by `diagnostic_replaces_source`, which matches
    source/asset ownership and exact authoritative payload pointers. A successfully resolved asset
   bypasses the generic candidate entirely.

This exhausts `NormalizedElementKind`; legacy draw normalizes into the linear
family. Recovery for geometry/work ceilings has its own resource code and does
not route back through the generic candidate.

## Budget and target qualifications

The diagnostic preflight retains the first crossing event. Exhausting the
diagnostic, memory or work envelope exits through a typed fatal resource error;
it does not return a successful artifact with an unrecorded missing primary
cause. `normalize_scene` failure and allocation/encoding failure likewise do not
emit this generic compatibility code. Frame selection changes participation,
not the kind-reader or semantic dispatch rules above; direct and prepared paths
use the same resolution implementation.

Authoritative payload matching compares escaped pointer bytes without allocating.
The earlier allocating comparison converted a one-shot allocation refusal into
`false`, which could drop a retained narrower cause at the generic guard. Review
identified that counterexample; allocation-free matching removes it rather than
assuming the refusal must persist through later diagnostic construction.

## Executable evidence and invalidation

`fast/compatibility_recovery_contract.rs` provides:

- `known_kind_dispatch_and_image_failures_have_no_generic_fallback`: a guard
  against supported-kind catalog expansion, direct rendering for every current
  kind and legacy draw, and complete reports for representative image failures;
- `known_fallback_is_suppressed_by_exact_narrower_causes`: complete reports and
  independent replacement/omission artifacts for malformed line, arrow, draw
  and freehand points with both positive and zero-area bounds;
- `mixed_property_recovery_order_and_suppression`: multiple field failures,
  malformed records, a separate unsafe-link cause and a fatal report prefix;
- `field_omission_keeps_required_point_placeholder`: independently authored
  reports and shell artifacts for invalid binding element IDs, gaps and focus
  together with malformed line/arrow points;
- the composed unsupported-value owner executes all current field-family
  matrices, including all image failure/authority and point-structure branches.

Re-review this proof if the kind catalog, `element_at`, normalization's
direct-compatibility match, geometry-availability conditions, diagnostic
preflight, replacement predicates, or image/scene dispatch changes. No code or
obligation is removed by this document, and no exclusion is approved merely by
its presence.
