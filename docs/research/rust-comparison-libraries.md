# Rust libraries for excalidraw-render-compare

Source audit, 2026-09-26. Inspected the local working tree (HEAD
`adb6f8e211e3a331e56495b393811a3a1c95e372`), including comparator source,
tests, profile, and renderer spec §14.3. Upstream evidence is limited to
published crate source/documentation and upstream GitHub source/history.
Versions below were the versions returned by docs.rs `latest` during this
audit; citations use explicit versions or commits. This is a source review,
not a compiled compatibility, performance, or cross-target experiment.

## Recommendation

**Keep the existing Pixelmatch and SSIM kernels. None of these three candidates
is a drop-in implementation of the current comparison protocol.** Here,
“drop-in” means preserving metric outputs, masks, diagnostics, and error
behavior through an adapter—not merely accepting similar image inputs.

| Candidate, verified published version | Fit for the existing comparator | Worth using for another part? |
| --- | --- | --- |
| `pixelmatch` **0.1.0** | Reject as a replacement: PNG-stream API, different alpha and floating-point operation order, and incompatible diagnostic alpha. | Little benefit here; the existing raw-buffer port already implements the pinned upstream algorithm. [P1–P3, M1, L2] |
| `image-compare` **0.5.0** | Its `MSSIMSimple` is a different metric: non-overlapping 8×8 blocks, unnormalized covariance sums, and area-weighted pooling. | Potentially useful for a separate exploratory heatmap/RMS/hybrid diagnostic tool, if that feature is actually wanted. It supplies no compatible canonical metric kernel. [I1–I4] |
| `dssim-core` / `dssim` **3.5.1** | Deliberate perceptual metric change: Lab, different blur/borders, f32/FMA, custom pooling, normally multiple scales. | Best of these for an **additional**, explicitly versioned perceptual diagnostic. Use the core library for existing pixels; the `dssim` wrapper adds image loading/CLI machinery. Published threaded 3.5.1 also has a score-ordering caveat. [D1–D7] |

No fourth candidate was added: these cover the direct Pixelmatch port, a
general-purpose Rust SSIM API, and a dedicated perceptual comparator. The
binding constraints already decide their suitability.

## What local compatibility actually requires

The following are executable requirements, not interchangeable SSIM defaults:

- **Inputs and projections:** exact-size, row-major straight-alpha sRGB RGBA8;
  clear RGB under zero alpha. Generate opaque alpha, black, and white views
  using the specified unsigned-integer compositing and rounding. There is no
  image decoding in the comparator's normal library path. [L1, L2]
- **Pixelmatch:** Mapbox **7.2.0**, commit
  `9faed09302aaecec130b4ce0e8505d5ed5221393`; gamma-coded YIQ distance;
  `threshold = 0x3fb999999999999a` (0.1), `includeAA = false`,
  `checkerboard = true`; strict `abs(delta) > 35215*t*t`, source-order binary64
  arithmetic, and exact AA traversal/ties. Union the three changed masks by
  coordinate **after** AA suppression. A sum or maximum of three counts cannot
  replace that union. [L1–L3, M1]
- **SSIM:** single-scale, no prefilter/downsampling; overlapping 11×11 valid
  windows, sigma 1.5, **121 frozen binary64 coefficient words**; population
  moments with the specified `(w*x)*x` and `(w*x)*y` evaluation order. Clamp
  negative variances to zero, leave covariance signed, and use frozen C1/C2.
  Sum scores in plain row-major f64 order and divide once. No reassociation,
  FMA, approximate math, or parallel reduction. Black/white luma is the
  specified gamma-space f64 expression; alpha is the byte value. The result
  is the **minimum of three independently computed scores**. [L1–L3]
- **Regions and failures:** extract crops before either metric so AA sees crop
  boundaries; declared crops must be at least 11×11. A smaller complete
  artifact passes only if all three view buffers are identical. Non-finite
  SSIM fails; exact integer budget equality and f64 score-floor equality pass.
  Both metrics must pass for the whole artifact and each crop. [L1, L3, L6]

Tests inspected:

| Local evidence | What a future replacement must preserve |
| --- | --- |
| `tests/pixelmatch_upstream.rs` | Twelve pinned upstream cases assert **counts and every diff RGBA byte**, including 143 at threshold .05, 106 at .1, diff masks, alternate colors, and identical-image output. [L4] |
| `tests/pixelmatch_vectors.rs` | Checkerboard vs white alpha, strict threshold equality and adjacent f64 values, AA gating, first-extremum ties, ECMAScript position arithmetic, equal-luma/chroma neighbors. [L4] |
| `tests/ssim_vectors.rs` | Kernel digest, exact identity bits, NaN/overflow failure, input lengths, minimum size, negative covariance, valid-edge behavior, **121 exact kernel-position score words**. [L5] |
| `tests/comparison_protocol.rs`, `tests/canonical_raster.rs` | Projection bytes, mask union, minimum alpha-view SSIM bits, crop boundaries, small-artifact handling, budgets and invalid dimensions. [L6] |
| `tests/profile.rs` and canonical profile | Source/vector hashes and the comparator's reachable lockfile closure are bound. A behavior-preserving dependency addition still changes comparator identity. [L7] |

## 1. pixelmatch 0.1.0: same family, not the pinned port

Published VCS identity: `faf5b854a266d12e559e3215a9b603ab0d425688`.
The crate's description says it is based on Mapbox Pixelmatch; it does not
establish 7.2.0 conformance. Source establishes these concrete differences:
[P1–P3]

1. **API mismatch.** `pixelmatch<IMG1: Read, IMG2: Read, OUT: Write>` decodes
   both streams with `PngDecoder` into `DynamicImage`, optionally writes a PNG,
   and returns a count. There is no public raw-RGBA comparator or returned
   Boolean changed mask. An adapter would need PNG round trips and mask
   extraction from a decoded diff-mask image, or a fork exposing the kernel.
   This duplicates work outside the current comparison boundary. [P2, L2–L3]
2. **Different alpha model.** The crate blends each translucent color against
   white separately and has no `checkerboard` option. Version 7.2.0 instead
   computes alpha-aware channel deltas with its position-dependent background.
   The local `[0,0,0,128]` versus `[127,127,127,255]` test explicitly distinguishes
   checkerboard from white. **Important scope distinction:** canonical views
   are already opaque, so this alpha difference alone does not prove different
   canonical gate counts; it does disqualify the public Pixelmatch kernel's
   full tested behavior. [P2, M1, L2, L4]
3. **Opaque arithmetic still differs.** The crate calculates
   `rgb2y(rgb1) - rgb2y(rgb2)` (and similarly I/Q); 7.2.0 first subtracts byte
   channels and then applies YIQ coefficients. These expressions are equal in
   real arithmetic, but not generally in binary64. AA brightness uses the same
   different order. Thus even supplying opaque views does not preserve the
   required source arithmetic; boundary classification and numerical AA ties
   need qualification, not an assumption of equivalence. Both implementations
   do use strict threshold comparison and x-outer/y-inner AA traversal. [P2, M1]
4. **Concrete diagnostic incompatibility.** `draw_gray_pixel` writes
   `Rgba([val,val,val,val])`; Mapbox and the local port write alpha **255**.
   For an identical opaque-black pixel with default diagnostic alpha .1, the
   crate's source gives `[229,229,229,229]`, versus `[229,229,229,255]` in the
   pinned implementation. This is source-derived, not a compiled probe. [P2, M1]

**Dependency weight:** the crate requires `image ^0.23.12` with image defaults
enabled. Its optional `structopt ^0.3.21` is only for `build-binary`; omitting the
binary does not remove image codecs. The compatible image 0.23.14 manifest
enables GIF/JPEG/PNG/TIFF and other formats, plus `jpeg_rayon`; setting
`pixelmatch`'s own `default-features = false` cannot disable those dependency
defaults. This is a substantial functionality mismatch for an existing
raw-buffer kernel, not a measured binary-size claim. License: MIT. [P1, P4]

**Maintenance evidence:** docs.rs lists a single release, 2021-01-14. The
upstream default-branch tip observed was `ab40c4ef…`, dated 2021-01-16
(“Update README.md”); repository metadata says it is not archived. That shows
little recent published/default-branch activity, not proof of abandonment or
unanswered support requests. [P3, P5]

**Decision:** retain the local port; use the exact Mapbox commit and existing
vectors as its authority. Adapting this crate would preserve very little value
after correcting the API and arithmetic.

## 2. image-compare 0.5.0: an SSIM-shaped metric change

Published VCS identity: `1bcd98373007f8eb6c89974fb0695f44268cffc3`.
The relevant public entry point is
`gray_similarity_structure(&Algorithm::MSSIMSimple, &GrayImage, &GrayImage)`.
It returns a f64 score and a float similarity image. [I1, I4, I5]

- `DEFAULT_WINDOW_SIZE = 8`; `Window::subdivide_by_offset(8)` tiles the image,
  includes clipped partial edge blocks, and visits block columns before rows.
  This is neither overlapping valid windows nor 11×11 Gaussian weighting. Its
  “MSSIM” means averaging local SSIM, not multi-scale SSIM. [I2–I3]
- Means are uniform block means. `covariance()` returns
  `sum((x-mean_x)*(y-mean_y))` **without dividing by block area**; variances use
  the same helper. Its test explicitly expects 72 for a nine-pixel block whose
  population variance is 8. C1/C2 still use L=255 and K1=.01/K2=.03. The missing
  normalization changes their relative contribution, even before considering
  the different window. [I2]
- Pooling is `sum(block_score * block_area) / sum(block_area)`. Rayon computes
  blocks with indexed `par_iter`, collects a Vec, then ordinary serial iterators
  sum it. **Rayon alone is not evidence of nondeterministic reduction here**;
  the demonstrable incompatibilities are the estimator, windows, pooling order,
  and input precision. There is no configuration for the normative kernel or
  those arithmetic rules. [I2–I3]
- `GrayImage` contains u8 samples, so it cannot directly carry the comparator's
  fractional f64 black/white luma. RGB structural comparison takes the minimum
  across separate R/G/B channel scores; hybrid comparison combines YUV and
  RMS/SSIM. Neither is the three-view protocol. Its alpha-blending helper uses
  normalized f32 arithmetic and truncation, not the integer projection rule.
  Reusing that helper would also change comparison inputs. [I3–I4, L1–L3]

**Dependency weight:** four normal direct dependencies: `image ^0.25`
with defaults disabled, `thiserror ^2.0`, mandatory `rayon ^1.10`, and
`itertools ^0.14`. Default crate features are empty; this does **not** make
Rayon optional. Cucumber/Tokio and PNG-enabled image are dev-dependencies,
not normal consumer dependencies. This avoids the pixelmatch crate's broad
image-codec defaults, but still adds an image/threading dependency graph to a
currently normal-dependency-free comparator. License: MIT. [I1, L8]

**Maintenance evidence:** release 0.5.0 is dated 2025-08-18; the documented
history includes 0.4.2 dependency updates in May 2025 and image 0.25 support in
0.4.0. The observed unarchived repository's default-branch tip is the release
commit above, dated 2025-08-18. That supports a release history through 2025;
it does not establish present response time or future support. [I5–I6]

**Decision:** no canonical-kernel reuse. Its visualization and alternative
metrics may be useful for separately named diagnostics; adding them to the
gate would require a new metric definition and new thresholds.

## 3. dssim-core / dssim 3.5.1: useful separate perceptual diagnostics

Published core VCS identity: `6e4579882f97907a75fd84133a4c4993a06d56ea`.
Use `Dssim::create_image_rgb`, `create_image_rgba`, or generic `create_image`
with imgref-backed input, then `compare`; optional saved maps expose f32 local
scores. The parent `dssim` package supplies CLI/image-loading functionality.
This API is a much better raw-pixel fit than the pixelmatch crate, but its
metric is intentionally different. [D1–D3, D7]

| Dimension | Published 3.5.1 behavior |
| --- | --- |
| Color/alpha | sRGB byte entry points convert to linear-light data and then a modified Lab representation. Generic f32 grayscale input is documented as linear-light and is transformed to L*. The RGBA path uses its own alpha treatment, not the three complete project views. [D3, D5] |
| Scale | Five default weights `[.028,.197,.322,.298,.155]`; configurable with `set_scales`. A one-weight configuration changes how many scales contribute, not the local metric. [D3] |
| Window/borders | Separable 5-tap blur derived from composing two 3-tap blurs; custom clamped-edge coefficients; same-size maps including edges. No normative 11×11 valid-only option. [D3–D4] |
| Precision | f32 planes, moments, and maps; explicit `mul_add` in local SSIM and f64 weighted accumulation. Lab conversion also uses FMA and an approximate cube-root routine. This conflicts with frozen binary64/non-FMA arithmetic. [D3, D5] |
| Pooling/output | Per-scale pooling uses mean absolute deviation, scale-dependent processing, and weighted combination; final result is `1 / max(score, EPSILON) - 1`, with zero best. It is **not** `(1-SSIM)/2`, nor the local mean SSIM simply relabeled. [D3] |

**Single-scale is not a compatibility switch.** Even `set_scales(&[1.0])`,
grayscale/custom input, disabled threads, and manual pooling of saved maps
would leave the wrong blur, edge treatment, f32 moments, and FMA. Inverting
the final DSSIM value recovers its internal pooled quantity, not this project's
SSIM. Existing `.999`/`.995` gates and exact score pins cannot be translated
into equivalent DSSIM limits by that inversion. [D3–D5, L1, L5]

**A specific determinism caveat:** published 3.5.1 `compare_inner` collects
scale results through `par_bridge()` and folds them in collection order.
Upstream commit **`0e44c9b7fe91a5265c8e463436c28512186fe9cd`**, dated
2026-09-24, fixes arbitrary map order and last-ULP score variation by sorting
scale results before accumulation, and adds a repeatability test. The inspected
3.5.1 source lacks that fix. Disabling default `threads` avoids this particular
parallel ordering path; the later commit's repeatability test is not proof of
cross-target bit identity or equivalence to our metric. [D3, D6]

**Dependency weight and licensing:** core 3.5.1 has three unconditional normal
direct dependencies (`imgref ^1.12.3`, `itertools ^0.15`, `rgb ^0.8.53`) plus
optional `rayon ^1.12.0`, enabled by default `threads`. `lodepng` is only a core
dev-dependency. Upstream documents `default-features = false` for single-threaded
WASM. The parent package additionally pulls in image loaders, PNG output,
channels, and CLI parsing; its manifest declares Rust 1.90, while the core
manifest declares edition 2024 without a `rust-version`. Both manifests declare
AGPL-3.0; upstream also offers a commercial license. This is a materially
different license choice from the current MIT/Apache comparator, not a
permissively licensed substitute. [D1–D2, D7, L7]

**Maintenance evidence:** core 3.5.1 was published 2026-08-29, following
3.4.0 on 2025-09-21. The unarchived upstream repository's observed tip is the
September 2026 determinism fix above. That is concrete recent maintenance,
including an issue directly relevant here; it does not certify correctness of
the published release. The README explicitly says scores are not directly
comparable to other tools and have changed between versions. [D6–D7]

**Decision:** if an additional perceptual signal is wanted, evaluate an exact
core version separately, with explicit scale/thread/color settings and license
selection. Do not use it as canonical SSIM or Pixelmatch.

## Integration consequences and limits of this audit

The current library has **zero normal dependencies** and five dev-dependencies.
The canonical profile binds source and the comparator's reachable lockfile
closure, rather than the entire workspace lockfile. Even a library already
present elsewhere in the workspace is not “free”: making it reachable from
this crate expands the comparator's bound closure. This is the boundary
established by ADR-0005 and asserted by `tests/profile.rs`. [L7–L8]

For a future contract-preserving replacement, all existing mask/diff-byte and
exact-SSIM-bit vectors remain acceptance criteria, alongside profile/closure
updates and target qualification. For an alternative metric, new score
semantics and thresholds are required; matching a few normal images is not
evidence that existing limits retain their meaning. [L1, L4–L7]

Dependency assessments above concern **declared direct dependencies and feature
edges**, not measured incremental package counts, build time, resident memory,
or binary size. Candidate dependency ranges were not resolved into this
workspace, candidates were not compiled, and tests were inspected rather than
run. No claim of cross-target compatibility or measured speedup is made.

## Primary sources

### Local contract and executable evidence

- **L1:** [Renderer spec §14.3](../renderer-spec-v1.md#143-canonical-raster-and-comparators).
- **L2:** [Canonical raster and projections](../../crates/excalidraw-render-compare/src/lib.rs), [Pixelmatch kernel](../../crates/excalidraw-render-compare/src/pixelmatch.rs).
- **L3:** [SSIM kernel](../../crates/excalidraw-render-compare/src/ssim.rs), [comparison orchestration](../../crates/excalidraw-render-compare/src/comparison.rs).
- **L4:** [Pinned upstream Pixelmatch tests](../../crates/excalidraw-render-compare/tests/pixelmatch_upstream.rs), [adversarial vectors](../../crates/excalidraw-render-compare/tests/pixelmatch_vectors.rs).
- **L5:** [Exact SSIM vectors](../../crates/excalidraw-render-compare/tests/ssim_vectors.rs).
- **L6:** [Comparison protocol tests](../../crates/excalidraw-render-compare/tests/comparison_protocol.rs), [canonical raster tests](../../crates/excalidraw-render-compare/tests/canonical_raster.rs).
- **L7:** [Canonical profile](../../tests/conformance/comparator-profiles/canonical-visual-v1.json), [profile validation](../../crates/excalidraw-render-compare/tests/profile.rs), [ADR-0005](../adr/0005-pin-the-comparator-to-its-own-dependency-closure.md).
- **L8:** [Comparator Cargo manifest](../../crates/excalidraw-render-compare/Cargo.toml).

### Pixelmatch

- **M1:** [Mapbox 7.2.0 pinned implementation](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js).
- **P1:** [pixelmatch 0.1.0 manifest](https://docs.rs/crate/pixelmatch/0.1.0/source/Cargo.toml.orig), [published VCS identity](https://docs.rs/crate/pixelmatch/0.1.0/source/.cargo_vcs_info.json).
- **P2:** [Complete published Rust implementation](https://docs.rs/crate/pixelmatch/0.1.0/source/src/lib.rs).
- **P3:** [Published documentation and release date](https://docs.rs/crate/pixelmatch/0.1.0).
- **P4:** [image 0.23.14 manifest and default codec features](https://docs.rs/crate/image/0.23.14/source/Cargo.toml.orig).
- **P5:** [Observed tip commit](https://github.com/dfrankland/pixelmatch-rs/commit/ab40c4ef76eac8d4f2ebf53a91876c7fc2c869b8), [repository metadata](https://api.github.com/repos/dfrankland/pixelmatch-rs).

### image-compare

- **I1:** [0.5.0 manifest](https://docs.rs/crate/image-compare/0.5.0/source/Cargo.toml.orig), [published VCS identity](https://docs.rs/crate/image-compare/0.5.0/source/.cargo_vcs_info.json).
- **I2:** [Published SSIM implementation and covariance tests](https://docs.rs/crate/image-compare/0.5.0/source/src/ssim.rs).
- **I3:** [Window subdivision, conversions, and alpha blending](https://github.com/ChrisRega/image-compare/blob/1bcd98373007f8eb6c89974fb0695f44268cffc3/src/utils.rs).
- **I4:** [Public API and algorithm dispatch](https://github.com/ChrisRega/image-compare/blob/1bcd98373007f8eb6c89974fb0695f44268cffc3/src/lib.rs).
- **I5:** [Published documentation, changelog, and release dates](https://docs.rs/crate/image-compare/0.5.0).
- **I6:** [Observed release/tip commit](https://github.com/ChrisRega/image-compare/commit/1bcd98373007f8eb6c89974fb0695f44268cffc3), [repository metadata](https://api.github.com/repos/ChrisRega/image-compare).

### DSSIM

- **D1:** [dssim-core 3.5.1 manifest](https://docs.rs/crate/dssim-core/3.5.1/source/Cargo.toml.orig), [published VCS identity](https://docs.rs/crate/dssim-core/3.5.1/source/.cargo_vcs_info.json).
- **D2:** [dssim 3.5.1 wrapper manifest](https://docs.rs/crate/dssim/3.5.1/source/Cargo.toml.orig).
- **D3:** [Core configuration, image creation, comparison, pooling, and tests at published commit](https://github.com/kornelski/dssim/blob/6e4579882f97907a75fd84133a4c4993a06d56ea/dssim-core/src/dssim.rs).
- **D4:** [Blur kernels and border implementation](https://github.com/kornelski/dssim/blob/6e4579882f97907a75fd84133a4c4993a06d56ea/dssim-core/src/blur.rs).
- **D5:** [Lab/grayscale conversion and arithmetic](https://github.com/kornelski/dssim/blob/6e4579882f97907a75fd84133a4c4993a06d56ea/dssim-core/src/tolab.rs).
- **D6:** [2026-09-24 deterministic ordering fix and regression test](https://github.com/kornelski/dssim/commit/0e44c9b7fe91a5265c8e463436c28512186fe9cd), [repository metadata](https://api.github.com/repos/kornelski/dssim).
- **D7:** [Published README, release history, metric interpretation, licensing, and WASM guidance](https://docs.rs/crate/dssim-core/3.5.1).
