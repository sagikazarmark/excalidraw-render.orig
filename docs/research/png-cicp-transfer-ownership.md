# BT.1886-family cICP transfers — Stage 1 (#85)

Base: `3a46eb056d0a535d14f93f08fedbd5d7ec32cc23`.

## Scope and evidence

This stage qualifies **176 additional cells**: transfer codes **6, 11, 14 and
15**, primary codes **1, 4, 5, 6, 7, 8, 9, 10, 11, 12 and 22**, range flags
**0/1**, and **RGBA8/16**. The earlier 220 cells retain their ownership, bringing
the qualified cICP selection to **396 of 704 cells**.

The [initial SDR qualification](png-sdr-color-ownership.md) and
[remaining-primary qualification](png-cicp-primary-ownership.md) establish the
collector and judgment. The maintainer-selected canonical Geometry/image limits
remain **1/400 changed pixels and minimum SSIM 0.999**, with exact semantic
anchors. The fixture producer, comparator, reference profile, exclusions and
catalog are unchanged. The 15×15 fields retain spatially varying color and alpha;
the RGBA16 sources retain their independently authored non-lattice low bytes.

The existing collector authenticated the prior record and preserved all **500
prior cases byte-for-byte**, adding exactly the selected 176 cases. The new
record has **676 cases**, each with source scene/request inputs, original PNG
bytes, official PNG output, an opaque companion and the official SVG digest.
Two fresh pinned Chromium **149.0.7827.55** processes collected forward/reversed
orders, with a fresh context per case and networking disabled. A separate
`verify` invocation reproduced the entire record byte-for-byte.

```sh
bash tools/reference-oracle/run-png-color.sh extend \
  285e8a10eb2ad47c74f7f12ff1b0f130621a41d6eea0d6f68fd47412f5d14629
bash tools/reference-oracle/run-png-color.sh verify
```

Current SHA-256 bindings:

- Reference record: `30a754e614199e6fdb325613af4802ba489f386ce899e5503641489730371555`.
- Shared helper: `c292fb168ab9ae601bad794f61c62adbb8cded281f7a63148a1568a88e9c5c00`.
- Owner source: `a6d023b43cc10bbfa26611743c1c9f1ae33b21aee0a3fa2869d2e9f9712d0445`.

## Inspected qualification gates

- The shared helper independently enumerates the complete **396-cell** cICP ID
  set, checks uniqueness and the 676-case count, and binds the reference digest.
  The native selector binds the shared helper digest and checks all six producer
  descriptors against their live files.
- Every new reference raster and opaque companion is **byte-identical** to the
  corresponding transfer-1 output, across all primaries, ranges and depths.
  This characterizes the H.273 reference EOTF: **BT.1886 gamma 2.4**. The existing
  renderer already selects that curve for these four codes; no renderer
  correction was required.
- For every new transfer and both depths, primary-1/full-range reference fields
  must discriminate transfers 4, 5, 8 and 13 under the same canonical image
  budget. The opaque neutral field anchor is exactly `[121,121,121,255]`,
  distinguishing this interpretation from sRGB identity, linear light and the
  inverse camera OETF. Native single-pixel anchors independently require the
  same result for each new transfer.
- Range-zero reference fields and companions match the corresponding untagged
  images exactly. Native exact anchors additionally require neutral 128 to
  remain 128 with no lower signal, become 188 with linear gAMA, and remain 121
  when full-range cICP takes priority over that gAMA. These qualify ignored-range
  fallback, not narrow-range conversion.
- The native owner runs **all SVG/PNG/PDF formats twice through fresh facade
  sessions**, with complete report checks and artifact repeatability. SVG
  preserves the exact original source payload. PNG retains exact dimensions,
  alpha, transparent-zero RGB and untagged output. PDF checks straight RGB and
  alpha through the actual page/image/ICC/soft-mask bindings against the opaque
  companion and authored source alpha. The canonical visual gates apply per
  complete image. For all 176 additions, native PNG and PDF artifacts and reports
  also match the corresponding transfer-1 facade output **byte-for-byte**.
- Browser-WASM executes the shared reference equivalence/discrimination gates,
  all 676 PNG cases twice, exact reports/alpha/recovery and image comparisons,
  plus byte-exact transfer-1 facade equivalence for the 176 additions.

The proposed owner remains
`png_color_contract::sdr_cicp_candidates_match_the_reference`. The new exact
anchor test is called by that selector, so running the owner executes its
semantic anchors as well as the full matrix.

## Verification and ownership audit

Commands used Rust **1.97.1** with this environment:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --locked -p excalidraw-render --test fast png_color_contract::
cargo clippy --locked --workspace --all-targets -- -D warnings
HEADLESS=1 WASM_BINDGEN_TEST_TIMEOUT=180 \
  CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p excalidraw-render --target wasm32-unknown-unknown \
    --lib wasm_tests::png_sdr_color_reference_in_wasm -- --exact
cargo run --locked -p xtask -- conformance refresh-test-owner-source png_color_contract
cargo run --locked -p xtask -- conformance verify-ownership
```

- Native color contracts: **11 passed, 0 failed, 0 ignored**, 8.24 seconds.
- Workspace/all-target Clippy with warnings denied: **passed**.
- Executed browser-WASM worker: **1 passed, 0 failed, 0 ignored**, 6.54 seconds.
  Local Chrome/ChromeDriver **151.0.7922.173** qualifies the consumer's browser
  execution; the pinned Docker Chromium149 collector supplies reference evidence.
  These are separate roles, not a cross-target artifact-equality measurement.
- Reference reproduction: **676 cases**, matching the complete record digest.
- Ownership validation: **1,876 total**, **1,432 assigned/excluded** and **444
  explicit gaps**: **1,429 proposed owners** and the same **three approved
  exclusions**. There remain **308 unqualified cICP cells**.
- Structured comparison against the base commit: all 500 prior cases and frozen
  producer bindings preserved; precisely 176 gap-to-owner changes; only the
  source hashes changed for 227 prior owners of this test module. Every other
  prior disposition, exclusion, catalog binding and approval is identical.
- Formatting and `git diff --check`: **passed**.

After inspecting these gates and their passing executions, `xtask` refreshed the
existing owner source binding and `record-test-owner` assigned only the explicit
176-cell product above. Existing owner selectors/rationales, exclusions, catalog
binding and pending global approval are preserved. The ledger remains incomplete;
**#178 remains open**. Independent Spec review confirmed all 176 assignments;
Standards review found no hard breaches or actionable smells. The parent full
workspace run passed **1,128 tests, with 18 ignored**, using the environment above;
all 537 fast tests passed in 914.06 seconds. All 37 registered conformance cases,
the 168-execution inventory and ownership validation passed before the stage commit.

## Stage 2 — remaining non-PQ/HLG transfers

Base: `e6b05602a2d4072f17db892d912e098aebc02b9c` (committed Stage 1).

### Scope and preserved evidence

This stage adds exactly **220 cells**: transfers **7, 9, 10, 12 and 17** ×
primaries **1, 4, 5, 6, 7, 8, 9, 10, 11, 12 and 22** × ranges **0/1** ×
**RGBA8/16**. The current cICP selection is **616 of 704**; the remaining **88**
cells are PQ/HLG (16/18). The complete reference record now contains **896 cases**.

```sh
bash tools/reference-oracle/run-png-color.sh extend \
  30a754e614199e6fdb325613af4802ba489f386ce899e5503641489730371555
bash tools/reference-oracle/run-png-color.sh verify
```

Extension authenticated the Stage 1 record and preserved all **676 prior cases
byte-for-byte**, including scene/request inputs, original PNG and opaque source,
official outputs and SVG digests. Collection used the same pinned Chromium149,
two fresh forward/reverse processes, fresh contexts and disabled networking.
Separate verification reproduced all 896 cases and the entire record digest.
Only the input-list descriptor changed; all five other producer/judgment source
descriptors are identical. The fixture producer, collector, runner, reference
profile, comparator, authored samples and existing recovery declarations remain
intact. No tolerance exception was added.

Current SHA-256 bindings:

- Reference record: `9c53ba1857d7e6406863200a685e9078e68e109569fed9c96bbcc1bcfb9c95ad`.
- Shared helper: `94b651e477d35ca0e938b0720eed6a85ea0dfe0405f50aaaa6bae1a54aee8259`.
- Owner source: `06ce72f765b68819f2afb597606eaaa3c11107a1a89dbf84f0ae72501fc6ed2e`.

### Interpretation and demonstrated correction

The inspected primary sources are Skia m149's
[transfer definitions](https://github.com/google/skia/blob/chrome/m149/include/core/SkColorSpace.h),
[MakeCICP and transfer lookup](https://github.com/google/skia/blob/chrome/m149/src/core/SkColorSpace.cpp),
and [SkPngRustCodec::CreateColorProfile](https://github.com/google/skia/blob/chrome/m149/src/codec/SkPngRustCodec.cpp).
The first explicitly declares transfers **9, 10 and 12 unsupported by
SkColorSpace::MakeCICP**; the decoder then considers ICC, sRGB and gAMA/cHRM.
They are valid required inputs, characterized as accepted ignored signaling,
not rejected profiles or logarithmic-conversion support.

The minimized red probe used neutral 128 under `cICP=[9,9,0,1]`: the renderer
returned **[89,89,89,255]**, whereas the independently collected untagged-equivalent
field and exact anchor require **[128,128,128,255]**. The correction in
`prepare_png_color_normalization` lets these three valid transfer codes fall
through to existing lower-metadata handling. The bypass is restricted to admitted
primaries, identity matrix and full range; existing narrow-range behavior and
strict invalid-metadata recovery are retained. The same probe passes after the
fix. Native anchors for each ignored transfer and both ranges additionally
require **188** under linear gAMA and **128** when sRGB takes precedence over
that gAMA, using non-sRGB primary 9 to expose an incorrectly retained gamut.

Transfers **7 (SMPTE240)** and **17 (cinema)** already passed independently
calculated exact anchors and needed no runtime correction. With normalized
encoded value V, transfer 7 uses `V/4` below `0.091286342118`, otherwise
`(0.899626676224*V + 0.100373323776)^(1/0.45)`. Transfer 17 uses
`(52.37/48)*V^2.6`. Applying the sRGB output curve and 8-bit rounding/clipping
gives these independent anchors:

| Encoded byte | SMPTE240 output | Cinema output |
| ---: | ---: | ---: |
| 8 | 22 | 0 |
| 23 | 41 | 7 |
| 24 | 42 | 8 |
| 128 | 141 | 118 |
| 255 | 255 | 255 |

These exercise both sides of the SMPTE240 toe and cinema scale/clipping. Exact
range-zero anchors require identity without lower metadata and linear-gAMA
fallback when present.

### Inspected qualification gates

- The shared native/WASM helper independently enumerates the exact **616-cell**
  ID set, checks **896 unique cases**, and binds the record digest. The native
  owner binds that helper's digest and checks all six live source descriptors.
- All **132** transfer-9/10/12 cells and **44** range-zero transfer-7/17 cells
  match their untagged reference controls exactly, including opaque companions.
  Their native PNG/PDF artifacts and reports, and browser-WASM PNG artifacts and
  reports, also match the corresponding untagged facade output byte-for-byte.
- Full-range primary-1 transfer-7/17 reference fields distinguish transfers
  4/5/8/13 and each other under the unchanged canonical Geometry/image budget.
  Transfer 7 also distinguishes transfer 1. The cinema-versus-BT.1886 field pair
  is inside that budget, so the authored neutral `(9,6)` sample is pinned to
  **118 versus 121** instead. Transfer 7 is pinned to **141**. Both source depths
  check these exact independent values in reference companions and actual native
  PNG/PDF and browser-WASM PNG output; the full-image comparator still applies.
- The native owner executes all **SVG/PNG/PDF** formats twice in fresh sessions,
  complete reports, exact SVG source payloads, output dimensions, alpha,
  transparent-zero RGB and untagged PNG output. Actual PDF page/image/ICC/soft-mask
  bindings supply straight RGB and source-alpha comparisons to independently
  captured opaque companions. The canonical limits remain **1/400 changed pixels
  and SSIM 0.999** with exact semantic anchors.
- The existing primary-6/7 equivalence checks now include all new transfers.
  All prior BT.1886 equivalence, primary-space, recovery and metadata gates remain
  active. Browser-WASM executes all **896 PNG cases twice** using the shared
  appearance, report, recovery, identity and semantic-anchor gates.

### Verification and ownership checkpoint

All Cargo verification used Rust **1.97.1**, `CARGO_INCREMENTAL=0`, and
`CARGO_PROFILE_TEST_DEBUG=0`:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --locked -p excalidraw-render --test fast png_color_contract:: -- --nocapture
cargo test --locked -p excalidraw-render --lib render::asset::tests::
cargo clippy --locked --workspace --all-targets -- -D warnings
HEADLESS=1 WASM_BINDGEN_TEST_TIMEOUT=180 \
  CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
    --lib wasm_tests::png_sdr_color_reference_in_wasm -- --exact
cargo run --locked -p xtask -- conformance refresh-test-owner-source png_color_contract
cargo run --locked -p xtask -- conformance verify-ownership
```

- Native color contracts: **13 passed**, no failures/ignores, **9.46 seconds**.
- Relevant existing asset-codec regressions: **66 passed**, no failures/ignores.
- Workspace/all-target Clippy with warnings denied: **passed**.
- Executed headless browser-WASM dedicated-worker test: **1 passed**, no
  failures/ignores, **6.97 seconds**, local Chromium/ChromeDriver
  **151.0.7922.173**. The local browser executes the consumer; pinned Docker
  Chromium149 separately supplies reference evidence. This is not a cross-target
  artifact-equality claim.
- Reference verification: **896 cases**, complete digest reproduced.
- Structured comparison to the base verifies exactly **220 gap-to-owner** changes
  and source-hash-only refreshes for **403 prior owners** of this module. All other
  prior dispositions, rationales, selectors, exclusions, catalog binding and
  pending approval are preserved.
- Ownership validation: **1,876 total**, **1,652 assigned/excluded**, **224 gaps**;
  **1,649 proposed owners**, the same **three approved exclusions**. All **616**
  qualified cICP cells use `png_color_contract::sdr_cicp_candidates_match_the_reference`.
- Formatting and `git diff --check`: **passed**.

After passing execution and implementation inspection, `record-test-owner`
assigned only the explicit 220-cell product above. Independent parent Spec review
confirmed all assignments. Standards review found no hard breaches and one
nonblocking duplication note for reference-comparison helpers. The parent full
workspace run passed **1,130 tests, with 18 ignored**; all 539 fast tests passed
in 933.12 seconds. All 37 registered conformance cases, the 168-execution inventory
and ownership validation passed before the stage commit.
Global ownership remains incomplete and unapproved,
**#178 remains open**, and PQ/HLG and subsequent stages remain separate work.

## Stage 3 — PQ/HLG and PNG gain maps

The [Stage 3 audit](png-hdr-gain-map-ownership.md) records the subsequent 88
PQ/HLG cells, bringing this cICP selection to **704**, and three PNG gain-map
obligations. The main record now contains **984 cases**, preserving all 896
Stage 2 cases exactly. A separately bound supplemental producer captures **108
gain-map/control cases** without changing the frozen color fixture builder or
collector. Current ownership is **1,743 assigned/excluded and 133 gaps**; global
approval and full-workspace testing remain separate. Parent Spec review approved
all 91 Stage 3 assignments; its Standards corrections and the license-consumer
binding refresh are recorded in the Stage 3 audit.
