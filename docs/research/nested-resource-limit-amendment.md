# Nested resource-limit contract amendment (#85)

Base: `a83724a9b2767f089265b26107b8c4b0c58df030`.

The user explicitly selected **Add nested detail (Recommended)**, authorizing
`nested_resource_limit` and affected schema/registry/case reapprovals. The prior
implementation reported nested resource exhaustion as `nested_source/removed`,
losing the configured limit and measured crossing and silently dropping safe
viewport content instead of the prescribed nested-resource placeholder.

## Implementation

The new closed detail carries field, configured ceiling, value kind, measurement,
asset chain, full nested paths, placeholder/omitted recovery and recipe identity.
Its paths participate in the existing canonical diagnostic ordering. Non-resource
decode/sanitization removals retain their existing detail.

Nested image slots select their own finite viewport, resolving px, absolute units,
percentages, nested viewBox and default 100% SVG viewports. Unrepresentable or
zero-sized slots omit. A root SVG rejected before a child is discovered uses the
current normalized image occurrence's bounds, not another reference's bounds.
The fixed official image-error geometry is embedded as a resource-free SVG;
existing source transforms, opacity and clipping remain in force. Mandatory
Font Awesome notices are merged into the artifact manifest.

The spec records the closed recovery construction envelope (8,192 work units and
8,192 temporary bytes), retained blob accounting and root viewer admission.
Insufficient local recovery work selects omission. Cached parent replay retains
every recovery occurrence, including transitive cached descendants. Buffers use
fallible reservations; whole-report and nested-path identity remain deterministic.

## Reapproval

Scene-surface digest: `93029f2dab63c203dee69da4497ea4c2988a72d73f2a0017e62c529a4caac202`.
Schema digest: `c8af2987f69e320bbc2673e252284fda77a18ddaf667bf998055c47cd3aa7341`.
All 36 affected case subjects were recomputed and recorded through the normal
approval command on their scoped issues. Scenes, expected reports, visual
artifacts and tolerances did not change. The rectangle PDF binding-only subject
is `sha256:9b8a1cbcbcd524029f49c5765797ee4d7f378e5ab532e679cf02352dcd8273f7`,
approved in [#44](https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5670966345).
The execution inventory and ownership authority descriptors are refreshed.

## Evidence and remaining work

Focused tests cover top-level asset byte/dimension/pixel boundaries, canonical
SVG passthrough silence, independent-frame asset-byte reports, optional metadata
output boundaries, coordinate omission, exact nested-depth details, root
occurrence bounds/local omission and required notices. The private trace seam
proves four recoveries across two cached nesting levels each pay and produce
identical standalone/prepared-frame artifacts, reports and traces.

Review found and corrected recovery allocation/accounting, current-owner bounds,
transitive cache journal replay, viewport defaults and tiny-dimension handling.
No resource ownership is claimed merely by these initial tests. The full ten-code
matrix, particularly all local-element branches
and nested raster limits, remains to be completed before the 30 assignments.
The global ledger remains 777 assigned / 151 gaps.

## Native baseline refresh

Clean implementation checkpoint: `d96cddd`. Canonical packaged collection passed
in 4m21s: 168 keys, each rendered twice in separate forward/reverse processes
(672 renders). Staging verified source archives and Git ancestry.

All 168 artifact/report samples and all 187 blobs (6,146,996 bytes) match the
previous approved baseline exactly. All 168 input bindings changed. Only the
baseline metadata and its sidecar were installed; output blobs are unchanged.

Exact subject `sha256:177a9f4a2f901659189dac9b10e6e766e061cee2023024905a860f5bb51783aa`
was approved under the continuing scoped authorization in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5671646910).
`verify-native-baseline` and canonical packaged snapshot replay pass (5m50s).
This approval does not assign any resource owners or close the remaining gaps.

The implementation's full-suite run passed render and doctest checks, with one
stale internal diagnostic assertion failing in the fast tier. That assertion
was corrected in `d96cddd`, and the complete canonical fast tier then passed.

## Follow-up review and semantic effects

Review reproduced top-level SVG work rejection incorrectly selecting nested
recovery. Both source-byte and sanitizer-record work crossings now use ordinary
top-level recovery. The public-facade regression first failed, then passed for
SVG/PDF omission and both PNG source-status recipes, with independent missing-file
artifact comparisons. The normative schema digest table was also reconciled.

The user selected **Conservative effect (Recommended)** for pre-inspection nested
SVG rejection. SVG/PDF now add `semantic` for rejected SVG/unknown content;
recognized raster signatures remain visual-only and claims cannot override bytes.
A fixed 48-byte decoded prefix bounds classification. PNG remains visual-only.
The exact depth-report regression failed before implementation and passed after;
additional tests cover wrong MIME claims, unknown content and both recoveries.

Nested raster dimension/pixel tests cover repeated source slots, exact complete
ordered reports, N-1/N/N+1 ceilings, bounded/unbounded recovery and SVG passthrough
silence. Nested raster work tests additionally cover the 8,192-unit recovery
envelope boundary and exact normalization-work crossings. These focused tests
extend the evidence but do not yet justify assigning the full resource matrix.

Two suspected accounting violations from Standards review were retracted after
validation: distinct payload occurrences legitimately share intrinsic work, and
the deterministic recovery trace identifier does not deduplicate charges. The
remaining review observations are heuristic duplication/naming improvements.

Local element recovery now adds semantic effects only for participating vector
content or live ordinary/generated text in SVG/PDF. Empty geometry, empty text
and nonreciprocal bound text do not claim lost source semantics. Regression tests
cover degenerate safe/unsafe links, empty-text work ceilings 31/32/33 and rejected
nonreciprocal text. Existing ordinary/generated text contract expectations were
updated; their font diagnostics and artifact expectations are unchanged.

Final follow-up review reports no definite remaining defect in these changes.
All 14 resource-contract tests, existing nested SVG tests and cache/frame trace
checks pass. Workspace/all-target typechecking and all ten canonical
conformance/lint checks pass. The complete fast/render/doctest suite passed in
the 22m56s combined run. Packaged replay in that run correctly refused dirty
renderer sources; it must run again after this checkpoint is committed.
