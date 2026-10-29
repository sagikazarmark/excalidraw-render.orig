---
status: accepted
---

# Route transcendental math through the pure-Rust `libm` crate

Artifact byte stability — identical bytes for identical scene documents, export
targets and options on `x86_64-unknown-linux-gnu` and `wasm32-unknown-unknown`
— is the product promise, and the renderer already pays for it wherever a
dependency lets the platform choose a floating-point result: tiny-skia builds
without `simd`, brotli with `portable-float` (ADR-0003), and the PDF backend
freezes the sRGB transfer curve as a 256-entry table (`render/pdf.rs`) rather
than recompute it with `powf`, as `color.rs` freezes the dark-mode
`hue-rotate(180deg)` matrix. The rest of the rendering math did not: at
`7a24ae6` the renderer made 108 calls to `f64::sin`, `cos`, `sin_cos`,
`atan2`, `powf`, `hypot`, `cbrt` and `powi` across eleven files — the Rough.js
port's ellipse points and hachure rotation, arrowhead and elbow geometry, bound
text placement, freedraw stroke radius, element bounds, and the PDF, PNG and
frame-export transforms — and every one of them resolved to whatever the target
provides. On Linux that is glibc's libm, whose `sin`, `cos` and `pow` are
correctly rounded in practice; on wasm32 it is the `compiler-builtins` port of
the `libm` crate, whose results are within one ulp but not always the same
double. Two-decimal Rough path quantization and 8-bit color quantization absorb
most of the difference, but a value within an ulp of a rounding boundary flips
a digit, and until now nothing but the two-target pins would have noticed.

The review (`docs/research/archive/review-2026-09-07/architecture.md` §7) named this
the single largest determinism gap and #148 asked for a strategy. Three were on
the table: freeze more tables, as the sRGB curve is; record why quantization
makes divergence impossible and keep the empirical pins; or call one
implementation on every target. Tables do not fit `sin`/`cos` over a continuous
angle or `pow` over a continuous base. The quantization argument turned out to
be false, not merely unproven: the PDF content stream writes a rotated
element's `cm` matrix — `cos θ` and `sin θ` themselves — through the `Display`
spelling `format_pdf_number` uses, so a one-ulp difference lands in the bytes
unquantized (see the measurement below). That leaves one implementation.

## Decision

Every transcendental function rendering math calls goes through one
package-private module, `crates/excalidraw-render/src/portable_math.rs`, which
routes it to the `libm` crate pinned at `=0.2.16`: `sin`, `cos`, `sin_cos`,
`atan2`, `powf`, `hypot`, `cbrt`, and two helpers, `square` and `cube`, that
replace `powi`. The module is the whole policy: a reviewer sees in one file
what is delegated and what is not, and a new function is a new wrapper there.

The workspace `clippy.toml` disallows the `std` methods the module wraps, and
every other non-correctly-rounded `f32` and `f64` method besides — `tan`,
`exp`, `ln`, the hyperbolics and their inverses — so the lint gate (`cargo
clippy -D warnings`) refuses a call site that bypasses the module rather than
leaving the rule to review. The rule applies to every workspace crate; none
outside the renderer called these functions.

What stays on `std`: `sqrt`, `abs`, `floor`/`ceil`/`round`/`trunc`, `min`/
`max`, `copysign`, `rem_euclid`, `mul_add`, and `to_radians`/`to_degrees`
(one multiplication each). IEEE 754 requires each of them to be exact or
correctly rounded, so every conforming implementation returns the same double
and a wrapper would add nothing.

`powi` is banned rather than wrapped. It is a compiler intrinsic with no
specified rounding: at run time LLVM expands a small constant exponent into
repeated multiplication and `compiler-builtins`' `__powidf2` does the same, but
a call whose base is a compile-time constant is folded with the *host*
compiler's `pow`, which is correctly rounded and so differs from the
multiplication chain — `0.3_f64.powi(3)` folds to `0.026999999999999996` where
`0.3 * 0.3 * 0.3` is `0.027`, and `arrowhead_points` in `render/geometry.rs`
had exactly that constant. Every `powi` site squared or cubed, so `square` and
`cube` — one or two IEEE multiplications, the same on every target under every
optimizer decision — replace it. The module's unit test checks them against
the run-time expansion bit for bit with the base hidden behind `black_box`.

`libm 0.2.16` was already compiled into the renderer on both targets, so the
direct dependency adds an edge to `Cargo.lock` and no package; the comparator
closure ADR-0005 pins is unchanged. (Today the path is `num-traits`, reached
through `csscolorparser` on every feature set.) Its default
`arch` feature is unified on by that path regardless of what the renderer asks
for; the only architecture-specific routines it selects are `sqrt`, `fma` and
the rounding functions, each correctly rounded, so no libm result depends on
the target. On wasm32 the change is expected to be a no-op in bytes, since
`compiler-builtins` is the same code; the wasm leg of the next cross-target
round is what confirms that.

## Measured blast radius

Measured on `x86_64-unknown-linux-gnu` with the pinned toolchain before and
after the routing, on the 39 scene documents in the repository (18 focused
conformance cases, 2 PDF backend qualification scenes, 15 public scenes, the
qualification board, 2 CLI fixtures and the oracle smoke fixture), each
rendered to SVG, PNG and PDF in light and dark mode at scale 1 and 2, plus
every frame export in every format: 624 rows, 493 of them artifacts (the rest
are scenes without frames, two public scenes without a version-2 envelope, and
PNG rasters over the surface limit at scale 2).

Four artifacts changed, all PDF, all one scene: the light and dark, scale 1
and 2 exports of `tests/public-scenes/antibody-amino-acid-chain`. Each differs
in two bytes, the last digit of the two `cos` entries of one rectangle's `cm`
matrix. The rectangle is rotated by `8.76981690235652` rad; glibc gives
`cos = -0.7930716491281027`, the correctly rounded value, and libm gives
`-0.7930716491281026`, one ulp below. No SVG moved — rotation is serialized as
`rotate(deg cx cy)` and every coordinate is quantized — and no PNG moved, the
`f64 → f32` boundary at tiny-skia absorbing the ulp. No frame export moved.

The gates agree: `ci-fast` (784 tests, two of them the module's own) passed
with no pin touched, so none of the digests in `tests/fast/*.rs`,
`render/tests.rs` or the PDF backend qualification moved; `ci-render` (17
tests) passed with every registered-case path comparison, extent comparison
and tolerance exception unchanged, and the two compatibility exception pins in
the SVG harness at their exact measurements. The ten packaged public facade
pins, including the PDF risk set and the PDF fallback families, and the ten
PDF and SVG viewer harness gates passed unchanged as well. No re-pin round was
needed, which is the outcome #148 hoped for: the change is one ulp in one
place today, and the point of making it is that tomorrow's ulp cannot appear
on one target only.

Cost: the CLI over the public scenes in SVG, PNG and PDF measures the same
before and after within run-to-run noise (about 12 s per pass either way on
the shared 4-core host), and the `ci-render` summary time is unchanged;
rasterization and deflate dominate a render, not the trigonometry.

## Consequences

- Spec §9 names the rule beside "target-specific fast paths are forbidden":
  transcendental functions are called only through the module, `powi` is not
  used, and the IEEE-exact functions stay on `std`.
- The residual exposure is inside dependencies, where the rule does not reach:
  `csscolorparser`'s OKLab/OKLCH, HSL and HWB conversions in `f32`, which
  `color-mix()` and gamut mapping in `color.rs` call, and whatever `tiny-skia`,
  `resvg`/`usvg`, `krilla`, `harfrust` (the shaper since #202,
  whose own float use is IEEE-exact rounding) and `moxcms` compute internally. Those
  stay under the empirical two-target pins as before; a divergence found there
  is handled as ADR-0003 handled brotli — adapter, pinned controlled fork,
  focused owned port, in that order.
- An `f32` site would need its own wrapper (`libm` has the `f` variants); the
  one `f32` call the renderer made was `powi(2)` in gamut mapping and `square`
  is generic over it.
- A libm bump is a byte-affecting dependency change like any other: pinned
  exactly, measured against the whole-corpus digest table before merging, and
  re-pinned in one round if anything moves.
