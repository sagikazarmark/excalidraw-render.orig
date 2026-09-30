# Applied user-approved six-case metadata transition

Batch: `sha256:3074ae5df1aa0c91743c68f4217f066370698f3d8970bac02dec5b1aacc00e84`.

Actual user decision:
https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5751047975

The five `issue-*-comment.json` files are fetched GitHub API records. `approvals.json`
maps their actual identity/date/URLs to the batch and six normal case subjects.
The #152 comment covers three related cases. No prior approval was reused.

`applied-transition.json` records the exact 15 changed protected/coverage metadata
files, immutable before-file descriptors into the reviewed packet, final approved
file descriptors, two retired gaps, 122 authority/12 case-owner/18 input-digest
transitions and final counts. `final-catalog.json` is the normal derived catalog
after the genuine case approvals; pending-preview hashes were not substituted.

The original reviewed proposal remains immutable at
`tests/conformance/proposals/png-device-phase-six-case-v1/`. Its before-state
verification is a pre-application check. Current applied-state preservation is:

```sh
node tools/reference-oracle/scripts/verify-png-stage4.mjs
```

That audit validates the approved batch and exact live transition, then uses its
authenticated before bytes solely to verify earlier historical checkpoints.
Current case/inventory/ownership/v7 CLI checks always read the real applied files.

Final audit: `docs/research/archive/png-stage4-final-metadata-application.md`.
