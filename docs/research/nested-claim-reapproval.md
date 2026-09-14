# Nested media-claim diagnostic: implementation and reapproval

Base: `d6ac707`. Implementation committed as `8e82246`. Status: **implementation and all 36 case subjects explicitly approved and recorded**; separate PDF/snapshot refreshes remain pending.

The user selected a new typed nested detail to preserve both source location and MIME values. `nested_asset_claim` carries, in order: asset chain, paths, claimed media type, validated media type and `repaired` recovery. It is emitted under `asset.media-type-mismatch` with `input_repair` effects and no recipe. The path contains every enclosing image node/href pair. Claimed/validated strings are required, since this event occurs only after successful nested validation.

The sanitizer now retains media repairs separately from unsafe removals, propagates their paths through recursive/cache replay, and emits them once per intrinsic nested occurrence. Final report ordering includes their nested paths. Ordinary top-level asset detail serialization is unchanged.

## Regression evidence

The first exact test failed with `image.unsafe-svg-content-removed`, a synthetic node-zero path and `removed` recovery. It now passes with the new detail and correct types/path. A recursive two-sibling test checks two distinct paths despite shared nested bytes and repeated outer references; scene/supplemental pointers, unreferenced silence and frame standalone/batch equality are covered. Correcting only the MIME claim produces byte-identical artifacts in SVG/PNG/PDF, proving the content is preserved.

All 40 diagnostic-contract tests, 15 sanitizer tests and six typed-registry tests pass. Dagger formatting, clippy and catalog checks pass. `git diff --check` passes.

## Authority changes

- Scene-surface schema admits `nested_asset_claim`: SHA-256 `df77f6d2cfc8dc5f77cfa216170d9adca1d59af00b5a0955f40f640627fd1ddf`.
- Scene-surface registry adds the nested claim scope/recovery to `asset.media-type-mismatch`: SHA-256 `4cc295860c1627485717ba1292add5ee6c28c7115ffda1019122ffff5d4c46cb`.
- Spec §11.3 defines the new closed detail shape and binding. Generated catalog and pinned authorities are refreshed.
- All 36 affected case manifests change only the scene-surface digest and clear their old approval. Scenes, reports, output artifacts, provenance and tolerances remain untouched. Every new case subject validates through the ordinary approval-subject command.

## Requested case reapproval bundle

`sha256:7988f03334cbe1b9ce1a050a7c20336b05b06cbedd0e9001071caef3a214f770`

This identifies canonical JSON (without trailing LF) in [nested-claim-reapproval.json](nested-claim-reapproval.json), containing each exact case subject and prior approval. Recording requires explicit approval of those subjects on their scoped issues; the implementation choice alone is not an evidence approval.

After case approval, refresh case-owner/inventory bindings, the separate rectangle PDF expectation subject and the native snapshot baseline using fresh packaged collection and byte comparison. Those approvals are independent. The extended media-type owner candidate calls the nested tests, but its historical scoped approval does not approve the expanded authority by itself.

Global totals remain 166 proposed owners / 762 gaps. Unsafe-content ownership still needs preparse DTD/entity/instruction boundary review; this round does not close those obligations. Approval-dependent case/ownership gates and downstream stale PDF/snapshot bindings remain pending at this staging checkpoint.

## Recorded approval

The maintainer explicitly approved the implementation and entire bundle on 2026-09-14. Each current case subject was recomputed and matched before recording through the ordinary verifier. Scoped comments:

- [#103](https://github.com/sagikazarmark/excalidraw-render.orig/issues/103#issuecomment-5661539427)
- [#152](https://github.com/sagikazarmark/excalidraw-render.orig/issues/152#issuecomment-5661539892)
- [#153](https://github.com/sagikazarmark/excalidraw-render.orig/issues/153#issuecomment-5661540449)
- [#121](https://github.com/sagikazarmark/excalidraw-render.orig/issues/121#issuecomment-5661540804)
- [#58](https://github.com/sagikazarmark/excalidraw-render.orig/issues/58#issuecomment-5661541021)
- [#112](https://github.com/sagikazarmark/excalidraw-render.orig/issues/112#issuecomment-5661541293)
- [#124](https://github.com/sagikazarmark/excalidraw-render.orig/issues/124#issuecomment-5661541531)
- [#32](https://github.com/sagikazarmark/excalidraw-render.orig/issues/32#issuecomment-5661541783)
- [#177](https://github.com/sagikazarmark/excalidraw-render.orig/issues/177#issuecomment-5661542011)
- [#137](https://github.com/sagikazarmark/excalidraw-render.orig/issues/137#issuecomment-5661542268)
- [#110](https://github.com/sagikazarmark/excalidraw-render.orig/issues/110#issuecomment-5661542536)

Inventory and owner hashes are refreshed to the approved case manifests. This records the scoped approvals without granting global closure or transferring approval to new downstream PDF/snapshot subjects.

After recording, ownership verification passes (166 assigned/excluded, 762 gaps), all eight Dagger conformance checks pass, and `git diff --check` passes.
