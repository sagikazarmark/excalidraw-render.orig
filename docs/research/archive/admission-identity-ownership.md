# Admission, timestamp and identity ownership (#85)

Base: `95505e4cbeed6ce230bbaf81743cd6eec3ac1bc8`.

## Scope

Six public-facade tests in `fast/admission_identity_contract.rs` add 27 reviewed
assignments: 15 required-envelope field/format obligations, six timestamp
field/format obligations, three deletion field/format obligations, and three
missing-ID diagnostic/format obligations. Totals become **544 assigned / 384
gaps** (230 surface gaps and 154 other gaps).

The parser owner covers every required root field missing or with each JSON
type, exact format-neutral error descriptors, integer-token version spellings,
schema-order error precedence and duplicate root members. Admitted nested scalar,
malformed, deleted and unreferenced values survive embedding. Complete authored
reports independently gate nested malformed-element and unknown-property probes.

Timestamp tests distinguish absent fields (canonical-zero repair) from present
invalid values (silent ignored fields), as the frozen manifest specifies. They
cover all traversed kinds, referenced files with repeated references and escaped
keys, selected/unselected frames, lazy replay, deleted/unreferenced silence,
ignored empty-key files and malformed-record suppression. Source embedding
retains the original values; repaired values never replace source metadata.

Deletion tests compare true with an empty/omitted baseline and invalid values
with explicit live content. They cover frame discovery, asset participation,
both frame kinds, all traversed kinds and legacy draw, as well as selection,
unknown and malformed diagnostic boundaries.

Missing-ID evidence combines exact known vectors and metamorphic coverage. The
existing qualified rectangle/seed/duplicate/collision vectors are asserted at
the facade. Additional text, frame and number-token/member-order vectors were
calculated independently with Python hashlib over the frozen revision-1
domain-separated little-endian transcript. Expected hashes are literal; tests
never recompute their own hash oracle. Broad JSON/kind probes additionally check
owner consistency, clean-scene equivalence, uniqueness, source-position effects,
cross-format stability, target participation and preservation. Those metamorphic
checks establish consistency; the literal vectors establish exact identity.

## Retained gaps

The broad `element.common /id` surface and duplicate-ID diagnostic assignments
remain gaps. The new duplicate-frame regression proves first-occurrence reference
resolution and frame selection for that scenario, but does not establish the
complete conditional visual/semantic-effects rule for every relationship.
No registry, producer, canonical snapshot, or case-evidence bytes were changed.

## Review and approval

The maintainer's continuing “approve all” instruction covers these specific
assignments. Their exact selectors, source descriptors and rationales are in the
ledger; global approval remains null and closure continues to reject the draft.

Two-axis review prompted complete admission reports, duplicate-member probes,
live ignored/malformed boundaries, malformed/empty-key file probes, and independent
non-rectangle/lexical identity vectors. These additions preserve the distinction
between exact vectors and broad metamorphic consistency tests.

## Verification

- Six facade contract tests pass; workspace/all-target typechecking passes.
- All ten Dagger conformance/lint checks pass.
- Final full workspace suite: `dagger check excalidraw-render:test` passed fast,
  render/parity and doctest tiers in 14m9s.
- Spec review confirmed all actionable findings resolved. Standards review
  withdrew its initial circularity concern after the independent vectors and
  oracle/consistency distinction; one nonblocking naming suggestion remains.
- Ownership verification reports 544 assigned/excluded and 384 explicit gaps.

This batch establishes evidence for existing behavior; it required no production
repair. Targeted tests were run after each contract slice. Global #85 closure
still depends on the remaining obligations and prerequisite issues.
