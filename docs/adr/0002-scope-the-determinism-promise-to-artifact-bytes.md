---
status: accepted
---

# Scope the determinism promise to artifact bytes and diagnostics

Resource limits and the semantic ledger stay. What changes is how the canonical
resource trace is held stable: as a cross-target equality property rather than
as pinned constants checked into the repository.

Pinned per-cell `canonicalResourceTraceSha256` values in the SVG font subset
qualification corpus, and the pinned trace digests in the resource-trace
qualification test, charged a refactoring tax on every accounting change while
proving nothing the equality asserts do not already prove. Each supported target
now computes trace digests fresh, and the `aggregate-svg-font-subset-targets`
step compares them across all five targets. The aggregate additionally writes
the cross-target trace digest to a committed evidence file, so a trace change
stays a reviewable diff that is updated mechanically instead of by hand.

The relative equality asserts (cold, warm, request-cache, and frame-batch paths
agree) and the absolute envelope-delta spot-checks (`working_bytes`,
`total_work_units`) remain. Per-cell artifact-byte pins
(`generatedWoff2Sha256`) also remain: they are the wasm-versus-native
nondeterminism canary — the class of check that caught issue #98 — and artifact
bytes, not trace bytes, are what the compatibility contract promises.

For bisecting an aggregate mismatch, the qualification test gains a diagnostic
mode that dumps per-cell trace digests.

## Journal replay stays

The frame batch path replays its normalization and relationship journal event by
event instead of charging one lump sum. That was measured, not assumed: with
lump-sum charging, `frame_batch_property` still agrees on artifact bytes,
reports, and first-crossing locations, but the canonical resource trace
diverges between the batch and standalone paths. Cross-target trace equality is
the determinism property this ADR keeps, and the revision-1 contract requires
the trace to be exact across the cold, warm, request-cache, and
frame-batch-cache paths alike, so the replay stays.
