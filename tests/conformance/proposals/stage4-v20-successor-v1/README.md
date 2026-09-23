# BMP and ICO umbrella facets and V20 successor — proposal

Three exact subjects for one #85 decision. Nothing is applied until a genuine
approval comment exists on #85.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `bb7e44e49db6a7ef532b0ffcdb071bc14efeab409579a4fdf39f9e0021fcefe5` |
| BMP color exceptions (`tests/oracle/bmp-ico-container/color-exceptions.json`) | `5cb51c8c0ee4175edb6c490d8417d1786437732a5d52adb00b37d04680a0446a` |
| Representative V20 (`representative.stage4-frames-text-assets.v20`) | `347cc831827ba921467c95d40e8713060cec042d17fbe91e9e756394b13ae510` |

Evidence and renderer changes are in `docs/research/bmp-ico-umbrella-facets.md`
(b858d30). The policy follows the user's GIF choice on #85: match the pinned
reference wherever it draws a complete, well-defined image, and keep strict
recovery where it loads a file but paints it partially or not at all. It also
declares one new divergence, a reference defect: a PNG icon entry that needs a
color transform. The reference paints it fully transparent, and the renderer
draws it as the same PNG standalone.

- **Owner tranche:** owners for the seven BMP and seven ICO facets, in
  `bmp_ico_container_contract` and `bmp_ico_limit_contract`.
- **Color exceptions:** six exact-ceiling views, in PNG and PDF, for three
  color-managed BMP rows: calibrated near-sRGB, embedded P3 and embedded
  linear. Each differs from Skia by one channel unit on one pixel, with 0
  Pixelmatch changes.
- **V20:** the 8.2 BMP/ICO edit staled V19. V20 is collected with bridge v14
  and reproduces V19 exactly.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- V20 `candidate-verify` and all xtask tests passed.
- `verify-ownership` reported 2389 of 2418 assigned or excluded, and 29 gaps.
- The 14 BMP/ICO owner tests passed. Before the dry run, the full library
  suite (436) and the rest of the fast suite (655) passed. The 12 remaining
  fast tests wait on the exception approval.
- The BMP/ICO record reproduces byte for byte.
- The Dagger native replay matches all 168 approved snapshots.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v20-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
