# PNG SDR color qualification (#85)

Base: `a9bac8be3caea91c25fbda4d467349f01e750969`.

This records the initial SDR tranche. The subsequent
[remaining-primary extension](png-cicp-primary-ownership.md) preserves all 340
cases below and adds 160, bringing the current record to 500 cases and the cICP
selection to 220 cells. `verify` now reproduces that extended record.

## Judgment and reference evidence

The maintainer explicitly selected **Canonical image budgets (Recommended)** in
the implementation session: use the existing Geometry/image limits (1/400 changed
pixels, minimum SSIM 0.999), alongside bit-exact semantic anchors, alpha,
dimensions, diagnostics, source-byte preservation and repeatability. No new
tolerance exception is introduced. This is the selected judgment class for this
tranche, not approval of the incomplete global ownership ledger.

The exploratory 75×4 strips exposed differences but cannot establish an approximate
SSIM judgment. The final authored fixtures are **15×15 spatially varying color and
alpha fields**, with grayscale-alpha/RGBA and 8/16-bit layouts. Their rows are
neither padding nor repeated easy pixels. The collector renders these new source
rasters directly; it does not resize reference artifacts.

`tests/oracle/png-color/inputs.json` authors ramps, alpha levels, layouts, metadata
signals and exact rejected-input classes. `png-color-fixtures.mjs` writes PNG
chunks, samples and minimal RGB ICC profiles directly. An opaque companion for
each source measures straight color independently of partial-alpha quantization.
The initial **340 cases** have complete scene/request inputs, official PNG
bytes, opaque-companion PNG bytes and official SVG digests.

Collection uses the immutable official Excalidraw v0.18.1 wrapper and exact-pinned
Chromium **149.0.7827.55**, linux/amd64, with networking disabled. Two fresh browser
processes execute forward and reversed case orders; each case has a fresh context.
Complete outputs agree. The record binds the inputs, fixture builder, collector,
runner, reference profile and existing comparator profile. The native and WASM
consumers share the record and visual/report gates.

Commands:

```sh
bash tools/reference-oracle/run-png-color.sh generate
bash tools/reference-oracle/run-png-color.sh verify
```

Generation refuses an existing record. The initial complete record digest was
`402845da735653bfbaf00e76f80726425f0cc84446c0ad4fbfd21f095127494c`.

## Recorded ownership scope

| Obligation group | Reference cases | Recorded selector in `png_color_contract` |
| --- | ---: | --- |
| sRGB intents, missing/invalid/duplicate signals | 40 | `srgb_intents_and_recovery_match_the_contract` |
| Linear gAMA | 4 | `linear_gamma_matches_the_reference` |
| Nonlinear gAMA, neutral boundaries, extremes and recovery | 48 | `nonlinear_gamma_and_recovery_match_the_reference` |
| cHRM, D65/D50 whites, with/without gAMA and unusable coordinates | 60 | `chromaticities_and_recovery_match_the_reference` |
| RGB ICC v2 matrix/TRC | 12 | `rgb_icc_v2_matrix_profiles_match_the_reference` |
| RGB ICC v4 parametric types 0–4 and unknown-type recovery | 32 | `rgb_icc_v4_parametric_profiles_match_the_reference` |
| Precedence, conflicting cHRM/gAMA, malformed higher signals, duplicates and late chunks | 84 | `metadata_precedence_and_recovery_match_the_contract` |
| SDR cICP: primaries 1/9/12 × transfers 1/4/5/8/13 × ranges 0/1 × RGBA8/16 | 60 | `sdr_cicp_candidates_match_the_reference` |

The recorded selection is seven existing metadata/profile obligations plus
**60 concrete cICP cells**. The range-zero cells characterize the reference's
ignored cICP signaling, rather than asserting narrow-range color conversion.
At this checkpoint, the other 644 cICP cells, broad ICC/LUT/model families, HDR/gain maps and profile
composition retain separate requirements.

Every exact selector also executes independent exact anchors: near-black sRGB
and ICC identity, BT.1886 transfer selection, narrow-range fallback, neutral-gamma
boundary decisions, linear gamma, extreme gamma and sRGB precedence. Identity
rasters compare byte-exactly. Transformed rasters use the unchanged canonical
Geometry/image comparator on complete alpha/black/white views, independently per
case. Reference-only discrimination checks require the valid D50-versus-D65 and
cHRM-conflict-versus-higher-priority pairs to fail this same visual gate, proving
that the new fixtures expose a wrong white point or metadata-selection path.
Exact dimensions, alpha and transparent-zero RGB are checked before visual
comparison. PDF RGB and attached mask samples are reached through the actual page,
image, ICC-space and soft-mask references, then compared with the opaque companion
plus independently authored source alpha. Source SVG payloads remain exact.

All formats compare complete reports and artifacts across two fresh facade
sessions. Rejected-input artifacts additionally match an independently authored
missing-resource control: omission in SVG/PDF, the declared placeholder in PNG.
Well-formed but unsupported color interpretations preserve their SVG payloads
without attempting color conversion.

## Demonstrated corrections

- The 8-bit CMS executor's 4096-entry output curve loses near-black samples even
  on an identity transform. Changing fixed-point matrix preference did not fix it.
  The 16-bit executor's 65536-entry curve does; source bytes expand exactly by 257
  and quantize once on output. A fixed 1024-word input/output scratch pair bounds
  temporary storage independently of image size. Existing asset-codec tests also
  exercise the shared WebP profile consumer; JPEG has its own transform path.
- CICP narrow-range metadata is not interpreted by the pinned PNG decoder; lower
  priority metadata is considered instead. BT.709-family transfers use the H.273
  reference EOTF, BT.1886 gamma 2.4, rather than the inverse camera OETF. Named sRGB
  and linear-sRGB cases avoid an unnecessary gamut round trip.
- cHRM without gAMA does not cause the reference to guess a transfer function.
  Without cHRM, `0.95 < 2.2*gamma < 1.05` is the reference's neutral-gamma window.
  Exact fixtures straddle both boundaries.
- `curve_from_gamma` represents an ICC u8Fixed8 exponent, which cannot represent
  the full PNG gAMA domain. Parametric powers retain the actual exponent instead
  of saturating large powers or rounding small powers to zero.
- Interpreted chromaticities outside the unit xy domain receive the existing
  unsupported-color recovery, rather than constructing a nonsensical gamut.

Reference implementation corroboration:

- [Chromium 149 PNG decoder](https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/png/png_image_decoder.cc)
  selects `SkPngRustDecoder`.
- [SkPngRustCodec, m149](https://github.com/google/skia/blob/chrome/m149/src/codec/SkPngRustCodec.cpp),
  `CreateColorProfile`, defines cICP range handling, metadata priority and legacy
  gamma/chromaticity behavior.
- [SkColorSpace, m149](https://github.com/google/skia/blob/chrome/m149/include/core/SkColorSpace.h)
  names the H.273 reference EOTFs and BT.1886 mapping.

## Rejection policy versus browser tolerance

The browser accepts some malformed ancillary metadata that the project's strict
PNG validator rejects. The input record declares each project outcome explicitly:
invalid sRGB intent bytes, duplicate sRGB/gAMA/cHRM/cICP and late color chunks use
`image.decode-failed`; zero gamma, unusable interpreted chromaticities, malformed
ICC or unsupported curve/signaling classes use `image.unsupported-format` for
PNG/PDF. Valid lower-priority source data is not invented after a declared
unsupported interpretation. The scene-surface manifest and §8.2 supply the exact
format-specific omission/placeholder contracts. Browser admission is recorded
separately and is not silently substituted for the project's rejection policy.

No valid profile is classified unsupported merely to avoid a visual mismatch.
The stricter recovery cases have exact complete diagnostics and artifact controls;
they are not exclusions or approximate visual comparisons.

## Verification checkpoint

Both review findings were addressed with a valid D50 white-point matrix and
distinguishable cHRM conflicts under sRGB/ICC/cICP. Final Spec review confirms all
**67** owners are eligible; Standards review has no hard breaches or actionable
smell findings. Source bindings for the existing sample oracles were refreshed
after their binding-aware PDF extractor was shared with these tests.

- All **nine** native SDR contract tests and **66** existing asset-codec tests pass.
- Workspace/all-target Clippy with warnings denied passes.
- Browser-WASM executes all **340** cases twice with the shared appearance,
  exact report/alpha and recovery gates: pass. The local worker uses installed
  Chrome/ChromeDriver 151.0.7922.173, separately from the pinned Chromium149
  reference collection. The 315 MB debug module timed out during runner
  processing; `CARGO_PROFILE_TEST_DEBUG=0` produced the passing optimized test
  module, without changing the asserted gates.
- Ownership validation passes: **1,096 assigned/excluded and 780 gaps**
  (1,093 proposed owners and three approved exclusions), **1,876** total.
- Final `cargo test --locked --workspace --no-fail-fast` with Rust 1.97.1:
  **1,106 passed, 18 ignored**. All 532 fast tests passed in 910.69 seconds;
  registered SVG/PNG parity, ownership and coverage CLI tests also passed.
  Ignored external-harness/materialization tests remain outside this local run.
- `bash tools/reference-oracle/run-png-color.sh verify` reproduced all **340**
  cases and the complete recorded digest byte-for-byte.
- `conformance verify-all` verified all **37** registered cases;
  `verify-inventory` verified the existing **168** executions. Formatting and
  diff checks pass. The temporary failing diagnostic probe was removed from the
  test suite; exploratory output remains clearly labeled under `target/`.

#178 and global release closure remain separate. No new exclusion, tolerance
exception, completed-ledger approval or target-equality claim is recorded.
