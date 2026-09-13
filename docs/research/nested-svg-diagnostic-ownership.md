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
