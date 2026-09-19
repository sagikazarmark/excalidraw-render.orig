# PNG/APNG profile inventory and sample-vector ownership (#85)

Initial inventory base: `d28fc51dc0eac8fa3c71fd8a5cc3a3c37ad7fbe9`.
APNG completion base: `c0ad04dac0d3171eff45181c2898e72f416a51e7`.

## Scope

The nine existing `codec:png:*` facet obligations are umbrella requirements, not
an exhaustive profile inventory. They remain in the ledger. The new authored
`tests/conformance/coverage/png-profiles.json` expands concrete input dimensions
and records additional qualification work instead of inferring coverage from
passing family-level examples.

This is an **initial PNG/APNG inventory**, not closure of the complete raster
profile matrix. In particular, composition across axes and canonical-viewer
qualification remain explicit work. The sample tranches establish independent
sample-vector contracts; they do not grant a new reference profile or exclusion.

## Expanded obligations

| Group | New obligations | Meaning |
| --- | ---: | --- |
| Sample representation | 180 | 30 layout/transparency combinations × two interlace modes × static/included-default APNG/separate-default APNG |
| cICP candidates | 704 | RGBA8/RGBA16 × 11 recognized primaries × 16 recognized transfers × two range flags, matrix 0 |
| eXIf orientation | 32 | Eight orientations × two TIFF byte orders × before/after IDAT |
| Further qualification | 32 | Precision, alpha rounding, palette, color/ICC/precedence, invalid signaling, gain maps, APNG controls, malformed classes, limits and axis composition |
| **Total additions** | **948** | Each becomes a distinct ledger obligation |

Recognized cICP header combinations are **qualification candidates**, not claims
that every combination is accepted, correct, or approved by the reference.
Their criteria require actual admission and canonical-reference characterization
or exact unsupported recovery. Reserved/unspecified values and nonidentity
matrices have a separate explicit rejection/ignore-policy obligation.

The sample matrix covers all fifteen legal PNG color-type/bit-depth pairs:

- Grayscale: 1, 2, 4, 8, 16 bits, with and without a transparency key.
- RGB: 8, 16 bits, with and without a transparency key.
- Indexed: 1, 2, 4, 8 bits, without tRNS, with a short tRNS prefix, and with a
  full palette-alpha table.
- Grayscale-alpha and RGBA: 8 and 16 bits with intrinsic alpha.

Sixteen-bit sample cells deliberately use byte-replicated 8-bit-lattice values.
Those exact sample anchors do not characterize low-byte precision, byte-order
discrimination, reduction thresholds, or 16-bit transparency-key aliasing. Those
requirements are separate obligations rather than being claimed by symmetric
samples. They now have reference-backed owners in the
[precision/alpha qualification](png-precision-ownership.md).

## Existing implementation and evidence map

The renderer specification §8.2 supplies the format contracts; §14 requires the
full profile matrix and forbids using missing evidence as an exclusion.

| Area | Implementation / existing evidence | Remaining qualification |
| --- | --- | --- |
| PNG headers, CRC and chunk order | `render/asset/png.rs::{read_png_ihdr,validate_static_png}`; `asset/tests.rs::png_codec`; decode diagnostic contracts | Systematic forbidden IHDR and malformed ancillary/container classes |
| Sample expansion and Adam7 | `png 0.17.16`, `decode_static_png`; manual sample matrices below and subsequent [precision/alpha vectors](png-precision-ownership.md) | Profile-axis composition and canonical-viewer qualification beyond the recorded scopes |
| Color interpretation | `asset/color.rs::prepare_png_color_normalization`; [SDR reference qualification](png-sdr-color-ownership.md) and [primary extension](png-cicp-primary-ownership.md) cover seven metadata/profile obligations and 220 cICP cells | Other 484 cICP cells, broader ICC model/LUT admission, HDR and profile-axis composition |
| HDR/gain maps | Selected PQ gray and SDR-base `gmAP`/`gdAT` examples in `asset/tests.rs` | Full reference tone-map and gain-map behavior; preserving/ignoring ancillary chunks does not prove interpretation |
| Orientation | `asset/color.rs::{exif_orientation,orient_rgba}`; eight quadrant permutations and selected before/after-IDAT examples | Non-square vectors, both TIFF orders, all positions and malformed/duplicate metadata policy |
| APNG | All frames validated; PNG/PDF select animation frame zero, including separate-default files; the sample matrices below cover all declared layouts; animation diagnostic owner covers occurrence semantics | Subframes, disposal/blend/control products and later-frame corruption |
| Limits and recovery | Resource and decode diagnostic owners already exercise PNG/APNG examples | Per-profile boundary composition and independent recovery evidence, not inferred ownership of every codec facet |

The cICP value lists are authored from the revision's producer checks and the
exact-pinned `moxcms 0.9.0` CICP definitions. They must be reviewed if that qualified
dependency or the admission policy changes. The inventory generator does not
inspect the runtime decoder to decide its required set.

## First evidence tranche

The first tranche recorded these exact selectors in `fast/png_profile_contract.rs`:

- `static_sample_matrix_covers_all_declared_layouts`: **60 static cells**.
- `apng_rgba_matrix_covers_both_default_image_layouts`: **4 RGBA8 APNG cells**,
  with included/separate defaults and both interlace methods.

For each cell, the selected test runs all five PNG row filters on 1×1 and 9×9
images. The tiny image skips empty Adam7 passes; the odd image exercises all
passes, predictor history and packed-row tail bits. The builders write PNG
chunks, packed samples, filter residuals and Adam7 pass order directly. They use
compression and CRC primitives, not the renderer or the PNG decoder to generate
expected pixels. Sixteen-bit samples and alpha use independently declared exact
lattice values.

Every payload is rendered through two fresh public-facade sessions for each
format. Assertions cover:

- Complete ordered reports: static and direct-SVG silence, and one exact
  animation-flattening row for each two-frame PNG/PDF occurrence.
- Direct SVG payload octets and canonical PNG data-URL media type/base64.
- Every decoded PNG pixel, exact dimensions, RGBA8 and untagged output.
- PDF page-content and image-resource references, the image's ICCBased RGB space
  bound to the output-intent profile, and its attached grayscale soft-mask
  reference; both referenced planes have exact dimensions, bit depth and authored
  sample bytes. Source-image use emits no PDF raster-fallback diagnostic.
- Byte-identical reports and artifacts across fresh calls.

The PDF oracle's mutation control first accepts the original artifact, then
rejects a detached soft mask, an image resource redirected to the alpha object,
and a soft-mask reference redirected to the RGB object. Finding otherwise-correct
but unused plane bytes cannot establish the cell's PDF evidence.

The separate APNG default image differs from animation frame zero, and later
frames differ too. A wrong selected image therefore fails the pixel oracle.
The exact selectors also check the inventory against an independently authored
layout list, so a matrix edit cannot silently acquire the old test owner.

## Append-only catalog migration

Before expansion, `ownership-catalog` was captured and its digest remains bound
by the existing ledger. `conformance extend-ownership PRIOR_CATALOG ISSUE REASON`
authenticates that catalog, rejects changed/removed prior obligations and an
approved global ledger, preserves every old disposition, and adds only explicit
gap rows. Selecting executable owners is a separate reviewed command.

This migration preserves the **928** prior dispositions, including all **three
approved exclusions**, and adds **948** gaps. The resulting catalog contains
**1,876** obligations. The three exclusion subjects remain valid because their
full obligation rows and proof bytes are unchanged. No reference artifacts,
execution inventory, approval subjects or case-level primary labels are rebuilt.

The inventory-only checkpoint was **846 assigned/excluded and 1,030 gaps**.
The first 64 reviewed vector cells were assigned, producing **910 assigned/excluded
and 966 gaps** (907 proposed owners and three approved exclusions). The larger gap
count exposes previously unenumerated work; it is not
a regression or a coverage percentage. The nine PNG umbrella facets and
`closure:atomic-profile-inventory` remain gaps.

## First-tranche verification checkpoint

Verification used the pinned Rust 1.97.1 toolchain and locked dependencies:

- Full workspace suite: **1,089 passed, 18 ignored**, before the final PDF
  resource-binding review fix and its additional mutation regression.
- After that fix, all **three** `png_profile_contract` tests passed, including
  both exact ownership selectors and the mutation control.
- After recording the owners, all **16** ownership unit tests and **five**
  coverage CLI integration tests passed, including preservation of live owners
  and approved exclusions, and rejection of invalid recording batches.
- Workspace/all-target Clippy with warnings denied, formatting and diff checks
  passed.
- `conformance verify-all` verified all **37** registered cases;
  `verify-inventory` verified the existing **168** executions;
  `verify-ownership` verified **1,876** dispositions with **910 assigned/excluded
  and 966 gaps**.

Independent Spec review found the PDF binding gap; the binding-aware oracle and
mutation regression resolved it. Standards review's container-domain suggestion
was addressed with an enum. Follow-up reviews reported no remaining findings and
confirmed eligibility for exactly the 64 narrow sample-vector cells.

The global ledger remains incomplete and pending approval. Broader raster profile
qualification and the separate #178 native/WASM equality blocker remain open.

## APNG sample completion

The additional recorded exact selector
`png_profile_contract::apng_non_rgba8_matrix_covers_both_default_image_layouts`
executes the remaining **116 APNG sample cells**:

| Layout family | Added cells |
| --- | ---: |
| Grayscale 1/2/4/8/16, with and without a transparency key | 40 |
| RGB 8/16, with and without a transparency key | 16 |
| Indexed 1/2/4/8, absent/prefix/full palette transparency | 48 |
| Grayscale-alpha 8/16 | 8 |
| RGBA16 | 4 |
| **Total** | **116** |

Each row crosses included/separate default images with both interlace modes.
Every cell executes the same five row filters, 1×1/9×9 vectors, all three formats
and two fresh renderer sessions described above: **6,960 facade render calls** in
the new selector. Existing static and RGBA8 selectors keep their ownership scope
and share the same layout runner. Sixteen-bit samples retain the explicitly
limited byte-replicated 8-bit-lattice scope.

The expanded matrix exposed a fixture alias: phase two of the original separate
default pattern equaled animation frame zero for one-bit grayscale and indexed
layouts. A new contrast assertion failed on the 1×1 grayscale case before the
fixture correction. The separate default now uses phase three, which changes
one-bit samples as well. Every APNG vector asserts that the later animation frame
differs from frame zero; every separate-default vector also asserts that its
default differs from frame zero. The two nonselected alternatives can coincide
in one-bit layouts, but neither can silently stand in for the expected frame.

Independent Spec review confirmed eligibility for exactly the 116 additional
cells and preserved eligibility for the first 64. Standards review reported no
hard breaches or actionable smell findings. The new selector owns the 116 cells;
the existing selectors keep the 60 static and four RGBA8 APNG cells, with their
test-source bindings refreshed after review.

All **180** declared sample-vector cells now have proposed owners. The APNG
completion checkpoint was **1,026 assigned/excluded and 850 gaps** (1,023 proposed owners plus three
approved exclusions). The inventory and its catalog digest are unchanged.
Non-lattice precision and alpha now have subsequent
[reference-backed owners](png-precision-ownership.md). APNG control/subframe/malformed
classes, color/ICC/HDR, orientation, composition, umbrella facets and release
closure retain their separate gaps.

### APNG completion verification

- The new frame-contrast guard failed with the old separate-default fixture;
  all **four** module tests pass with the corrected fixture.
- Workspace/all-target Clippy with warnings denied, formatting and diff checks
  pass.
- `verify-ownership` passes with **1,876** dispositions, **1,026 assigned/excluded
  and 850 gaps**.
- Final `cargo test --locked --workspace --no-fail-fast` with Rust 1.97.1:
  **1,091 passed, 18 ignored**, including all four sample-contract tests, the 16
  ownership unit tests and five coverage CLI integration tests. The fast test
  binary completed in 983.68 seconds. Ignored external-harness/materialization
  tests remain outside this local run.
- `conformance verify-all` verified all **37** registered cases;
  `verify-inventory` verified the existing **168** executions.
