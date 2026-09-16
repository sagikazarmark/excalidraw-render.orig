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
| Codec/profile obligations | 999 | 51 base facets plus 948 PNG/APNG inventory obligations |
| Interactions | 4 | Explicit spec-anchored policy |
| Closure obligations | 4 | Explicit unfinished inventory/layer/PDF/target work |
| **Total** | **1,876** | |

1,023 primary owners are proposed (118 case labels, the AVIF rejection test,
119 diagnostic/format obligations, all 605 current surface-field/format obligations,
and all 180 PNG/APNG sample-vector cells). Three generic-fallback entries have approved
exclusions. The PNG/APNG inventory expansion preserved all 928 prior dispositions
and added 948 obligations; the sample tranches assign 60 static and 120 APNG
cells, including the final 116 non-RGBA8 APNG cells. Current totals are **1,026
assigned/excluded and 850 gaps**. See
[the PNG/APNG inventory assessment](../../../docs/research/png-profile-ownership.md).
The 21 metadata failure assignments use
the explicit library-test adapter and are assessed in [the metadata audit](../../../docs/research/metadata-failure-ownership.md).
The PDF raster-fallback assignment and
shared-plan adapter are assessed in [the PDF audit](../../../docs/research/pdf-raster-fallback-ownership.md).
The six compatibility-recovery assignments
are assessed in [the compatibility audit](../../../docs/research/compatibility-recovery-ownership.md);
the three generic-fallback exclusions bind
[the reachability proof](../../../docs/research/known-element-replacement-reachability.md)
under [scoped #85 approval](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5700122166).
The eight nested SVG/link diagnostic
assignments are assessed in [the SVG/link audit](../../../docs/research/svg-link-diagnostic-ownership.md).
All 30 resource diagnostic/format obligations
now have proposed owners; the final 15 are assessed in
[resource diagnostic completion](../../../docs/research/resource-diagnostic-ownership.md).
Native and browser-WASM execution is recorded in
[the target evidence report](../../../docs/research/resource-recovery-target-evidence.md);
the #178 equality failures remain explicit.
The three optional-metadata assignments are
assessed in [the resource amendment](../../../docs/research/nested-resource-limit-amendment.md#optional-metadata-ownership).
The six raster dimension/pixel assignments are
assessed in [the resource amendment](../../../docs/research/nested-resource-limit-amendment.md#raster-dimension-and-pixel-ownership).
The three nested-node assignments are
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
All 122 current diagnostic/format obligations have a disposition: 119 proposed
test owners and three approved exclusions; see [the per-code audit](../../../docs/research/diagnostic-ownership.md).
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
# Explicit root library tests using private test-only operation seams:
cargo run --locked -p xtask -- conformance record-library-test-owner MODULE FUNCTION RATIONALE ID...
cargo run --locked -p xtask -- conformance refresh-library-test-owner-source MODULE
# Capture the old catalog before editing inventory inputs, then extend only:
# cargo run --locked -p xtask -- conformance ownership-catalog > target/old-catalog.json
cargo run --locked -p xtask -- conformance extend-ownership target/old-catalog.json ISSUE REASON
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

`png-profiles.json` adds explicit PNG/APNG sample, cICP and orientation axes and
remaining qualification rows. Its descriptor is part of the catalog digest.
Sample-cell evidence is limited to the declared untagged 8-bit-lattice scope;
non-lattice precision, color interpretation and axis composition are distinct
requirements. See [the inventory assessment](../../../docs/research/png-profile-ownership.md).

`extend-ownership` authenticates a captured prior catalog against the existing
ledger, refuses changes/removals to any prior obligation, preserves all prior
dispositions, and adds new rows only as issue-linked gaps. It refuses an approved
global ledger. This is an append-only reviewed inventory migration, not a stale
digest override, automatic owner selection, or approval refresh.

Dispositions are strict tagged objects:

- `owned`: one `owner` plus rationale. Case owners are allowed only for existing
  case labels and must be registered candidates with exact manifest digests.
- `owned` test adapter: an exact non-ignored `#[test]` function in a compiled
  renderer `fast` integration-test module, source digest, binary and complete
  `cargo test --locked --release -p excalidraw-render --test fast MODULE::TEST -- --exact`
  argument list. Other layouts require an explicit adapter that validates their
  executable selection. Anchors establish executable presence;
  the ordinary test tier executes them, and maintainer review establishes whether
  the assertions actually meet the named criterion.
- `owned` library-test adapter: `kind: test`, `binary: lib`, a root renderer
  `src/MODULE.rs` registered exactly as `#[cfg(test)] mod MODULE;` in `src/lib.rs`,
  a source digest, and the exact
  `cargo test --locked --release -p excalidraw-render --lib MODULE::TEST -- --exact`
  command. Selected functions are unconditional built-in tests without ignore or
  feature/target guards. Parsed Cargo manifests must retain the standard library
  test harness, default library path and explicit workspace membership without
  exclusions. Disabled, redirected and procedural-macro targets are rejected.
  This adapter supports private operation-failure contracts while keeping test
  controls out of the production API; see [ADR-0010](../../../docs/adr/0010-test-metadata-operation-failures-through-private-library-seams.md).
- `gap`: positive issue number and a concrete reason. Always blocks closure.
- `excluded`: reason, digest-bound proof source/anchor and an existing explicit
  #85 approval binding the canonical exclusion subject (obligation + reason +
  proof). The three generic-fallback exclusions have the scoped approval linked
  above. Difficulty or missing evidence is a gap, not an exclusion.

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
