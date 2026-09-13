# Image diagnostic registry amendment: reapproval proposal

Base: `b8b5454`. Status: applied and verified as an unapproved candidate; **36 case reapprovals pending**.

Proposal bundle identity (SHA-256 of canonical JSON without its trailing LF):

`sha256:8c5fbc924eb5160ffed99ce2912632d5ae4c0dc0e2d597ac2597f8d18b8159d2`

[image-registry-reapproval.json](image-registry-reapproval.json) lists every exact case subject, manifest path, originating issue and prior approval. The bundle is a review identifier; each case still requires its own verified subject and scoped issue comment when approval is recorded.

## Changes

The scene-surface digest changes from `579e57f6a10e032ed4f460d3576c9be6bf4e86f595bd6a1acef2232bf22141ce` to `ae42d894a5acad8394405f8b1f0b4a2b29fc520506dc320b874512badcd77f3c`.

Only three `registryMappings` entries change:

- `asset.malformed-file-record`: narrower field diagnostics suppress a duplicate malformed-payload diagnostic. A file-owned recipe uses the first participating reference in canonical source order within the selected target; each referring PNG image still paints its own status-selected placeholder.
- `image.unsupported-format`: top-level rules remain, and nested failures admit `nested_source` details, removed-reference recovery and no recipe, preserving accepted surrounding SVG content.
- `image.decode-failed`: the same nested detail/recovery rules are added for failed nested decoding, including malformed nested data URLs. This shared emission path was reviewed alongside unsupported images to avoid leaving the same mismatch in the adjacent code.

The first-reference policy and nested-registry alignment were selected explicitly by the user in the preceding session. No production renderer change occurs in this authority-staging round. Conditional semantic-effects clauses are retained; the tested corrupt raster/data-URL failures select visual effects only. A blanket claim that semantic effects are impossible has not been made.

## Evidence preservation

All 36 case subjects compute through `xtask conformance approval-subject`. A structural comparison of each manifest against the base proves that only `reference.scene_surface.digest` and `approval` changed. Scenes, expected reports, reference artifacts, provenance, coverage, exceptions and comparison settings are unchanged. Original approvals are preserved in the proposal and Git history; candidate approvals are null.

The incorporated scene-surface artifact, catalog/generated catalog identity, spec identity table and verifier constants are refreshed. Runtime field-disposition tables are unchanged. Coverage inventory and owner-source descriptors are refreshed to the candidates; no new owner selection is made.

## Tests and checks

- Expanded exact nested tests cover unsupported payload bytes, invalid nested base64 and a corrupt PNG signature. Each is tested at two distinct nested paths, shared outer references and unreferenced silence in SVG/PNG/PDF.
- All 21 diagnostic-contract tests pass.
- Catalog materialization and verification pass.
- `dagger check excalidraw-render:lint excalidraw-render:conformance:catalog`: all three checks pass.
- `git diff --check` passes.

## Approval sequence

1. Approve the 36 exact subjects in the proposal bundle and record them on their scoped issues.
2. Refresh inventory/case-owner hashes to the newly approved manifests.
3. Refresh the rectangle PDF expectation's separate case-subject/manifest bindings and obtain its new #44 approval subject. Its tolerance and output expectations must remain unchanged unless separately justified.
4. Collect clean-commit native snapshots, compare all outputs with the approved baseline, and prepare the separate #85 baseline subject. Existing baseline bytes and approval remain untouched during this staging round.
5. Reassess the remaining nested-semantic and member branches before assigning broad diagnostic ownership.

Registered-case and ownership gates are intentionally approval-blocked at this checkpoint. Native baseline binding is stale after the inventory change. Global counts remain **149 proposed owners / 779 gaps**; this proposal does not approve snapshots, PDF expectations or global closure.
