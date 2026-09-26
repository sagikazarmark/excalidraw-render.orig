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
