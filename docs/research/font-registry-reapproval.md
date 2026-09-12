# Font registry reconciliation: approval proposal

Base commit: `ea78042`. Status: staged, **awaiting explicit maintainer reapproval**.

Proposal identity (SHA-256 of canonical JSON without trailing LF):

`sha256:235b7e8b3573ddb11362159596c7d51663ad0b60ce2123fa914e31ab5ed02531`

The [machine-readable proposal](font-registry-reapproval.json) contains all 36 exact case approval subjects, original approval records and originating issue URLs. Its digest identifies this bundle for review; the existing verifier still requires each case's own subject and appropriately scoped issue comment when recording approval.

## Authority correction

Scene-surface SHA-256 changes from `14309697cdb5bf9ebe660dccf27c4c929c0f5edb7174443dd4f962544fcc4bea` to `579e57f6a10e032ed4f460d3576c9be6bf4e86f595bd6a1acef2232bf22141ce`.

Only font diagnostic registry wording changes:

- Liberation Sans substitution belongs to surviving ordinary Helvetica text and visible compatibility labels, with no recipe. Renderer-owned frame names and embeddable labels are silent, as §8.3/#175 already requires.
- Noto substitution belongs to surviving ordinary text or embeddable labels, only when Noto supplies a glyph. Ordinary text has no recipe; embeddables name the offline recipe. Frame names are outside this substitution scope.
- Missing-glyph occurrences follow logical scalar order in surviving text, visible frame names and generated labels, excluding default-ignorables and truncated/omitted text.

The spec's identity table, pinned verifier constants, incorporated catalog and generated catalog identity are refreshed. The runtime field-disposition policy is unchanged. For every affected case, a structural comparison proves that only `reference.scene_surface.digest` and `approval` changed. Scenes, expected reports, artifacts, provenance, semantic expectations, exceptions and comparison settings remain byte-identical. Previous approvals are preserved in the proposal and Git history, not reassigned to new subjects.

## Completed tests

The combined font owner candidate now checks zero-width as well as zero-height embeddables: PNG omits labels and diagnostics; SVG/PDF retain Noto and missing-glyph diagnostics. Geometry limit 20 admits the embeddable outline but rejects the generated label at 716 segments. Work limit 2000 rejects label shaping at 25152 units; PNG additionally rejects the surviving outline's pixel work at 1635904 units. Exact full reports retain those resource diagnostics and reject font diagnostics for omitted labels.

The prior geometry probe at limit 10 rejected the outline itself, so it was replaced by limit 20 to exercise the intended label-accounting branch. Temporary probes were removed. No production rendering implementation changes are part of this staging round.

## Verification and pending gates

- All 36 case subjects computed successfully through `xtask conformance approval-subject`, which validates each candidate without requiring approval.
- `cargo run --locked -p xtask -- catalog verify`: passed.
- All 13 `diagnostic_contract` tests passed.
- `dagger check excalidraw-render:lint`: formatting and clippy passed.
- `git diff --check`: passed.
- Ownership/registered-case gates currently reject missing case approvals, as intended.
- Native baseline verification rejects its stale inventory binding. Its approved metadata and expected output bytes have not been repinned. After case approval, refresh the inventory and case-owner hashes again, then prepare/review the baseline binding or fresh collection as required by its provenance contract.

The ledger retains 137 proposed owners and 791 gaps; the nine font obligations remain gaps pending approval and downstream evidence reconciliation. This proposal does not approve baseline changes or global release closure.
