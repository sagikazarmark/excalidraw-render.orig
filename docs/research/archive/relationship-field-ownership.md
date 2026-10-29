# Relationship and binding ownership (#85)

Base: `30c6a6ef0e27977c71d5b8ce7c5581609b1c5ea8`.

## Completed scope

Four primary selectors in `fast/relationship_field_contract.rs` own 45 obligations:
groupIds/frameId (6), boundElements/legacy boundElementIds and nested id/type (12),
start/end binding plus ordinary/fixed elementId/focus/gap (24), and fixedPoint
(3), all SVG/PNG/PDF. A fifth focused regression is invoked by the relevant
owners. Exact selectors, source hashes and rationales are in the ledger.

Evidence covers missing/default and malformed JSON/domain values, narrow recovery,
preserved original bytes, complete ordered reports, direct/lazy frame equality,
deleted/nonselected participation, first-alias/cycle behavior via the existing
identity contract, and group/clip semantics. Valid outside-frame targets expose
whether a recovered or surviving reference actually affects selection.

## Demonstrated fixes

- Fixed-point bindings now reject arrays longer than two components rather than
  silently reading their first pair. Short/malformed components use narrow
  pointers; missing/null bindings remain silently absent.
- Malformed grouping/bound metadata no longer forces a generic whole-element
  replacement; understood geometry survives with field-level recovery.
- Bound-element object records report missing/invalid members individually and
  retain valid sibling references. Malformed mixed non-object arrays substitute
  at the parent boundary without additionally diagnosing discarded children.
- Relationship selection now drops endpoints omitted by binding normalization;
  raw malformed source cannot keep selecting an outside target.
- Any present modern boundElements value suppresses legacy migration. Empty
  legacy ID strings no longer discard other resolvable string siblings.

The obsolete source-array-length flags used only by generic replacement were
removed. Source JSON preservation is independent of all these rendering repairs.

## Review and approval

Two-axis review found no hard Standards violations; pointer-role coupling and
repeated source-position arithmetic remain nonblocking design suggestions.
Spec review exposed parent/child diagnostic flooding and two legacy precedence
defects. Exact regressions now cover all three; the final review supports all
45 assignments with no remaining actionable finding in scope.

The maintainer's continuing “approve all” / “do it” authorization covers the
assignments and fixes. Totals: **739 assigned / 189 gaps** (38 surface and 151
other). Global #85 closure remains incomplete.

Targeted field tests, workspace/all-target typechecking and all ten canonical
Dagger conformance/lint checks pass. The final full workspace suite
(`dagger check excalidraw-render:test`) passed fast, render/parity and doctest
tiers in 18m8s.
