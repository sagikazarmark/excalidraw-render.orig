# Nested SVG diagnostic ownership follow-up

Base: `ef941db`. Both forbidden-resource and unsafe-content families retain their six explicit ownership gaps.

## Fixes established by exact tests

The two-level SVG event-handler regression initially failed: its diagnostic lost the outer image node/href path and incorrectly declared a visual effect. The sanitizer's recursive branch appended child removals verbatim; removal records had no parent-slot path. They now retain enclosing image slots, prefixing each recursion level before the diagnostic builder emits the complete node/attribute path.

Event handler attributes (`on*`) cannot affect static appearance and now select semantic-only effects. Visual-plus-semantic removals now serialize effects in registry order (`visual`, then `semantic`). The existing sanitizer effect-class test was updated for `onload`/`onclick`; foreign content still selects both effects.

New public tests cover direct handler versus foreign-content removal, exact details and path ordinals, repeated outer-reference multiplicity, unreferenced silence and frame standalone/batch equality. The recursive handler test compares its PNG bytes with an explicitly cleaned document and checks cleaned-document silence in every format. Forbidden-resource tests record the existing bounded-placeholder versus zero-extent-omission branches with escaped file pointers and recipe identity.

## Remaining review

- Complete unsafe boundary categories: DTDs/entities, processing instructions, stylesheet/declaration rules, animation, unsupported namespaces, other attributes and deeper sibling-order cases.
- The forbidden-resource constructor currently always adds `semantic`, while the registry makes that conditional on lost nested semantics. Nested external hrefs currently emit `image.unsafe-svg-content-removed` rather than the registry's forbidden-resource placeholder/omission mapping. Reconcile these paths before assigning ownership.
- Bounds selection scans referring source geometry; deleted/nonselected and mixed-bounds references need exact tests before claiming smallest-occurrence recovery.
- The recursive removal path fix preserves path identity, but canonical ordering across sibling nested assets still needs its own adversarial test (local ordinals alone do not establish a total nested-source order).

These tests and fixes are partial progress, not coverage closure. Totals remain **166 proposed owners / 762 gaps**. Approved registry and evidence files are unchanged.

Verification: the exact recursive-handler regression failed before the fixes and passes afterward; all 15 sanitizer unit tests and 34 diagnostic-contract tests pass. Ownership validation passes, as do all ten Dagger conformance/lint checks and all three Dagger workspace checks. `git diff --check` passes and temporary probes are removed.

## Scoped approval of implemented fixes

The maintainer explicitly approved the changes in commit `4a3b15b608022dcb9c7e10917d4cacf3bbc583b8` on 2026-09-13, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5657151285). This approval covers the path/effect fixes and partial test evidence; the six ownership obligations remain gaps and the incomplete global ledger retains null approval.

## Nested ordering and category follow-up

Base: `fff481b`. An adversarial pair of nested SVG siblings failed: the first child removed local node 3, the second removed local node 0, and the report incorrectly listed the second first. Image diagnostics now retain their actual authoritative field position rather than adding child-local ordinals. Final sorting compares complete nested path segments before pointer/code/retention order, covering both removals and nested image failures. A mixed failure/removal test proves that producer loop order does not override source-path order. Comparison allocates no path copies.

Category tests cover DTD removal, declared entity references, stylesheet processing instructions, scripts, animation, unsafe stylesheet content, foreign namespaces, unsafe style attributes and uppercase event handlers. Safe static CSS with local gradients remains silent. Supplemental paths, repeated references, deleted/unreferenced silence and frame/batch equality are asserted explicitly.

Two effect regressions are fixed: script removal is semantic-only because scripts do not execute in static rendering; removing a resource href changes visual appearance, while removing an action link on `a` is semantic-only. Both tests failed against the previous classifier before the change. Stylesheet processing instructions remain visual-plus-semantic because the omitted stylesheet can affect appearance.

### Remaining ownership blocker

`sanitize_nested_data_image` still appends a `Removal` named `media-type` at node zero whenever the nested data URL's claim differs from the validated type. It preserves and canonicalizes the image payload, yet emits `image.unsafe-svg-content-removed` with removed recovery and semantic effects. Reconcile this claim-repair path with media-type diagnostics and the real nested resource slot before claiming the full unsafe-removal obligation. Preparse DTD/entity/instruction records also use node-zero identity; complete narrowest-boundary semantics need review. No additional obligations are assigned in this round.

Verification: the new ordering and category regressions pass, along with all 15 sanitizer tests and six diagnostic-order unit tests. All ten Dagger conformance/lint checks and all three Dagger workspace test checks pass. Ownership remains 166 assigned/excluded with 762 gaps. `git diff --check` passes.

The maintainer explicitly approved this follow-up on 2026-09-14, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5657363810) against commit `d9901e6f28bf2ff37dafa875b1b972b2375c7572`. The approval covers the fixes and partial test evidence, not complete ownership of the remaining nested SVG obligations.

Next follow-up: the user selected a new typed nested MIME detail. The implementation now separates media repairs from removals and preserves full paths and MIME values; see [the implementation and reapproval subjects](archive/nested-claim-reapproval.md). Preparse boundary identity remains to be reviewed.
