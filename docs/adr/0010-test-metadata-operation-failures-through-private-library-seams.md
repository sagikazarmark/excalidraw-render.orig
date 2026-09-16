---
status: accepted
---

# Test metadata operation failures through private library seams

The metadata failure contract needs deterministic operation refusal, while
normal serializer inputs cannot reliably produce those failures. Classify errors
at the real metadata operation sites and exercise them through the public
renderer using scoped, thread-local hooks compiled only into library tests.
This keeps fault configuration out of the production request interface and
avoids rebuilding a different renderer inside an integration-test binary.

An explicit ownership adapter therefore supports unconditional root library
tests under exactly `cfg(test)`, with Cargo target-layout checks and exact
`--lib MODULE::FUNCTION -- --exact` selectors. This is a reviewed extension of
the existing fast-test adapter, not permission to accept arbitrary internal,
conditional or ignored tests. Operational fault evidence and ordinary input
reachability remain distinct claims; selecting an owner does not grant approval.
