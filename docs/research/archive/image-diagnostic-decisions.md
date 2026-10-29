# Image diagnostic decisions and narrower-cause fix

Base: `e1f4ba5`. The user selected both decisions in the implementation session:

1. A single file-owned diagnostic uses the first participating reference's status in canonical source order within the selected target. Each PNG reference still paints its own status-selected placeholder.
2. Extend the registry to admit existing nested unsupported-image removal: `nested_source` details, `removed` recovery, no recipe, and effects based on removed content.

## Implemented and tested

The null-payload regression initially reported both `compatibility.unsupported-property-value` and `asset.malformed-file-record` for `/files/file/dataURL`. Runtime suppression only recognized asset-shaped details. It now recognizes a replacing field diagnostic at the exact escaped authoritative payload pointer. The same matching rule preserves image-specific recovery instead of creating a generic element box. Null, boolean, numeric, array and object payload values are covered; an overlapping file-ID prefix is a negative test against suppressing another file's failure. Saved/error PNG artifacts match their status-selected missing-image equivalents, and duplicate references and frame batches preserve the narrower diagnostic.

Malformed-record recipe selection now uses the first live source reference instead of any error-status reference. Frame selection uses the earliest normalized source index, matching runtime payload diagnostic ownership. The test covers null/missing/malformed/unsupported payloads, both status orders, whole-scene and frame exports, and standalone/batch equality.

Nested unsupported-image tests cover two rejected nodes with distinct canonical paths and repeated references to the outer SVG. They emit two intrinsic diagnostics, not four, and unreferenced SVG assets are silent. Exact reports preserve the outer asset pointer, asset chain, node/attribute ordinals, visual effect and removed recovery.

## Authority proposal

[image-diagnostic-contract-proposal.json](image-diagnostic-contract-proposal.json) records replacement registry rows against the current authority digest. Proposal SHA-256 (canonical JSON without trailing LF):

`cb1a875bd95749581f2965f97dd793c44dddae40f742a8373ab998935c6c9250`

This is a decision/amendment proposal, not a case-approval subject. The active scene-surface authority and its existing approvals have not been rewritten. Applying it requires regenerating catalog/case bindings, preparing the affected case subjects and downstream PDF/snapshot binding refreshes, and obtaining approval of those exact subjects. The existing conditional semantic-effects clause is preserved; tests here establish the visual-only unsupported raster boundary, not every nested semantic-content case. Related nested decode-failure registry scope should be reviewed in the same authority round to avoid another partial schema correction.

Subsequent staging: the amendment and adjacent nested decode-failure rules are now applied, and all 36 affected case subjects are explicitly approved and recorded. See [the reapproval record](image-registry-reapproval.md). Original approvals are preserved in the bundle; separate PDF and snapshot binding refreshes remain pending.

The six malformed-file/unsupported-format obligations remain explicit gaps pending that authority and remaining branch review. Global ownership stays at 149 proposed owners / 779 gaps.

## Verification

- The null-payload and mixed-status exact tests failed before their respective fixes and pass afterward.
- All 21 diagnostic-contract tests pass, including exact nested paths and the negative pointer-prefix case. Additional null-payload artifact/frame assertions pass.
- `verify-ownership`: 928 assignments, 149 assigned/excluded, 779 gaps; approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks pass.
- `dagger check excalidraw-render:test`: fast, render and doctest checks all pass.
- `git diff --check` passes; no temporary logging remains.
