# Asset claim and supplement diagnostic ownership

Initial base: `e7009e6`. Supplement-conflict ownership is scoped-approved; the subsequent media-type completion below adds three proposed owners.

## Media claims

The exact `media_claims_report_each_authoritative_field_once` test covers both scene `mimeType` and data-URL claims against validated PNG bytes, in source-member order, with repeated references. The initial test failed with each field warning duplicated for each referring image. Runtime emission now checks the authoritative pointer, file ID, claim and validated kind before retaining another identical occurrence. Different file IDs and fields remain independent even when their payload bytes match.

Further cases cover file-source order opposed to reference order, escaped IDs, supplemental root pointers and lexical order, valid/deleted/unreferenced silence, and scene/supplemental frame replay.

An exploratory malformed-domain probe remains unresolved: with a valid PNG payload and `mimeType: "not-a-mime"`, normalization emits `asset.media-type-mismatch` with null validated type, then resolution emits the same code/pointer/claim with `image/png`. `mimeType: null` emits one null/null warning; `IMAGE/PNG` and `image/png;foo=bar` emit a strict-domain normalization warning despite the later case/parameter-normalized comparison matching. Resolve those overlapping stages and the authority's intended domain semantics before claiming complete media-type ownership. These observed reports were not installed as approved expectations.

## Supplement conflict owner

`diagnostic_contract::supplement_conflicts_report_authority_once_in_every_format` executes exact full reports for SVG/PNG/PDF. It covers identical and differing supplemental bytes, repeated references, escaped IDs and file-source order opposing reference order, valid scene precedence with unchanged artifact bytes, malformed/missing/unsupported scene payload precedence, and deleted/unreferenced silence. A request-aware frame helper verifies selected-file diagnostics, outside-file silence, fresh standalone equality and batch replay.

The current conflict contract diagnoses competing authority whenever a referenced scene entry and supplemental entry share a file ID, including identical bytes. A request-only asset has no competing scene authority and produces no conflict (the combined helper checks this silence). This establishes why the registry's root alternative does not require an invented request-only conflict.

Counts: **163 proposed owners / 765 gaps**, including **44 diagnostic owners / 78 diagnostic gaps**. The three new assignments await scoped approval. Global ownership remains unapproved.

Verification: all 29 diagnostic-contract tests pass; ownership validation passes; all ten Dagger conformance/lint checks and all three Dagger workspace test checks pass. The shared-reference media test failed before the fix with four diagnostics instead of two. Temporary malformed-domain probes were removed and preserved as findings above. `git diff --check` passes.

## Scoped maintainer approval

The maintainer explicitly approved the three supplement-conflict assignments on 2026-09-13, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5656670825). The approval identifies commit `2b68e411c88ec4eaecf79071e6d241b6fc13c161`, binding the dispositions, selector, source digest and rationales. Media-type ownership remains a gap; the incomplete global ledger retains null approval with 765 gaps.

## Media-type completion

Follow-up base: `7cce93e`. The user explicitly selected preservation of the approved strict scene-MIME domain. Uppercase/parameterized/out-of-domain scene values still receive a repair warning; supplemental claims retain their existing case/parameter-normalized equivalence behavior. No registry or approval-bound evidence change is required.

Successful payload validation now enriches the existing strict-domain diagnostic's `validated_media_type` in place, preserving its owner, pointer, order, message and one diagnostic resource charge. The subsequent exact-claim check recognizes the enriched occurrence, preventing a duplicate. Null/nonstring claims also receive the validated type. If payload validation fails, the type stays null; it is not inferred from an unvalidated claim.

`diagnostic_contract::media_type_contract_in_every_format` is the proposed owner for all three formats. It includes the existing two-field/source/supplemental ordering matrix and adds PNG/JPEG/GIF/WebP/BMP/ICO/SVG validated-kind vectors, invalid/nonstring/empty claims, uppercase/parameterized scene repair, equivalent and absent supplemental silence, unsuccessful payload validation, repeated references, deleted silence and frame standalone/batch equality. Full ordered reports and fresh artifact/report bytes are asserted.

The initial invalid-claim regression failed because validated PNG bytes left a null validated type. After enrichment, both that failure and the duplicate malformed-string warnings are resolved. Counts: **166 proposed owners / 762 gaps**, including **47 diagnostic owners / 75 diagnostic gaps**. The new media-type assignments await scoped approval; global ownership remains pending.

Final verification: all 31 diagnostic-contract tests pass; ownership validation passes; all ten Dagger conformance/lint checks and all three Dagger workspace test checks pass. `git diff --check` passes. Approved registry and snapshot evidence are unchanged.

### Scoped media-type approval

The maintainer explicitly approved the reporting changes and three media-type assignments on 2026-09-13, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5656883187). The approval identifies commit `02c247506d6282869b35ff0a1182869ee20ab556`, binding the implementation, dispositions, selector, source digest and rationales. The incomplete global ledger retains null approval with 762 gaps.
