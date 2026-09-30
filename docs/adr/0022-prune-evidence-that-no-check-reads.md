---
status: accepted
---

# Prune evidence that no check reads

`tests/conformance/proposals/` and `tests/conformance/candidates/` had grown to
about 516 MB across the stage4, JPEG187–190, WebP191 and PDFium192 rounds,
and each round left its whole collection behind: frozen whole-tree freezes,
successor packets, superseded representative revisions and colour-bridge
collections. ADR-0017 and ADR-0019 already retired the producers and the
approval hashes that once read them, so most of that tree was data nothing
opened. It still cost every clone, checkout, Dagger source upload and
repository search.

The rule until now was that historical records "stay as they were". That rule
exists so a digest refresh never rewrites history to make a check pass, and it
still holds for every record that is kept. It does not require keeping a record
that no check reads: Git history already keeps those bytes, and nothing
reachable from a check can observe their absence.

## Decision

A proposal or candidate directory stays in the tree while something reads it:

- the gate or a test reads a file in it (the registry, the hard-coded tables in
  `conformance_candidate_gate.rs` and `conformance_candidate_exceptions.rs`,
  representative `evidence_record` bindings, and the paths tests construct);
- a checked `{path, sha256}` descriptor in kept evidence points into it;
- a script in the tree names it, even one CI does not run, so a rerun does not
  fail on a missing input.

A directory that meets none of these goes. On 2026-09-30 that removed 64
directories (about 464 MB, the tree at `20ab732` has them):

- proposals: `jpeg187-requalification-v1`/`v2`, `jpeg188-requalification-v1`/`v2`,
  `jpeg189-requalification-v1`, `jpeg190-mpf-requalification-v1`,
  `webp191-composition-requalification-v1`, `pdfium192-alpha-audit-v1`,
  `pdfium192-successor-v1`, `png-device-phase-six-case-v1`,
  `stage4-format-requalification-v1`/`v2`, `stage4-v15-successor-v1` through
  `stage4-v26-successor-v1`, `varmq84-controls-v1`,
  `varmq84-dark-diagnosis-v1`, `varmq84-pdf-subpixel-exception-v1`,
  `varmq84-pdf-typography-exception-v1`, `velox84-pdf-exception-v1` and
  `slides84-pdfium-avatar-exception-v1`;
- candidates: `stage4-frames-text-assets-v13` through `-v25`,
  `stage4-pdf-colorspace-bridge-v1` through `-v19`,
  `stage4-image-colour-diagnosis-v1` and `stage4-image-icc-controls-v1`.

Kept records are not edited to drop mentions of removed directories. Free-text
rationales in `coverage/ownership.json`, frozen freezes, application logs and
archived research still name them; those names resolve against Git history at
`20ab732`. Deleting from the tree does not shrink the repository's history, and
history is not rewritten.

## Consequences

- A future round deletes its own collection once the gate no longer reads it,
  in the change that makes it unread, instead of leaving it for a later sweep.
- An audit of a removed round starts from `git checkout 20ab732 -- <path>`.
- Nothing about what the gate checks changes: every record it reads is still
  present with the same bytes.
