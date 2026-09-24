# closure:pdf-judgments record owner and V24 successor — approved and applied

Two exact subjects for one #85 decision, approved by the user on 2026-09-24
([#85 comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5811151087))
and applied with `apply.py`. After application, V24 `candidate-verify` and
`verify-ownership` (2401 assigned/excluded, 17 gaps) passed. The rest of this
README preserves the reviewed proposal.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `ab2822e95238105de6d92429cbd069712dc94f8f26297b160e6c497f9329937f` |
| Representative V24 (`representative.stage4-frames-text-assets.v24`) | `9cdcbf3efdb0ab888f3a3ec4672f2da09f07bf02fc6d2300a68c155daeeb2f67` |

ADR-0015, accepted by the user on #85, adds the `record` owner kind. The
xtask implementation is in the commit after 6de50c5. It adds:

- the owner kind, with every validation the ADR requires;
- a compiled-in verifier registry, whose only entry today is
  `pdf-judgments`;
- `record-closure-owner` and `record-ownership-approval`.

- **Owner tranche:** `closure:pdf-judgments` gets a record owner bound to
  `tests/conformance/coverage/pdf-judgments.json`, subject
  `sha256:f0b9119f…`, which was approved on #85. Every ledger verification
  runs the verifier, with approval required.
- **V24:** the xtask changes staled V23. V24 is collected with bridge v18 and
  reproduces V23 exactly.

One test fixture was corrected after the freeze (6a0c1b4):
`xtask/tests/coverage.rs` now picks non-closure rows. The representative
does not pin that file.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- The record owner was recorded.
- `verify-ownership` reported 2401 of 2418 assigned or excluded, and 17 gaps.
- V24 `candidate-verify` and all xtask tests passed (120 unit tests and
  every integration target).
- The Dagger native replay of a temporary dry-run commit matched all 168
  expectations.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v24-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
