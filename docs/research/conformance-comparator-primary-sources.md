# Conformance comparator primary sources

Research for [issue #21, "Define the conformance comparison and approval protocol"](https://github.com/sagikazarmark/excalidraw-render.orig/issues/21). Sources were checked on 2026-07-19. This note identifies algorithms and reproducibility decisions; it does not select approval limits or implement a comparator.

## Decision summary

1. An exact Rust port of released Mapbox Pixelmatch should name **Pixelmatch v7.2.0, commit `9faed09302aaecec130b4ce0e8505d5ed5221393`, `index.js` blob `be64dc92dae160d6d5ac21b9778b70553c023292`**. That release uses gamma-space YIQ distance. A major-version pin is insufficient: post-release `main` has changed to OKLab HyAB and a different threshold test.
2. "SSIM" is not an executable definition. The closest primary-source single-scale profile is the original paper plus Zhou Wang's `ssim_index.m`: 11x11 Gaussian, sigma 1.5, `K1=0.01`, `K2=0.03`, population-weighted local moments, valid-only windows, and the arithmetic mean of the map. Kernel coefficients, color handling, arithmetic, and thresholding still need to be frozen by the protocol.
3. Pixelmatch's `threshold` is a per-pixel color-distance tolerance, not an allowed mismatch percentage. SSIM's approval threshold is not supplied by the paper. The protocol must define separate score limits, inclusive/exclusive comparisons, and how the two gates compose.

## Pixelmatch v7.2.0

### Stable identity

- Upstream release: [`v7.2.0`](https://github.com/mapbox/pixelmatch/releases/tag/v7.2.0), released from [`9faed09302aaecec130b4ce0e8505d5ed5221393`](https://github.com/mapbox/pixelmatch/commit/9faed09302aaecec130b4ce0e8505d5ed5221393). Its [`package.json`](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/package.json#L1-L6) says `7.2.0`.
- Normative implementation candidate: [`index.js`](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js), Git blob `be64dc92dae160d6d5ac21b9778b70553c023292`, fetched-file SHA-256 `52c6026c4916a07548d4ce55090ec4ed918d353e15095406dabc3a8a5135d788`.
- Upstream executable examples: [`test/test.js`](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/test/test.js#L1-L78) and its same-commit PNG fixtures. The tests assert both mismatch counts and exact diff-image bytes.
- Version hazard: commit [`50872182c2f5d9bd488b0b7d5535f0b2404c42f7`](https://github.com/mapbox/pixelmatch/commit/50872182c2f5d9bd488b0b7d5535f0b2404c42f7) changed unreleased `main` from YIQ to OKLab HyAB. Current pinned-main source at [`c6fee35afac3c52576b2cb424bd1061ab6a4bd06`](https://github.com/mapbox/pixelmatch/blob/c6fee35afac3c52576b2cb424bd1061ab6a4bd06/index.js) also has `windowSize`. It is not the v7.2.0 algorithm.

### Inputs and parameters

The released function requires equal-size, tightly indexed 8-bit RGBA arrays of exactly `width * height * 4` bytes and returns an integer mismatch count ([source](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L22-L105)).

| Parameter | v7.2.0 default | Effect on mismatch count |
| --- | --- | --- |
| `threshold` | `0.1` | Yes; per-pixel YIQ tolerance |
| `includeAA` | falsy/omitted | Yes; when false, detected anti-alias pixels are excluded |
| `checkerboard` | `true` | Yes for pixels with either alpha below 255, including AA brightness comparisons |
| `alpha` | `0.1` | No; unchanged-pixel rendering in the optional diff only |
| `aaColor` | `[255,255,0]` | No; diff visualization only |
| `diffColor` | `[255,0,0]` | No; diff visualization only |
| `diffColorAlt` | `diffColor` | No; diff visualization only |
| `diffMask` | falsy/omitted | No; diff visualization only |

The source does not validate `threshold` to the documented `[0,1]` range. A conformance profile should reject out-of-range and non-finite values rather than inherit accidental JavaScript behavior.

### Color-delta math

For a pixel at byte offset `k = 4 * (y * width + x)`, channels are raw gamma-coded bytes; v7.2.0 performs no sRGB linearization. Let the two RGBA pixels be `(r1,g1,b1,a1)` and `(r2,g2,b2,a2)`, and start with `dr=r1-r2`, `dg=g1-g2`, `db=b1-b2`, `da=a1-a2` ([source](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L205-L252)).

If either alpha is below 255, it compares both pixels after compositing over the same position-dependent background. With `checkerboard=false`, `(rb,gb,bb)=(255,255,255)`. With `checkerboard=true`:

```text
rb = 48 + 159 * (k % 2)
gb = 48 + 159 * (ToInt32(k / 1.618033988749895) % 2)
bb = 48 + 159 * (ToInt32(k / 2.618033988749895) % 2)

dr = (r1*a1 - r2*a2 - rb*da) / 255
dg = (g1*a1 - g2*a2 - gb*da) / 255
db = (b1*a1 - b2*a2 - bb*da) / 255
```

`k` is a byte offset and is always divisible by four for ordinary input, so the released expression makes `rb` always 48. The `|0` expressions in source are ECMAScript `ToInt32`, not generic mathematical floor; the normative conversion is specified by [ECMA-262 (2026) `ToInt32`](https://tc39.es/ecma262/2026/multipage/abstract-operations.html#sec-toint32).

The YIQ-like transform and squared weighted delta are exactly:

```text
y =  0.29889531*dr + 0.58662247*dg + 0.11448223*db
i =  0.59597799*dr - 0.27417610*dg - 0.32180189*db
q =  0.21147017*dr - 0.52261711*dg + 0.31114694*db
D = 0.5053*y*y + 0.299*i*i + 0.1957*q*q
delta = (y > 0) ? -D : D
```

The sign only selects `diffColorAlt`: negative means image 2 is darker according to `y`. Counting uses `abs(delta)` ([source](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L238-L252)).

### Threshold interpretation

The source computes `maxDelta = 35215 * threshold * threshold` and marks a candidate only when `abs(delta) > maxDelta` ([source](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L55-L75)). Therefore:

- Equality with the boundary is a match.
- `threshold=0.1` means `maxDelta=352.15`; it does not mean "allow 10% mismatching pixels."
- The return value is the count after AA exclusions. Any whole-image ratio, absolute mismatch budget, crop weighting, or pass/fail inequality is a separate protocol rule.

### Anti-alias detection

AA detection runs only for a pixel already over the color threshold. Unless `includeAA` is truthy, the pixel is excluded if `antialiased` returns true for either image, with image roles swapped for the second call ([source](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L64-L94)). The exact detector is in [`antialiased`](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L108-L180) and [`hasManySiblings`](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L182-L204):

1. Inspect the clipped 3x3 neighborhood, excluding the center, in source loop order (`x` outer, `y` inner).
2. Compare each neighbor to the center with brightness only, using the same alpha compositing and checkerboard background indexed by the center offset. Brightness is `0.29889531*dr + 0.58662247*dg + 0.11448223*db` ([source](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/index.js#L254-L293)). Exact raw RGBA equality returns zero immediately; otherwise a numerically zero luma delta also counts as equal.
3. Initialize the equal-neighbor count to one when the center is on any image edge. If it becomes greater than two, return non-AA immediately.
4. Retain the strictly darkest and strictly brightest neighbors. Ties therefore depend on traversal order. If either side is absent, return non-AA.
5. Return AA only if the darkest or brightest selected neighbor has more than two equal siblings in **both** images. Sibling equality is exact packed-RGBA equality; boundary positions again begin with one synthetic equal sibling.

This detector is a categorical exclusion, not a reduced weight. Excluded pixels are not included in the returned count.

### Exact-port consequences

ECMAScript Number is the IEEE-754 binary64 set, with round-to-nearest ties-to-even for the normal arithmetic operations used here ([ECMA-262 (2026) Number type](https://tc39.es/ecma262/2026/multipage/ecmascript-data-types-and-values.html#sec-ecmascript-language-types-number-type)). A Rust oracle port must preserve source expression order, binary64 rounding, strict comparisons, `%`/`ToInt32` behavior, and avoid fused or reassociated operations at threshold boundaries. Raw four-byte equality is the semantic fast path; native-endian `Uint32Array` representation does not change equality, but the JS source also requires aligned byte offsets.

The same-commit upstream fixtures should be imported as conformance vectors. They cover exact diff bytes and counts including `143` at threshold `0.05`, `106` at the default threshold, `0` for identical images at threshold zero, and the v7.2 checkerboard option ([tests](https://github.com/mapbox/pixelmatch/blob/9faed09302aaecec130b4ce0e8505d5ed5221393/test/test.js#L7-L38)). Additional port vectors are still needed exactly on the threshold, at image edges, with alpha, with equal-luma/different-chroma neighbors, and with AA darkest/brightest ties.

## Windowed SSIM

### Original definition

The primary paper is Wang, Bovik, Sheikh, and Simoncelli, *Image Quality Assessment: From Error Visibility to Structural Similarity*, IEEE TIP 13(4), 600-612 (2004), [DOI `10.1109/TIP.2003.819861`](https://doi.org/10.1109/TIP.2003.819861), [author-hosted PDF](https://ece.uwaterloo.ca/~z70wang/publications/ssim.pdf). The fetched PDF SHA-256 was `c83f6f0f5357eb2714ce084d97bef4006358574d06ba546210e505149438b489`.

For each aligned local window, the paper sets `alpha=beta=gamma=1`, `C3=C2/2`, and defines (eq. 13):

```text
SSIM(x,y) = ((2*mu_x*mu_y + C1) * (2*sigma_xy + C2))
            / ((mu_x^2 + mu_y^2 + C1) * (sigma_x^2 + sigma_y^2 + C2))
C1 = (K1*L)^2
C2 = (K2*L)^2
```

`L` is the pixel-value dynamic range. The paper uses `L=255` for 8-bit grayscale, `K1=0.01`, and `K2=0.03`, giving `C1=6.5025` and `C2=58.5225`.

The local window in section III-C is an 11x11 circular-symmetric Gaussian weighting function with standard deviation 1.5 samples, normalized to unit sum. For weights `w_i` (eqs. 14-16):

```text
mu_x     = sum_i w_i*x_i
sigma_x2 = sum_i w_i*(x_i-mu_x)^2
sigma_xy = sum_i w_i*(x_i-mu_x)*(y_i-mu_y)
```

The paper's earlier unweighted exposition uses unbiased `1/(N-1)` variance and covariance (eqs. 4 and 11), but the later Gaussian local-window equations replace those estimators and contain no Bessel correction. The paper's MSSIM is the uniform arithmetic mean of all local SSIM values (eq. 17); it mentions nonuniform pooling as an application-specific alternative but says the paper uses uniform weighting.

### Author code and ambiguities

Zhou Wang's primary [SSIM page](https://ece.uwaterloo.ca/~z70wang/research/ssim/) publishes two Version 1.0 MATLAB files:

- [`ssim_index.m`](https://ece.uwaterloo.ca/~z70wang/research/ssim/ssim_index.m), copyright 2003, fetched SHA-256 `aeab78fa3763951edfd2175ea67deef6325c4d85ae86db95f3fc4bb07288f044`.
- [`ssim.m`](https://ece.uwaterloo.ca/~z70wang/research/ssim/ssim.m), copyright 2009, fetched SHA-256 `7e41e698b918e56a97dc3dc8e26b1ed4800cca33f48971f966086b691434cbe8`.

`ssim_index.m` resolves several paper ambiguities: it creates `fspecial('gaussian',11,1.5)`, renormalizes the window, uses `filter2(...,'valid')`, computes population moments as `E[x^2]-E[x]^2` and `E[xy]-E[x]E[y]`, returns an `(H-10)x(W-10)` map, and applies `mean2`. Thus it discards a five-pixel border rather than padding it.

The paper alone is still not byte-executable:

- It does not publish exact binary Gaussian coefficients, coefficient-generation rounding, convolution accumulation order, floating type, or result serialization.
- "Moves pixel-by-pixel over the entire image" does not itself define edge windows. The author code's valid convolution is evidence for dropping the border, but other primary implementations legitimately made different choices.
- The formula is for grayscale/luminance. The paper's experiments use the luminance component; the author page says its RGB database results first use MATLAB `rgb2gray`. No RGBA, alpha, sRGB transfer-function, or channel-pooling rule is defined.
- Scale is separate from the local formula. The paper's experiments average 2x2 then downsample by two. The author page recommends `F=max(1,round(N/256))`; the later `ssim.m` concretely uses `round(min(H,W)/256)`, symmetric filtering, and subsampling, while `ssim_index.m` does no automatic downsampling. A conformance protocol must explicitly choose one.
- The paper defines no conformance pass threshold and no treatment for NaN, undersized images, masks, excluded regions, or empty valid-window sets.

### Primary-source alternatives

These are useful as executable semantic choices, not interchangeable implementations of a single fully specified metric.

| Source identity | Window, moments, and edges | Other material behavior |
| --- | --- | --- |
| **scikit-image 0.26.0**, commit [`ee0a7a3ebd9ac8c2602f40e55bc015a3c8a81ae8`](https://github.com/scikit-image/scikit-image/commit/ee0a7a3ebd9ac8c2602f40e55bc015a3c8a81ae8), [`_structural_similarity.py`](https://github.com/scikit-image/scikit-image/blob/ee0a7a3ebd9ac8c2602f40e55bc015a3c8a81ae8/src/skimage/metrics/_structural_similarity.py#L15-L292) | Paper-like mode requires `gaussian_weights=True`, `sigma=1.5`, `use_sample_covariance=False`, and explicit `data_range`. It uses Gaussian `truncate=3.5` to obtain 11 taps, reflect filtering, then crops five pixels before the mean, making pooled centers valid-only. | The defaults are instead a 7x7 uniform window and sample covariance. Channels are evaluated independently and averaged; no color conversion occurs. Integer inputs normally compute in float64 and the final mean requests float64, but SciPy's filter remains part of the behavior. |
| **TensorFlow 2.21.0**, commit [`a481b10260dfdf833a1b16007eead49c1d7febf3`](https://github.com/tensorflow/tensorflow/commit/a481b10260dfdf833a1b16007eead49c1d7febf3), [`image_ops_impl.py`](https://github.com/tensorflow/tensorflow/blob/a481b10260dfdf833a1b16007eead49c1d7febf3/tensorflow/python/ops/image_ops_impl.py#L4249-L4503) | Defaults are 11x11, sigma 1.5, `K1=.01`, `K2=.03`, population-weighted covariance (`compensation=1.0`), and `VALID` depthwise convolution. Gaussian weights are generated by float32 softmax over sampled exponents. | Inputs and `max_val` are converted to float32. It averages valid-window scores spatially, then averages channels, with no color conversion. Exact results can depend on the selected TensorFlow convolution backend unless the protocol replaces it with a frozen scalar oracle. |
| **OpenCV QualitySSIM 5.0.0**, contrib commit [`755e50675d97db9b7d449d8bd6b09888646f6c6e`](https://github.com/opencv/opencv_contrib/commit/755e50675d97db9b7d449d8bd6b09888646f6c6e), paired core commit [`40738fb16ceddb5fb3fea747585f7ce6abb0605b`](https://github.com/opencv/opencv/commit/40738fb16ceddb5fb3fea747585f7ce6abb0605b) | [`qualityssim.cpp`](https://github.com/opencv/opencv_contrib/blob/755e50675d97db9b7d449d8bd6b09888646f6c6e/modules/quality/src/qualityssim.cpp#L14-L111) uses 11x11 Gaussian blur, sigma 1.5, population moments, and means the **entire same-size map**. Omitted `GaussianBlur` border mode is `BORDER_DEFAULT`, which equals `BORDER_REFLECT_101` in the paired core [`BorderTypes`](https://github.com/opencv/opencv/blob/40738fb16ceddb5fb3fea747585f7ce6abb0605b/modules/core/include/opencv2/core/base.hpp#L332-L344). | Constants are fixed at `6.5025` and `58.5225`, effectively fixing `L=255`. Channels remain separate and the API returns per-channel means. [`expand_mat`](https://github.com/opencv/opencv_contrib/blob/755e50675d97db9b7d449d8bd6b09888646f6c6e/modules/quality/include/opencv2/quality/quality_utils.hpp#L29-L52) promotes small integer inputs to float32 but existing float32/float64 to float64. OpenCL/optimized blur paths are not a portable bit-level oracle. |

For a paper-faithful deterministic definition, the least surprising basis is `ssim_index.m` semantics with no automatic downsampling: an explicitly serialized 11x11 kernel, population moments, valid centers only, and uniform mean. Merely calling a library function by name will not freeze those semantics.

## Reproducibility contract

The following details must be normative before results can be reproduced exactly.

### Common input

- Canonical decoded input bytes and dimensions, including row order, pixel origin, channel order, bit depth, endianness, row stride, and whether alpha is straight or premultiplied.
- Decoder and color-management behavior: ICC profiles, PNG `gAMA`/`sRGB`, transfer function, and conversion/rounding into the canonical bytes. Prefer hashing the decoded canonical buffer, not only the encoded PNG.
- Required equal-size behavior, crop rectangle coordinates, masks/exclusions, and operation order. Freeze whether excluded pixels are removed, neutralized, or omitted from denominators/windows.
- Alpha handling and background. Pixelmatch v7.2 defines its own compositing for comparison; SSIM does not. Transparent RGB bytes can differ while rendered pixels do not.
- Failure behavior for malformed dimensions, undersized images, empty regions, non-finite values, and integer overflow limits.

### Pixelmatch profile

- The full upstream identity above and every score-affecting option: `threshold`, `includeAA`, and `checkerboard`. Do not inherit defaults implicitly.
- Exact v7.2 expression order and ECMAScript binary64, `ToInt32`, remainder, strict `>` tests, AA traversal/tie behavior, and raw RGBA sibling equality.
- The returned quantity and denominator: absolute mismatch count or `count/(width*height)` after exclusions. If exclusions should change the denominator, that is a project extension and must be stated.
- Approval limit and inclusivity, for example `count <= B` versus `ratio < R`; per-case overrides; and whether Pixelmatch must pass independently or is advisory to SSIM.
- If the diff PNG is evidence, also freeze output typed-array clamping, all visualization options, PNG encoder/version/settings, metadata policy, and artifact hash. Those do not affect the count but do affect bytes.

### SSIM profile

- Single-scale versus MS-SSIM, and whether any prefilter/downsampling is performed. If downsampled, freeze factor calculation, kernel, border mode, phase/origin, and rounding.
- Scalar input signal: exact grayscale/luma coefficients and transfer domain, or exact per-channel rule and channel weights; alpha compositing; `L`/`data_range`; `K1`, `K2`, and constants.
- Exact window dimensions, center convention, and all 121 normalized Gaussian coefficients as binary32/binary64 bit patterns. Serializing coefficients avoids platform `exp` and normalization differences.
- Population versus sample covariance; algebraic form (`E[x^2]-mu^2` versus centered sums); handling of tiny negative variances from rounding; and zero-denominator/NaN policy.
- Edge handling and pooled centers: valid-only, reflected, replicated, or another explicit rule. Freeze mask interaction with windows and whether local weights are renormalized near masks.
- Arithmetic type, rounding mode, multiply/add order, FMA policy, convolution traversal, scan order for the final sum, and prohibition of parallel/reassociated reductions if exact f64 bits are required.
- Returned representation (for example, canonical little-endian IEEE-754 binary64 bits), approval threshold and inclusive comparison (normally higher SSIM is better), and a fail-closed rule for non-finite scores. Never threshold a rounded display string.

### Oracle evidence

- Publish small adversarial raw-buffer vectors and exact expected outputs for both metrics, plus larger upstream Pixelmatch fixtures. Include threshold equality, one-ULP neighbors, alpha, image borders, AA ties, constant SSIM windows, negative covariance, and masks/crops.
- Cross-check the Rust port against the pinned JavaScript Pixelmatch release and against a deliberately selected SSIM oracle, then freeze expected integer results and SSIM result bits in the protocol bundle.
- Version the metric profiles independently of approval limits. Any upstream revision, coefficient, preprocessing, arithmetic, or threshold change creates a new profile and requires explicit re-approval of references.

## Decision facts

- **Pixelmatch port target:** v7.2.0 at `9faed093...`, not `main`; it is YIQ, uses strict `> 35215*t^2`, and excludes AA by default.
- **SSIM statistical convention:** the paper's Gaussian local equations and author code use population-weighted moments; `N-1` belongs to the earlier unweighted exposition. scikit-image's default sample covariance is not paper-equivalent.
- **SSIM edges:** author MATLAB and TensorFlow use valid-only windows; scikit-image computes reflected borders but excludes them from the scalar mean; OpenCV includes reflected-edge windows. This choice can change approval near image boundaries.
- **SSIM color and scale:** the original definition is luminance/grayscale and does not define RGBA. Automatic downsampling is a separate usage profile, not an intrinsic part of the local SSIM formula.
- **Approval:** Pixelmatch color tolerance, allowed mismatch amount, SSIM floor, inequality boundaries, metric composition, exclusions, and non-finite handling are all independent decisions that must be recorded.
