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

Checkpoint `b5c4191` passed clean packaged replay in 5m53s; scoped approval is
recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5672655461).

## Depth-sensitive cache follow-up

A proposed depth-owner probe exposed a production defect: the same nested SVG
sanitized at a shallow depth was reused deeper without enforcing the descendant
depth ceiling. Reversing discovery order could instead reuse a rejected result
at a shallower admitted edge. The facade regression failed with no diagnostic
and now passes both orders in all formats, with independent shallow-red/deep-grey
PNG pixel assertions.

Sanitized variants are keyed by source identity and current depth. Intrinsic work
remains shared by source identity through fallibly reserved bookkeeping. Review
identified duplicate direct-SVG raster validation when rebuilding depth variants;
a trace regression failed with four admissions instead of the three distinct
SVG/SVG/PNG identities, and passes after deduplicating validation work. Cached
recovery occurrences remain separately charged. Follow-up review found no
remaining definite defect in this correction.

The depth matrix now additionally covers exact N-1/N/N+1 reports, source and
supplemental authority, multiple nested paths, repeated scene references,
independent frame scope/batch replay and deleted silence. Full depth ownership
still needs multi-owner/conflict cases and independent SVG/PDF recovery oracles.
The full 16-test resource module, typechecking and all 13 canonical full-suite,
conformance and lint checks pass (20m40s). No new ownership is claimed yet.

## Nested-depth ownership

The subsequent evidence-only follow-up selects
`resource_diagnostic_contract::nested_depth_contract_covers_owners_paths_frames_and_silence`
for all three `resource.nested-asset-depth-exceeded` format obligations. It
unconditionally executes the complete depth, effect, recovery, authority, cache
and frame contract. Multi-owner file ordering differs from reference ordering;
unreferenced and conflicting payloads are silent. An additional hidden resource
beyond the rejected edge proves recursion stops before occurrence admission.

Independent recovery evidence derives the frozen image-error glyph paths from
the authority manifest and places them in a worked 30x20 viewport (size 8,
offset 11,6). It compares nested SVG structure, all PNG pixels plus literal
grey/white/blue samples, and nonempty PDF drawing streams, while checking
transforms, opacity, clipping, unaffected siblings and mandatory notices. Omission
matches a separately authored removed-slot scene at the 8191-unit envelope limit.
Both discovery orders test depth-sensitive cache results.

Spec review supports these three assignments with no materially missing criterion.
Current totals: **780 assigned / 148 gaps**; **27 of the 30 resource assignments
remain open**. This selection does not claim completion of the ten-family batch.

Verification: the exact selected owner, workspace/all-target typechecking,
ownership consistency and all ten canonical conformance/lint checks pass.
The final full fast/render/doctest suite passes (16m19s). Standards review reports
no definite violations; fixture duplication and naming remain heuristic notes.

## Nested-node ownership

The next evidence-only follow-up selects
`resource_diagnostic_contract::nested_node_limits_cover_root_and_child_reports_and_silence`
for SVG/PNG/PDF `resource.nested-svg-complexity-exceeded`. It runs exact root and
child boundary reports, finite/unbounded recovery, current-root occurrence bounds,
the recovery-work envelope, source/supplemental and multiple-owner ordering,
frame/deleted silence, and the independent manifested artifact oracle.

A five-node fixture independently counts root, whitespace, desc, its text and
trailing whitespace at limits 4/5/6; comment and attribute controls are not nodes.
Over-limit root and child SVGs contain another invalid image, with aggregate
asset-entry ceilings proving node rejection stops discovery before that image.
The existing depth owner retains its full scenarios through shared test helpers.
Spec review supports all three node assignments without material missing criteria.
Current totals: **783 assigned / 145 gaps**; **24 resource assignments remain**.

## Raster dimension and pixel ownership

The next six assignments select `image_dimension_contract_covers_codecs_recovery_and_scope`
and `image_pixel_contract_covers_codecs_recovery_and_scope` in the resource contract
module, each for SVG/PNG/PDF. They cover six codecs (including selected multientry
ICO dimensions), N-1/N/N+1 limits, saved/error status, source/supplemental authority,
top/nested and multiple-owner scope, frame replay and deleted/conflict silence.
SVG passthrough is explicitly silent. Nested 8191/8192/8193 recovery-envelope
boundaries compare omission against an independently removed slot; placeholder
geometry reuses the independent manifested SVG/PNG/PDF oracle.

The user explicitly approved **Once per file (Recommended)**: intrinsic top-level
dimension/pixel failures emit once per authoritative file ID, with null element ID,
while each reference keeps its own status-selected recovery. A public-facade
regression failed before implementing this separation. Mixed saved/error references
now compare against independent ordinary missing-file recovery artifacts.

Another red/green regression established width-before-height precedence for
simultaneous crossings; top-level and nested resolution previously measured the
larger side. Side crossings also precede per-asset pixel crossings.

Spec review supports all six assignments and reports no must-fix production issue.
Standards review finds no definite violations. Workspace/all-target typechecking,
focused owners, all ten canonical conformance/lint checks and full fast/render/doc
suite pass (19m47s combined). Current totals: **789 assigned / 139 gaps**;
**18 of the requested resource assignments remain open**.

## Optional-metadata ownership

The next three assignments select
`optional_metadata_contract_covers_components_limits_and_frames`, which
unconditionally runs eight public-facade scenarios. Each optional component has
exact output boundaries. Work vectors cover generation, measurement and retention;
scratch vectors cover generation and output-buffer retention. The vectors derive
canonical scene/zlib/wrapper/manifest lengths and use normative record/work costs.
PNG prepays 3200 empty-raster units before metadata and reserves the remaining
1600 emission units with retention.

Ordered multi-component omissions preserve accepted output, continue to later
components, retain mandatory notices, and keep a nonempty pre-metadata summary
frozen. Required output and diagnostic budget failures retain exact report
prefixes. Different frame names yield independent acceptance under one limit,
with atomic embedded-scene omission and direct/batch artifact/report equality.
The companion trace regression proves generation/measurement rejection commits
no rejected delta and preserves only prior work. No production change was needed.

Spec review supports all three assignments; Standards review has no hard
violations. Focused tests, typechecking, all ten conformance/lint checks and the
full fast/render/doctest suite pass (19m47s). Current totals: **792 assigned /
136 gaps**, with **15 resource assignments remaining**.

## Coordinate admission checkpoint (ownership still open)

The coordinate audit found normalized text, images and frames bypassing the
outline-only guard. The public-facade regression failed with no diagnostic for
text at x=21 under limit20. A shared normalized-coordinate check now rejects
these occurrences before their content-specific resolution.

The user approved scene invalid-frame subtree omission and an explicit fatal
`resource.coordinate-range-exceeded` for an invalid standalone frame viewport.
The spec and public error constant record the fatal resource-limit details with
null recovery. Scene descendants are omitted only through repaired, active frame
clipping dependencies; cycle-removed edges and disabled clipping do not remove
unaffected children. Omitted surface fallbacks retain omission diagnostics with
null recipe, and their existing compatibility-omission accounting applies.

Image semantic effects use bounded authoritative-payload classification so
unlinked SVG content is not visual-only, while absent linked images do not claim
lost semantics. Scene authority wins over supplements. Invalid frame checks run
after journal replay but before viewport selection and do not clone a doomed
prepared target. Direct/prepared frame outcomes, reports and traces match.

Review reproduced and corrected raw-frame-ID traversal, disabled-clip subtree
loss, SVG image effects, stale placeholder recipes, and an infallible clone in
the initial prepared-target guard. Focused regressions and typechecking pass.
The full fast/render/doc suite and conformance checks passed; the single Clippy
loop-style finding was corrected and lint passed. Comprehensive derived control
point/transform/intermediate admission and coordinate ownership remain unfinished.
No coordinate assignment is claimed by this checkpoint: totals stay 792/136.

### Derived-coordinate recovery follow-up

The source-only checkpoint is superseded by the admission and recovery work
documented in [coordinate-admission.md](coordinate-admission.md). It covers
generated controls and scanlines, source and rotated bounds, export transforms,
image crops/intrinsic dimensions, text and generated labels, nested SVG paths
and clip transforms, and the clipping-envelope invariant. Coordinate omissions
cannot leave unsafe bounds or orphaned text masks. Rejected shaped text retains
aggregate-fatal preflight, and the CJK trigger is evaluated before recovery.
The existing source-member diagnostic ordering and fatal frame-target contract
remain covered alongside derived-bound direct/prepared replay regressions.

### Resource ownership completion

The final coordinate, asset-byte, asset-work, element-complexity and element-work
owners are assessed in [resource-diagnostic-ownership.md](resource-diagnostic-ownership.md).
All 30 resource diagnostic/format obligations now have proposed primary owners;
the global ledger is **807 assigned / 121 gaps**, pending global approval.
Native/browser-WASM execution and the remaining #178 equality failures are
recorded in [resource-recovery-target-evidence.md](resource-recovery-target-evidence.md).
