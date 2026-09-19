# Remaining cICP primary sets (#85)

Base: `2f2b58f2f8a8ca62d8bae64dada271655c8f186f`, including the intervening
rendering-library investigations and module refactors.

## Scope and evidence extension

This tranche crosses primary codes **4, 5, 6, 7, 8, 10, 11 and 22** with the
already-qualified transfers **1, 4, 5, 8 and 13**, range flags **0/1**, and
**RGBA8/16**: **160 additional owned cells**. Together with the first 60,
the selected transfer/range/depth matrix now covers all eleven primary codes.
Other transfer characteristics, HDR/gain maps and cross-axis composition remain
separate qualification work.

The [existing SDR collector and gates](png-sdr-color-ownership.md) retain the
maintainer-selected canonical Geometry/image budgets plus exact invariants and
semantic anchors. No comparison limit changed. Every cell renders SVG/PNG/PDF
twice through fresh facade sessions, checking complete reports, preserved SVG
payloads, PNG appearance/metadata, and the actual PDF RGB/soft-mask bindings
against independently collected opaque companions and authored alpha.

The new reference record contains **500 cases**. Collection executes two fresh
pinned Chromium149 browser processes in forward/reversed orders. Its digest is
`285e8a10eb2ad47c74f7f12ff1b0f130621a41d6eea0d6f68fd47412f5d14629`.

The collector's explicit `extend` mode authenticates the prior record digest,
requires additional unique cases, and compares every complete prior case against
the newly collected case before writing. Schema, options, visual class, collection
policy, fixture producer, reference profile and comparator bindings must remain
unchanged; only the input-list and collector/runner descriptors may move. All
**340 prior cases were preserved byte-for-byte**. `generate` still refuses an
existing record, and `verify` reproduces the complete current record.

```sh
bash tools/reference-oracle/run-png-color.sh extend \
  402845da735653bfbaf00e76f80726425f0cc84446c0ad4fbfd21f095127494c
bash tools/reference-oracle/run-png-color.sh verify
```

## Discriminating gates

- Reference-only comparisons for primary codes 4, 5, 6, 8, 10 and 22 against
  primary 1 must fail the same canonical image budget, proving these fields can
  distinguish a substituted gamut/white-point interpretation.
- Primary codes 6 and 7 are equivalent under H.273; their reference rasters must
  agree exactly for every selected transfer.
- Cinema-P3 (11) and display-P3 (12) share primaries but have different whites.
  Their field difference is inside the normal image budget, so a separate exact
  opaque anchor is mandatory: input `[2,2,64,255]` under linear transfer gives
  `[20,20,143,255]` for 11 and `[22,22,143,255]` for 12. The test checks both the
  independently captured companion pixels and actual facade outputs.
- Narrow-range signaling continues to be ignored by the pinned decoder; those
  cells retain whole-image exact identity assertions rather than claiming
  narrow-range conversion support.

The selected `png_color_contract::sdr_cicp_candidates_match_the_reference` calls
the primary-space anchors and the full 220-cell matrix. Its shared helper checks
the complete independently declared cICP ID set and the 500-case uniqueness/count
guard. The native owner binds both the reference record and helper fingerprint.

## XYZ correction

The first expanded run failed the primary-10/full-range matrix: 115 of 225 pixels
changed and SSIM was 0.83142. A reference check ruled out ignored signaling: the
pinned decoder really interprets these XYZ-based primaries.

The minimized facade case was an opaque neutral `[128,128,128]` under
`cICP=[10,1,0,1]`: the renderer emitted green `[0,255,0]`, while the independently
captured neutral anchor is `[121,121,121]`. The CMS's generic xy constructor
normalizes each primary to Y=1; the XYZ basis has zero-luminance X and Z axes, so
that representation collapses the basis. The adapter now supplies the explicit
XYZ unit triplet and equal-energy white to `update_rgb_colorimetry_triplet`, which
retains the existing D50 adaptation, transfer handling and bounded conversion.
The exact regression and full expanded matrix pass after the correction.

The pinned-reference implementation corroborates the interpretation:
[SkColorSpace::MakeCICP and the primary table](https://github.com/google/skia/blob/chrome/m149/src/core/SkColorSpace.cpp)
route primary 10 through its declared XYZ chromaticities. The correction preserves
that basis instead of adding a fallback or excluding the input class.

## Verification checkpoint

Spec review confirms all 160 assignments are eligible and independently checked
preservation of all prior cases. Standards review found no hard breaches; its
small duplication suggestion was addressed by sharing the discrimination closure.

The existing exact selector now owns all 220 cells. Its source binding was
refreshed after review. Current totals are **1,256 assigned/excluded and 620 gaps**
(1,253 proposed owners and three approved exclusions), with **484 cICP cells**
still unqualified.

- All **ten** native SDR contract tests and **66** existing asset-codec tests pass.
- Browser-WASM executes all **500** cases twice through the shared appearance,
  exact report/alpha and recovery gates: pass. This local worker run used the
  installed Chrome/ChromeDriver 151.0.7922.173 with the pinned Rust toolchain and
  `CARGO_PROFILE_TEST_DEBUG=0`; the reference collector uses pinned Chromium149.
- The stale-base extension attempt was rejected before collection/writing, and
  the current reference digest remained unchanged.
- Ownership validation passes with the recorded **1,256 / 620** totals.
- Full `cargo test --locked --workspace --no-fail-fast`, Rust 1.97.1:
  **1,127 passed, 18 ignored**. The 536-test fast binary passed in 933.90 seconds.
  The first build exhausted disk during linking; after clearing Cargo debug
  artifacts, the complete retry passed with `CARGO_PROFILE_TEST_DEBUG=0` and
  incremental compilation disabled. Ignored external-harness/materialization
  tests remain outside this local run.
- Reference reproduction verified all **500** cases and the recorded digest.
- `conformance verify-all` verified **37** registered cases, `verify-inventory`
  verified the existing **168** executions, and ownership validation passed.
- Final workspace/all-target Clippy with warnings denied, formatting and diff
  checks pass. Temporary diagnostic instrumentation was removed.

The global ledger remains incomplete and unapproved; #178 target equality and
broader profile qualification remain separate release requirements.
