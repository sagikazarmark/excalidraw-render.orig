# Stage 4 — parent-review remediation (qualification incomplete)

**Current checkpoint:** [follow-up validation and remaining blockers](png-stage4-followup.md).
The earlier completion claims below are historical, not the current disposition.

> **Follow-up review hold:** the common-header and matrix/TRC allocation paths,
> and the representative candidate's complete viewer/approval integration, are
> under further correction. The earlier “approval only” and blanket 59-owner
> eligibility claims below are retracted. Subject `74bc0584…` does **not** qualify
> the requested representative scene and must not be presented for approval.
> Final follow-up evidence and eligibility will supersede this historical report.

Base: `629930b`. **Uncommitted, pending independent parent review and the parent
full-workspace run.** This supersedes the initial completion/eligibility claims
in [the historical handoff](png-stage4-ownership.md).

## Disposition

All five reported blockers are fixed. The two concrete composition/projection
rows are now implemented and tested, rather than deferred as external blockers.
The initial **57 owners were provisional** while the affected APNG, malformed
PNG, ICC and allocation paths failed review. Their owner functions now include
the review regressions and source bindings are refreshed only after execution.
The current proposal adds **59 owners** relative to the base: the same 57 plus:

| Obligation | Exact owner |
| --- | --- |
| `codec:png:composition:qualified-axes` | fast `png_stage4_contract::complete_profile_composition_matches_independent_reference` |
| `codec:png:limits:projection` | lib `png_projection_contract::profile_workspace_masks_sharing_and_recovery_have_exact_boundaries` |

Totals: **1,799 proposed owners + three unchanged exclusions = 1,802
assigned/excluded**, with **74 gaps** out of 1,876. All **948 enumerated PNG/APNG
inventory obligations** have proposed owners. This does not close the broader
codec umbrella or corpus/target obligations.

## Reported defects: red/green proof

`cargo test --locked -p excalidraw-render --test fast png_stage4_contract::review_`
initially failed all three tests on the reported symptoms, then passed after the
fixes:

1. **Adam7 subframes.** png 0.17 writes interlaced subframes at the IHDR canvas
   stride. Validation now examines actual frame rows at that stride; SDR decoding
   explicitly repacks rows before interpreting the packed frame. Narrow 1×2
   indexed frames on a 3×2 canvas distinguish the two rows, and a bad index in a
   later row must reject **SVG too**. The broad crossing matrix additionally
   exercises 15×13 frames at offset (1,2) on 17×16 canvases, all represented
   layouts, alpha modes and profiles, interlaced and noninterlaced.
2. **ICC declared extent.** The declared profile length bounds the tag table,
   every tag-data range, and the slice passed to the CMS/owned LUT parser. The
   original tail is never available to that parser. Declarations 132, 144, 215,
   216, 296 and 308 with a complete trailing profile require unsupported-color
   recovery; the complete 312-byte control remains valid and SVG preserves bytes.
3. **Lower-depth tRNS.** Revision-1's strict malformed-PNG contract rejects a
   grayscale/RGB key component outside the IHDR sample range. Comparison is not
   delegated to png's discarded high bytes. Grayscale 1/2/4/8 and every RGB8 key
   channel cover zero, maximum, first crossing, 0x0100 and 0xffff, including the
   black-alias case. The shared suite has 35 executions (31 distinct vectors).
   Legal 16-bit keys remain covered by the unchanged precision suite.

### Allocation safety and CMS failure classification

The newly enabled moxcms LUT path was not allocation-safe. It is **no longer
enabled**. `asset/icc_lut.rs` is an owned, bounded source-profile adapter:

- It parses `mft1`, `mft2` and `mAB` from validated byte extents; supports their
  8/16-bit tables, one/three input channels, output curves, CLUT and M/matrix/B
  stages, normalized XYZ and Lab PCS, and A2B0/A2B1 selection.
- Curves, coordinates, interpolation corners and matrix state are fixed arrays.
  It reads the encoded table directly. There is **no expanded CLUT, baked grid,
  dynamic curve table, stage list or per-pixel allocation**.
- The encoded selected table is copied with `try_reserve_exact`; fixed program
  storage is also a fallibly reserved one-element vector. These are actual
  retained allocations, not a fake preflight ahead of an infallible dependency.
- Unselected saturation/inverse/gamut LUT entries are filtered from the
  fallibly copied matrix/TRC input before calling moxcms. Thus its infallible LUT
  reader is also unreachable, including for unselected LUTs. Saturation detection
  examines actual tag-table entries, not incidental bytes inside tag payloads.
- The same adapter routes source LUTs in PNG, JPEG, WebP and PNG-in-ICO/nested PNG.
  Other-codec umbrella qualification is still separate. The moxcms **0.9.0**
  dependency has default features disabled and no `lut` feature; the separate
  pre-existing image-library dependency is not confused with this adapter.
- Source-owned LUT/TRC vectors are not deep-cloned. All new variable copies are
  fallible. No fork, dependency license change or production `unwrap` is added.
  New transcendental evaluation goes through `portable_math::powf`.

`CmsError::OutOfMemory(_)` maps to fatal `ResolveError::Memory` at parsing,
transform construction and execution boundaries. The failure seam returns the
same dependency error at each real call boundary; separate seams exercise the
actual owned profile and program allocations. Temporarily restoring the old
mapping made the facade test fail with **“CMS Parse allocation refusal must be
fatal in Png”**. The restored mapping passes. Tests cover PNG/JPEG/WebP matrix
paths, every accepted LUT case, and the one-dimensional gray LUT cases; SVG
remains source-preserving because it does not interpret profiles. Errors require
the exact fatal code/category/format, memory subject, empty accumulated report
in these fixtures, repeatability, and successful rendering after the fault scope.

The parser/executor was authored against ICC encodings and existing qualified
numeric semantics. No Skia or moxcms parser/interpolator implementation was copied.
Stage 3's actual Skia adaptation and its distributed notices remain unchanged.

## Completed composition proof

The new independent encoded matrix has **1,968 crossing cases**:

- **16 profile families:** v2/v4 gray, LUT8/LUT16, XYZ/Lab, B-only and full
  mAB stages, A2B fallback/saturation/MPE precedence, nonlinear legacy gamma,
  non-sRGB chromaticities, parametric/sRGB ICC, Rec.2020 PQ and XYZ HLG.
- Legal 8/16-bit intrinsic/key/opaque layouts; gray packed 1/2/4 key/opaque;
  indexed 1/2/4/8 with palette alpha where the model permits it.
- Both interlace modes; static, included-default APNG and offset multi-row
  separate-default subframes; two non-square orientation permutations.
- Spatially varying RGB/gray and alpha, not a padded constant field. Native
  SVG/PNG/PDF are repeated with exact reports, dimensions, alpha, zero RGB,
  original SVG source and attached PDF straight RGB/mask checks. Every case also
  checks late-EXIF **artifact/report equality** against its early-EXIF input in
  PNG/PDF; WASM executes the same PNG equivalence.

An additional **12 one-dimensional gray LUT programs** (mft1/mft2/mAB × v2/v4 ×
8/16-bit sources) compare against the independently captured linear-gray field.
Their affine XYZ table lies along D50 white, so under the pinned normalized-XYZ
mapping it is analytically the same linear-gray transform. This establishes
actual execution of one-channel LUTs, not merely parsing them.

### Why the color oracle is a selected-frame static canvas

Two independently repeated collections exposed pinned-browser limitations:
late eXIf is ignored, and animated orientation/natural-size handling introduces
resampling even with an early eXIf. The source PNG/APNGs and both observation
records are retained unchanged. They are not silently relabeled passing evidence.

The passing color oracle is a **separately authored selected-frame static canvas**:
its builder starts from the declared sample field, expands only the specified
packing/palette/key representation, places the frame on the declared transparent
canvas and carries the same profile and orientation. No production decoder,
renderer output or fitted color supplies those samples. The pinned official
renderer then renders that static source and its opaque companion in two fresh
Chromium149 processes. Original APNG bytes are still the native/WASM input.
There is no post-render resizing/alignment, altered budget, or browser-produced
expected compatibility report. Each row records this `reference_basis` explicitly.

### Equivalence argument for the remaining axis product

This is a reviewable argument over the actual seams, supported by the crossings:

1. The full existing sample matrix qualifies packing/filter/interlace expansion.
   The new stride regressions and subframes qualify placement separately. Their
   result is the declared sample/alpha canvas, independent of the color profile.
2. SDR normalization takes only expanded samples and the selected profile;
   owned LUT evaluation is pointwise and does not receive container, orientation,
   row-filter, palette-index or tRNS state. Gray expansion and channel consistency
   are explicit. All legal alpha representations cross the profile classes.
3. HDR has its own precision/premultiplication path. The existing full 704-cell
   product and exact anchors qualify its transfer/gamut behavior; the new colored
   PQ/XYZ-HLG crossings exercise both precisions, alpha representations, Adam7,
   subframes and orientation in that separate path.
4. Every orientation is a bijective permutation of complete RGBA pixels. It
   commutes with pointwise color/alpha conversion and cannot change table choice.
   The 32 authored non-square orientation cells plus all-case late-placement
   equality establish the metadata/permutation seam without inheriting the
   browser's late-metadata defect.
5. APNG frame selection and complete later-frame validation precede conversion.
   Source/over on the initially transparent frame-zero canvas gives the same
   selected samples; disposal/later frames cannot affect that static export.
   The control matrix, narrow multi-row cases and independent static canvas
   enforce this, including malformed later-frame rejection before SVG success.

The composed correctness claim is these defined transformations and their
interfaces, not enumeration of every possible byte string or ICC coefficient.
The selected canonical **1/400 changed pixels, SSIM 0.999** limits remain intact,
with exact semantic/report/source/alpha gates.

## Completed projection/resource proof

The owner runs **504 profile/layout/alpha/container sources**: 492 distinct
noninterlaced orientation-6 representatives of the reference product plus the
12 one-channel LUT programs. Each crosses **plain, rounded-image and frame-mask
contexts**, with **two rendered references**, in **SVG, PNG and PDF**: **4,536
baseline contexts**. Interlace/orientation change the payload size but not the
specified workspace/plane/mask formulas; their decode behavior is separately
covered by the full composition product.

Assertions include:

- One color/decode transaction per shared identity, independent of occurrences;
  exact `1 MiB + 32*max(width,height)` workspace, `256+4P` decoded record,
  payload-byte and `4P` source-pixel weights.
- **N−1/N/N+1** for the earlier decode workspace and the global working-byte
  peak. PNG's larger later encoder envelope is distinguished from successful
  admission of the earlier profile workspace. Fatal outcomes retain the exact
  authored diagnostic prefix; repeated errors/descriptors agree.
- **N−1/N/N+1** validation and full decode/projection local-work boundaries,
  including additional APNG frame validation. Local recovery artifacts equal an
  independently specified missing-source control under each mask context.
- Frame masking charges **4P** for this two-image target: main surface, one
  shared frame mask and one image-owned mask per occurrence. First crossing is
  fatal before flattening; equality/spare capacity preserve the full artifact.
- Per-source alias tests cross equal bytes under two IDs and distinct identities
  made by an ancillary insertion. Aggregate payload bytes never deduplicate;
  decoded pixels do. Exact byte/pixel first crossings are atomic. At a second
  decode crossing, PNG has no flattening prefix; PDF retains its already planned
  first occurrence. Both are asserted explicitly. Incoherent per-asset/aggregate
  configurations are rejected by the request builder.
- SVG's low decoded-pixel ceilings do not invoke color/decode operations; its
  validation work and source preservation are independently checked.
- Actual profile/program allocation refusal and CMS OutOfMemory handling are
  included by the owner. Logical workspace accounting is not misrepresented as
  physical allocator-byte equality.

The native owner passed in **125.62s**. Three executed WASM workers partition
the same contexts by mask (79.60s, 38.39s and 42.64s), under the requested 180s
worker timeout. No trace digest is pinned and #178 is not waived.

## Representative scene: materialized, awaiting approval

Directory: `tests/conformance/candidates/stage4-frames-text-assets/`.
It contains the authored scene and raster/nested SVG assets, official SVG/PNG and
RGBA reference evidence, provenance, three authored complete reports, native
SVG/PNG/PDF artifacts, a manifest and canonical approval-subject bundle.

The candidate test passes the canonical direct-PNG comparison, text/link/clip,
native nested-SVG PDF and raster/soft-mask structure, embedded metadata,
complete reports and artifact repeatability. The spec's existing SVG frame-clip
authority divergence is stated explicitly. Reference generation and verification
reproduce **200×120** evidence in fresh pinned Chromium149 processes with only
manifested package assets available.

```sh
cargo run --locked -p xtask -- conformance candidate-approval-subject \
  tests/conformance/candidates/stage4-frames-text-assets
```

**Prepared approval subject:**

`sha256:74bc05841a04b85e0ebdb45ef1362ed00010b98a55b193ad380743232bb327bd`

The new read-only candidate command checks every descriptor, complete artifact/
report set, pinned profile/producer bindings, canonical budget and subject bundle.
The candidate stays outside the approved registry, preserving all **168 execution
keys**. `interaction:frames-text-assets` remains a gap until the maintainer approves
the actual candidate. No approval was fabricated or recorded.

## Executed checks and remaining blockers

Using Rust 1.97.1, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_TEST_DEBUG=0`:

- **27 Stage 4 fast tests**, three Stage 4 library contracts (mixed diagnostics,
  CMS failures, full projection limits), and **147 existing focused regressions**:
  **177 distinct native tests**, no failures/ignores in the final runs.
- One additional candidate-approval binding test passes, rejecting changed,
  missing and detached evidence in an isolated copy: **178 focused native/tool
  tests total**. Candidate validation lives in its own module so the pre-existing
  source-bound conformance validator and inventory descriptors stay unchanged.
- **11 executed WASM workers**: the five earlier PNG/color/gain-map workers,
  review regressions/gray LUTs, the full composition matrix, CMS failures, and
  three projection-limit workers. Every invocation uses `-- --exact`,
  `HEADLESS=1` and `WASM_BINDGEN_TEST_TIMEOUT=180`.
- All three new **1,968-case** reference records and the representative reference
  independently verify. A combined observation-verification invocation exceeded
  its host command timeout; the affected collector was rerun alone and reproduced
  its full hash. No evidence or expected outcome was refreshed for that retry.
- Workspace/all-target Clippy (`-D warnings`), formatting, catalog, inventory,
  ownership, candidate-subject and read-only preservation checks pass.

The remaining **74 gaps** are 41 other-codec facets, four PNG umbrella facets,
the one **approval-blocked representative interaction**, 24 tolerance scopes and
four closure obligations. They are not automatically closed by PNG examples.
There is no remaining implementation deferral for the two concrete PNG rows.

The pre-existing native baseline still reports
`stale snapshot source tests/conformance/coverage/inventory.json`. Its source,
expectations and approval are untouched. Parent clean-commit collection/binding
review/reapproval and the independent full-workspace run remain required; #178
remains open. This session makes no commit.

## Evidence hashes

| Evidence | SHA-256 |
| --- | --- |
| Original 984-case color | `2aae93308b4289d3374ec92e960e756106af621f9bfc64e4b29314f5162857d0` |
| Original 108-case gain-map | `dee2a45432ce2a4e211d1e51e8e80f571bfd8f90221c7bf85153b57df88714ad` |
| Original 186-case Stage 4 ICC | `42111900327cd1e0d70cac36763dc7296e14b94c716458f5e54cdd3f68310fd1` |
| Late-EXIF/animation observations, 1,968 | `def2563be8d963fe188792c603720b50b02855adde0b1ab957366e7fd5466445` |
| Early-EXIF/animation observations, 1,968 | `fb97e4b298deebca1800f8919b14ce223099e86eb41df64a3ad0148b43167bc0` |
| Selected-frame static color oracles, 1,968 | `9de893992bef08f966c0bb149f0cd61f3c20d6f3c2e701ded5706b39c60953f3` |
| Representative official PNG | `f4617d05ac40ffa94de75bb969afe4a4c9f9ab7fcec3b566056e46d0706030bb` |
| Owned LUT adapter | `20a49641eb19284c63a55d209cbf3a0fcddf35cf04d5af392aee320302c6bee8` |
| PNG owner source | `9232176f198035c06994ae5e21189b190fb2b92d57e2cbf275338f109ec6a8e7` |
| Projection owner source | `b46d71eacbb4008de64631e62926c60b1b02d607407187470ff37e6f806f6720` |
| Ownership ledger | `6882c37db5a89ebcccce0ee0b0a5bf399cbcc891cb20a10e19f3bfea1e0d7788` |

`node tools/reference-oracle/scripts/verify-png-stage4.mjs` authenticates exact
owner selections and all old dispositions, sources/records, helper pins and the
candidate subject, and prints current production/owner digests and changed files.

## Complete changed-file list (60, relative to the base)

Under `crates/excalidraw-render/` (19):

```text
src/cms_failure_contract.rs
src/lib.rs
src/png_projection_contract.rs
src/render/asset.rs
src/render/asset/color.rs
src/render/asset/icc_lut.rs
src/render/asset/jpeg.rs
src/render/asset/png.rs
src/render/asset/tests.rs
src/render/mod.rs
src/stage4_diagnostic_contract.rs
tests/common/mod.rs
tests/common/png_stage4.rs
tests/common/png_stage4_composition.rs
tests/common/png_stage4_reference.rs
tests/common/png_stage4_review.rs
tests/fast/main.rs
tests/fast/png_stage4_contract.rs
tests/fast/stage4_interaction_contract.rs
```

Under `tests/conformance/candidates/stage4-frames-text-assets/` (16):

```text
README.md
approval-subject.json
expected-report-pdf.json
expected-report-png.json
expected-report-svg.json
manifest.json
native.pdf
native.png
native.svg
nested.svg
official-png.rgba
official.png
official.svg
provenance.json
raster.png
scene.json
```

Remaining paths (25):

```text
docs/research/png-stage4-ownership.md
docs/research/png-stage4-review.md
tests/conformance/coverage/OWNERSHIP.md
tests/conformance/coverage/ownership.json
tests/oracle/png-color/stage4-composition-reference.json
tests/oracle/png-color/stage4-oracle-reference.json
tests/oracle/png-color/stage4-qualified-reference.json
tests/oracle/png-color/stage4-reference.json
tools/reference-oracle/run-png-stage4-oracle.sh
tools/reference-oracle/run-png-stage4-qualified.sh
tools/reference-oracle/run-png-stage4-review.sh
tools/reference-oracle/run-png-stage4.sh
tools/reference-oracle/run-stage4-representative.sh
tools/reference-oracle/scripts/png-stage4-composition-fixtures.mjs
tools/reference-oracle/scripts/png-stage4-evidence.mjs
tools/reference-oracle/scripts/png-stage4-fixtures.mjs
tools/reference-oracle/scripts/png-stage4-oracle-evidence.mjs
tools/reference-oracle/scripts/png-stage4-oracle-fixtures.mjs
tools/reference-oracle/scripts/png-stage4-qualified-evidence.mjs
tools/reference-oracle/scripts/png-stage4-qualified-fixtures.mjs
tools/reference-oracle/scripts/png-stage4-review-evidence.mjs
tools/reference-oracle/scripts/stage4-representative.mjs
tools/reference-oracle/scripts/verify-png-stage4.mjs
xtask/src/conformance_candidate.rs
xtask/src/main.rs
```

`Cargo.toml` is back to its original moxcms feature configuration; `Cargo.lock`,
the original conformance validator, coverage inventory/requirements, approved
records and all distributed license bundles match the base.
