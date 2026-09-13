# Font diagnostic ownership follow-up

Follow-ups to `4d6fb67`, under #85. The three font codes cover nine diagnostic/format obligations. All nine now have a proposed executable owner after the fixes, authority reapproval, snapshot reconciliation and final assessment below. Global ownership approval remains pending.

## Authority correction history

Spec §8.3, updated by #175, requires Liberation Sans substitution diagnostics for ordinary Helvetica text and compatibility-placeholder labels, but **silence for frame names and embeddable labels**, whose face is renderer-owned chrome. Noto substitution diagnostics apply to ordinary text and embeddable labels. The registry initially described a broader scope; the correction and all 36 affected case subjects are now approved. The tests follow the explicit spec silence rules.

## Implemented corrections

- **Embeddable Noto recipe:** a public render of an embeddable with `link: "https://example.com/😀🎉"` produced a Noto diagnostic with null recipe. The exact test failed. The typed registry now has an embeddable-specific Noto variant carrying `official.embeddable-offline@sha256:d1d9207be2a4022886b9916ef0068b654c47f64d9ea59cd6d31d6432c5eecb01`; ordinary text retains null recipe.
- **Omitted Helvetica silence:** preflight emitted the substitution before reciprocal-bound-text admission and post-layout local recovery. Emission now occurs for a surviving resolved text occurrence after local-limit checks. An unreciprocated `containerId` test proves that omitted text contributes no font diagnostics.
- **Frame-name missing glyphs:** frame shaping already records missing scalars in the final truncated label, but accounting never emitted them. The exact `A\u{10000}B` frame-name test failed with an empty report. Accounting now emits one missing-glyph diagnostic per retained scalar under the frame owner, with source ordering and null-recipe missing-glyph details. Hidden names and scalars removed by truncation are silent.

`fast/diagnostic_contract.rs::font_diagnostics_cover_text_generated_labels_and_silence` exercises full authored ordered reports and repeated artifact/report bytes in SVG, PNG and PDF. Scenarios cover:

- ordinary Helvetica/direct-Liberation text, multiple emoji per occurrence, repeated missing scalars in logical order rather than numeric order, and source IDs opposing lexical order;
- empty, deleted and unreciprocated text silence;
- frame-scoped text ownership, nonselected text silence and standalone/batch equality;
- plain ordinary/magic frame names and iframe/embeddable chrome substitution silence;
- missing scalars in frame names, embeddable labels and compatibility labels;
- hidden/truncated frame names and too-small compatibility-label silence;
- selected default-ignorable controls (bidi marks, zero-width characters, combining grapheme joiner, Arabic letter mark, variation selector and BOM).

## Soft-hyphen SVG failure and fix

The broader default-ignorable test exposed an independent SVG subset validation failure. Individual control probes isolated it to **U+00AD**, without any bound-text omission or other control characters. The following scene with a default SVG request fails with `encode.svg-font-subset-failed`, stage `validate`, resolved family `Liberation Sans`, root pointer, and an empty compatibility report:

```json
{"type":"excalidraw","version":2,"elements":[{"id":"control","type":"text","text":"A\u00adB","fontFamily":9,"fontSize":20,"width":500,"height":40,"updated":1}],"appState":{},"files":{}}
```

The failed asset is `fonts/Liberation/LiberationSans-Regular.woff2@sha256:006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8`. Expected behavior under §8.3 is successful rendering with no diagnostic for the soft hyphen. The initial loop stopped at SVG; PNG/PDF behavior for this isolated fixture has not been independently asserted.

The follow-up based on `0467847` reproduces the failure with U+00AD alone. Mapping, advance and outline validation succeed; the shaping comparison fails because the source returns one hidden zero-advance space glyph per soft hyphen, while the subset returns no glyphs. HarfBuzz uses the nominal U+0020 glyph to hide default-ignorables, and removes them if that mapping is absent. The subset plan retained requested Unicode mappings but omitted this implicit shaping dependency.

`font_subset::derive` now retains the source's space mapping whenever the requested scalars include a default-ignorable and the source has a space glyph. The semantic scalar input and identity remain unchanged; the extra mapping is shaping closure. Existing strict mapping, metric, outline, cluster and position validation stays intact. The condition also covers default-ignorables absent from the source cmap, since the shaper still hides them through space. Ordinary `AB` subsets do not gain a space mapping.

The public regression `soft_hyphen_render::soft_hyphen_renders_without_diagnostics` covers U+00AD alone and `A\u00ADB`, with success, silence and repeatability in SVG/PNG/PDF. The font-level test `default_ignorables_keep_the_space_dependency_and_hidden_clusters` checks soft hyphens and a joiner, source/subset shaping agreement in both directions, retained hidden glyph cardinality, zero advance and no outline, plus the no-default-ignorable negative case. This failure would have been caught by a qualification vector with a default-ignorable but no literal space.

### Qualification refresh required

The fix changes `font_subset.rs`, a producer bound by `tests/conformance/svg-font-subset-viewer-qualification/revision-1.json`. Its existing approval does not cover this producer change. No approved descriptor, byte pin or approval record was rewritten.

Verification of the follow-up:

- Both focused regression tests pass.
- Dagger conformance/lint: nine checks pass; `conformance:svg-font-subsets` rejects the stale producer digest.
- Dagger workspace tests: render and doctest checks pass; fast runs 865 tests with 864 passing and one failure, the same offline viewer-qualification stale-digest check (eight tests skipped by the configured tier).
- Temporary diagnostic probes were removed; `git diff --check` passes.

The expanded corpus and both viewer reports have now been regenerated and approved, with matching clean-commit native/browser-WASM qualification and packaged-consumer evidence bound as `aggregated` / `pass`. See [the completed refresh record](soft-hyphen-qualification-refresh.md). The soft-hyphen qualification blocker is resolved.

## Work identified at the initial checkpoint

- Reconcile registry wording with §8.3/#175 before assigning broad source scopes.
- Cover the full default-ignorable boundary classes, including non-ignorable Hangul fillers and paragraph separators; the soft-hyphen fix is qualified and approved.
- Complete local-resource omission and generated-label branch matrices, including frame/placeholder Unicode behavior and format-specific zero-size embeddables.
- Only then assign complete test owners. Global totals remain **137 proposed owners / 791 gaps**.

## Boundary and omission follow-up

The follow-up based on `c9e10ac` adds authored endpoint vectors for every default-ignorable class listed in §8.3, including U+00AD, supplementary musical controls and tag/variation-selector ranges. Hangul fillers U+115F/U+1160/U+3164/U+FFA0 and U+2029 are checked as non-ignorable missing scalars. These all pass in SVG/PNG/PDF with full ordered report and repeatability assertions.

Zero-height embeddables now have exact format-specific tests: PNG omits the generated label and its font diagnostics; SVG/PDF retain the Noto recipe diagnostic and missing scalar. Source text has preflight-scalar and post-layout-geometry rejection tests, both proving absence of font diagnostics for omitted occurrences.

The scalar-rejection test exposed a false Liberation diagnostic for a cleared compatibility-placeholder label. The original text was replaced with a box, the replacement label exceeded the limit and was cleared, but fallback resolution still created an empty shaped-label object. Resolution now represents that empty label as `None`. The test preserves both resource recovery diagnostics (source replaced, label omitted) and rejects any font diagnostic. No diagnostic expectation was weakened to accept the false substitution.

### Registry correction scope

The following correction is required in `tests/oracle/scene-surfaces/revision-1.json`, based on the already documented §8.3/#175 behavior:

| Code | Correct source/owner scope | Recipe selection |
| --- | --- | --- |
| `font.liberation-sans-substituted` | One per surviving ordinary Helvetica text or visible compatibility-placeholder label; frame names and embeddable chrome are silent | None |
| `font.noto-emoji-substituted` | One per surviving ordinary text or generated embeddable label where Noto supplies a glyph; no frame-name owner | None for ordinary text; offline-embeddable recipe for generated embeddable labels |
| `font.missing-glyph` | Surviving ordinary text, visible frame names and generated labels; logical scalar order, excluding default-ignorables and truncated/omitted text | None |

The scene-surface digest is bound by **36 approved focused-case manifests**, the generated catalog, spec identity, and coverage/snapshot metadata. It is not merely a consuming-test hash. This follow-up records the correction without rewriting those approval-bound authorities. Carry it through an explicit case-evidence reapproval round, preserving outputs and comparing approval subjects, before claiming registry-based font ownership.

Generated-label work/geometry rejection and zero-width branches also remain to be combined with the passing matrix. No new owner assignments are made in this follow-up.

Subsequent follow-up: generated-label geometry/work rejection and zero-width assertions are now included and pass. The registry correction and all 36 affected case subjects have been explicitly approved and recorded; see [the reapproval record](font-registry-reapproval.md). The native snapshot reconciliation is also separately approved with canonical packaged replay passing. Final ownership assessment and assignment remain.

Verification: all 13 `diagnostic_contract` tests pass; ownership validation passes with 137 assigned/excluded and 791 gaps; all ten Dagger conformance/lint checks and all three Dagger workspace test checks pass. The empty-label regression failed before the fix with an extra Liberation diagnostic. `git diff --check` passes and temporary measurement logging was removed.

## Verification

- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 13 passed.
- `cargo test --locked -p excalidraw-render --lib diagnostic_registry`: six passed.
- `cargo run --locked -p xtask -- conformance verify-ownership`: 137 assigned/excluded, 791 gaps; approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks passed.
- `dagger check excalidraw-render:test`: fast, render and doctest checks passed.
- `git diff --check`: passed. Temporary control-character logging was removed.

These initial-checkpoint checks preceded the completed qualification, registry and snapshot refreshes described above.

## Final ownership assessment

The exact unconditional selector `diagnostic_contract::font_diagnostics_cover_text_generated_labels_and_silence` runs the combined matrix and its local helpers. Review against the current registry's nine format obligations found the required severity, ordered effects, detail/recovery/recipe fields, source owners, logical scalar order, occurrence multiplicity and silence assertions in that selector. Every authored report comparison checks the complete ordered array, rejecting extra diagnostics; fresh render comparisons include full message/report bytes and artifacts. Frame cases also compare standalone and batch results.

| Code | Evidence within the owner |
| --- | --- |
| Liberation Sans substitution | Multiple surviving Helvetica occurrences; direct-family silence; visible compatibility labels; empty/deleted/unreciprocated text; renderer-owned chrome silence; scalar and post-layout rejection; cleared/tiny fallback-label silence |
| Noto substitution | Multiple emoji count once per affected text/embeddable occurrence; exact null versus offline-embeddable recipe; unaffected/empty/omitted silence; frame selection; generated-label geometry/work rejection and both zero-extent axes |
| Missing glyph | Repeated scalars in logical rather than numeric order across ordinary text, frame names, embeddables and compatibility labels; hidden/truncated/tiny/omitted silence; default-ignorable class endpoints and Hangul/paragraph-separator exclusions; frame ownership and replay |

Prerequisites are complete: the registry/case subjects are approved, the soft-hyphen fix has approved dual-viewer and native/WASM qualification, and the separately approved native baseline replay matches all 168 expectations. The nine font obligations are now proposed owned. Counts: **27 diagnostic owners / 95 diagnostic gaps; 146 owners / 782 gaps globally**. This assignment does not approve the incomplete global ledger or close #85.

Final checks on the working tree based on `bbb64a9`: the exact combined font test passes; `verify-ownership` validates 928 assignments with 146 assigned/excluded and 782 gaps; `verify-native-baseline` still passes; `dagger check excalidraw-render:conformance:coverage-ownership` passes. Only the nine font dispositions and documentation change in this assignment round; the approved inventory/baseline and test sources retain their digests.

### Scoped maintainer approval

The maintainer explicitly approved these nine assignments on 2026-09-13, recorded in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5653057971). The approval identifies commit `b9cba674c626f7e4c9284077ad483e14aec25f54`, which binds the exact nine dispositions, test selector, source digest and rationales. It is a scoped review record; the ledger's global approval remains null because 782 obligations remain gaps.
