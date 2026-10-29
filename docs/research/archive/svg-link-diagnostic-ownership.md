# Nested SVG and link diagnostic ownership (#85)

Base: `cf9d8253d0958eedcf3d6e8a800823f20a593285`.

## Scope and primary selectors

The `fast/svg_link_diagnostic_contract.rs` module consolidates eight
diagnostic/format obligations:

| Diagnostic | Formats | Primary selector |
| --- | --- | --- |
| `image.forbidden-resource` | SVG, PNG, PDF | `forbidden_resource_contract_in_every_format` |
| `image.unsafe-svg-content-removed` | SVG, PNG, PDF | `unsafe_svg_contract_in_every_format` |
| `link.unsafe-omitted` | SVG, PDF; PNG silence tested | `unsafe_link_contract_in_semantic_formats` |

The report oracle compares the entire ordered array and every machine field:
severity, effects, format, element ID, source pointer, nested chain/path,
recovery and applicable recipe. Messages must be nonempty and stable across
fresh calls. Independently authored clean scenes, frozen recipe paths, complete
SVG drawing trees, PNG pixels and PDF drawing streams check actual recovery.

## Forbidden-resource repair

The first independent artifact test failed: the diagnostic selected the
manifested forbidden-resource placeholder, but SVG/PDF omitted the image and
PNG painted its source-status-selected ordinary image placeholder. The manifest
requires the image-error glyph in all three formats.

Non-data scene-file URL references now take a typed forbidden path before any
payload decoding or fetching. Each referring image builds the bounded error
recipe in its normalized local viewport through the existing resource-recovery
constructor. The file-level diagnostic reflects whether any participating
reference can paint it. Deleted references and references outside a selected
frame cannot supply the viewport. Frame diagnostics are scoped before retention;
direct and prepared target artifact/report/trace comparisons exercise both
zero-extent and bounded references.

The replacement keeps rotation, flips, opacity and corner/frame clipping. An
authored crop of the unavailable payload is not applied again to the generated
viewport-sized recipe. A transformed/cropped example initially failed the
independent drawing oracle and now agrees in all formats. Embedding preserves
the canonicalized original scene document, including that crop and rejected URL.

Nested rejected resource hrefs have `nested_source` forbidden-resource details.
An image slot with usable bounds paints the same recipe; an unbounded slot,
unsupported resource slot without a viewport, or insufficient recovery budget
omits only that resource. The recovery transcript is preserved for cached
subtrees. The two Font Awesome notices follow actual placeholder selection.
The generated SVG is not misreported as validation of the rejected payload's
advisory MIME type.

Dual `href`/`xlink:href` input previously produced two rewritten `href`
attributes and failed the whole containing SVG. The renderer now selects the
SVG 2 `href` in preference to its legacy alias, independent of source attribute
order, and serializes one effective reference. The ignored alias is neither
fetched nor recovered. A sibling-preserving regression checks both orders and
an admitted primary href with a forbidden shadow alias.

Replacement attributes, escaped text and markup now reserve destination space
fallibly before writing. A cached nested-recovery trace test exercises four
rejected references through two reused subtree levels, checks four independent
construction charges when admitted (zero below the envelope), and compares
standalone/prepared artifacts, reports and complete traces.

Supplemental values remain payload bytes: a byte string containing an HTTP URL
is an unsupported payload, not a request to fetch it. Scene authority continues
to win over conflicting supplements.

## Unsafe SVG boundaries

Existing exact-report cases cover DTD/entity handling, processing instructions,
scripts, event handlers, animation, foreign namespaces/content and unsafe CSS.
Resource href rejection is distinct from removing an unsafe content boundary;
nested anchor actions and unsafe style attributes keep their content-removal
mapping. Local fragments and safe styles remain silent.

Additional assertions cover identical payloads under distinct authoritative
file IDs, source-file order opposing reference/lexical order, supplemental ID
order, repeated nested payload occurrences, mixed resource/content diagnostics,
scene/frame scope, deleted and unreferenced silence, conflicts and fatal report
prefixes. Clean-subtree oracles prove that valid siblings and static text/paint
survive removal. Optional scene embedding retains the original unsafe source.

Entity references used to all identify the document root. Preprocessing now
keeps their original text-node or attribute boundary, including an entity-only
text node that disappears from the rendered document. A temporary location
document and source-offset map preserve later source ordinals; only the clean
document drives semantic operations and serialization. Comment, CDATA and
processing-instruction text is not mistaken for an entity or DTD declaration.
Tests compare separate text occurrences, attribute values, vanished text and
subsequent unsafe attributes against independently authored clean scenes.
Coalesced CDATA/text uses the complete lexical span rather than the parser's
first-token range, including references immediately before or after CDATA.

## Unsafe scene links

The owner exercises ordinary geometry, freehand, text, images, frames, generic
fallbacks, iframes and embeddables. Unsafe schemes, the pinned sanitizer's entity
and whitespace behavior, invalid value types, safe URL canonicalization,
source order, fatal diagnostic prefixes, deletion and frame scope have complete
report assertions. PNG is silent for scene-link diagnostics.

Generated labels are part of the evidence: an unsafe embeddable link produces
`about:blank`, while an iframe keeps `IFrame element`. The actual artifact has
no SVG anchor or PDF URI annotation. An independently authored safe-label
reference checks paint; only the reference's explicit anchor and transparent hit
rectangle are removed for SVG comparison. Source preservation is checked
separately from visible output.

## Verification and ownership status

The selected owners are subject to the normal compiled-selector/source-digest
checks and Standards/Spec review. This work adds no diagnostic code or schema:
the forbidden recipe and both top-level/nested detail bindings were already
declared by the scene-surface manifest. Global ownership remains a proposal,
not a release approval. The previously recorded #178 cross-target equality
differences remain separate from these eight diagnostic obligations.

The repository ownership command records all eight selectors with their current
source digest and reviewed rationale. The ledger is now **815 assigned/excluded
/ 113 explicit gaps**, including **91/122** diagnostic/format owners.

Verification under Rust 1.97.1:

- The full default workspace suite passed **1,045 tests**, with eight existing
  ignored tests skipped. After the final narrow CDATA-location and diagnostic
  allocation fixes, all three owner tests, 15 SVG-unit tests and three
  forbidden-resource/trace tests passed again.
- Workspace formatting, Clippy with warnings denied, documentation tests and
  `wasm32-unknown-unknown` type-checking passed.
- All 37 registered cases and the catalog, inventory, ownership, font-subset
  and viewer evidence verification gates passed.
- Standards and Spec closure reviews confirmed their findings were addressed,
  including duplicate href recovery, entity/CDATA attribution, fallible output
  growth, reuse of existing diagnostic boxes, and independent recovery/trace
  evidence. No new cross-target execution claim is made by the WASM compile check.
