# Font diagnostic ownership follow-up

Working-tree follow-up to `4d6fb67`, under #85. The three font codes cover nine diagnostic/format obligations. They remain explicit gaps; passing representative tests are not full ownership approval.

## Authority correction

Spec §8.3, updated by #175, requires Liberation Sans substitution diagnostics for ordinary Helvetica text and compatibility-placeholder labels, but **silence for frame names and embeddable labels**, whose face is renderer-owned chrome. Noto substitution diagnostics apply to ordinary text and embeddable labels. The scene-surface registry still describes affected text/frame/generated-label owners and an embeddable Liberation recipe more broadly. Reconcile that stale registry scope through the authority-update workflow before assigning the affected obligations. This round follows the explicit spec silence rules and does not add chrome substitution diagnostics.

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

## Unresolved soft-hyphen SVG failure

The broader default-ignorable test exposed an independent SVG subset validation failure. Individual control probes isolated it to **U+00AD**, without any bound-text omission or other control characters. The following scene with a default SVG request fails with `encode.svg-font-subset-failed`, stage `validate`, resolved family `Liberation Sans`, root pointer, and an empty compatibility report:

```json
{"type":"excalidraw","version":2,"elements":[{"id":"control","type":"text","text":"A\u00adB","fontFamily":9,"fontSize":20,"width":500,"height":40,"updated":1}],"appState":{},"files":{}}
```

The failed asset is `fonts/Liberation/LiberationSans-Regular.woff2@sha256:006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8`. Expected behavior under §8.3 is successful rendering with no diagnostic for the soft hyphen. The initial loop stopped at SVG; PNG/PDF behavior for this isolated fixture has not been independently asserted.

The failing probe is preserved here rather than made an expected failure or accepted error contract. The passing control matrix deliberately does not claim coverage for U+00AD or the entire default-ignorable class. Diagnose subset mapping/shaping validation separately, with the producer/evidence approval constraints respected.

## Remaining ownership work

- Reconcile registry wording with §8.3/#175 before assigning broad source scopes.
- Fix the soft-hyphen SVG failure and cover the full default-ignorable boundary classes, including non-ignorable Hangul fillers and paragraph separators.
- Complete local-resource omission and generated-label branch matrices, including frame/placeholder Unicode behavior and format-specific zero-size embeddables.
- Only then assign complete test owners. Global totals remain **137 proposed owners / 791 gaps**.

## Verification

- `cargo test --locked -p excalidraw-render --test fast diagnostic_contract`: 13 passed.
- `cargo test --locked -p excalidraw-render --lib diagnostic_registry`: six passed.
- `cargo run --locked -p xtask -- conformance verify-ownership`: 137 assigned/excluded, 791 gaps; approval pending.
- `dagger check excalidraw-render:conformance excalidraw-render:lint`: all ten checks passed.
- `dagger check excalidraw-render:test`: fast, render and doctest checks passed.
- `git diff --check`: passed. Temporary control-character logging was removed.

These checks validate the implemented fixes and passing matrix, not the documented soft-hyphen reproducer or full font-ownership closure.
