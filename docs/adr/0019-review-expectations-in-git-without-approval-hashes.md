---
status: accepted
---

# Review expectations in Git without approval hashes

Hash-bound issue-comment approvals duplicated repository review and coupled
routine tooling maintenance to historical evidence bundles. The maintainer chose
normal Git/PR review plus reproducible tests as the acceptance mechanism for all
evidence families. Approval subjects, reviewer identities, dates and comment URLs
are historical metadata, not executable gates; approval-writing commands retire.

Expected artifacts, diagnostics, explicitly scoped tolerances, completeness,
independent negative controls and fresh replay remain authoritative. Downloaded
tool checksums, input/output descriptors and native-library identity checks remain
enforced. Historical producer-source hashes describe past runs and do not require
a metadata refresh for source-only edits; collection still checks that sources
do not change during a run. Old records remain readable without being relabeled
as newly generated results.

This supersedes the approval-gate requirements in ADR-0011, ADR-0015, ADR-0016,
ADR-0017 and older conformance/specification workflow prose. It does not remove
their substantive rendering, scope, ownership or evidence-completeness checks.

Amended 2026-10-14: the rule covers all reference-oracle and PDF.js harness
code, including their dependency pins. A Dependabot security update to
`tools/reference-oracle/package-lock.json` and `tools/pdf-harness/pdfjs/` failed
every PR gate, although no gate runs those producers: about fifteen codec
contract tests, the frame reference profile, the SVG and PDF.js viewer profiles,
the conformance runners and the candidate and diagnosis chains still compared
their digests to the live files, so any edit to a shared oracle script failed
them too.
Everything under `tools/reference-oracle/` except the fixtures it reads, and the
PDF.js harness, is now historical wherever it is bound (`historical_producer` in
the candidate gate, `is_oracle_producer` in the renderer tests). `catalog verify`
still reads the Excalidraw release from the oracle lockfile. A PDF.js update
needs no requalification of its own: the adapter records the release it ran in
each receipt, and the harness gates judge its fresh rasters against the
expectations, so a release that renders differently fails there and its new
expectations are reviewed like any other.

The same amendment drops the digest pins that eight codec contract tests and
`png_projection_contract` held on shared test helpers (`common/raster.rs`,
`png_profile_contract.rs` and others). A pinned helper failed unrelated tests on
any edit; what a helper computes is checked by the assertions that use it, as
ADR-0017 already holds for source anchors.
