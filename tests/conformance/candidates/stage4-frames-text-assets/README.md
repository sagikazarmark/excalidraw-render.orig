# Representative scene candidate — Stage 4

Project-authored frames/clips, text, raster and nested SVG, links and embedded
metadata. This candidate is materialized and tested independently of the approved
case registry. Approval is null; the 168 existing execution keys are preserved.

Collect/verify reference evidence with `tools/reference-oracle/run-stage4-representative.sh`.
After native artifacts are rendered, `node tools/reference-oracle/scripts/stage4-representative.mjs seal`
binds the scene, all evidence and authored reports in the manifest. Obtain the
approval subject with `cargo run --locked -p xtask -- conformance candidate-approval-subject tests/conformance/candidates/stage4-frames-text-assets`.

The native SVG clips framed images according to the spec's existing canvas
authority divergence; official SVG's namespaced clip attribute is retained as
reference evidence, not mistaken for the native clip semantics. No new tolerance
exception or approval is asserted.
