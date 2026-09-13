# Image diagnostic ownership follow-up

Base: `9eb8059`. Three missing-file format obligations have proposed owners. The six malformed-file/unsupported-format obligations retain specific gaps.

## Exact report coverage

`diagnostic_contract::image_authorities_have_exact_ordered_diagnostics` covers SVG/PNG/PDF with both saved/error source statuses:

- File-source order `z/~, a/~` opposes lexical and reference order `a/~, z/~`; repeated references use distinct element IDs and diagnose each authoritative file once.
- Null records and records missing `dataURL` have exact distinct escaped pointers and null element owners.
- Unsupported scene payloads retain authoritative `/files/.../dataURL` pointers and first-referring element ownership.
- Supplemental payloads have root pointers, lexical file-ID order opposed to reference order, and first-referring owners.
- Valid scene and supplemental images are silent for these failure codes; unreferenced records and payloads are silent.
- A scene entry remains authoritative over a valid same-ID supplement, including null/missing/unsupported payload cases. Exact full arrays include the separate supplement-conflict diagnostic and its correct order.

`diagnostic_contract::missing_image_contract_in_every_format` calls the existing status/multiplicity regression plus authority/silence scenarios, and adds missing/null/empty IDs, escaped IDs, source order opposed to element-ID order, frame-selected ownership and nonselected silence. Its frame helper compares exact reports and artifact bytes across fresh standalone calls and batch replay. It is the proposed executable owner for all three `image.missing-file` obligations.

## Deleted-status defect

One live saved image and one deleted error-status image referenced the same null file. PNG incorrectly reported the error recipe for the live saved occurrence. `referenced_image_has_error_status` scanned deleted source records even though they are silent. It now rejects `scene.element_is_silent(index)` before considering status. The exact all-format regression failed before the change and passes afterward. This is a source-participation fix; no registry, approved snapshot or reference expectation was changed.

## Remaining gaps

Malformed records still need the complete malformed-member matrix, mixed-status live shared-file rules and frame scoping. Unsupported payloads likewise need mixed-status/frame coverage. Both registry entries declare conditional semantic effects for lost nested semantics; establish the reachable nested paths (or an evidence-backed inapplicability argument) before assigning these broader obligations. The passing top-level matrix is partial evidence for those six rows.

Current totals: **149 proposed owners / 779 gaps**, with **30 diagnostic/format owners / 92 diagnostic gaps**. The prior scoped font approval is unaffected; the new missing-image assignments remain proposals and global ownership approval is pending.

## Verification

- All 16 `diagnostic_contract` tests pass, including the deleted-status regression that failed before the fix.
- `verify-ownership`: 928 assignments, 149 assigned/excluded, 779 gaps; global approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks pass.
- `dagger check excalidraw-render:test`: render and doctest checks pass. Fast tier: 867 passed, one failed, eight skipped by tier configuration.
- The sole failure is `pdf_renderer_expectations::focused_pdf_expectations_stay_bound_to_their_case_and_dependencies`: `rectangle-outline/pdf-renderer-expectations.json` still binds case subject `sha256:9744b816d8689d53db597fddc383b784b64dafe5006ab6374642a3328d2cb6ef`, while the reapproved case subject is `sha256:96638d660453deb84fa5f4d1844fc758c2b40c6c1b01450b0a7929455d816dcc`. This is a remaining downstream binding from the font-registry refresh, unrelated to the image change. Its separately approved expectations and dependency descriptors require a reviewed reapproval update; the test and existing signature were not weakened.
- `git diff --check` passes. Temporary report probes were removed.
