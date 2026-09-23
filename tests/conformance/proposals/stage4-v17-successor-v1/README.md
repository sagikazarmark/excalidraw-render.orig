# PNG umbrella-facet owners and V17 successor — pending approval

Two exact subjects for one #85 decision. Nothing here is approved or applied.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `7524f797cc5ef104abfd6395f0414471e0f5381f70e94656978ee307eab63859` |
| Representative V17 (`representative.stage4-frames-text-assets.v17`) | `f7c47c5ba6f8d4af3fa2a4d62630efe7771a577ea36a04f9f1a4371590df5df5` |

## Owner tranche

These are the four PNG umbrella facets, with evidence and rationale in
`docs/research/png-umbrella-facets.md` (f9847ce):

- `codec:png:hdr-gain-map`: `png_color_contract::hdr_peak_luminance_and_tone_mapping_paths_match_the_reference`.
  The 170-row HDR metadata record came with a renderer fix, tone mapping from
  the `cLLI`/`mDCV` peak. Spec 8.2 states the rule.
- `codec:png:alpha` and `codec:png:color-profile`:
  `png_stage4_contract::png_alpha_and_profiles_compose_across_every_layout`,
  covering 1,968 frozen plus 840 supplemental crossings.
- `codec:png:limits`: `png_stage4_contract::png_limits_have_exact_boundaries_and_precedence`,
  covering the validation boundaries plus 13.2 multi-crossing precedence.

## V17

The 8.2 edit staled V16. V17 is collected by `collect.py` with bridge v11. It
reproduces V16's native artifacts, reports, viewer rasters and three exception
ceilings exactly, and `candidate-verify` passes.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was reverted.
V17 `candidate-verify` and all xtask tests passed. `verify-ownership` reported
2359 of 2418 assigned/excluded and 59 gaps (41 other-codec facets, 8 #190
cells, 6 #187 cells, 4 closure).

## Applying

After a genuine approval comment on #85:

    python3 tests/conformance/proposals/stage4-v17-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
