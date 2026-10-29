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

On 2026-10-02 the same rule removed nine more directories (about 25 MB, the
tree at `48c0b22` has them). It also covers the completed coverage
applications under `coverage/`, whose transactions have no remaining reader:

- coverage: `jpeg187-application-v2`, `jpeg188-application-v2`,
  `jpeg189-application-v1`, `jpeg190-application-v1`,
  `webp191-application-v1`, `pdfium192-application-v1`,
  `stage4-format-application-v2` and `stage4-six-case-application`;
- candidates: `stage4-pdf-backend-requalification-v1`.

`stage4-format-application-v2`, `stage4-six-case-application` and the
candidate were named only by `jpeg187-application-v2/before-files.json`, which
went in the same change. The candidate was also read by
`backend_requalification_reports_retain_exact_bytes`, which built its path from
a version loop and so escaped the search; the test now reads only `-v2`, whose
approved PDFs and reports are byte-identical to the removed ones. `tests/conformance/.gitattributes` went too: every
path it named had been removed.

On 2026-10-14, after ADR-0019's amendment made producer code and measurement
records historical, the same rule removed twelve more directories (about
12 MB, the tree at `63736a3` has them). It also retired the scripts that were
their only readers, finished-round producers that nothing runs:

- candidates: `stage4-frames-text-assets-v3`, `-v5`, `-v7` through `-v12` and
  `-v26`;
- proposals: the superseded `slides84-evidence-v1`, `varmq84-evidence-v1` and
  `varmq84-evidence-v2`, whose retention tests now read the current versions;
- scripts: the eight `tools/reference-oracle/scripts/*-ownership.mjs` and the
  v3 producer (`stage4-representative-v3.mjs`, its runner and Dagger entry).

On 2026-10-28, after the subset profile refresh requalified the
representative as V28, the same rule removed `stage4-frames-text-assets-v6`
and `-v27` (about 3 MB, the tree at `83960b50` has them). V27's readers now
read V28, whose PDF, scene, judgment plan and judged rasters are
byte-identical; one of them built its path from a version number and so
escaped the literal search. V6 was the tree's only dev-profile native capture,
kept as the negative fixture its README asked to preserve; the two tests that
refused it now carry that build identity themselves and stage the capture
under `target/`.

On 2026-10-29 the same rule removed the six `tests/oracle/*/prior-catalog.json`
captures (about 7.7 MB, the tree at `7173a673` has them), the `extend-ownership`
inputs of the GIF, JPEG EXIF, JPEG gain-map and WebP sample migrations, which
authenticate only the migration that applied them and are read by nothing
afterwards; and the V7 representative's owner-transition record and its
approval comment under `coverage/`, whose owner V28 replaced.

Later the same day it removed the four one-shot migration commands whose
preconditions can no longer hold — `reconcile-stage4-bindings`,
`rebind-font-packaging`, `rebind-font-packaging-ownership` and
`ownership-authority-migration` each authenticated one past ledger or spec
state and refused every input since — together with the four
`coverage/*-reconciliation*.json` records only they read (about 230 KB, the
tree at `303a9110` has them) and five finished-round producer and diagnostic
scripts that no runner, script or test named (`encode-codec-fixtures.mjs`,
`stage4-excalifont-canary.mjs`, `verify-png-stage3.mjs`,
`stage4-image-decoded.mjs`, `stage4-typography-compare.rs`). Frozen
provenance still names two of the scripts under paths the gate treats as
historical, so nothing reachable from a check observes their absence.

## Consequences

- A future round deletes its own collection once the gate no longer reads it,
  in the change that makes it unread, instead of leaving it for a later sweep.
- An audit of a removed round starts from `git checkout 20ab732 -- <path>`.
- Nothing about what the gate checks changes: every record it reads is still
  present with the same bytes.
