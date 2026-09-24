# JPEG umbrella facets, native baseline and V22 successor — proposal

Three exact subjects for one #85 decision. Nothing is applied until a genuine
approval comment exists on #85.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `cc97ec311758f244fad5c5f41dc21591fae40e1e9f46fa419ee221bae6e7050e` |
| Native baseline (`tests/conformance/coverage/native-baseline`) | `9dfb7dc101a5bf5350248bee33028aa17e8b0b8228fb622f10839bb13985c2e7` |
| Representative V22 (`representative.stage4-frames-text-assets.v22`) | `ae96b4c98f3ab2cbb61b4c31bddb7391ba8be2500d34785b2114da30864ecb31` |

Evidence and renderer changes are in `docs/research/jpeg-umbrella-facets.md`
(ec30177). The policy is the user's choice on #85: match what the reference
draws completely, keep strict recovery where it paints partially, keep the
approved strict MPF recovery as a declared divergence, and close hdr-gain-map
with the #190 HDR-base pin.

- **Owner tranche:**
  - Owners for the eight JPEG facets, in `jpeg_container_contract` and
    `jpeg_limit_contract`.
  - 28 JPEG ICC rows whose criteria were rewritten from strict recovery to the
    reference's untagged drawing (ec30177) move to
    `jpeg_icc_contract::malformed_profiles_and_sequences_draw_untagged_as_the_reference`.
  - Three `image.unsupported-format` rationales now say that unparseable
    JPEG and WebP profiles are ignored.
- **Native baseline:**
  - Two keys change: `focused.image-codecs.v1` PNG and PDF.
  - The case's JPEG carries an sRGB-equivalent ICC profile, which now draws
    untransformed, as the reference draws it.
  - Against the official PNG, the changed region's error falls from 11816 to
    3981, and the maximum channel difference from 3 to 2.
  - The report is unchanged, and the case still verifies against its
    official evidence.
- **V22:** the section 8.2 JPEG edit staled V21. V22 is collected with bridge
  v16 and reproduces V21 exactly.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- V22 `candidate-verify`, `verify-native-baseline` and all xtask tests
  passed.
- `verify-ownership` reported 2400 of 2418 assigned or excluded, and 18 gaps.
- All 58 JPEG fast tests passed.
- The Dagger native replay of a temporary dry-run commit matched all 168
  expectations of the staged baseline.
- Before the dry run, the full library (462) and fast (681) suites and clippy
  passed, and the JPEG record reproduces byte for byte.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v22-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
    cargo run --locked --release -p xtask -- conformance verify-native-baseline
