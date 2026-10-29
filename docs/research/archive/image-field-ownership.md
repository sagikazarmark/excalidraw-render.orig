# Image fields and asset authority ownership (#85)

Base: `a58fa3fed79c3c511e6e5acb17e79d232d02b97b`.

## Completed scope

Five public-facade selectors in `fast/image_field_contract.rs` supply 42 reviewed
field/format assignments: image fileId/status/scale/crop (12), file map key and
id/mimeType/dataURL (12), and the six crop members (18). Each applies to SVG,
PNG and PDF. The ledger records exact selectors, source hashes and rationales.

The matrices cover missing/invalid JSON values, crop positivity and scale
length/component boundaries, scene and supplemental authority, null-ID recovery,
pending/saved/error statuses, original-source embedding, fresh sessions, direct
and lazy selected frames, nonselected frames and deleted/unreferenced silence.
Field-level expectations are authored independently of runtime policy.

Positive crop/flip evidence uses independent quadrant pixel expectations,
fractional crop/flip SVG viewport and PDF matrix oracles, nonzero X/Y crop
offsets, and proportional crop-coordinate equivalence. Existing complete MIME,
missing/malformed/unsupported/decode/forbidden-resource and supplement-conflict
contracts execute through the field selectors. Codec-profile qualification is
still a separate obligation family.

The file key remains lookup authority even when a valid record ID differs.
Missing/empty/nonstring IDs repair to the map key once. A referenced empty map
key receives a malformed-file diagnostic, distinct from an absent empty key's
missing-file report. Revision-1 payload admission retains its canonical base64
envelope requirement; malformed envelopes preserve their original bytes and
use the established recovery/attribution contract.

## Demonstrated fixes

1. An invalid image `fileId` normalized to null but selected a generic whole-
   element replacement. It now follows explicit-null image painting: PNG's
   status-selected image placeholder, SVG/PDF omission, with the field's exact
   substitution report rather than an additional generic replacement.
2. An invalid file-record `id` was diagnosed by both surface classification and
   normalization. Normalization now solely owns absent/invalid ID repair, so
   one source field produces one diagnostic.
3. Review found that the new SVG/PDF null-ID early return lacked its normalized
   working-record release. It now registers that record; an existing private
   trace-seam regression verifies the exact bytes are released before encoder
   workspace acquisition for absent, null and invalid IDs.

## Review and approval

Two-axis review found no hard Standards violations and two low-priority fixture
naming/duplication suggestions. The Spec review's accounting and positive-vector
findings were corrected and rechecked; the final review supports all 42 owners
with no remaining actionable findings in this scope.

The maintainer's continuing “approve all” / “do it” authorization covers these
assignments and fixes. Totals: **646 assigned / 282 gaps** (131 surface and 151
other). Global approval remains pending; #85 is not yet complete.

## Verification

- Five new field matrices and all 40 existing diagnostic-contract tests pass.
- The focused normalized-image lifetime trace regression passes after failing
  against the missing-release implementation.
- Workspace/all-target typechecking passes.
- All eight canonical conformance checks and both lint checks pass (the initial
  Clippy let-and-return finding was corrected and its gate rerun successfully).
- Final full workspace suite: `dagger check excalidraw-render:test` passed fast,
  render/parity and doctest tiers in 16m22s.
