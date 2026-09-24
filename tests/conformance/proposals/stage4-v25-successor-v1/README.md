# Five-layer integration and V25 successor — proposal

Three exact subjects for one #85 decision. Nothing is applied until a genuine
approval comment exists on #85.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `76520561d8e4e167d9d598b12f2984fbc388a0c5f0054891f66366e962191e1b` |
| Layer-integration record (`tests/conformance/coverage/layers.json`) | `cd0d6e4a2c9ea1baa4d36c8e24b6be783a960810889dfc6cd4a9f18a600599c6` |
| Representative V25 (`representative.stage4-frames-text-assets.v25`) | `bab27df4f9dc2ff5b2424f2dcab27980271006ac41bc8cf25def63eafa348bef` |

This closes `closure:all-five-layers` through ADR-0015. The design is in
`docs/research/five-layer-integration.md`.

- **Owner adapter:** test owners may now name a nested library module, a root
  test binary of the renderer, or the comparator crate's test binaries. Each
  is validated as strictly as before. Existing owners keep their compact form.
  Case owners are allowed on single-candidate `layer` rows.
- **Catalog:** a new `layer` category adds 21 rows as #85 gaps.
  - **Vectors:** Rough.js, Perfect Freehand, PRNG, number format, fractional
    index, pixelmatch, SSIM, geometry, colour, shaping/fallback and format
    planning.
  - **Focused:** frame export.
  - **Adversarial:** the six fuzz and boundary targets.
  - **Regression:** the three regression corpora.
- **Owner tranche:** one reviewed owner per layer row, each an existing
  non-ignored test (20), plus the frame-export case. Every owner test passes.
  `closure:all-five-layers` gets a record owner bound to `layers.json`.
- **Layer-integration record:** the `layers` verifier rebuilds each layer's
  items by rule. It also checks the three #84 representative cases directly:
  their approval subjects (a port of `case.py`), sanitization digests and
  crops.
  - Every item must map to an owned row whose owner validates, and no
    `layer:*` row may be orphaned.
  - It binds only what defines the layers' contents: the rules, the
    family-sentence text, vector corpora, case ids, the #84 cases, fuzz risks
    and regression corpora.
  - A future successor, spec edit or catalog change doesn't make it stale,
    which a simulated successor apply confirmed. A new vector corpus, case,
    fuzz target, risk or regression id does, until it is re-approved.
- **V25:** the xtask changes staled V24. V25 is collected with bridge v19 and
  reproduces V24 exactly. Two earlier V25 collections were discarded unapproved
  while the record's bindings were reworked.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- All 21 owners and the closure owner were recorded, and
  `verify-layers --require-approval` passed.
- `verify-ownership` reported 2423 of 2439 assigned or excluded, and 16 gaps:
  14 delayed JPEG cells, `atomic-profile-inventory` and
  `process-target-equality`.
- V25 `candidate-verify` and all xtask tests passed.
- The Dagger native replay of a temporary dry-run commit matched all 168
  expectations.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v25-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
