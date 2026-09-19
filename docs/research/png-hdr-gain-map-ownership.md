# Stage 3 — PQ/HLG and PNG gain maps (#85)

Base: `edbacbde66b7442b5e66a31cf7b53d749eddb263` (committed Stage 2).

## Scope and judgment

This stage proposes exactly **91 additional owners**:

- **88 cICP cells**: transfers **16/18** × primaries **1,4,5,6,7,8,9,10,11,12,22**
  × ranges **0/1** × **RGBA8/16**. All **704** enumerated cICP cells now have owners.
- `codec:png:gain-map:sdr-base`, `codec:png:gain-map:admission`, and
  `codec:png:gain-map:precedence`.

The user-selected canonical Geometry/image limits remain **1/400 changed pixels
and minimum SSIM 0.999**, independently per image. Exact dimensions, alpha,
transparent-zero RGB, reports, source bytes and semantic anchors remain hard
gates. No comparator, tolerance, inventory criterion or exclusion changed.

The existing collector's authenticated `extend` preserved all **896** previous
cases byte-for-byte and added only the 88 HDR cells. The current main record has
**984 cases**. A new supplemental input/fixture builder/collector/runner captures
**84 gain-map cases and 24 base controls**, leaving the original fixture builder,
collector and runner frozen. Every supplemental control is compared in full
(apart from its group label) to the main record. Both records use the exact
official v0.18.1 wrapper and pinned Chromium **149.0.7827.55**, two fresh browser
processes in forward/reverse order, fresh contexts and disabled networking.
Separate verification reproduced both complete record digests.

```sh
bash tools/reference-oracle/run-png-color.sh extend \
  9c53ba1857d7e6406863200a685e9078e68e109569fed9c96bbcc1bcfb9c95ad
bash tools/reference-oracle/run-png-gain-map.sh generate
bash tools/reference-oracle/run-png-color.sh verify
bash tools/reference-oracle/run-png-gain-map.sh verify
node tools/reference-oracle/scripts/verify-png-stage3.mjs
```

The read-only Stage 3 audit authenticates the base record, the exact new cell
set, frozen producer bindings, all prior cases, all live source descriptors,
exactly 91 gap-to-owner changes and source-hash-only refreshes for **623** earlier
owners. Every other disposition, selector, rationale, exclusion and approval
remains equal to Stage 2. The parent-review license correction below additionally
refreshes the inventory/catalog's packaged-consumer source bindings; the audit
authenticates the exact one-line license-pin change and permits only its three
source anchors and requirements descriptor to move.

## Primary-source interpretation

Chromium's pinned [DEPS][deps] selects Skia commit
`53348aa333da02b77c4b5797e2de722f5abde7d0`. The inspected `chrome/m149` files
matched that exact revision byte-for-byte. The important paths are:

- [PNG decoder][png-decoder]: selects **SkPngRustDecoder**.
- [Skia image decoder][image-decoder]: uses the decoded image's color space and
  N32/premultiplied storage for the ordinary browser path.
- [Transfer definitions][transfer]: PQ reference white **203 nits**; HLG white
  **203**, peak **1000**, system gamma **1.2**.
- [Color-space transform][xform]: PQ scales decoded absolute luminance by
  `10000/203`; HLG uses the inverse OETF, a luminance-dependent OOTF and `1000/203`.
  The OOTF luminance vector is defined in Rec.2020, transformed to source space.
- [Raster pipeline][pipeline]: the OOTF extends the power with its **sign**.
- [Chromium tone-map selection][tone-map] and [Skia AGTM implementation][agtm]:
  default RWTMO uses a 1000-nit peak, a Rec.2020 linear-light working space,
  maximum-component gain, and eight Hermite knots for the SDR Bezier shoulder.
  Below the knee its gain is **1/2**; values beyond the maximum are capped.

### Demonstrated production fixes

The initial real-facade probe failed: PQ neutral **128 → 24**, versus independently
captured **132**. The old single-pixel regression encoded the CMS default's
unscaled transfer, not canonical browser tone mapping. It now requires 132.

`asset/hdr.rs` implements the source-defined default BT.2100/RWTMO pipeline with
portable libm math, retaining extended/negative values through gamut conversion
until final output clipping. HLG neutral **128 → 99**. Independent exact anchors
also cover source bytes 0,8,32,64 and 255. Range-zero anchors require identity
without lower metadata and **188** under linear gAMA. Full-image reference
discrimination rejects sRGB, linear, BT.1886 and substituted PQ/HLG curves.

The first full matrix exposed two further discrepancies:

1. XYZ/HLG produced **18 changed pixels**, SSIM **0.96616**, when negative
   luminance was clamped. Skia's signed OOTF fixes this. An exact colored facade
   anchor requires `[255,16,16] → [0,255,0]` under primary 10/transfer 18.
2. Applying the transfer directly to straight source samples missed the browser's
   encoded-premultiplication step. At the authored XYZ/HLG RGBA8 sample
   `[160,32,32,85]`, the old projection was `[0,66,0,85]` while the reference was
   `[129,0,21,85]`. Preserving RGBA16 samples through encoded premultiplication is
   also necessary: stripping low bytes before this step left a failing cell.

The PNG HDR decoder therefore derives **two projections** from the source:
straight canonical RGB for PDF's opaque-companion contract, and the encoded-
premultiplied/draw-time projection for PNG. Alpha is preserved and invisible RGB
is zero. Row-wise decoding retains 16-bit precision without a full-image 16-bit
temporary; the two ordinary output planes and bounded row scratch suffice.
Adam7 rows use the decoder's pass information; a facade regression repacks four
independent reference sources into Adam7 and checks PNG/PDF against the same
reference images. Existing asset transaction tests and PNG resource tests pass.
No new semantic operation, nested discovery, or resource-policy constant is added.

## Gain-map selection and admission

The pinned **Rust** PNG decoder has no gain-map override, nor does its base
class; [SkCodec][codec] returns false by default. Skia's libpng decoder and its
`PngGainmapTest` exercise a different path. Implementing libpng gain-map
reconstruction here would contradict the canonical browser.

The supplemental fixtures follow [SkPngEncoderImpl][encoder] and
[SkGainmapInfo][gain-info]: **version-only outer gmAP**, a complete PNG in gdAT,
and **full ISO metadata in the inner PNG's gmAP**. They include valid SDR and HDR
bases, gain ratios **1..16**, black/white/checker gain pixels, multichannel
metadata and alternate color-space signaling. HDR-base white maps would change
neutral 128 to approximately 31 under gain reconstruction; the captured value
is exactly **128**. Both opaque and partial-alpha reference rasters instead
match the selected base **byte-for-byte**, including PQ/HLG bases and competing
ICC/sRGB/gAMA/cHRM. Native PNG/PDF and WASM PNG artifacts and reports also match
their base controls exactly. This establishes actual **ignored gain signaling
and base interpretation**, rather than merely demonstrating chunk acceptance.

Negative cases include absent and partial pairs, empty/truncated/future-version
metadata, missing/truncated inner metadata, zero rational denominator/gamma,
duplicate metadata/payloads, split/late payloads, empty/non-PNG/truncated nested
data, bad inner CRC, enormous inner dimensions and ten nested containers.
Because gdAT is ignored, no inner image is allocated or recursively decoded.
The outer payload still pays admission bytes: exact-size acceptance and a
one-byte-under-limit rejection are checked with complete reports/recovery in all
formats, while dimension=15, pixels=225 and nested-depth=1 limits permit the
actual 15×15 base. A bad **outer** CRC remains `image.decode-failed`.

Malformed selected cICP/ICC/legacy signals retain the existing strict
`image.unsupported-format` contract for PNG/PDF, with original SVG payloads;
duplicate cICP and outer-CRC failures retain decode-failure omission/placeholder
contracts. The supplemental collector records browser admission independently.
The renderer's stricter declarations are not silently replaced by browser
tolerance, and no valid failing profile is reclassified unsupported.

## Executed gates and ownership

The historical selector `png_color_contract::sdr_cicp_candidates_match_the_reference`
now executes the **704-cell** matrix, including the HDR anchors. The three gain
obligations select
`png_color_contract::gain_map_base_admission_and_precedence_match_the_reference`,
which also calls the bounded-payload tests. Both owner paths bind the shared
helper digest, reference digests, live producer descriptors and exact case sets.

Native checks execute SVG/PNG/PDF twice in fresh facade sessions: complete reports,
source-preserving SVG payloads, untagged PNG, dimensions/alpha/zero RGB and actual
PDF page/image/ICC/soft-mask bindings. WASM executes both records twice in a
dedicated browser worker with the shared appearance, report, equivalence and
semantic-anchor gates. Local Chrome/ChromeDriver **151.0.7922.173** executes the
consumer; pinned Docker Chromium149 supplies reference evidence.

All Cargo commands used:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --locked -p excalidraw-render --test fast png_color_contract:: -- --nocapture
cargo test --locked -p excalidraw-render --lib render::asset::tests::
cargo test --locked -p excalidraw-render --test fast png_precision_contract::
cargo test --locked -p excalidraw-render --test fast png_profile_contract::
cargo test --locked -p excalidraw-render --test fast static_png_asset_render::
cargo clippy --locked --workspace --all-targets -- -D warnings
export HEADLESS=1 WASM_BINDGEN_TEST_TIMEOUT=180
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
  --lib wasm_tests::png_sdr_color_reference_in_wasm -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
  --lib wasm_tests::png_gain_map_reference_in_wasm -- --exact
cargo run --locked -p xtask -- conformance refresh-test-owner-source png_color_contract
# record-test-owner selected only the explicit 88-cell product and three IDs above.
cargo run --locked -p xtask -- conformance verify-ownership
cargo fmt --all -- --check
git diff --check
```

Results: **17 color contracts**, **66 asset regressions**, **5 precision**,
**4 sample/profile**, **29 static-PNG** tests pass, with no failures/ignores.
Final color run: **9.75s**. Workspace/all-target Clippy passes with warnings denied.
Final executed WASM workers: **1 passed** each (**6.79s** / **0.62s**).
Reference verification: **984 + 108 cases**, exact complete hashes reproduced.
Ownership validation: **1,876 total**, **1,743 assigned/excluded**, **133 gaps**;
**1,740 proposed owners**, the same **three approved exclusions**.

Parent Spec review approved all **91** owners. Standards review identified two
hard breaches and a provenance question, addressed below. Follow-up independent
reviews confirmed both breaches and the provenance concern resolved, and retained
eligibility for all 91 owners.
No Stage 3 color/gain-map semantic decision blocker was found. This is **not all-profile HDR
qualification**: mastering-display/content-light/AGTM metadata, broader ICC/LUT
families, profile-axis composition and umbrella facets remain outstanding.
The Adam7 regression does not assign composition ownership. Global ledger approval
and #178 cross-target artifact/trace equality remain open.

## Parent-review corrections and adaptation provenance

The implementation process directly consulted Skia's `SkHdrAgtm.cpp`, including
`PopulateUsingRwtmo` and gain-curve evaluation. The Rust implementation
specializes the source's SDR alternate, algebraically reduces its Bezier
coefficients for default headroom, and translates its Hermite log-gain evaluation.
It was **not independently developed from the standard's equations alone**.
Although no wholesale file copy occurred, it is documented and distributed as
an adaptation, without making a legal claim about which individual equations
are protected expression.

`asset/hdr.rs` now retains **Copyright 2025 Google LLC**, the exact source
revision, and a reference to the distributed license. The authoritative
`crates/excalidraw-render-assets-base/license-parts/skia-BSD-3-Clause.txt`
contains that attribution, modification description, and Skia's complete
[BSD-3-Clause license][skia-license], including its **Copyright (c) 2011 Google
Inc.** notice, conditions and disclaimer. `xtask` includes this part in all five
distributed license bundles. The public `license_bundle()` and CLI `--licenses`
paths therefore carry the complete attribution. Rendered pixels contain no Skia
code; this is a software-distribution notice, not a new output-traveling asset
notice or an alteration of the frozen scene-surface catalog.

The two Standards fixes are:

1. `portable_math::{exp,exp2,log2}` now wrap the same pinned libm functions;
   every HDR transcendental call goes through that module, including the existing
   `powf`. This restores the ADR-0007/spec §9 boundary without changing the math.
2. `rwtmo_gain` uses fallible bounded interval selection instead of production
   `unwrap`. Nonfinite input, a missing/invalid interval or a nonfinite/nonpositive
   computed gain returns `ResolveError::DecodeFailed`. `project_linear` checks
   all channels before `max` can discard a NaN and checks its output before
   quantization. `project_pixel` and `decode_hdr_png` propagate errors with `?`.
   There is no substitute interval or default pixel for an invalid projection.
   Two new private tests exercise curve bounds, NaN/infinity/out-of-domain input,
   missing intervals, invalid coefficients and projection-failure propagation.

### Consuming bindings and remaining baseline action

The public bundle's intentional addition changes its exact license-only hash.
The fast license test additionally requires the entire new license part. The
packaged public-facade source changes **only that license hash**, preserving all
artifact/report pins and test bodies. Its three evidence-source anchors in the
coverage requirements, their derived inventory copies, the requirements source
descriptor, and the ownership catalog digest are refreshed explicitly. The
read-only Stage 3 audit checks these exact changes and proves all **168 execution
keys**, coverage criteria, milestones and ownership dispositions are preserved.
No owner-source refresh is needed: `png_color_contract.rs` and its shared helper
are unchanged by the review corrections. All **91 owners** remain assigned.

The additional `verify-native-baseline` check now fails with
`stale snapshot source tests/conformance/coverage/inventory.json`: the approved
baseline binds the previous complete inventory descriptor. **Its record,
artifact/report expectations and approval have not been rewritten.** Parent
clean-commit collection, binding review and baseline reapproval are required;
the workflow is documented in [the earlier baseline reconciliation][baseline].
This downstream binding requirement is distinct from the passing Stage 3
color/gain-map qualification. Packaged-consumer replay and baseline reapproval
remain a release follow-up. The parent full workspace run passed **1,136 tests,
with 18 ignored**, including all 543 fast tests in 938.74 seconds. All 37
registered conformance cases, inventory and ownership validation also passed.

### Verification after corrections

Using the same Rust/toolchain/WASM environment documented above:

```sh
cargo test --locked -p excalidraw-render --lib render::asset::hdr::tests::
cargo test --locked -p excalidraw-render --lib portable_math::tests::
cargo test --locked -p excalidraw-render --test fast png_color_contract::
cargo test --locked -p excalidraw-render --lib render::asset::tests::
cargo test --locked -p excalidraw-render --test fast license_bundle::
cargo test --locked -p excalidraw-render-cli --test render_cli \
  standalone_commands_expose_revision_identity_and_exact_licenses -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
  --lib wasm_tests::png_sdr_color_reference_in_wasm -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
  --lib wasm_tests::png_gain_map_reference_in_wasm -- --exact
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo run --locked -p xtask -- catalog verify
node tools/reference-oracle/scripts/verify-png-stage3.mjs
cargo run --locked -p xtask -- conformance verify-ownership
cargo run --locked -p xtask -- conformance verify-inventory
# Explicit outstanding parent action; currently rejects the old source binding:
cargo run --locked -p xtask -- conformance verify-native-baseline
cargo fmt --all -- --check
git diff --check
```

Passed: **2 HDR failure/boundary tests**, **4 portable-math regressions**, all
**17 color/gain-map contracts** (9.67s), **66 asset regressions**, **1 public
license test**, **1 CLI license test**, and both executed WASM workers (**1 test
each**, 6.81s/0.61s). Workspace/all-target Clippy, catalog verification,
inventory verification (320 coverage rows/168 executions/8 pending milestones),
ownership validation, read-only preservation audit, formatting and diff checks
pass. The native-baseline failure is recorded above. No reference record,
producer, comparator or rendering expectation was regenerated for these fixes.

Correction hashes:

| Source/binding | SHA-256 |
| --- | --- |
| `src/portable_math.rs` | `46e6f48f745239c94597abf9e1053a44e750dc93d74a7744364d9fccbb3cc0b5` |
| `src/render/asset/hdr.rs` | `03d21c699554e351ce44a6bfeb036890b8a8bf38533a7607639f99a3eec1f6ac` |
| `src/render/asset/png.rs` | `f6de3ed9261c8d198b20da6347a4323efdb0aa8c32c9f402f64f3fa674ee4872` |
| All five `LICENSE-BUNDLE.txt` copies | `cadd243b2228954efb8f61f62d492bcde65a23f8e7e96a2db5a6cc42ce78a9f2` |
| `license-parts/skia-BSD-3-Clause.txt` | `ad9374feb42087da25889dbee8a46c54e190f2e2aac993064e4e99f83c1a3a58` |
| Packaged public-facade source | `bee1e60b4121cc220eef1299c1a0718c6976fb9ab07dde0ebbf888a16d4a4aed` |
| Coverage requirements | `806d3fa98354cab1cff3b4103b4c262f47cfcd440805fe0fbdc709a7b6b53b44` |
| Coverage inventory | `9db3087526fc25ca1a68ab5f3270ef863765e7f009684fe66d41446747869245` |
| Ownership catalog subject | `fb90d519e1ddb2a21b40357df48ab47f02efdce02403705f8890e9e1e7ef5001` |
| Ownership ledger file | `914b97af9cdc082bb3d650a272cf6bcada69c319b79080deddff771348b33b46` |
| `verify-png-stage3.mjs` | `6d03b2dfcfef82ff5927b1752bbac809cb76c19276e604ab65ce4271abb45ea7` |

[skia-license]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/LICENSE
[baseline]: font-registry-snapshot-reconciliation.md

## SHA-256 bindings

The read-only audit command prints and rechecks every local binding below.

| Local source | SHA-256 |
| --- | --- |
| Main reference record | `2aae93308b4289d3374ec92e960e756106af621f9bfc64e4b29314f5162857d0` |
| Gain-map reference record | `dee2a45432ce2a4e211d1e51e8e80f571bfd8f90221c7bf85153b57df88714ad` |
| Shared helper `tests/common/png_color_reference.rs` | `e779eadd553a42a007c792e71f3053bc617588c7e8736cab0b572c15f77c8fa0` |
| Owner `tests/fast/png_color_contract.rs` | `396366a746030aa8cfd4cca1b320c7af55ec20cd915ddd2cbc519849891a4eca` |
| `png-color/inputs.json` | `96b60a6d266ea46a7a1a519ef6ca7910c3a4d56067e4311efef62a9e4a5f1177` |
| `png-color/gain-map-inputs.json` | `4d0e7e6a96610d756a5d8f88d581624837b0b76b2bd6d2d2ee42b8c66086c385` |
| `reference-profiles/revision-1.json` | `1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71` |
| `png-color-fixtures.mjs` | `7bf1941bf782ba08363b4f61ffa8258a18a67431cc3d0e9c8e8c3af8d72af0b0` |
| `png-color-evidence.mjs` | `ef74852856a69245cf9d867fffe4153c1dc906052b3f964c98fece85d1687756` |
| `run-png-color.sh` | `7bcc68ff1a3eec6db51c5c186255c1ae22c1a7cba37fc7664d599641ce8f93b5` |
| Canonical comparator profile | `e969fd7a3bc49ff3014b2cd3b3622cd18abddbc9371744d4e47927d0484bcfc7` |
| `png-gain-map-fixtures.mjs` | `949429b0d92ee173a106d158aa5bced3fe00057f0b7407a9d73dd2b0194e644c` |
| `png-gain-map-evidence.mjs` | `e5c646e195de4972f5a6357214c7711b8ec7e1cb00358ad9486b44b2888e017d` |
| `run-png-gain-map.sh` | `b3fbc2241aae491ced6c92c1fdc4acc92269d84d8c24d2cf196466bfc45ed725` |

Fetched upstream source hashes (not producer inputs):

| Skia path at the exact commit above | SHA-256 |
| --- | --- |
| `include/core/SkColorSpace.h` | `a9efb317a071c1497b36e5aa4e505638a95b3cefce63988b6ae686f4a86e2a45` |
| `src/core/SkColorSpace.cpp` | `a4b1c80dd726333088caa4c534f47fe39bfb695e75eb76723625f5241d2dbe76` |
| `src/core/SkColorSpaceXformSteps.cpp` | `5281a941624f6737bfee1e57acdf8847e0fb5b3dd445cd7dbad9aa2ebdbd5f5f` |
| `src/codec/SkPngRustCodec.cpp` | `8345f55ea5f89e5b654be983ea61d781dd4ac9f4c98ccb26d5389472e82c41d0` |
| `src/codec/SkPngRustCodec.h` | `220fc8348cfb4b17df6a4b2766f33b1d1c32b1a5f93dccb65761972bdef98f7f` |
| `src/codec/SkPngCodecBase.h` | `de00ae44442d8f822f27c3a363b7be222351bffa4e7bc220bab8be2b1e6de587` |
| `include/codec/SkCodec.h` | `98e2a2cb1be4919944b265c37b3f610045239b02a43b2571d8f0978925826c14` |
| `src/codec/SkHdrAgtm.cpp` | `ae0ee6d203de0903eb99058188ed3a2238b2a4fc8b8d67a1278f0302f2544450` |
| `src/codec/SkHdrMetadata.cpp` | `f220175e416b3c2f02990c26e4ed35e0692b89fea46a6c089e47d27cefc307ac` |
| `src/opts/SkRasterPipeline_opts.h` | `de28e6782a6e59bfe9906999317d455defe22799495f61da246becf2a46757b6` |
| `src/codec/SkGainmapInfo.cpp` | `5f4cc2f1a458fbae74430e27bc3e17ac8f02bd13685aac32f87f4ccd21b1d527` |
| `src/encode/SkPngEncoderImpl.cpp` | `6e63d689151d37a2261a92f0d510b8e3f190b1f719dbb91726cde7b773bcbce7` |

| Chromium149 source | SHA-256 |
| --- | --- |
| `DEPS` | `c4ab905e7b7970c39bce296ed0f86d89b782cb5d93ab07969c0aa68b683f31a7` |
| `png/png_image_decoder.cc` | `1e5cfe87a026049c1f60a11c72eccb1b4fbdd9608b5e7236b543792307fddc01` |
| `skia/skia_image_decoder_base.cc` | `0d1bcf1a76c931e97dc8dcc8e5187e31aca637da758729c977cb47d596a7a730` |
| `cc/paint/tone_map_util.cc` | `cbc2b6eedb67a41826e666dee54a0e9f05e03f2eb876ef060592cb30f2a2ff4d` |

[deps]: https://github.com/chromium/chromium/blob/149.0.7827.55/DEPS
[png-decoder]: https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/png/png_image_decoder.cc
[image-decoder]: https://github.com/chromium/chromium/blob/149.0.7827.55/third_party/blink/renderer/platform/image-decoders/skia/skia_image_decoder_base.cc
[tone-map]: https://github.com/chromium/chromium/blob/149.0.7827.55/cc/paint/tone_map_util.cc
[transfer]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/include/core/SkColorSpace.h
[xform]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkColorSpaceXformSteps.cpp
[pipeline]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/opts/SkRasterPipeline_opts.h
[agtm]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/codec/SkHdrAgtm.cpp
[codec]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/include/codec/SkCodec.h
[encoder]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/encode/SkPngEncoderImpl.cpp
[gain-info]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/codec/SkGainmapInfo.cpp
