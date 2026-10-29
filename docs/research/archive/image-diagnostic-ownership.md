# Image diagnostic ownership follow-up

Initial base: `9eb8059`. After the follow-ups and final assessment below, all nine missing-file/malformed-file/unsupported-format obligations have proposed owners. Global approval remains pending.

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

## Mixed-status/frame follow-up

The follow-up based on `02153f2` adds exact reports for boolean/number/string/array file records and malformed base64, including unreferenced silence. A two-frame scene shares one file between a saved image in the first frame and an error-status image in the second. Null records, missing payloads, malformed base64 and unsupported payloads are tested in all three formats and both source-reference orders, with standalone/batch equality.

The null-record PNG test failed: whole-scene normalization selected the error recipe even for the frame containing only the saved image. After frame selection, pending malformed-file diagnostics now reselect their typed image recovery from the retained normalized references. Original pointer, owner, message and order metadata are preserved. The same logic applies to replayed batch diagnostics; all new frame tests pass.

### Concrete remaining findings

1. **Null payload overlap.** Render an image referring to `file` with `files: {"file":{"id":"file","created":1,"dataURL":null}}`. Every format emits both `asset.malformed-file-record` (element owner `image`) and `compatibility.unsupported-property-value` (null element owner) at `/files/file/dataURL`. Both claim replacement/omission of the same payload. Resolve narrower-cause suppression and the PNG field recipe before assigning the malformed-file obligations. This output was observed, not installed as an approved expectation.
2. **Reachable nested contract mismatch.** Use a valid SVG data URL whose body is `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><image href="data:image/png;base64,aW52YWxpZA==" width="40" height="30"/></svg>`. All formats emit `image.unsupported-format` at the outer `/files/file/dataURL`, null element owner, effects `[visual]`, details kind `nested_source`, one asset-chain entry `file/0`, path node ordinal 1 / attribute `href` ordinal 0, recovery `removed`. The registry currently admits only asset details and top-level placeholder/omitted recoveries for this code. Nested malformed base64 instead emits `image.decode-failed` with the same nested path/recovery. Thus nested unsupported behavior is reachable and cannot be dismissed as an exclusion.
3. **Whole-scene mixed-status policy.** File-classification diagnostics currently select error if any live referring source is error; runtime payload diagnostics select the first referring occurrence. A single file-owned recipe cannot describe two different per-reference recipes without an explicit rule. The new frame tests establish the unambiguous single-status-per-target case; global mixed-status semantics still need a contract decision.

The six broad obligations remain gaps with these concrete reasons. Temporary report probes were removed; no passing expectation endorses the unresolved null/nested output. The global totals stay 149 proposed owners / 779 gaps.

Subsequent follow-up: the null-payload overlap is fixed; the user selected first-reference status for shared-file diagnostics and registry alignment for nested removal. Scene/frame tests cover the implemented selection and exact nested paths. See [the decisions and authority proposal](image-diagnostic-decisions.md); full ownership remains pending the registry/evidence amendment and nested-semantic branch review.

Follow-up verification: all 18 diagnostic-contract tests pass, ownership validation passes, all ten Dagger conformance/lint checks pass, and all three Dagger workspace test checks pass. The previously stale rectangle PDF binding is now approved and its fast check passes. `git diff --check` passes.

Current totals: **149 proposed owners / 779 gaps**, with **30 diagnostic/format owners / 92 diagnostic gaps**. The prior scoped font approval is unaffected; the new missing-image assignments remain proposals and global ownership approval is pending.

## Verification

- All 16 `diagnostic_contract` tests pass, including the deleted-status regression that failed before the fix.
- `verify-ownership`: 928 assignments, 149 assigned/excluded, 779 gaps; global approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks pass.
- `dagger check excalidraw-render:test`: render and doctest checks pass. Fast tier: 867 passed, one failed, eight skipped by tier configuration.
- The sole failure is `pdf_renderer_expectations::focused_pdf_expectations_stay_bound_to_their_case_and_dependencies`: `rectangle-outline/pdf-renderer-expectations.json` still binds case subject `sha256:9744b816d8689d53db597fddc383b784b64dafe5006ab6374642a3328d2cb6ef`, while the reapproved case subject is `sha256:96638d660453deb84fa5f4d1844fc758c2b40c6c1b01450b0a7929455d816dcc`. This is a remaining downstream binding from the font-registry refresh, unrelated to the image change. Its separately approved expectations and dependency descriptors require a reviewed reapproval update; the test and existing signature were not weakened.
- `git diff --check` passes. Temporary report probes were removed.

## Final ownership assessment

Follow-up base: `5e49bbe`. The approved registry amendment and separately approved PDF/native baseline refreshes resolve the prior authority and binding blockers. Two unconditional fast-test selectors now execute the complete family matrices:

- `diagnostic_contract::malformed_file_contract_in_every_format`
- `diagnostic_contract::unsupported_image_contract_in_every_format`

Both call same-file helpers for source/supplemental authority and ordering, escaped pointers, multiplicity, first-reference status, target scoping and standalone/batch equality. The malformed-file owner includes invalid record types, missing payloads, data-URL separator/base64 failures, narrower field suppression, and deleted-reference silence. The unsupported owner includes nested paths and both JPEG/WebP invalid-ICC paths: direct SVG preserves those payloads silently, whereas PNG/PDF reject their color profiles with the declared recovery.

### Conditional semantic-effect reachability

The registry's conditional clause is retained. The reviewed paths do not lose admitted nested text/link/animation/vector semantics:

- `asset/payload.rs::scene_payload` returns `Malformed` for unavailable fields, invalid data-URL syntax and failed base64 before creating a payload. Supplemental assets are already byte vectors and cannot take this syntax path. File-record classification likewise precedes payload admission.
- Nested malformed URLs are converted to `SvgDecodeFailed` in `asset/svg.rs::sanitize_nested_data_image`; they do not emit `asset.malformed-file-record`.
- `asset.rs::Cache::resolve` dispatches SVG-looking payloads to the sanitizer before raster sniffing. Unknown signatures produce `Unsupported` without admitting nested content. The other reviewed `Unsupported` paths are JPEG/WebP ICC profile parsing, which discard raster color interpretation rather than nested semantic content.
- In nested SVG, `safe_href` admits data payloads only on `image`/`feImage`; a rejected image reference leaves its containing SVG and siblings intact. Text/link sanitization uses separate removal diagnostics. Recursively propagated unsupported paths retain the same rejected-reference boundary.

The new two-level nested test checks exact path segments and compares PNG bytes against an explicit repaired document with only the failed href removed. The sibling text survives byte-for-byte in the raster. This is an applicability argument and a positive preservation test within the obligations, not a blanket exclusion of nested semantic effects from future renderer paths.

No production code or approved evidence changed in this final assignment round. The six assignments are proposals, not global approval. New totals: **155 proposed owners / 773 gaps**, including **36 diagnostic owners / 86 diagnostic gaps**. `image.decode-failed` remains a separate incomplete decoder-matrix obligation despite the nested decode samples reused here.

Final verification: all 25 diagnostic-contract tests pass; ownership validation and approved native baseline verification pass; all ten Dagger conformance/lint checks and the Dagger fast test tier pass. `git diff --check` passes, and temporary ICC probes were removed.

### Scoped maintainer approval

The maintainer explicitly approved the six malformed-file/unsupported-image assignments on 2026-09-13, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5655207357). The approval identifies commit `560955e9908a6dbb1168834da61b556f5ac7c671`, binding the exact dispositions, test selectors, source digest and rationales. The incomplete global ledger retains null approval with 773 gaps; this scoped record does not approve unrelated assignments.
