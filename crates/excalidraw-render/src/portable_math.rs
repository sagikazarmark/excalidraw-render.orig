//! The transcendental functions rendering math may call, each routed through
//! the pure-Rust `libm` crate so that one input gives one double on every
//! supported target (ADR-0007).
//!
//! `f64::sin` and its siblings are not correctly rounded, and which
//! implementation answers them is a property of the target: on
//! `x86_64-unknown-linux-gnu` they resolve to glibc's libm, on
//! `wasm32-unknown-unknown` to the `compiler-builtins` port of this very
//! crate, and the two can differ in the last ulp. Rough path coordinates are
//! quantized to two decimals and colors to eight bits, so most of that
//! difference vanishes, but a value within an ulp of a rounding boundary flips
//! a digit and with it the artifact bytes. Calling one implementation on every
//! target makes artifact byte stability structural for these functions rather
//! than something the cross-target pins observe after the fact.
//!
//! What stays on `std`: `sqrt`, `abs`, `floor`/`ceil`/`round`/`trunc`,
//! `min`/`max`, `copysign`, `rem_euclid`, `mul_add`, and `to_radians` and
//! `to_degrees` (one multiplication each). IEEE 754 requires each of them to
//! be exact or correctly rounded, so every conforming implementation agrees.
//! `powi` is a compiler intrinsic with no specified rounding: at run time
//! LLVM expands a small constant exponent into repeated multiplication and
//! `compiler-builtins`' `__powidf2` does the same, but when the base is a
//! compile-time constant LLVM folds the call with the *host* compiler's
//! `pow`, which is correctly rounded and therefore differs from the
//! multiplication chain — `0.3.powi(3)` folds to `0.026999999999999996` while
//! `0.3 * 0.3 * 0.3` is `0.027`. Every site that used it squared or cubed,
//! so they call [`square`] and [`cube`] here, which are the one or two IEEE
//! multiplications on every target and under every optimizer decision.
//!
//! The workspace `clippy.toml` disallows the `std` methods this module wraps,
//! so a new call site has to come through here.

/// Sine of an angle in radians.
pub(crate) fn sin(radians: f64) -> f64 {
    libm::sin(radians)
}

/// Cosine of an angle in radians.
pub(crate) fn cos(radians: f64) -> f64 {
    libm::cos(radians)
}

/// `(sin, cos)` of an angle in radians, the pair `f64::sin_cos` returns.
pub(crate) fn sin_cos(radians: f64) -> (f64, f64) {
    (sin(radians), cos(radians))
}

/// Four-quadrant arctangent of `y / x`, in radians.
pub(crate) fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// `base` raised to the real power `exponent`.
pub(crate) fn powf(base: f64, exponent: f64) -> f64 {
    libm::pow(base, exponent)
}

/// Euclidean length of the vector `(x, y)`.
pub(crate) fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// Cube root.
pub(crate) fn cbrt(value: f64) -> f64 {
    libm::cbrt(value)
}

/// `value * value`, the one multiplication `powi(2)` performs.
pub(crate) fn square<T: Copy + std::ops::Mul<Output = T>>(value: T) -> T {
    value * value
}

/// `value * value * value`, the two multiplications `powi(3)` performs. IEEE
/// multiplication is commutative, so `(v * v) * v` and `v * (v * v)` are the
/// same double and the association here is not a choice.
pub(crate) fn cube(value: f64) -> f64 {
    value * value * value
}

#[cfg(test)]
mod tests {
    use super::{cube, sin_cos, square};

    // The module's claim about the run-time `powi` expansion is checked
    // against the intrinsic itself, which nothing else in the crate may call.
    // `black_box` keeps the base out of the optimizer's hands: a constant base
    // is folded with the host's `pow` instead, which is the hazard the module
    // documents, and the call sites this replaced never had a constant base.
    #[allow(clippy::disallowed_methods)]
    #[test]
    fn square_and_cube_match_the_run_time_powi_expansion_bit_for_bit() {
        let samples = [
            0.0_f64,
            -0.0,
            1.0,
            -1.0,
            0.3,
            0.7,
            1e-300,
            1e300,
            123.456,
            -987.654,
            0.1 + 0.2,
        ];
        for value in samples.map(std::hint::black_box) {
            assert_eq!(square(value).to_bits(), value.powi(2).to_bits(), "{value}");
            assert_eq!(cube(value).to_bits(), value.powi(3).to_bits(), "{value}");
            let single = value as f32;
            assert_eq!(
                square(single).to_bits(),
                single.powi(2).to_bits(),
                "{single}"
            );
        }
    }

    #[test]
    fn sin_cos_is_the_pair_of_sin_and_cos() {
        for radians in [0.0, 0.5, 1.0, -2.5, std::f64::consts::PI, 1e6] {
            let (sin, cos) = sin_cos(radians);
            assert_eq!(sin.to_bits(), super::sin(radians).to_bits());
            assert_eq!(cos.to_bits(), super::cos(radians).to_bits());
        }
    }
}
