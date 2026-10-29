# Duplicate identity effects follow-up (#85)

Base: `c3f2468117c40cc6f8e674b20ae6cf80d1b8a401`.

## Demonstrated defect

Normalization emitted every `normalization.duplicate-element-id` diagnostic with
only `input_repair`. The frozen registry additionally requires visual/semantic
effects when disambiguated relationships affect those channels. Three public
regressions failed before correction: duplicate-frame clipping, reciprocal
bound-text placement, and endpoint-binding frame selection.

Resolution now enriches existing occurrences using the resolved first-occurrence
authority and current target's consumers. Scene diagnostics retain their original
order and resource charge. Pending frame diagnostics are scoped after selection
and before their normal retention/charging; prepared and standalone paths use the
same decision. SVG/PDF add visual and semantic together for these consumers;
PNG adds visual only. Unconsumed identity repair stays input-only.

The source IDs and generated-ID algorithm are unchanged. Original scene
embedding retains duplicate source IDs. Explicitly disambiguated clean scenes
serve as artifact oracles; qualified exact ID vectors remain in the adjacent
admission contract.

## Channel assessment

- Frame membership affects scene bounds/clipping when enabled, including bounds
  of empty text, and affects opacity only for paint consumers. Frame export
  selection is independent of persisted scene clipping controls.
- Text container disambiguation affects both surviving placement and omission
  when the first container lacks reciprocity.
- Reverse `boundElements` and legacy `boundElementIds` select the first text;
  later nonempty duplicates can lose reciprocity even if the first text is empty.
- Endpoint bindings use persisted geometry for scene exports but participate in
  frame selection. Group IDs occupy a separate namespace and do not select an
  element merely because their string equals a duplicate element ID.
- Deleted and selection records do not displace the first live owner. Later
  duplicates have independent repaired IDs, in source order.
- Retained compatibility placeholders consume frame opacity/clipping too.
  Empty or suppressed text does not consume opacity, while scene bounds remain
  a separate membership consumer.

## Tests and review

Ten public-facade tests cover these branches in SVG/PNG/PDF. Tests compare full
machine-readable reports, complete artifact bytes against clean scenes, embedded
originals, fresh calls, and standalone/lazy frame equality. Text-loss and
placeholder cases also run through reversed fresh frame target order.

Two-axis review found unchecked graph indexing and additional channel branches.
Graph lookups now use checked access. Regression follow-ups cover empty-text
opacity silence versus clipping/bounds effects, nonreciprocal-first text loss,
and all empty/nonempty first/later text combinations. The Spec reviewer confirmed
the demonstrated production blockers resolved. Repeated graph scanning and
private string-based channel classification remain architecture follow-ups.

## Ownership status

The following describes the previous checkpoint; the six assignments are now
completed as recorded below.

The six broad `/id` and duplicate-ID obligations remain explicit gaps. This
batch improves implementation and evidence, but does not yet establish the full
cycle-removal interaction and resource-boundary/trace matrix required for those
assignments. The ledger remains **544 assigned / 384 gaps**. Only the edited
admission-test source binding is refreshed; no new owner is claimed.

The maintainer's continuing “approve all” instruction covers the implemented
correction and these tests. It does not turn partial evidence into complete
ownership. #85 remains open.

## Verification

- Targeted duplicate-identity and admission-identity tests pass.
- Workspace/all-target typechecking passes.
- All ten canonical Dagger conformance/lint checks pass on the final code.
- Full final workspace suite: `dagger check excalidraw-render:test` passed fast,
  render/parity and doctest tiers in 13m14s.
- The host's newer Clippy initially reported unrelated comparator chunk-iterator
  lints; the canonical pinned Dagger Clippy passes. New collapsible-if findings
  were corrected.
- Two-axis review: unchecked indexing corrected; all demonstrated Spec
  production defects corrected and rechecked. Remaining ownership and repeated
  scan/representation concerns are explicitly retained above.

## Ownership completion

Follow-up base: `3afa93fd51070f5ea340d729266970577f793faa`.

The remaining focused questions are resolved:

- First-occurrence alias resolution can create or avoid a cycle. Authored
  reports establish duplicate/cycle order, the closing-edge pointer and separate
  effects, with a clean disambiguated/cycle-free artifact oracle. Selected-frame
  execution agrees with lazy batch execution.
- A diagnostic-count failure during normalization retains the input-repair-only
  report accumulated before relationships resolve. A later output failure
  retains enriched effects. No future output channels are claimed at the early
  crossing. Both have repeatable complete serialized reports and descriptors.
- Per-target diagnostic budgets remain independent. Output N−1/N/N+1 probes
  verify failure versus success and standalone/lazy equality.
- `render::frame_batch_property::duplicate_identity_cycle_and_resource_boundaries_replay_exact_traces`
  uses the existing private trace seam to compare complete cold/prepared traces,
  outcomes, failure locations and reports at work boundaries around normalization
  and relationship events, diagnostic ceilings, peak memory and output limits.
  These are fresh relative comparisons, not historical trace pins (ADR-0002).

The single executable primary owner is
`duplicate_identity_contract::identity_and_duplicate_relationship_contract_in_every_format`.
It executes all existing duplicate scenarios, the new cycle/failure contracts,
and the missing/invalid identity qualification helpers. The admission module is
independently source-bound by existing owners; the trace test executes in the
ordinary library test tier. No production change was required in this follow-up.

The six `/id` field and duplicate-ID diagnostic obligations now have owners in
SVG, PNG and PDF. Totals: **550 assigned / 378 gaps** (227 surface and 151 other).
The user's continuing approval authorization covers these six selections, whose
exact executable, source binding and rationale are recorded in the ledger.
Global coverage closure remains incomplete.

Two-axis review: 0 Spec findings; 0 hard Standards violations and one low-priority
test-helper duplication suggestion. The Spec review explicitly supports all six
assignments within the requested cycle/resource scope.

Completion verification: the exact consolidated owner and focused trace test
pass; workspace/all-target typechecking and all ten canonical Dagger
conformance/lint checks pass. The final full workspace fast/render/doctest suite
passed in 13m24s. No snapshot or renderer producer changed in this completion.
