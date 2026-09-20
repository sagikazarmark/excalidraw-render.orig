# Stage 4 — PNG ICC/LUT, orientation, malformed inputs and interactions

> **Parent-review correction:** the initial handoff below is historical and is
> superseded by [the completed review remediation](png-stage4-review.md).
> The review correctly invalidated affected parts of the initial 57-owner proposal.
> After regression fixes and completed composition/resource engineering, the
> current proposal has **59 new owners, 1,802 assigned/excluded and 74 gaps**.
> The two concrete PNG rows are now qualified; the representative scene is
> materialized with a prepared approval subject and remains unapproved.
> No commit, baseline reapproval or #178 waiver has been made.

## Historical initial handoff

Base: `629930b` (committed Stage 3). This stage is an **uncommitted proposal** for
parent review, full-workspace testing and commit. No global approval is granted.

## Scope and ownership

**57 new primary owners** are proposed, reducing the ledger from **133 to 76 gaps**:

| Selection | New owners | Executable in `fast/png_stage4_contract.rs` unless noted |
| --- | ---: | --- |
| All EXIF orientations × little/big TIFF × before/after IDAT, plus PNG orientation facet | 33 | `all_orientation_cells_preserve_source_and_permute_pixels` |
| Invalid/absent/duplicate EXIF | 1 | `invalid_orientation_is_ignored_but_duplicate_chunks_fail` |
| Suggested palette and malformed palette/tRNS | 2 | `palettes_and_transparency_chunks_have_strict_sample_contracts` |
| ICC grayscale | 1 | `gray_icc_profiles_transform_expanded_samples` |
| ICC LUT and multi-process classes | 1 | `lut_profiles_select_tables_and_recover_unsupported_classes` (includes the additional Lab/M/B/table-selection cases) |
| Incompatible ICC models/classes | 1 | `incompatible_icc_models_preserve_svg_and_recover_projections` |
| Malformed iCCP | 1 | `malformed_iccp_has_explicit_ignore_or_recovery_contracts` |
| Invalid cICP fields | 1 | `reserved_cicp_fields_preserve_svg_and_recover_projections` |
| One-frame, subframe, blend/disposal, plus PNG animation facet | 4 | `apng_one_frame_subframes_blend_and_disposal_are_qualified` (also executes occurrence and malformed-later-frame checks) |
| APNG counters/order and late streams | 2 | `apng_sequence_counts_and_later_streams_fail_before_passthrough` |
| Malformed IHDR and PNG malformed facet | 2 | `malformed_png_headers_chunks_and_streams_have_complete_recovery` (also executes chunk/palette/scanline/APNG/ICC/EXIF failures) |
| Malformed chunks | 1 | `malformed_chunks_fail_and_unknown_ancillary_survives` |
| Malformed scanlines | 1 | `scanline_streams_and_adam7_extents_are_fully_validated` (includes bounded streaming controls) |
| Profile validation/decode boundaries | 1 | `profile_validation_and_decode_limits_have_exact_boundaries` |
| PNG default-frame and source-passthrough facets | 2 | `sample_color_orientation_animation_axes_compose` |
| Specified #154 limitation | 1 | `stage4_interaction_contract::nested_svg_pdf_dark_mode_retains_the_specified_154_limitation` |
| Independent lazy frame limits and atomic errors | 1 | `stage4_interaction_contract::lazy_frames_reset_limits_replay_assets_and_keep_errors_atomic` |
| Mixed diagnostic source order | 1 | **lib** `stage4_diagnostic_contract::mixed_normalization_render_resource_metadata_preserve_source_order` |

Result: **1,797 proposed owners + the same three approved exclusions = 1,800
assigned/excluded**, of **1,876** obligations. The 57 additions comprise **32 EXIF
cells, 17 concrete PNG qualification rows, five PNG umbrella facets and three
global interactions**. Every previously owned/excluded/gap disposition outside
this explicit selection remains byte-equivalent as a JSON value. No old owner
source refresh is needed. Criteria, catalog digest and all **168 execution keys**
are unchanged.

The read-only audit `tools/reference-oracle/scripts/verify-png-stage4.mjs`
authenticates these exact selections, their complete executable commands and live
source hashes, all prior dispositions, unchanged old records/approvals/inventory,
reference record/source digests, and consuming helper pins. Ordinary ownership
validation independently checks executable syntax and catalog consistency.

## Independent byte-format contracts

`tests/common/png_stage4.rs` supplies portable byte builders and authored pixels
and reports. CRC and deflate primitives encode inputs; neither production
decoders nor renderer reports supply expectations. Native tests render SVG, PNG
and PDF twice, requiring identical artifact/report bytes. Success checks include
source-exact SVG, untagged RGBA8 PNG, exact dimensions and pixels, and PDF
page/image/ICC/attached soft-mask bindings through the existing mutation-qualified
PDF oracle. Failures compare the entire ordered report, apart from nonempty prose
messages, and require the artifact to equal a missing-file recovery control.

| Byte-vector family | Cases |
| --- | ---: |
| Non-square 3×2 orientation permutations | 32 |
| Absent/invalid/truncated/unsupported TIFF and duplicate EXIF | 149 |
| Forbidden IHDR combinations and method/placement boundaries | 66,292 |
| CRC, length/truncation/type, unknown critical/ancillary, IEND/trailing bytes | 88 |
| Zlib/Adler, invalid filters, short/long rows, Adam7 extents, IDAT runs | 271 |
| Palette sizes/unused entries, optional PLTE, indexes and tRNS | 50 |
| APNG one/two/three-frame controls | 74 |
| APNG malformed controls/counters/data plus valid split-data controls | 2,078 |
| Reserved/unspecified/unknown cICP fields | 994 |
| Large output-block / single-byte input-chunk controls | 4 |

IHDR includes **all 65,521 forbidden color-type/depth pairs**, every forbidden
compression/filter/interlace method byte, zero dimensions and malformed placement.
Palette negatives have valid scanlines for their respective models; they cannot
pass solely because an unrelated wrong row length caused rejection. Independent
valid grayscale/RGB/gray-alpha controls check that isolation.

APNG controls cross both default layouts, frame counts 1/2/3, full/subframe
rectangles, source/over blend and none/background/previous disposal. First-frame
rectangle constraints, all invalid blend/disposal bytes on first and later frames,
zero delay/denominator, large play count, sequence/count failures, missing/extra
controls and data, malformed later zlib/Adler with valid outer CRCs and dimension
mismatches are exercised. The malformed group contains **four accepted split-data
controls** and **2,074 rejection cases**. In particular, **ancillary chunks between
fdAT pieces are legal**; they are distinct from forbidden nonconsecutive IDAT.

The occurrence contract additionally runs **18 source configurations** (static,
one-frame and two-frame × both layouts × zero/one/two live references, with a
deleted reference) in all formats. PNG/PDF flatten once per live multi-frame
occurrence; static, one-frame, deleted/unreferenced and SVG cases are silent.

## Reference-backed ICC interpretation

The separate stage-4 builder/collector/runner records **186 cases**:

| Reference group | Cases |
| --- | ---: |
| Gray v2/v4, gamma 1/2.2, 8/16-bit grayscale-alpha | 8 |
| LUT base classes and unsupported multi-process classes | 62 |
| Additional LUT PCS, table selection and mAB stages | 16 |
| Incompatible model/class signatures, including genuine RGB device-link | 44 |
| iCCP framing/profile failures and duplicate-selection behavior | 56 |

LUT inputs include **mft1, mft2 and mAB**, grids 2/3, all four header intents,
conflicting A2B0/A2B1/A2B2 tables, A2B1-only and saturation-only profiles, XYZ and
Lab PCS, B-only curves and an A/CLUT/M/matrix/B pipeline. A real **mpet D2B0 with
one 3→3 float `matf` element** establishes unsupported-class recovery independently
of the empty-mpet negative; a competing A2B table establishes actual selection.
Unsupported CMYK/Lab/XYZ/multichannel model signatures, gray-on-RGB, device-link,
abstract and named-color classes preserve SVG source bytes and recover PNG/PDF.
These are explicit project recovery contracts, not claims that the browser
rejects every malformed or incompatible declaration.

Malformed iCCP cases distinguish ignored framing/compression data from a selected
but unusable profile: missing terminator, empty/long names, method/stream errors,
empty/short profiles, invalid signature/table count/tag offset/tag size/declared
length and first-profile duplicate precedence. The original unaligned mAB
matrix/curve-offset fixture was initially mislabeled accepted. Its source PNG,
scene and captured reference bytes were retained; only its authored outcome was
corrected to strict unsupported recovery, and a separate correctly aligned case
was added. The collector's authenticated extension explicitly checks this narrow
correction and equality of every other prior case.

The unchanged official v0.18.1 wrapper runs on pinned Chromium **149.0.7827.55**,
in two fresh processes in forward/reverse order, fresh contexts and disabled
networking. Verification reproduced the full final record hash. The existing
**984-case color** and **84-gain-map + 24-control** records and their producers
remain unchanged.

Color appearance uses the user's selected **canonical Geometry/image gate:
1/400 changed pixels, minimum SSIM 0.999**, per image, with no new tolerance.
Dimensions, alpha, transparent-zero RGB, source preservation and complete reports
remain exact. PDF straight samples use the independent opaque companion with the
authored alpha plane. Exact opaque neutral anchors independently require linear
128→188, gamma-2.2 128→129, normalized half-linear 128→137 and saturation-only
fallthrough 128→128. A2B-over-mpet requires 137, distinguishing the float matrix.
The anchors apply to the stated 8-bit inputs, rather than pretending their
non-replicated 16-bit companions contain the same source sample.

## Demonstrated fixes and implementation review

- **Gray ICC:** the former RGBA-layout CMS call rejected a parsed grayscale
  profile. Expanded grayscale now feeds the CMS GrayAlpha layout; incompatible
  colored samples fail explicitly. Native PNG/PDF and browser-WASM pass.
- **LUT execution:** enable only the portable `moxcms` **lut** feature, keeping
  default/SIMD features disabled. Normalize the CMS destination's XYZ encoding,
  select A2B0 then A2B1, and keep saturation-only fallthrough. Table choice is
  separated from the CMS's implicit v4 black-point compensation. mAB XYZ matrix
  encoding is applied before B curves. Red cases showed neutral 255 instead of
  188, 188 instead of 137, Lab mAB SSIM 0.998619, and a multi-stage mAB failure;
  the final inputs pass the existing budgets. Source-class/direction/declared-size
  guards retain typed unsupported recovery. This feature is shared CMS code;
  passing PNG evidence does not qualify other codecs' LUT paths.
- **PNG validation:** reject excess inflated bytes, checksum errors, out-of-range
  palette indexes, invalid/misordered tRNS and palettes, undeclared extra APNG
  frames and malformed controls/data before SVG passthrough. A fixed streaming
  inflater counts the exact IHDR/Adam7/frame scanline extent, stops excess output
  at its first block, verifies zlib completion, and drains buffered output when
  input ends. A 37×33 positive control caught the initial missing-drain bug.
- **Legal fdAT interleaving:** review against PNG 3 §4.9.2 corrected an initially
  over-strict fixture/validator assumption. The png 0.17 sample decoder also
  closes the stream at intervening ancillary chunks. A borrowed `Read` adapter
  hides legal non-sample ancillary chunks inside that stream after the outer
  validator has checked every original byte/CRC. Pre-IDAT-ordered metadata stays
  visible; original SVG bytes and EXIF semantics are preserved. Both layouts
  pass exact source/report/pixel controls with interspersed text chunks.
- **APNG local work:** projection's precheck now includes already-selected extra
  PNG animation-frame validation work. Equality and one-less tests failed before
  this correction and pass after it. No accounting event or policy constant was
  introduced. The fixed logical decoder workspace remains the spec's workspace;
  the tests do not claim physical allocator-byte equality.

Primary-source corroboration: [PNG 3][png], [ICC.1:2022][icc], pinned
[SkPngRustCodec][skpng] and [skcms][skcms] (`read_mft_common`, `read_tag_mab`,
`skcms_ParseWithA2BPriority`), plus the pinned moxcms 0.9.0 parser/transform APIs
and black-point stage. The PNG framing/orientation builders and validator logic
are authored here; no Skia parser or interpolation implementation was copied.
CMS corrections use its public data/API and the numeric encoding relationships.
Stage 3's Skia adaptation notices and all distributed license bundles are retained.

## Composition, limits and honest remaining scope

The crossing contract executes **3,600 vectors**: 15 layouts × two interlace
modes × eight orientations × static/included/separate-default containers × five
signals (linear gAMA, linear cICP, linear RGB ICC, PQ, HLG). SDR fields contain
nontrivial colored samples; HDR uses independently pinned neutral anchors and
opaque/transparent alpha. Packed grayscale HDR crossings use endpoints. These
vectors establish the **default-frame and passthrough** facets but are not an
exhaustive color/profile/alpha composition proof.

**90 layout/interlace/container sources** exercise byte, dimension, pixel and
work equality/first crossing in all formats, with source/pixel/recovery controls.
SVG's dimension/pixel-limit silence is distinguished from PNG/PDF projection.
The projection tracer exercises gray16, Lab mAB and multi-stage mAB profiles,
duplicate versus distinct semantic identities, two occurrences, exact aggregate
byte and pixel boundaries, per-occurrence reference-image crops and atomic typed
errors. Lazy frames additionally replay one shared raster through success/error/
success targets under independent exact limits.

Remaining **76 gaps**, by category:

- **41 other-codec facets**: their full profiles, malformed classes, HDR,
  orientation, animation and resource behavior need their own evidence.
- **Four PNG umbrella facets:** alpha, color-profile, HDR/gain-map, limits.
  The current finite inventories do not close broader profile-composed alpha,
  mastering-display/content-light/AGTM metadata and complete resource behavior.
- **Two concrete PNG rows:** `codec:png:composition:qualified-axes` still needs
  broader LUT/gray-ICC/nonlinear/HDR-color × transparency/subframe crossings or
  reviewed equivalence arguments; `codec:png:limits:projection` still needs the
  full profile-workspace/mask/recovery boundary product. Their substantial tracers
  above are not promoted to full-criterion owners.
- **One interaction:** `interaction:frames-text-assets` requires an **approved
  representative scene**. The new authored tracer combines clipping, text,
  raster/nested SVG, links and embedded metadata in all formats, but has no
  representative-corpus manifest/reference/approval and receives no owner.
- **24 tolerance scopes** and **four corpus/PDF/inventory/target closure rows**
  remain unchanged, including **#178**.

## Executed verification

Toolchain/environment:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --locked -p excalidraw-render --test fast stage4 -- --nocapture
cargo test --locked -p excalidraw-render --lib stage4_diagnostic_contract::
cargo test --locked -p excalidraw-render --lib render::asset::tests::
cargo test --locked -p excalidraw-render --test fast png_color_contract::
cargo test --locked -p excalidraw-render --test fast png_precision_contract::
cargo test --locked -p excalidraw-render --test fast png_profile_contract::
cargo test --locked -p excalidraw-render --test fast static_png_asset_render::
cargo test --locked -p excalidraw-render --test fast resource_accounting::
cargo test --locked -p excalidraw-render --test fast frame_export::
cargo clippy --locked --workspace --all-targets -- -D warnings
bash tools/reference-oracle/run-png-stage4.sh verify
export HEADLESS=1 WASM_BINDGEN_TEST_TIMEOUT=180
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
# Each worker is executed separately with -- --exact:
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render --lib wasm_tests::png_stage4_icc_reference_in_wasm -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render --lib wasm_tests::png_stage4_byte_contracts_in_wasm -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render --lib wasm_tests::png_stage4_ihdr_contracts_in_wasm -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render --lib wasm_tests::png_sdr_color_reference_in_wasm -- --exact
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render --lib wasm_tests::png_gain_map_reference_in_wasm -- --exact
cargo run --locked -p xtask -- catalog verify
cargo run --locked -p xtask -- conformance verify-inventory
cargo run --locked -p xtask -- conformance verify-ownership
node tools/reference-oracle/scripts/verify-png-stage4.mjs
cargo fmt --all -- --check
git diff --check
```

Native: **22 stage-4 fast tests**, **one mixed-diagnostic library test**, **66
asset regressions**, **17 color/gain-map**, **five precision**, **four sample/PDF
oracle**, **29 static PNG**, **16 accounting**, **10 frame-export** tests passed.
That is **170 distinct focused/regression tests**, with no failures or ignores.
The final full stage-4 fast run before the six-case MPE extension passed in
29.38s; the extended LUT owner separately passed all **78 LUT cases** afterward.
Five browser-WASM workers passed; the ICC worker passed against the final
186-case record in **2.59s**. The final byte/IHDR/old-color/gain-map workers passed
in **1.76s / 12.63s / 6.74s / 0.62s**, respectively. The byte worker shares orientation/malformed/APNG/cICP and
gAMA/PQ/HLG crossing vectors; the dedicated IHDR worker executes all 66,292 inputs.
These are executed target tests, not a #178 cross-target equality approval.

**Existing baseline blocker:** `conformance verify-native-baseline` still fails
with `stale snapshot source tests/conformance/coverage/inventory.json`. Stage 3's
reviewed license-source binding correction caused this before Stage 4. The
baseline, expectations, approvals, inventory and execution keys are not rewritten.
Separate parent **clean-commit collection/binding review/reapproval** is required.
The full-workspace run and independent parent review remain parent actions.

## Final reference hashes

| Record | SHA-256 |
| --- | --- |
| Original 984-case color record | `2aae93308b4289d3374ec92e960e756106af621f9bfc64e4b29314f5162857d0` |
| Original 108-case gain-map record | `dee2a45432ce2a4e211d1e51e8e80f571bfd8f90221c7bf85153b57df88714ad` |
| New 186-case Stage 4 record | `42111900327cd1e0d70cac36763dc7296e14b94c716458f5e54cdd3f68310fd1` |
| Shared byte fixture/report helper | `56e0401fc0ce0bee42135225a39e428923ea134c2b2e9528d78f754fe6bc1e16` |
| Shared Stage 4 reference helper | `b205eb29d00133aac197f3a293d361c200819710ee50ee06640b7cb437384cbd` |
| PNG owner source | `0a84bcf1acd46f38800aa7c6bd655497989842bca8efe989ded6c28be02dd289` |
| Interaction owner source | `a3ad632bc6b4be3ea7fda7c942142d834754598e217c61128e0f7ac9c64f3e5a` |
| Library diagnostic owner source | `aa85d965584536aa9b8ceffa367f1b4f131281cdcb8ae3dc5d34bf157038b59f` |
| Ownership ledger | `f0a2414dc2b89b0e8fdf7e7feab165126f9a1723456938165f1f2552b8e65e84` |

The read-only audit prints every current owner/production digest; the ownership
ledger binds the three exact owner sources and the tests pin all reused helpers.

## Changed files (20)

Modified:

1. `Cargo.toml`
2. `crates/excalidraw-render/src/lib.rs`
3. `crates/excalidraw-render/src/render/asset.rs`
4. `crates/excalidraw-render/src/render/asset/color.rs`
5. `crates/excalidraw-render/src/render/asset/png.rs`
6. `crates/excalidraw-render/tests/common/mod.rs`
7. `crates/excalidraw-render/tests/fast/main.rs`
8. `tests/conformance/coverage/OWNERSHIP.md`
9. `tests/conformance/coverage/ownership.json`

Added:

10. `crates/excalidraw-render/src/stage4_diagnostic_contract.rs`
11. `crates/excalidraw-render/tests/common/png_stage4.rs`
12. `crates/excalidraw-render/tests/common/png_stage4_reference.rs`
13. `crates/excalidraw-render/tests/fast/png_stage4_contract.rs`
14. `crates/excalidraw-render/tests/fast/stage4_interaction_contract.rs`
15. `docs/research/png-stage4-ownership.md`
16. `tests/oracle/png-color/stage4-reference.json`
17. `tools/reference-oracle/run-png-stage4.sh`
18. `tools/reference-oracle/scripts/png-stage4-evidence.mjs`
19. `tools/reference-oracle/scripts/png-stage4-fixtures.mjs`
20. `tools/reference-oracle/scripts/verify-png-stage4.mjs`

[png]: https://www.w3.org/TR/2025/REC-png-3-20250624/
[icc]: https://www.color.org/specification/ICC.1-2022-05.pdf
[skpng]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/codec/SkPngRustCodec.cpp
[skcms]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/modules/skcms/skcms.cc
