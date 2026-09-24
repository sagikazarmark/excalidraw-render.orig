# PDF judgment record and V23 successor — approved and applied

Two exact subjects for one #85 decision, approved by the user on 2026-09-24
([#85 comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5810015754))
and applied with `apply.py`. After application,
`verify-pdf-judgments --require-approval`, V23 `candidate-verify` and
`verify-ownership` passed. The rest of this README preserves the reviewed
proposal.

| Subject | SHA-256 |
| --- | --- |
| PDF judgment record (`tests/conformance/coverage/pdf-judgments.json`) | `f0b9119f08e53d2d51873960722d4ab15619f6e2510291aa0389e6ad2f390f4a` |
| Representative V23 (`representative.stage4-frames-text-assets.v23`) | `cc937c4080445feaa4457256a6295174374612fc219202d9371ac97c8fc6c7dd` |

This is the design the user chose on #85 for `closure:pdf-judgments`: one
approved #85 record binds every PDF expected report and every specialized PDF
judgment file (730771d). The record has two parts.

- **Expected reports:** there is one for each of the 56 PDF executions. Before
  this, 48 had none.
  - They are derived by `xtask/src/coverage/pdf_judgments/derive.rs` from each
    case's approved SVG/PNG declarations, under the spec's format rules.
  - Per-case PDF-only declarations each cite a spec anchor. All 37 are empty
    today: no case has a PDF-only diagnostic.
  - The 8 typography and frame keys reference their existing
    manifest-owned reports.
  - All 56 derived reports equal the approved native baseline byte for byte.
    They are derived independently; none is copied from it.
- **Judgment files:** 20 files are bound by digest, with their existing
  approval references:
  - the #44 backend structures;
  - viewer qualification;
  - the structure and viewer profiles;
  - the per-case PDF exceptions and structures;
  - the representative PDF reports.

  A completeness rule classifies every PDF file under `tests/conformance`; 39
  are explicitly non-judgments. A new, unbound PDF judgment file fails
  verification.

Native snapshot verification, staging and replay now compare all 168 reports
with an expected report, where before only 120 were compared.

**V23:** the new xtask module and commands staled V22. V23 is collected with
bridge v17 and reproduces V22 exactly.

`closure:pdf-judgments` stays a ledger gap. This record is the evidence it
needs, but no ledger owner kind can own a closure row yet.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- `verify-pdf-judgments --require-approval` passed.
- `verify-native-baseline` passed with all 168 reports equal to their
  expected reports.
- V23 `candidate-verify` and all xtask tests passed.
- `verify-ownership` is unchanged: 2400 of 2418 assigned or excluded, 18 gaps.
- The Dagger native replay of a temporary dry-run commit matched all 168
  expectations and all 168 expected reports.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v23-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-pdf-judgments --require-approval
