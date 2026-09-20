# Global ownership ledger (#85)

`ownership.json` has exactly one disposition for each obligation derived by
`ownership-catalog`. It is an **unapproved, incomplete proposal**. A passing
consistency check means assignments/links are well formed; it does not mean
coverage is complete or the chosen primary owners have received human approval.

## Current scope

### JPEG color/sampling extension

The [JPEG matrix](../../../docs/research/jpeg-color-sampling-qualification.md)
adds 38 narrow untagged 8-bit color/sampling requirements, with 17 proposed test
owners and 21 concrete [#187](https://github.com/sagikazarmark/excalidraw-render.orig/issues/187)
gaps. All prior 1,874 dispositions, including the eight coarse JPEG facets,
are preserved. **Current totals: 1,912 obligations, 1,820 assigned/excluded
(1,817 owners and three exclusions), 92 gaps.** The tables and Stage-4 narrative
below remain the prior 1,874-obligation checkpoint; the codec/profile category
now has 1,037 obligations. The current representative is approved V8, as recorded
in the [Stage-4 completion audit](../../../docs/research/png-stage4-completion.md).

| Obligation family | Count | Source |
| --- | ---: | --- |
| Existing focused-case labels | 118 | Current approved manifests via execution inventory |
| Surface field × applicable format | 605 | `fieldDispositions[].scope.formats` |
| Diagnostic × declared format rule | 122 | `registryMappings[].formatRules` |
| Manifest tolerance-exception scopes | 22 | Case + variant + format + region |
| Codec/profile obligations | 999 | 51 base facets plus 948 PNG/APNG inventory obligations |
| Interactions | 4 | Explicit spec-anchored policy |
| Closure obligations | 4 | Explicit unfinished inventory/layer/PDF/target work |
| **Total** | **1,874** | |

1,800 primary owners are selected in the proposed global ledger (118 case labels, the AVIF rejection test,
119 diagnostic/format obligations, all 605 current surface-field/format obligations,
all 180 PNG/APNG sample-vector cells, three PNG precision/alpha obligations,
seven SDR metadata/profile obligations, all 704 enumerated cICP cells and three
PNG gain-map obligations, all 32 EXIF cells, 19 further PNG qualification rows,
five PNG umbrella facets and four global interactions).
Three generic-fallback entries have approved
exclusions. The PNG/APNG inventory expansion preserved all 928 prior dispositions
and added 948 obligations; the sample tranches assign 60 static and 120 APNG
cells, including the final 116 non-RGBA8 APNG cells. The reference-backed
[precision/alpha tranche](../../../docs/research/png-precision-ownership.md) adds
three owners. The [SDR color tranche](../../../docs/research/png-sdr-color-ownership.md)
adds 67 more, and the [remaining-primary extension](../../../docs/research/png-cicp-primary-ownership.md)
adds another 160. The [BT.1886 transfer stage](../../../docs/research/png-cicp-transfer-ownership.md)
adds 176; its [remaining non-PQ/HLG stage](../../../docs/research/png-cicp-transfer-ownership.md#stage-2--remaining-non-pqhlg-transfers)
adds 220. The [Stage 3 HDR/gain-map qualification](../../../docs/research/png-hdr-gain-map-ownership.md)
adds the remaining 88 PQ/HLG cells and the three PNG gain-map obligations.
The [Stage 4 qualification](../../../docs/research/png-stage4-ownership.md) adds
59 test owners after [parent-review remediation](../../../docs/research/png-stage4-review.md)
for ICC/LUT, orientation, malformed inputs, APNG controls, composition, projection
boundaries and interactions. The [approved release representative v7](../../../docs/research/png-stage4-v7-approved-owner.md)
adds the 60th Stage-4 owner. Current totals are **1,803 assigned/excluded and 71
gaps**. All 948 enumerated PNG/APNG inventory obligations now have proposed owners.
Broader HDR metadata, four PNG and 41 other-codec umbrella facets remain gaps.
The representative interaction is now assigned through the strict adapter to
`representative.stage4-frames-text-assets.v7`, under the user's
[exact scoped approval](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5749841192).
Its approved release consumer returns three supplemental execution receipts;
v4/v6 remain historical evidence. Global ledger approval remains pending. See
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

The [Stage 4 binding reconciliation](../../../docs/research/png-stage4-binding-reconciliation.md)
applies the user-approved §8.5 authority transition, the proven `fuzz.codecs`
locator refresh, and 101 focused-tested owner-source bindings. All 168 complete
execution records, criterion/disposition semantics and three approved exclusions
are preserved. Inventory/catalog/ledger validation now passes without stale
locators; global coverage and approval remain incomplete.

The [final six-case PNG requalification](../../../docs/research/png-stage4-final-metadata-application.md)
applies the user's exact batch approval: four remeasured whole-PNG exceptions and
two withdrawals now passing canonical budgets. Only those two unowned tolerance
obligations were retired. It refreshes 122 authority and 12 case-owner manifest
bindings while preserving all other criteria/dispositions and three exclusions.
All 168 semantic execution identities remain; 18 manifest-derived input digests
changed and the other 150 full execution records are identical. Both affected
parity targets now pass. The native baseline is unchanged and still
binds the old inventory descriptor and needs parent clean-commit collection and
reapproval; see the [review correction audit](../../../docs/research/png-hdr-gain-map-ownership.md#parent-review-corrections-and-adaptation-provenance).
This ledger does not replace baseline approval subjects or rerender their evidence.
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
