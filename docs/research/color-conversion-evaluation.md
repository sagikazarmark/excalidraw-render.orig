# CSS color-conversion kernel: retain the current implementation

**Issue [#185](https://github.com/sagikazarmark/excalidraw-render.orig/issues/185),
2026-09-18.** Renderer source:
`cbc39ae798dc92e3fb1a599fed5ad926c8d48af9`. Candidates:
**palette 0.7.6** and **Linebender color 0.3.3**, both with only `libm` enabled;
resolved math implementation **libm 0.2.16**.

**Recommendation: retain the current generic kernel.** Neither narrow adapter
preserves the current CanonicalColor boundary. Palette has substantial extended
range/gamut differences; Linebender's mandatory f32 conversions produce byte
changes at rounding boundaries even with project-owned f64 mixing and quantization.
Both show representative native/browser-WASM equality, but portability does not
make either a compatibility-preserving replacement. This completes the bounded
evaluation; it does not authorize conversion, grammar or tolerance changes.

## Evidence and adapter seam

- [Reproduction and scope](../../tools/color-evaluation/README.md).
- [Generated numeric, canonical, artifact and browser matrix](../../tools/color-evaluation/matrix.md).
- [Versioned source audit](color-library-source-audit.md).
- `tools/color-evaluation/expected-{native,wasm,reference}.json`: exact observations.

The probe builds the actual production `color.rs`, plus two copies with narrowly
replaced conversion functions. It does not transcribe the current math. Palette
uses unclamped typed f64 conversions, explicit D50 Lab and Bradford adaptation;
Linebender uses ordinary `ColorSpaceTag::convert`, narrowing at its f32 boundary.
Neither library receives alpha or chooses final channel quantization.

The replacement covers sRGB encode/decode, sRGB↔XYZ D65, D50↔D65, Lab D50↔sRGB,
and rectangular Oklab mixing conversions. Palette adapts XYZ directly rather than
adding an RGB round trip. Its other routes use linear sRGB; Linebender uses its
ordinary dynamic conversion routes. The paired sRGB/XYZ matrices are delegated
together. Wide-RGB source matrices and A98/ProPhoto/Rec.2020 curves remain owned:
their shared sRGB/XYZ/adaptation edges exercise the candidate, but this is not a
complete replacement of those spaces with library-defined colorimetry.

The following remain explicit project policy in the compiled comparison:

- Existing CSS admission, system colors, parser f32 number tokens and missing
  components (including the current default-to-zero behavior).
- Canonicalized stops before mixing, f64 weights, premultiplied interpolation,
  sub-100% alpha multiplier, zero-alpha handling, nesting limits and hue methods.
- Existing Lab and Oklab mix f32 boundary into `canonical_css_color`, gamut mapping
  and final clamp/half-away-from-zero RGBA8 rounding. Direct `color()` still clips,
  whereas Lab goes through the existing gamut mapper.
- Direct Oklab/Oklch parsing, polar Oklch/HSL/HWB conversion and the gamut mapper's
  csscolorparser operations. Evaluating a replacement for those is further work.
- The CSS **hue-rotate filter matrix**, image filter ordering, and format-specific
  raster-image behavior. No perceptual hue-rotation convenience API is used.
- Raster ICC normalization through moxcms, outside this experiment.

## Measured differences

| Corpus | Samples | Palette changed canonical RGBA | Linebender changed canonical RGBA |
| --- | ---: | ---: | ---: |
| Fixed CSS cases | 133 | 16 | 1 |
| Half-byte-targeted CSS/mix cases | 1,530 | 0 | 460 |
| Admitted Lab sweep | 4,096 | 2,836 | 61 |

The Lab sweep's maximum absolute **R/G/B/A byte deltas** are **244/99/186/0** for
Palette and **1/1/1/0** for Linebender. No sampled alpha changes. The broad Palette
differences are not explained by precision alone: its negative sRGB transfer
extension changes what the retained gamut mapper sees. All exact fixed-case RGBA
results and raw first-difference bit patterns are in the observations.

Concrete witnesses:

- `color(display-p3 -0.5 0.2 1.2)`: current/Linebender **[0,59,255,255]**;
  Palette **[0,53,255,255]**, delta **[0,-6,0,0]**. Both pinned browser surfaces
  give [0,59,255,255].
- `lab(50 125 -125 / 1)`: current/Linebender **[188,113,255,255]**;
  Palette **[183,88,255,255]**, delta **[-5,-25,0,0]**. At alpha 0, 1/255 and
  128/255 the RGB differences persist; the alpha byte is unchanged.
- `color-mix(in a98-rgb, red, blue)`: current **[129,0,129,255]**;
  both candidates **[129,0,128,255]**, delta **[0,0,-1,0]**.
- `color-mix(in srgb-linear, black 99.98482365082256%, white)`: current/Palette
  **[0,0,0,255]**, Linebender **[1,1,1,255]**, delta **[1,1,1,0]**. The existing
  parser retains this weight as f64; the adapter alone introduces f32 conversion.
- Existing double-precision sRGB mix witnesses at **49.9999999%** and
  **50.0000001%** remain **#7f0080** and **#80007f** for both adapters: preserving
  mix policy avoids additional differences, but cannot undo conversion precision.

There are also **36,864 raw numeric comparisons**, with per-channel maxima and
bitwise stream digests. Almost all RGB→XYZ/Lab samples change for both libraries;
small raw deltas often disappear at the canonical boundary. The raw Oklab inverse
stress domain reaches far outside the usual gamut and Linebender differs by over
1,000 in an unbounded encoded channel there. This is not an RGBA8 error estimate:
csscolorparser's existing inverse extends **all negative linear channels linearly**,
like Palette, while Linebender applies a sign-reflected nonlinear sRGB transfer.
Conversely, the project's own f64 encode/decode helpers already use sign reflection.
The current kernel therefore has two different extended-range behaviors. A library
cannot silently unify them while claiming to preserve this contract.

## Artifact bytes and format-specific parity

Twenty selected swatches cover every differing fixed CSS case, the first Lab
differences, the first half-byte difference, and an unchanged Oklab mix control.
Each is projected through the public renderer in light/dark mode and SVG/PNG/PDF.
The current canonical projection is asserted byte-identical to rendering the
original CSS directly, on both targets. Candidate projections use exact
`#rrggbbaa` values at the same boundary; this is not an end-to-end patched renderer
or a full-corpus migration qualification.

| Format | Swatches × modes | Palette changed artifacts | Linebender changed artifacts |
| --- | ---: | ---: | ---: |
| SVG | 40 | 34 | 6 |
| PNG | 40 | 20 | 4 |
| PDF | 40 | 34 | 6 |

The smaller PNG count is meaningful: zero-alpha/low-alpha and dark-filter rounding
can hide canonical differences in pixels while SVG/PDF still serialize different
color values. PNG observations include decoded-pixel digests and center pixels;
all artifact length/digest observations agree across native/browser WASM.

All **17 applicable existing pinned-Chromium unit RGB witnesses** remain unchanged
for both candidates. Supplemental Chrome **149.0.7827.55** measurements draw 132
unique CSS strings directly into an sRGB canvas and independently through an SVG
image; canvas and SVG pixels agree on all of these. They are browser observations,
not newly approved reference-renderer exports. No SSIM or wider tolerance is used.

The browser measurements also expose existing differences. For example, its
opaque out-of-gamut Lab witness above is **[214,0,255,255]**, unlike all three
canonical results; its A98 mix is **[128,0,128,255]**, unlike all three; and its
half-byte witness matches Linebender rather than the current kernel. A candidate
moving one example toward the browser does not establish overall format-specific
parity, nor authorize changing artifact bytes. At low alpha, canvas
premultiplication/unpremultiplication adds another observation boundary; raw
canonical channels must not be mistaken for recovered raster channels. These
findings merit a separately scoped diagnosis before any contract decision.

## Portable math and resolved dependencies

The actual tool graph includes the production renderer dependency closure. Both
native and WASM `cargo tree -e features` show **palette/libm only** and
**color/libm only**. Neither has std unified on. libm 0.2.16 has `default,arch`;
the retained csscolorparser has std, and num-traits has std and libm. Candidate
conversion math therefore uses libm, while retained csscolorparser math can still
select platform float methods. The [source audit](color-library-source-audit.md)
details dispatch, FMA, white-point constants and optional fast packing paths.
Neither adapter calls a library's byte-packing approximation.

The empirical result matters independently of those features:

- Both candidates have **identical input/output binary64 stream digests on both
  targets for every raw operation**, including f32 results widened by Linebender.
- Current raw Oklab forward/inverse streams differ across targets; its owned
  libm-based raw operations agree. All sampled canonical results and artifacts
  nevertheless agree. This is residual dependency-math exposure of the kind
  ADR-0007 describes, not proof of a reachable artifact-byte defect or a fix for #178.
- Test environment: Linux x86-64 and browser wasm32, Rust **1.97.1**, optimization
  level **2**, wasm-bindgen-test-runner **0.2.126**, Chrome/ChromeDriver
  **149.0.7827.55**. No SIMD override or tolerance widening.

An exact-version packaging problem was observed: palette 0.7.6's compatible
dependency range resolves palette_derive **0.7.7**, whose output references
`crate::lms` / `xyz::meta` absent in palette 0.7.6. The experiment explicitly pins
**palette_derive 0.7.6** as well; there is no registry patch. A fresh unlocked
palette-0.7.6 resolution is not equivalent to this evaluated closure.

## Ownership and footprint

Literal source accounting is reproduced in the matrix. The seven replaced helper
functions occupy **91 non-test source lines**, including signatures. The paired
XYZ inverse matrix and rectangular Oklab arms bring the eligible substitution to
roughly **110–120 lines**. This is the gross implementation displaced, not net
code saved: typed-space dispatch, white-point selection, f64/f32 conversion and
canonical wrappers still need ownership. The shared two-candidate adapter is
**85 lines**; production would retain one dispatch implementation, plus the
wrappers that feed the existing gamut and quantization boundary.

Most of the **1,083-line non-test color module** remains: wide-RGB matrices/curves,
CSS admission, system-color policy, mixing, polar conversion, gamut search, alpha,
quantization, theme and image filters. csscolorparser cannot be removed by this
seam. A Palette sign-reflection compatibility wrapper could reduce its most
obvious discrepancy, but would retain more glue and would not reconcile its
constants with the current kernel. Matching every constant and operation ordering
would substantially erode the benefit of delegating these small functions.

Dependency impact, relative to the unchanged production lock:

- Palette adds **palette 0.7.6, palette_derive 0.7.6, fast-srgb8 1.0.0 and
  by_address 1.2.1**. Existing proc-macro dependencies are shared.
- Linebender adds **color 0.3.3**; its only selected runtime dependency, libm, is
  already present. Both candidates are MIT/Apache-2.0; see the audit for dependency
  license details.
- Shared registry versions/checksums remain identical to production; the tool's
  separate lock and `compare.py` check this. Production dependencies add **zero**
  packages for the retain outcome.
- **Binary-size, canonical WASM footprint, Brotli size, peak memory, conversion
  throughput and build-time deltas are unmeasured.** The debug experiment module
  contains both adapters, instrumentation and the full renderer, so its byte size
  would not be a meaningful adoption delta. A full footprint run is unnecessary
  to reject the demonstrated compatibility changes.

## Scoped follow-up plan

1. Keep production conversion and CSS admission as they are. Preserve this probe
   as a reproducible no-go result; no contract/pin refresh follows from #185.
2. Separately diagnose the sampled current-kernel/browser differences, especially
   out-of-gamut Lab and the two negative-transfer behaviors. Use admitted scene
   documents, the applicable official reference profile, both official SVG/canvas
   artifacts and existing parity budgets. Decide whether each is a current defect,
   intended format-specific behavior, or needs a compatibility-revision decision.
3. If renewed adoption is justified, prototype **one** further seam first:
   Palette with explicit sign-reflected transfers and project-qualified white
   points/matrices, or Linebender only in an already-f32 Oklab path. Preserve the
   present adapter as a control and count compatibility glue after that experiment.
   Do not replace the mix parser or use library missing-component/interpolation
   semantics as part of the same change.
4. Before production adoption: exact native/browser-WASM replay under the resolved
   production feature graph; complete artifact/report and semantic resource
   inventory; official format-specific parity gates; approved handling of any
   byte-affecting contract change; then canonical footprint and throughput
   measurement. A libm feature or these bounded probes alone are insufficient.

The retain decision requires no ADR change. A later change to existing color
semantics needs its own evidence and decision rather than a wider tolerance here.

## Validation

The complete isolated native suite passed (five tests, including exact observation
replay); browser-WASM observation replay and all 132 supplemental browser-reference
replays passed. The lock/matrix/cross-target checker, Clippy with warnings denied,
and Rust formatting check passed. Separate standards and issue-coverage reviews
reported no actionable findings. These are evaluation checks, not a production
full-corpus release qualification.
