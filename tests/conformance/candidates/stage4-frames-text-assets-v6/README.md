# Representative candidate v6 — historical dev-profile evidence, unapproved

Spec2 review identified that §14.4 requires release-mode native qualification.
V6's measured results remain valid historical dev-profile observations, but its
subject must **not** be requested for final approval. Preserve these files and
collect a fresh release-profile successor only after the parent supplies final
production/spec hashes. The current adapter deliberately rejects this dev capture.

V6 restarts the incomplete v5 collection after restoring the exact pinned
PDFium wheel. The scene, reference inputs, request contract and crops are copied
unchanged from v4. V5's partial observations remain preserved.

Historical dev-profile subject (superseded for final qualification):
`sha256:ed8d3128b2cc9f964055af6df971599c0b916d54568962f680d46e9b216e979b`.
The manifest still has `approval: null`; approved execution must refuse it.

All three native artifacts/full reports repeat in two fresh processes. All six
viewer/input pairs repeat in two fresh processes. Both SVG comparisons and all
nontext crops are exact. PNG whole/typography pass canonically with two changed
pixels and SSIM 0.9995282750642132 / 0.9973446254064746 respectively.

The three exact PDF exception proposals are bound into the subject: PDFium whole
and typography, and PDF.js whole. PDF.js typography remains canonical. All
effective visual gates, semantic assertions and producer/profile/derivation
checks pass. These are proposals for human review, not granted exceptions.

See [the collection audit](../../../../docs/research/png-stage4-v6-collection.md)
for exact limits, evidence paths, review commands and the separate CMS resource
gap that this representative qualification does not close.
