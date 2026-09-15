# Global ownership ledger (#85)

`ownership.json` has exactly one disposition for each obligation derived by
`ownership-catalog`. It is an **unapproved, incomplete proposal**. A passing
consistency check means assignments/links are well formed; it does not mean
coverage is complete or the chosen primary owners have received human approval.

## Current scope

| Obligation family | Count | Source |
| --- | ---: | --- |
| Existing focused-case labels | 118 | Current approved manifests via execution inventory |
| Surface field × applicable format | 605 | `fieldDispositions[].scope.formats` |
| Diagnostic × declared format rule | 122 | `registryMappings[].formatRules` |
| Manifest tolerance-exception scopes | 24 | Case + variant + format + region |
| Codec/profile facets | 51 | Explicit spec-anchored `ownership-policy.json` |
| Interactions | 4 | Explicit spec-anchored policy |
| Closure obligations | 4 | Explicit unfinished inventory/layer/PDF/target work |
| **Total** | **928** | |

783 primary owners are proposed (118 case labels, the AVIF rejection test,
59 diagnostic/format obligations and all 605 current surface-field/format obligations).
145 obligations remain explicit gaps. The three nested-node assignments are
assessed in [the resource amendment](../../../docs/research/nested-resource-limit-amendment.md#nested-node-ownership).
The three nested-depth assignments are
assessed in [the resource amendment](../../../docs/research/nested-resource-limit-amendment.md#nested-depth-ownership).
The last eight surface assignments are
assessed in [the final surface audit](../../../docs/research/final-surface-ownership.md).
Points/freehand/arrow assignments are
assessed in [the point/stroke audit](../../../docs/research/point-stroke-ownership.md).
Relationship/binding assignments are
assessed in [the relationship audit](../../../docs/research/relationship-field-ownership.md).
Frame assignments are assessed in
[the frame field audit](../../../docs/research/frame-field-ownership.md).
Text assignments are assessed in
[the text field audit](../../../docs/research/text-field-ownership.md).
Image/asset/crop assignments are assessed
in [the image field audit](../../../docs/research/image-field-ownership.md).
Geometry/style/color assignments are
assessed in [the geometry audit](../../../docs/research/geometry-style-ownership.md).
Duplicate-ID closure is assessed in
[the duplicate identity audit](../../../docs/research/duplicate-identity-ownership.md#ownership-completion).
Admission/identity additions are assessed
in [the admission audit](../../../docs/research/admission-identity-ownership.md).
The earlier surface assignments are assessed in
[the surface ownership audit](../../../docs/research/surface-field-ownership.md).
The diagnostic audit retains 63 of its 122 obligations as gaps with specific
missing assertions or branches; see [the per-code audit](../../../docs/research/diagnostic-ownership.md).
These counts are **not a coverage percentage**:
codec facets still need expansion into the full admitted-profile matrix, and
specialized PDF/vector/fuzz owners still need adapters and reviewed mappings.
The nine font diagnostic assignments have a [scoped maintainer approval](../../../docs/research/font-diagnostic-ownership.md#scoped-maintainer-approval);
the incomplete global ledger remains pending approval.
The six malformed-file/unsupported-image assignments also have a
[scoped maintainer approval](../../../docs/research/image-diagnostic-ownership.md#scoped-maintainer-approval).
The three decode-failure assignments have a
[scoped maintainer approval](../../../docs/research/decode-diagnostic-ownership.md#scoped-maintainer-approval).
The two animation-flattening assignments have a
[scoped maintainer approval](../../../docs/research/animation-diagnostic-ownership.md#scoped-maintainer-approval).
The three supplement-conflict assignments have a
[scoped maintainer approval](../../../docs/research/asset-claim-diagnostic-ownership.md#scoped-maintainer-approval).
The three media-type assignments have a
[scoped maintainer approval](../../../docs/research/asset-claim-diagnostic-ownership.md#scoped-media-type-approval).
The four closure obligations prevent that remaining scope from disappearing
behind a zero-gap claim for only the current coarse inventory.

The execution inventory and native baseline are unchanged. This ledger references
them but does not replace their approval subjects or rerender their evidence.
Existing local `role: primary` labels remain intact in approved case manifests;
they are candidates here, and only this proposed global selection chooses a
single primary home for each scoped label. Other candidates stay secondary evidence.

## Resolving the 27 case-label collisions

| Label(s) | Proposed primary case | Reason |
| --- | --- | --- |
| arrow.elbowed | arrow-suite | General elbow/binding route suite; rotation is a separate interaction |
| arrowhead.roughness-clamp | cartoonist-arrowheads | Dedicated per-kind roughness clamp case |
| bound-text.diamond/ellipse/rectangle | bound-text-containers | Dedicated container-shape behavior |
| bound-text.stored-placement | bound-text-placement | Explicit stored placement case |
| element.embeddable, link.admitted | embeddable-links | Dedicated admitted embeddable/link matrix |
| export.dark-mode | ellipse-diamond-fills | General geometry dark-mode variant; image/frame-specific obligations remain separate |
| export.max-dimension, export.scale | freedraw | Explicit sizing variants |
| fill.hachure, fill.zigzag, stroke.dashed, stroke.dotted | ellipse-diamond-fills | Focused fill/stroke matrix |
| font.liberation-sans-substitution | helvetica-text | Dedicated substitution case |
| font.missing-glyph | missing-glyph | Dedicated missing-codepoint recovery |
| frame.clip | frame-clip-mask-sharing | Dedicated clipping/mask sharing |
| frame.name | frame-name-truncation | Name sizing/truncation behavior |
| image.crop | image-codecs | General crop across codec families; rounded crop remains secondary interaction |
| reference.direct-png, reference.svg | rectangle-outline | Baseline reference export contract |
| roughness.cartoonist | cartoonist-roughness | General cartoonist drawing behavior |
| text.align | text-alignment-vertical | Dedicated horizontal/vertical alignment matrix |
| text.bidi | bidi-text | General bidi case; bidi-alignment adds interaction coverage |
| text.line-height-detection | legacy-line-height | Dedicated restore/default detection |
| transform.rotation | transform-opacity-groups | General transform coverage; rotated bounds/rough paths remain distinct regressions |

All selections must name a currently registered candidate and bind its exact
manifest bytes; the consistency command additionally verifies case approvals.
These are label-level ownership decisions only, not proof that a chosen scene
covers every diagnostic/format/profile obligation introduced by this ledger.

## Commands and checks

```sh
dagger check excalidraw-render:conformance:coverage-ownership
dagger api call evidence ownership-catalog --output target/ownership-catalog.json
cargo run --locked -p xtask -- conformance verify-ownership
# Record explicit reviewed gap selections; this does not grant approval:
cargo run --locked -p xtask -- conformance record-test-owner MODULE FUNCTION RATIONALE ID...
# Refresh consuming-test location hashes in an unapproved ledger after review:
cargo run --locked -p xtask -- conformance refresh-test-owner-source MODULE
# Machine-readable all-gap template, for explicit reviewed migrations only:
cargo run --locked -p xtask -- conformance seed-ownership > target/ownership-template.json
# Both fail until the ledger is complete (closure also requires approval):
cargo run --locked -p xtask -- conformance ownership-approval-subject
cargo run --locked -p xtask -- conformance verify-closure
```

`ownership-catalog` derives the source-bound obligation set on every invocation.
Its digest binds current source descriptors and complete criteria. The ledger
must have exactly one row for every identity: no omitted gaps, unknown identities,
duplicates, stale catalog bindings, or silently selected candidates. Policy rows
must refer to an actual current spec anchor. Required base codec facets and
closure milestones cannot be removed without a code change as well.

Dispositions are strict tagged objects:

- `owned`: one `owner` plus rationale. Case owners are allowed only for existing
  case labels and must be registered candidates with exact manifest digests.
- `owned` test adapter: an exact non-ignored `#[test]` function in a compiled
  renderer `fast` integration-test module, source digest, binary and complete
  `cargo test --locked --release -p excalidraw-render --test fast MODULE::TEST -- --exact`
  argument list. V1 deliberately rejects other layouts until an explicit adapter
  validates their executable selection. Anchors establish executable presence;
  the ordinary test tier executes them, and maintainer review establishes whether
  the assertions actually meet the named criterion.
- `gap`: positive issue number and a concrete reason. Always blocks closure.
- `excluded`: reason, digest-bound proof source/anchor and an existing explicit
  #85 approval binding the canonical exclusion subject (obligation + reason +
  proof). No exclusions have been granted by this implementation. Difficulty or
  missing evidence is a gap, not an exclusion.

Approval verification is offline: it checks the exact subject and scoped GitHub
comment URL syntax, not remote comment existence or authorship. The maintainer's
recorded decision remains the authority, as in the existing case approval system.
The test adapter parses Rust syntax and requires exactly one top-level external
module and one top-level `#[test]` function for the selected owner. Comments,
strings, nested scopes, conditional/ignored tests, module path redirection and
unknown attributes at the selected boundaries cannot establish ownership. Doc
attributes and unrelated test attributes are allowed. This source check does not
resolve imports or expand macros: imports named `test`, glob imports, item macro
invocations and macro-use imports in the checked scopes require an explicit
adapter rather than guessing whether `#[test]` is built-in. The test marker must
be an outer attribute. These checks do not replace compiled test execution.
Live source hashes are excluded
from the final ownership decision subject for test owners, preserving producer-only
approval scope when unrelated consuming tests change.

A completed-ledger subject binds the whole catalog and every disposition,
excluding only the ledger's approval object. It contains strings/integers, not
floating-point request JSON. Subject generation refuses remaining gaps; release
closure additionally requires its scoped four-field approval. Recording a final
ledger approval will be added when the complete inventory and mappings are ready;
this incomplete proposal cannot be marked complete by a metadata flag.

## Remaining mapping work

Start with exact per-format assertions for the 122 diagnostic rows, then surface
field branches and codec subprofiles. Use existing fast tests first, but review
their severity/effects/details, silence, recovery and ordering assertions before
assigning a broad obligation. Connect specialized PDF structural/visual exceptions
to their consuming comparators and stale-exception tests with a dedicated adapter.
Expand codec facets into explicit accepted profile/color/orientation/alpha/animation
subrows; add representative interactions and register the five corpus layers.
Only then request final ledger approval and attempt #85 release closure.
