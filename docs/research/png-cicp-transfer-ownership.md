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
