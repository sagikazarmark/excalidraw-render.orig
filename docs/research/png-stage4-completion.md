# Stage 4 completion — 2026-09-20

The user approved both final formatting-requalification subjects and finishing
the Stage-4 commit. The actual decision is recorded on
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5751675894).
This document is the final checkpoint; earlier pending/failed candidate and
proposal records remain immutable historical evidence.

## Applied approval and ownership

- Representative: `representative.stage4-frames-text-assets.v8`.
- Representative subject:
  `sha256:aaa772430f342a60807ef97255bf2296041cbcf23dff4c2aa4284052deb92454`.
- Formatting/source-requalification v2 subject:
  `sha256:844e401e1601f0f36d8511f709ebe8834224d83340e2b10fd444dd545c39f6ef`.
- Actual approver: `sagikazarmark`, approval date `2026-09-20`.
- Applied records: `tests/conformance/coverage/stage4-format-application-v2/`.

The authenticated transaction changed exactly 57 test-owner source bindings and
the existing representative selection from V7 to approved V8. It preserved
criteria, other dispositions, all three approved exclusions, all 168 original
inventory execution keys, the six focused-case approvals and the catalog.

| Measure | Final value |
| --- | ---: |
| Obligations | 1,874 |
| Primary owners | 1,800 |
| Approved exclusions | 3 |
| Assigned/excluded | 1,803 |
| Explicit gaps | 71 |
| New Stage-4 owners | 60 |
| Supplemental representative execution keys | 3 |

The guarded application and subsequent `verify-applied` both passed. Real V8
execution, registry execution, strict representative ownership, inventory and
ownership verification passed during the transaction. The applied audit also
authenticates the saved formatting decision against its actual GitHub comment
and the corresponding V8 approval.

## Verification

- The substantive full workspace run passed all targets except the two parity
  targets affected by the six stale exception records. Following the separately
  approved metadata update, those targets passed: PNG 10, SVG 8. This is combined
  verification, not a claim of a subsequent clean full-workspace rerun.
- After formatting, 64 targeted Rust tests passed: PNG Stage-4 25, interactions 4,
  dark-mode/frame/image 10, typography 6, protected PDF 1, candidate binding and
  approval negatives 18.
- Fourteen isolated approval/rollback tests passed. They cover optimized Python
  rejection, required exact authorization, persisted decision consistency,
  unrelated transaction rejection, write ordering, rollback and retry.
- Workspace Clippy with warnings denied and workspace formatting checks passed.
- Independent Standards and Spec re-review found no remaining scoped blocker
  after correcting the new approval-application checker.

Sixteen artifact/full-report comparisons across 32 fresh release processes were
byte-identical to the pre-format approved captures: representative SVG/PNG/PDF,
protected backend PDF and SVG/PNG for six focused cases. Representative viewer
rasters and both protected-PDF viewers were unchanged. The release executable
was also byte-identical. The same three PDF exception scopes remain approved;
no limits were widened for formatting.

The six renderer/test files equal pinned rustfmt output. The external typography
test's standard-math to pinned-libm correction is separately recorded as a
test-only portability change. The sole accompanying xtask change versions the
immutable colour-space bridge directory. Original approval packets are preserved.

## Current audit command

```sh
python3 tests/conformance/proposals/stage4-format-requalification-v2/requalify.py verify-applied
```

This is the current formatting-successor audit. Historical V7, protected-PDF and
six-case checks against their old source snapshots remain historical; their
bindings were not silently rewritten. The rejected formatting v1 application
entry point must not be used.

## Remaining release work

The native baseline still needs reviewed source/inventory requalification;
packaged native/WASM equality work remains open under #178. Global ledger
approval and release closure remain pending with 71 explicit gaps and eight
unresolved inventory milestones. The pre-existing fixed-size moxcms transform
allocation/resource gap remains open: these scoped approvals do not establish
end-to-end CMS out-of-memory safety.
