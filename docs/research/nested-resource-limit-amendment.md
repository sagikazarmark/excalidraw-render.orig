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
matrix, particularly conditional semantic effects, all local-element branches
and nested raster limits, remains to be completed before the 30 assignments.
The global ledger remains 777 assigned / 151 gaps.

Native baseline recollection and final verification follow this implementation
checkpoint because staging requires a clean named commit.
