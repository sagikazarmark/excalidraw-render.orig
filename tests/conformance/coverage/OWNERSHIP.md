# Global ownership ledger (#85)

`ownership.json` has exactly one disposition for each obligation derived by
`ownership-catalog`. A passing consistency check means assignments/links are
well formed and every selected owner resolves in the current tree; it does not
mean coverage is complete. Acceptance is Git/PR review (ADR-0019); the ledger's
`approval` member is historical metadata and stays `null`.

Test owners resolve their package, module and exact test selector against the
current Rust tree. Their recorded source digests are historical provenance:
adding tests, comments or implementation changes to the same file needs no
ledger refresh. Removing, renaming, disabling or redirecting a selected test
still fails verification. Group membership, case manifests, exclusion proofs and
judgment records retain their existing integrity checks.

## Current scope

The ledger has 2,440 obligations: 118 focused-case labels, 605 surface field ×
format rows, 122 diagnostic × format rows, 22 manifest tolerance-exception
scopes, 1,543 codec/profile obligations, 4 interactions, 4 closure obligations
and 22 advisory `layer:*` rows. 2,421 are owned, 3 are excluded (the
generic-fallback entries, bound to
[the reachability proof](../../../docs/research/known-element-replacement-reachability.md))
and 16 are explicit gaps: two closure obligations
(`closure:atomic-profile-inventory`, `closure:process-target-equality`), the
six JPEG 1x4/2x4/4x2 sampling cells (#187) and the eight JPEG MPF gain-map
cells (#190), all 0.1.0 follow-ups in the
[release policy](../../../docs/release-0.1.0.md). These counts are **not a
coverage percentage**: the
four closure obligations keep the remaining scope from disappearing behind a
zero-gap claim for the current inventory, and `verify-closure` fails until
they are complete.

The representative interaction is owned through the strict adapter by
`representative.stage4-frames-text-assets.v29`, the registered candidate.
Earlier representative revisions were pruned under ADR-0022.

Five-layer mappings are advisory. Ordinary `verify-ownership` does not
validate or require `layer:*` assignments or the `closure:all-five-layers`
record owner. Run `conformance layers-report` to inspect current discovery and
mapping findings without refreshing a stored record. Non-layer owners and
actual case/evidence validation remain enforced. Explicit global closure audits
still check the complete historical ledger and cannot treat advisory gaps as
completed coverage.

The ledger was built up in reviewed tranches, one per obligation family or
codec facet. Each tranche's selections, rationale and verification are recorded
in a note under
[`docs/research/archive/`](../../../docs/research/archive/README.md)
(`*-ownership.md`, `*-qualification.md`, `*-umbrella-facets.md`); the
`rationale` strings in the ledger name the note that assigned each owner. Two
notes stay live because code cites them for policy and provenance:
[PNG SDR color](../../../docs/research/png-sdr-color-ownership.md) and
[PNG HDR/gain maps](../../../docs/research/png-hdr-gain-map-ownership.md).

Existing local `role: primary` labels remain intact in case manifests; they
are candidates here, and only this global selection chooses a single primary
home for each scoped label. Other candidates stay secondary evidence.

## Resolving the 27 case-label collisions

| Label(s) | Primary case | Reason |
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
manifest bytes; the consistency command verifies case evidence and expectations.
These are label-level ownership decisions only, not proof that a chosen scene
covers every diagnostic/format/profile obligation in this ledger.

### Five-layer rows

The `layer` policy category adds the 22 `layer:*` rows: `layer:vector:*`,
`layer:focused:*`, `layer:adversarial:*` and `layer:regression:*`.
`layer:vector:geometry`, `color`, `shaping-fallback` and `format-planning`
cover spec families that no owned row pinned by exact values. A `layer` row
may name registered candidate cases; `record-case-owner` assigns only a row
whose single candidate is the case. `layers.json` maps every layer item to
these rows and to existing rows such as `interaction:diagnostic-order` and
`codec:*`. It is the record owner of `closure:all-five-layers`; see
[README.md](README.md#five-layer-integration-closureall-five-layers).

## Commands and checks

```sh
just conformance
cargo run --locked -p xtask -- conformance ownership-catalog > target/ownership-catalog.json
cargo run --locked -p xtask -- conformance verify-ownership
# Record explicit reviewed gap selections:
cargo run --locked -p xtask -- conformance record-test-owner MODULE FUNCTION RATIONALE ID...
# Explicit root library tests using private test-only operation seams:
cargo run --locked -p xtask -- conformance record-library-test-owner MODULE FUNCTION RATIONALE ID...
# Own a closure row with its verified judgment record (ADR-0015/0019):
cargo run --locked -p xtask -- conformance record-closure-owner closure:pdf-judgments pdf-judgments RATIONALE
# Inspect optional five-layer coverage without changing the ledger:
cargo run --locked -p xtask -- conformance layers-report
# Own a `layer:*` row with the registered case that is its single candidate:
cargo run --locked -p xtask -- conformance record-case-owner CASE RATIONALE ID...
# Capture the old catalog before editing inventory inputs, then extend only:
# cargo run --locked -p xtask -- conformance ownership-catalog > target/old-catalog.json
cargo run --locked -p xtask -- conformance extend-ownership target/old-catalog.json ISSUE REASON
# Machine-readable all-gap template, for explicit reviewed migrations only:
cargo run --locked -p xtask -- conformance seed-ownership > target/ownership-template.json
# Fails while substantive coverage or execution obligations remain incomplete:
cargo run --locked -p xtask -- conformance verify-closure
```

`ownership-catalog` derives the source-bound obligation set on every invocation.
Its digest binds current source descriptors and complete criteria. The ledger
must have exactly one row for every identity: no omitted gaps, unknown identities,
duplicates, stale catalog bindings, or silently selected candidates. Policy rows
must refer to an actual current spec anchor. Required base codec facets and
closure requirements cannot be removed without a code change as well.

`png-profiles.json` adds explicit PNG/APNG sample, cICP and orientation axes and
remaining qualification rows. Its descriptor is part of the catalog digest.
Sample-cell evidence is limited to the declared untagged 8-bit-lattice scope;
non-lattice precision, color interpretation and axis composition are distinct
requirements. See [the inventory assessment](../../../docs/research/archive/png-profile-ownership.md).

`extend-ownership` authenticates a captured prior catalog against the existing
ledger, refuses changes/removals to any prior obligation, preserves all prior
dispositions, and adds new rows only as issue-linked gaps. This is an append-only
reviewed inventory migration, not a stale digest override or automatic owner
selection. Historical approval metadata does not block maintenance.

Dispositions are strict tagged objects:

- `owned`: one `owner` plus rationale. Case owners are allowed only for existing
  case labels and must be registered candidates with exact manifest digests.
- `owned` test adapter: an exact non-ignored `#[test]` function in a compiled
  renderer `fast` integration-test module, historical source digest, binary and complete
  `cargo test --locked --release -p excalidraw-render --test fast MODULE::TEST -- --exact`
  argument list. Other layouts require an explicit adapter that validates their
  executable selection. Anchors establish executable presence;
  the ordinary test tier executes them, and maintainer review establishes whether
  the assertions actually meet the named criterion.
- `owned` library-test adapter: `kind: test`, `binary: lib`, a root renderer
  `src/MODULE.rs` registered exactly as `#[cfg(test)] mod MODULE;` in `src/lib.rs`,
  a historical source digest, and the exact
  `cargo test --locked --release -p excalidraw-render --lib MODULE::TEST -- --exact`
  command. Selected functions are unconditional built-in tests without ignore or
  feature/target guards. Parsed Cargo manifests must retain the standard library
  test harness, default library path and explicit workspace membership without
  exclusions. Disabled, redirected and procedural-macro targets are rejected.
  This adapter supports private operation-failure contracts while keeping test
  controls out of the production API; see [ADR-0010](../../../docs/adr/0010-test-metadata-operation-failures-through-private-library-seams.md).
- `owned` record adapter ([ADR-0015](../../../docs/adr/0015-own-closure-obligations-with-approved-judgment-records.md)):
  `kind: record`, a `record` path + sha256, `approval_subject_sha256` and a
  `verifier` name. Closure rows (`closure:*`) accept only record owners, and
  record owners own only closure rows. The record's bytes match the digest, it
  names the row as its `obligation`, and has the expected schema/revision.
  `approval_subject_sha256` is retained only for legacy record compatibility.
  `verifier` selects a verifier compiled into xtask for the record's
  schema, never a shell command (`pdf-judgments` runs `verify-pdf-judgments`,
  `layers` runs `verify-layers --strict`). Any failure
  is stale or unbound ownership. `record-closure-owner` checks all of this before
  writing a gap or existing record row; review changed records and ownership in Git.
- `gap`: positive issue number and a concrete reason. Always blocks closure.
- `excluded`: reason and digest-bound proof source/anchor, reviewed in Git.
  Historical decision metadata may be retained. Difficulty or missing evidence
  is a gap, not an exclusion.

Acceptance is normal Git/PR review; verifiers check the evidence itself.
The test adapter parses Rust syntax and requires exactly one top-level external
module and one top-level `#[test]` function for the selected owner. Comments,
strings, nested scopes, conditional/ignored tests, module path redirection and
unknown attributes at the selected boundaries cannot establish ownership. Doc
attributes and unrelated test attributes are allowed. This source check does not
resolve imports or expand macros: imports named `test`, glob imports, item macro
invocations and macro-use imports in the checked scopes require an explicit
adapter rather than guessing whether `#[test]` is built-in. The test marker must
be an outer attribute. These checks do not replace compiled test execution.
For test owners, verification checks historical digest syntax and resolves the selected
test in the current tree, without requiring its source bytes to match the recorded
digest.

Closure checks the catalog, dispositions, executable owners and remaining gaps.
It cannot be made complete by changing a status flag.

## Remaining mapping work

Start with exact per-format assertions for the 122 diagnostic rows, then surface
field branches and codec subprofiles. Use existing fast tests first, but review
their severity/effects/details, silence, recovery and ordering assertions before
assigning a broad obligation. Connect specialized PDF structural/visual exceptions
to their consuming comparators and stale-exception tests with a dedicated adapter.
Expand codec facets into explicit accepted profile/color/orientation/alpha/animation
subrows; add representative interactions. The five corpus layers are registered
in the optional `layers-report`. Review owners and expectations with their tests;
attempt release closure only when the substantive obligations are complete.

## Executable test groups (ADR-0018)

An owner may select a digest-bound `test_group` manifest under `test-groups/`.
The group has one package, one binary and a nonempty list of unique exact test
selectors. Every member must resolve to an unconditional compiled Rust test.
Closure obligations still require verified judgment records, and tolerance
exceptions still require the dedicated direct-PNG adapter.

The unsupported-property-value group preserves all 33 routines of the former
umbrella. Its members run independently in the existing CI tiers, so the runner
does not repeat 31 tests through a second serial caller. Two formerly private
helpers are now standalone tests. The three format-scoped obligations keep one
primary owner each, all referencing that same group manifest.

To run the group alone:

```sh
filter=$(cargo run --quiet --locked -p xtask -- conformance test-group-filter \
  tests/conformance/coverage/test-groups/unsupported-property-values.json)
cargo nextest run --locked -p excalidraw-render --test fast -E "$filter"
```

The legacy `refresh-test-owner-source`, `refresh-test-owner-sources`,
`refresh-library-test-owner-source` and `refresh-exception-test-owner-source`
commands remain available for explicitly recording newer provenance. They are
not needed after ordinary source edits.
