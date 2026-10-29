# Frame controls and names ownership (#85)

Base: `5b7816a7bfb6156b6cd4076dde18958878d542c7`.

## Completed scope

Two public-facade selectors in `fast/frame_field_contract.rs` own 21 obligations:
the app-state frameRendering object (3), its enabled/clip/name/outline members
(12), and ordinary/magic frame names (6), all in SVG/PNG/PDF. Exact selectors,
source hashes and rationales are recorded in the ledger.

Control evidence covers absent/default values, invalid JSON classes, unknown
descendants and exact global reports even for empty scenes or disabled output.
All sixteen persisted combinations are compared with equivalent request
controls, then with three conflicting explicit request sets against the logical
AND contract. Existing clip/outline geometry and pixel assertions supply positive
semantics. Frame exports enforce their own crop/name/outline scope independently
of persisted scene flags.

Name evidence covers absent/null/empty/valid/long/malformed values for both
frame kinds, source preservation, visible text and three-dot truncation, grey
name pixels, generated-font diagnostics, deleted silence and request/persisted
enabled/name suppression. Scene and metadata-enabled frame exports preserve
the complete original document; direct and lazy artifacts/reports agree.

## Demonstrated fix

The persisted `name:false` regression initially failed: painting was disabled,
but normalization still emitted an invalid-name warning. Effective name
participation now combines the request with persisted enabled/name controls.
Missing/invalid booleans retain the default true interpretation; explicit false
disables participation. The effective gate is shared by classification and
per-kind normalization. Tests cover both disabled flags with malformed and
missing-glyph names; request-level disabling and frame-export silence remain
independently checked.

## Review and approval

Two-axis review found no hard Standards violations; its default-policy/fixture
reuse/naming suggestions are nonblocking. Spec findings prompted the explicit
conflicting-control matrix and metadata-enabled frame preservation assertions.
The final Spec review supports all 21 assignments with no remaining essential
findings in scope.

The maintainer's continuing “approve all” / “do it” authorization covers these
assignments and the demonstrated participation fix. Totals: **694 assigned /
234 gaps** (83 surface, 151 other). Global #85 closure remains incomplete.

Targeted facade tests, workspace/all-target typechecking and all ten canonical
Dagger conformance/lint checks pass. The full suite passed render/parity and
doctests but exposed two old internal expectations that request controls could
override persisted name disabling. Those assertions now follow the facade's
specified conjunction; both targeted tests and the entire fast tier passed on
rerun (13m39s). All three workspace tiers are verified.
