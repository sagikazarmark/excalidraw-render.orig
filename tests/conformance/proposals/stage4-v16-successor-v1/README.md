# #191 partial-alpha PDF exceptions, owner tranche and V16 successor — approved and applied

Three exact subjects for one #85 decision, approved by the user on 2026-09-23
([#85 comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5796115385))
and applied with `apply.py`. After application, the WebP contract, V16
`candidate-verify`, all xtask tests and `verify-ownership` (2355 assigned/excluded,
63 gaps) passed. The rest of this README preserves the reviewed proposal.

| Subject | SHA-256 |
| --- | --- |
| #191 PDF exceptions (`tests/oracle/webp-sample/pdf-partial-alpha-exceptions.json` without `approval`) | `db59f3eb97375586589fd1b6934542a92b23a938c17c891dea9680a03e52af3c` |
| Owner tranche (`owner-tranche.json`) | `e08b4107a594c6b3e9efc80d519040ad8c6346a39f9671fb76ae580262203fa5` |
| Representative V16 (`representative.stage4-frames-text-assets.v16`) | `fb11e4c041b715f06b11b465e615cb440ae1d56bbfaf8ce5052eeff064428c40` |

## #191 exceptions

The 24 partial-alpha WebP vectors (12 cells) already match the reference in SVG
and PNG and keep exact PDF alpha and lossless straight samples. Their only
failing views are the 48 PDF black/white composites: 6–43 differing pixels,
maximum channel difference 1, 0 Pixelmatch changes, and the small-artifact
mismatch outcome (all are 9×7). The cause is diagnosed in
`docs/research/webp191-pdf-comparison-decision.md`. The canvas truncates
partial-alpha premultiplication at decode, while the PDF carries the exact
straight samples, which both pinned viewers composite with rounding. Each view's
exception records its exact observed values with no margin.

Spec section 14.4 gains the small-artifact form of an exception, because a
sub-11×11 artifact has no SSIM score (fa50bf0). No comparator, budget or
production change is made.

`webp_sample_contract::partial_alpha_cells_match_reference_under_approved_pdf_exceptions`
owns the 12 cells. It fails when the approval does not bind the record, a view
grows beyond its ceilings, a failing view has no exception, or an exception's
view passes the canonical comparator (dead). The ceiling-growth and
missing-exception mutations were checked.

## V16

The spec edit staled V15 again. V16 is collected by `collect.py` with bridge
v10. It reproduces V15's native artifacts, reports, viewer rasters and three
exception ceilings exactly, and `candidate-verify` passes.

## Dry run

With placeholder approvals, `apply.py` validated end to end and was reverted.
The WebP contract (9 tests), all 631 fast tests, all xtask tests and V16
`candidate-verify` passed. `verify-ownership` reported 2355 of 2418
assigned/excluded and 63 gaps.

## Applying

After a genuine approval comment on #85:

    python3 tests/conformance/proposals/stage4-v16-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
