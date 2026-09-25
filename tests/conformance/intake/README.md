# Representative intake review queue

These bundles are pending review, not registered conformance cases or approved
evidence. `manifest.json` binds immutable source bytes, license/provenance and
the proposed applicability plan. `eligible_executions` is always empty.

Prepare and verify bundles with `cargo run --locked -p xtask -- representative
prepare|verify` (see `tools/representative-intake/README.md` for arguments).
Bundles remain observations. Promotion requires reference and viewer checks,
independently authored expectations, normal code review, and deliberate
integration with the case registry and execution inventory. There is no separate
issue-comment approval ceremony for new representative cases.
