# Nested-SVG umbrella facets and V21 successor — approved and applied

Two exact subjects for one #85 decision, approved by the user on 2026-09-24
([#85 comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5806017518))
and applied with `apply.py`. After application, V21 `candidate-verify`,
`verify-ownership` (2392 assigned/excluded, 26 gaps) and the three owner tests
passed. The rest of this README preserves the reviewed proposal.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `9836b65afd5ce3a40070d9329d91e10be1a614e901fb7aebdd05efb9c75f4c23` |
| Representative V21 (`representative.stage4-frames-text-assets.v21`) | `1f9e1f8ad8b92823984342194ca9bcd93fcd67db6ce52137ffe3931ef7feed50` |

Evidence and renderer changes are in `docs/research/nested-svg-umbrella-facets.md`
(d455790). The policy is the user's choice on #85: fix the sanitizer bugs and
match the reference wherever the result stays static and local. Animation
state, `foreignObject` and the external-image placeholder are declared
divergences.

- **Owner tranche:** owners for sanitization, references and pdf-fallback.
- **V21:** the section 8.2 nested-SVG edit staled V20. V21 is collected with
  bridge v15 and reproduces V20 exactly.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- V21 `candidate-verify` and all xtask tests passed.
- `verify-ownership` reported 2392 of 2418 assigned or excluded, and 26 gaps.
- The three owner tests passed.
- The full library (453) and fast (670) suites and clippy passed.
- The nested-SVG record reproduces byte for byte.
- The Dagger native replay matches all 168 approved snapshots.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v21-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
