# varmq #84 evidence proposal, v2

Unapproved compact collection for `representative.varmq-queue-diagram.v1`,
re-collected after the PDFium geometry-loss fix (`ca5c3a2`). The reference
collection is unchanged from v1; the native capture and viewer evidence are new.
`compact.json` binds the digest of every file of the full collection.

Results: 2 of 10 comparisons pass (both light SVG viewers), as in v1. PDFium now
draws both cross-hatch directions; see `../varmq84-evidence-v1/DECISION.md`
(follow-up section) for the diagnosis and remaining open work, and
`../varmq84-pdf-subpixel-exception-v1/PROPOSAL.md` for the proposed light PDF
`worker-arrow` exceptions.
