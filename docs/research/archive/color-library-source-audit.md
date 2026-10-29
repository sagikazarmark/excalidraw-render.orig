# Color-library source audit: palette 0.7.6 and color 0.3.3

Source audit for [#185](https://github.com/sagikazarmark/excalidraw-render.orig/issues/185),
2026-09-18. Inspected the published Cargo registry sources at these **exact
versions**, downloaded with `cargo info`, plus the renderer at
`cbc39ae798dc92e3fb1a599fed5ad926c8d48af9`. Links below point to versioned primary
sources. Numerical error, artifact-byte effects, footprint and native/browser-WASM
equality belong to the separate evaluation; this note does not establish them.

**Finding:** Palette can preserve `f64` component precision, but its built-in
sRGB transfer extension and colorimetric constants differ from the current
kernel. Linebender's `color` implements CSS-oriented conversions with mandatory
`f32` arithmetic. Both select `std` over `libm` when both features are enabled.
Neither is established as a contract-preserving replacement by this audit.

The completed [measured evaluation](color-conversion-evaluation.md) retains the
kernel. Its resolved graph confirms both candidates' libm-only selection; it also
pins **palette_derive 0.7.6** explicitly after 0.7.7's generated references to
`lms`/`xyz::meta` failed to compile with palette 0.7.6. The snippets below are
source recipes; the compiled probe's exact routes lived in `tools/color-evaluation`
(retired 2026-09-30; in Git history at `20ab732`).

## Types, white points and conversion behavior

| Concern | `palette = 0.7.6` | Linebender `color = 0.3.3` |
| --- | --- | --- |
| Precision / types | `Srgb<f64>`, `LinSrgb<f64>`, `Xyz<D65, f64>`, `Xyz<D50, f64>`, `Lab<D50, f64>`, `Oklab<f64>`; `f32` is also supported and is the default. **Bare `Lab` defaults to D65**, not CSS Lab's D50. [P1–P3] | Typed spaces are markers: `OpaqueColor<Srgb>`, `OpaqueColor<LinearSrgb>`, `OpaqueColor<XyzD65>`, `OpaqueColor<XyzD50>`, `OpaqueColor<Lab>`, `OpaqueColor<Oklab>`. Components are always `[f32; 3]`; there is no `f64` component parameter. Runtime `ColorSpaceTag::convert` also takes/returns `[f32; 3]`. [C1–C3] |
| Units | RGB: unit channels; XYZ: white Y = 1; Lab L: 0–100; Oklab L: 0–1. These are nominal ranges, not adapter clipping instructions. [P1–P4] | Same units; Lab conversion matrices incorporate D50 normalization. [C2] |
| D65 / D50 | XYZ D65 = `[0.95047, 1, 1.08883]`; D50 = `[0.96422, 1, 0.82521]`. Bradford is the default of `AdaptFrom` / `AdaptInto`, with explicit `Method::Bradford` available. Adaptation builds a matrix from those white points and rounded Bradford/inverse coefficients. `FromColorUnclamped` alone does not change white points. [P4–P5] | `Chromaticity::D65 = (0.3127, 0.3290)`, D50 = `(0.3457, 0.3585)`; XYZ white components are respectively `[3127/3290, 1, 3583/3290]` and `[3457/3585, 1, 986/1195]`, evaluated in `f32`. XyzD50 and Lab use precombined Bradford/linear-sRGB matrices. [C2, C4] |
| Negative sRGB | Decode tests **`x <= 0.04045`**, encode **`x <= 0.0031308`**: every negative value takes the linear branch. Neither tests `abs(x)` nor reflects the nonlinear branch. Thus `FromColorUnclamped` preserves range but does **not** supply the CSS sign-preserving transfer extension. [P6] | Both directions test `abs(x)` and use `copysign(x)` on the nonlinear result. Breakpoints are 0.04045 and 0.0031308. This matches the current kernel's sign-reflected structure, not necessarily its rounding. [C2, R1] |
| Negative Lab / Oklab | Lab uses a linear branch below its positive epsilon; Oklab takes signed cube roots of LMS values. [P2–P3] | Same structural distinction; no fractional power of a negative LMS component in Oklab. [C2] |
| Clipping / route | Use `FromColorUnclamped` / `IntoColorUnclamped`. `FromColor` explicitly calls `.clamp()`. Direct linear-sRGB↔Oklab and XYZ↔Oklab routes exist and use different matrices. [P3, P7] | Ordinary `convert` preserves extended values and generally goes through linear sRGB; same-space conversion is a no-op. `clip`, alpha/premultiplication, packing and dynamic missing-component handling are separate APIs. [C1–C3] |

**Constants are an independent compatibility question.** Palette's built-in
sRGB→XYZ first row is `[0.4124564, 0.3575761, 0.1804375]`; the renderer's is
`[0.4123908, 0.35758434, 0.18048079]`. Linebender uses rational CSS-derived XYZ
coefficients evaluated in `f32`, and precombines some matrices. The renderer
itself combines its rounded RGB/Bradford matrices with Lab D50
`[0.96422, 1, 0.82521]`; neither library's complete recipe should be assumed to
reproduce that combination. Palette also uses a fused multiply-add in nonlinear
sRGB decoding, while the renderer divides `(abs(x) + 0.055)` by 1.055. [P6, C2, R1]

One exact-version caveat: `color`'s `Lab` implementation embeds D50 adaptation in
its ordinary conversion matrices but does **not** override the trait's default
`WHITE_POINT = D65`. Use ordinary `convert` for this experiment; do not infer Lab
white-point metadata or substitute `convert_absolute`, whose implementation uses
`WHITE_POINT`. This is a source-observed inconsistency, not a measured failure of
ordinary conversion. [C2]

## Math selection, unification and fast paths

The proposed dependency declarations should be exact pins:

```toml
palette = { version = "=0.7.6", default-features = false, features = ["libm"] }
color = { version = "=0.3.3", default-features = false, features = ["libm"] }
libm = "=0.2.16"
csscolorparser = "=0.8.3" # defaults enabled
cssparser = "=0.37.0"
cssparser-color = "=0.5.0"
```

- **Palette:** `num/libm.rs` is included only under
  `all(not(feature = "std"), feature = "libm")`. Scalar `f64` power/cube root/FMA
  use `libm::pow`, `cbrt`, `fma`; `f32` uses `powf`, `cbrtf`, `fmaf`. Its libm-mode
  integer powers use its multiplication-based `Powu` helper; std-mode `Powi`
  forwards to the primitive intrinsic. Enabling `std` therefore changes more
  than the power/cube-root provider. Palette depends directly on libm, **not on
  num-traits**. [P8–P9]
- **Color:** `FloatFuncs` supplies the libm shims only without `std`; `powf` and
  `cbrt` route to `libm::powf` and `libm::cbrtf`. The crate explicitly documents
  that `std` overrides `libm`. Its ordinary dependency build can use these shims
  even in a std-based application. [C1, C5]
- **Unification answer:** Features are package-specific and additive. The listed
  parser crates have no dependency edge to Palette or Linebender color, so their
  defaults do **not** switch these two adapters to std. Neither does the current
  renderer dependency closure, which contains neither candidate. Another edge
  enabling **that candidate's** `std`/defaults would override libm; resolver 2
  does not isolate ordinary dependencies from each other. [P9, C6, D1–D3, F1]
- **The whole experiment is nevertheless not libm-only.** Default
  `csscolorparser 0.8.3` enables its `std`; its source imports `num_traits::Float`
  only without `std`, and uses primitive `.powf()` / `.cbrt()` with defaults.
  Its `num-traits/libm` dependency does not redirect those inherent calls.
  Its sRGB inverse takes `12.92*x` for every negative linear component, unlike
  the renderer's owned sign-reflected f64 helper; this matters for extended-range
  Oklab comparison as well as for portability. [D4]
  Adding the renderer also unifies `num-traits/std` through image/moxcms:
  num-traits itself gives std precedence over libm. This latter issue would
  matter even if csscolorparser's own std were disabled. [D1, D4–D5]
- **libm features:** Palette disables libm defaults, but `color/libm` and the
  proposed direct `libm` dependency enable default `arch`. The existing renderer
  also enables it. For the requested x86_64/WASM targets, inspected architecture
  dispatch concerns sqrt/FMA/rounding-class operations, not an alternative fast
  power/cube-root feature. `arch` is not a std-provider switch. This is consistent
  with [ADR-0007](../../adr/0007-route-transcendental-math-through-libm.md), but still
  requires browser equality evidence. [M1]
- **Fast approximations:** Palette's scalar float→same-float sRGB path uses
  power, not `fast-srgb8`. Its specialized float-linear→sRGB-`u8` path uses
  `fast_srgb8::f32_to_srgb8`, including an `f64`→`f32` narrowing first; `u8` decode
  uses lookup tables. `fast-srgb8` documents approximation and a separate SSE2
  four-channel entry point. Palette's optional `approx` feature is approximate
  comparison support, not a fast-math switch. Linebender's scalar conversions
  use power/cube root, but `to_rgba8` uses `(x * 255 + 0.5) as u8`; its docs name
  `0.0019607842` as an input rounded to 1 rather than 0. **Keep final quantization
  outside both adapters**; enabling libm does not remove these packing paths.
  [P6, P9–P10, C2, C7]

Resolved evidence obtained here: `cargo tree --locked -p excalidraw-render -e
features -i libm` shows `libm 0.2.16` with `default,arch`; the corresponding
`-i num-traits` shows `num-traits 0.2.19` with `default,std,libm`. A second
num-traits tree with `--target wasm32-unknown-unknown` also shows std and libm.
These are observations of the **existing renderer lockfile**; the candidate
selection above is derived from published manifests/source. Capture the final
evaluation graph (and transitive versions) after its manifest/lockfile exists:

Historical transcript; the probe manifest is only in Git history (`20ab732`).

```sh
# Repeat for x86_64-unknown-linux-gnu and wasm32-unknown-unknown.
cargo tree --locked --manifest-path tools/color-evaluation/Cargo.toml --target wasm32-unknown-unknown -e features -i palette@0.7.6
cargo tree --locked --manifest-path tools/color-evaluation/Cargo.toml --target wasm32-unknown-unknown -e features -i color@0.3.3
cargo tree --locked --manifest-path tools/color-evaluation/Cargo.toml --target wasm32-unknown-unknown -e features -i libm@0.2.16
cargo tree --locked --manifest-path tools/color-evaluation/Cargo.toml --target wasm32-unknown-unknown -e features -i num-traits
```

## Adapter API recipes

Source-checked recipes for `convert([f64; 3])`; compilation and numerical tests
belong to the evaluation tool. Keep alpha as a separate project-owned `f64` and
preserve existing CSS admission, missing-component, gamut and color-mix policy.
Lab uses L = 0–100, Oklab L = 0–1, and XYZ uses Y = 1. [P1–P7, C1–C3]

Palette: this deliberately simple all-pairs dispatcher uses linear sRGB as its
hub. It exposes the unmodified library behavior, including negative transfers.
The hub adds matrix round trips for e.g. XYZ D50↔Lab D50; for a direct-edge
comparison call `Lab::<D50, f64>::from_color_unclamped(xyz50)` and the inverse
directly. Record the route being measured; changing it changes arithmetic.

```rust
use palette::{Lab, LinSrgb, Oklab, Srgb, Xyz};
use palette::chromatic_adaptation::{AdaptFrom, Method};
use palette::convert::FromColorUnclamped;
use palette::white_point::{D50, D65};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Space { Srgb, LinearSrgb, XyzD65, XyzD50, LabD50, Oklab }

fn from_d50(x: Xyz<D50, f64>) -> LinSrgb<f64> {
    let d65 = Xyz::<D65, f64>::adapt_from_using(x, Method::Bradford);
    LinSrgb::from_color_unclamped(d65)
}

fn convert_palette(v: [f64; 3], from: Space, to: Space) -> [f64; 3] {
    if from == to { return v; }
    let [a, b, c] = v;
    let linear: LinSrgb<f64> = match from {
        Space::Srgb => Srgb::new(a, b, c).into_linear(),
        Space::LinearSrgb => LinSrgb::new(a, b, c),
        Space::XyzD65 => LinSrgb::from_color_unclamped(Xyz::<D65, f64>::new(a, b, c)),
        Space::XyzD50 => from_d50(Xyz::new(a, b, c)),
        Space::LabD50 => from_d50(Xyz::from_color_unclamped(Lab::<D50, f64>::new(a, b, c))),
        Space::Oklab => LinSrgb::from_color_unclamped(Oklab::new(a, b, c)),
    };
    match to {
        Space::Srgb => {
            let x: Srgb<f64> = Srgb::from_linear(linear);
            [x.red, x.green, x.blue]
        }
        Space::LinearSrgb => [linear.red, linear.green, linear.blue],
        Space::Oklab => {
            let x = Oklab::from_color_unclamped(linear);
            [x.l, x.a, x.b]
        }
        Space::XyzD65 => {
            let x: Xyz<D65, f64> = Xyz::from_color_unclamped(linear);
            [x.x, x.y, x.z]
        }
        Space::XyzD50 | Space::LabD50 => {
            let d65: Xyz<D65, f64> = Xyz::from_color_unclamped(linear);
            let x: Xyz<D50, f64> = Xyz::adapt_from_using(d65, Method::Bradford);
            if to == Space::XyzD50 { [x.x, x.y, x.z] } else {
                let lab = Lab::from_color_unclamped(x);
                [lab.l, lab.a, lab.b]
            }
        }
    }
}
```

Linebender: pass tags `Srgb`, `LinearSrgb`, `XyzD65`, `XyzD50`, `Lab`, `Oklab`
to the runtime version; `Lab` means D50 in this ordinary conversion API. The
casts are part of the adapter's measured error, not a way to obtain f64 math.
The explicit identity branch retains the caller's f64 values.

```rust
fn convert_color(v: [f64; 3], from: color::ColorSpaceTag,
                 to: color::ColorSpaceTag) -> [f64; 3] {
    if from == to { return v; }
    from.convert(to, v.map(|x| x as f32)).map(f64::from)
}

// Static equivalent; S/T are e.g. color::Srgb / color::Lab.
fn convert_color_typed<S: color::ColorSpace, T: color::ColorSpace>(
    v: [f64; 3],
) -> [f64; 3] {
    if core::any::TypeId::of::<S>() == core::any::TypeId::of::<T>() { return v; }
    S::convert::<T>(v.map(|x| x as f32)).map(f64::from)
}
// Also: color::OpaqueColor::<color::Srgb>::new([r, g, b])
//           .convert::<color::Lab>().components
```

## Licenses, dependencies and evaluation blockers

- **Palette:** `MIT OR Apache-2.0`, declared MSRV 1.60. Selected direct
  dependencies are `fast-srgb8 ^1.0.0`, `palette_derive ^0.7.6`, and
  `libm ^0.2.1` (satisfied by the direct `=0.2.16` pin). `fast-srgb8 1.0.0` is
  dependency-free and `MIT OR Apache-2.0 OR CC0-1.0`. `palette_derive 0.7.6` is
  `MIT OR Apache-2.0`, a host proc-macro using by_address, proc-macro2, quote and
  syn; its optional find-crate is off here. Pinning Palette does **not** pin
  palette_derive to 0.7.6: preserve the actual resolved version in the tool lock.
  Defaults would additionally enable named lookup/phf, std/alloc and approx;
  optional wide/SIMD, rand, serde and bytemuck are off in this proposal. [P9–P11]
- **Color:** `Apache-2.0 OR MIT`, MSRV 1.82. With only its libm feature, the sole
  direct dependency is `libm ^0.2.16`; serde and bytemuck are optional/off.
  `libm 0.2.16` is MIT and has no normal dependencies. [C6, M1]
- **Existing parser costs:** csscolorparser is `MIT OR Apache-2.0`, with
  num-traits and default named-color phf/uncased; cssparser and cssparser-color
  are MPL-2.0. cssparser's default fast-match features select parsing macros/phf,
  not color-conversion math. These are shared parser dependencies, not new
  candidate costs. Binary/WASM size and compile-time deltas remain unmeasured.
  [D1–D3]

The main evaluation needs to resolve four concrete blockers: (1) Palette's
negative transfer extension, (2) white-point/matrix constants and operation
order for both candidates, (3) Linebender's f64→f32 boundary, and (4) resolved
math plus actual native/browser-WASM artifact byte stability. A Palette
compatibility wrapper could reflect its positive transfer function, but that
would be retained project glue and a separately labeled experiment. Even a
successful generic conversion adapter leaves alpha, final half-byte rounding,
CSS admission, system colors, gamut/missing-component behavior and color-mix
policy owned by this project. Excalidraw's CSS hue-rotate matrix and raster ICC
normalization are outside this conversion experiment. [#185, R1, ADR-0007]

## Primary-source references

- **P1:** [Palette RGB types/API](https://docs.rs/crate/palette/0.7.6/source/src/rgb/rgb.rs), [RGB aliases](https://docs.rs/crate/palette/0.7.6/source/src/rgb.rs).
- **P2:** [Palette Lab type and piecewise XYZ conversion](https://docs.rs/crate/palette/0.7.6/source/src/lab.rs).
- **P3:** [Palette Oklab type, signed roots and direct RGB route](https://docs.rs/crate/palette/0.7.6/source/src/oklab.rs).
- **P4:** [Palette white-point constants](https://docs.rs/crate/palette/0.7.6/source/src/white_point.rs), [XYZ conversions](https://docs.rs/crate/palette/0.7.6/source/src/xyz.rs).
- **P5:** [Palette Bradford matrices and adaptation traits](https://docs.rs/crate/palette/0.7.6/source/src/chromatic_adaptation.rs).
- **P6:** [Palette sRGB matrices, transfer functions and specialized u8 paths](https://docs.rs/crate/palette/0.7.6/source/src/encoding/srgb.rs).
- **P7:** [Palette clamping conversion trait](https://docs.rs/crate/palette/0.7.6/source/src/convert/from_into_color.rs).
- **P8:** [Palette numeric feature dispatch](https://docs.rs/crate/palette/0.7.6/source/src/num.rs), [libm implementations](https://docs.rs/crate/palette/0.7.6/source/src/num/libm.rs).
- **P9:** [Palette manifest: features, dependencies, license/MSRV](https://docs.rs/crate/palette/0.7.6/source/Cargo.toml).
- **P10:** [fast-srgb8 1.0.0 implementation/approximation](https://docs.rs/crate/fast-srgb8/1.0.0/source/src/lib.rs), [manifest/license](https://docs.rs/crate/fast-srgb8/1.0.0/source/Cargo.toml).
- **P11:** [palette_derive 0.7.6 manifest](https://docs.rs/crate/palette_derive/0.7.6/source/Cargo.toml).
- **C1:** [Color scope, f32 representation and feature precedence](https://docs.rs/crate/color/0.3.3/source/src/lib.rs), [typed color containers](https://docs.rs/crate/color/0.3.3/source/src/color.rs).
- **C2:** [ColorSpace trait and all audited conversion implementations](https://docs.rs/crate/color/0.3.3/source/src/colorspace.rs): trait lines 93–206; sRGB 374–413; XYZ 1018–1163; Oklab 1194–1248; Lab 1349–1432.
- **C3:** [ColorSpaceTag conversion dispatch](https://docs.rs/crate/color/0.3.3/source/src/tag.rs).
- **C4:** [Color chromaticities and Bradford matrices](https://docs.rs/crate/color/0.3.3/source/src/chromaticity.rs).
- **C5:** [Color std/libm shims](https://docs.rs/crate/color/0.3.3/source/src/floatfuncs.rs).
- **C6:** [Color manifest](https://docs.rs/crate/color/0.3.3/source/Cargo.toml).
- **C7:** [Color packing approximation](https://docs.rs/crate/color/0.3.3/source/src/color.rs), `to_rgba8` and `fast_round_to_u8`, lines 582–600 and 727–739.
- **D1:** [csscolorparser 0.8.3 manifest](https://docs.rs/crate/csscolorparser/0.8.3/source/Cargo.toml).
- **D2:** [cssparser 0.37.0 manifest](https://docs.rs/crate/cssparser/0.37.0/source/Cargo.toml).
- **D3:** [cssparser-color 0.5.0 manifest](https://docs.rs/crate/cssparser-color/0.5.0/source/Cargo.toml).
- **D4:** [csscolorparser float-method selection](https://docs.rs/crate/csscolorparser/0.8.3/source/src/color.rs), [Oklab roots](https://docs.rs/crate/csscolorparser/0.8.3/source/src/utils.rs).
- **D5:** [num-traits 0.2.19 float dispatch](https://docs.rs/crate/num-traits/0.2.19/source/src/float.rs), std implementation and `all(not(feature = "std"), feature = "libm")` implementations.
- **M1:** [libm 0.2.16 manifest](https://docs.rs/crate/libm/0.2.16/source/Cargo.toml), [architecture dispatch](https://docs.rs/crate/libm/0.2.16/source/src/math/arch/mod.rs).
- **F1:** [Cargo feature unification and resolver 2](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification).
- **R1:** [Renderer kernel at the audited commit](https://github.com/sagikazarmark/excalidraw-render.orig/blob/cbc39ae798dc92e3fb1a599fed5ad926c8d48af9/crates/excalidraw-render/src/color.rs#L834-L1008).
