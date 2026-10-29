# Animation-flattening diagnostic ownership

Base: `349b3a8`. Proposed owner for PNG/PDF obligations:
`diagnostic_contract::animation_flattening_contract_in_every_format`.

The exact selector covers two-frame APNG, GIF and WebP, both scene-file and supplemental authority. Two live references to the same bytes produce two diagnostics in source order (element IDs oppose lexical order), with informational severity, visual effect, asset identity/validated MIME, flattened recovery, null recipe and referring `/fileId` pointers. Deleted references are silent.

Static PNG/GIF/WebP and one-frame animation containers are silent. APNG's one-frame case has a separate default image, proving that default plus one animation frame is not counted as a multi-frame animation. Direct SVG reports no flattening. Unreferenced supplements and files referenced only by deleted elements are silent.

Frame selection retains two inside references and excludes an outside reference to the same file. Fresh standalone frame reports/artifacts equal batch replay. Full scene assertions also compare fresh artifact/report bytes.

A PNG element-work limit omits the occurrence and reports only the resource failure; the same limit leaves PDF image rendering intact and therefore retains its flattening diagnostic. An intrinsic asset-byte rejection suppresses flattening in every format. Exact resource reports prevent silent loss or duplicate diagnostics from hiding behind a code-only assertion. PNG animation artifacts match a static PNG of the expected first animation frame for each codec.

No renderer implementation or approved evidence changes are required. The two assignments are proposals, not global approval. Totals: **160 owners / 768 gaps**, including **41 diagnostic owners / 81 diagnostic gaps**.

Verification: the exact animation selector, ownership validation and approved-baseline validation pass. All ten Dagger conformance/lint checks and the Dagger fast tier pass. `git diff --check` passes; temporary resource-report probes were removed.

## Scoped maintainer approval

The maintainer explicitly approved the two PNG/PDF assignments on 2026-09-13, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5656100492). The approval identifies commit `6a5824b4d496098028935c0629b4434804e80d4a`, binding the dispositions, selector, source digest and rationales. The incomplete global ledger retains null approval with 768 gaps.
