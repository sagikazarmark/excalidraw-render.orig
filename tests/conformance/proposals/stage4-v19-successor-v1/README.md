# GIF umbrella facets and V19 successor — pending approval

Two exact subjects for one #85 decision. Nothing here is approved or applied.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `1880c14ff7b0663ec38787172e4833746c83a6a3fdad2c19dec360c9a749610f` |
| Representative V19 (`representative.stage4-frames-text-assets.v19`) | `871ad2497cfce1861dcfabd890f87853f1efba818e562c577d6ced3e37dd0e57` |

Evidence and renderer changes are in `docs/research/gif-umbrella-facets.md`
(250425f). The policy is the user's choice on #85: match the pinned reference
wherever it draws a complete, well-defined image, and keep strict recovery for
first frames it paints only partially.

- **Owner tranche:** owners for the eight GIF facets. The 27 frozen control
  rows the reference draws completely move to the record's malformed owner.
  The 17 strict control rows keep their suites with reference-backed
  rationales. All 44 control criteria are rewritten (250425f).
- **V19:** the 8.2 GIF edit staled V18. V19 is collected with bridge v13 and
  reproduces V18 exactly.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. V19 `candidate-verify` and all xtask tests passed.
`verify-ownership` reported 2375 of 2418 assigned/excluded and 43 gaps. The
full library (412) and fast (653) suites passed. The GIF record reproduces
byte for byte, and the Dagger native replay matches all 168 approved
snapshots.

## Applying

After a genuine approval comment on #85:

    python3 tests/conformance/proposals/stage4-v19-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
