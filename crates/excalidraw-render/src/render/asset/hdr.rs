//! Pinned Chromium149 SDR projection: BT.2100 decoding followed by Skia's
//! default reference-white tone mapping (RWTMO), in linear Rec.2020. Keep HDR
//! values unclipped until after gamut conversion and tone mapping.
use moxcms::ColorProfile;

use crate::portable_math::{exp, exp2, log2, powf};

use super::ResolveError;

pub(super) struct Hdr {
    to_rec2020: [[f64; 3]; 3],
    to_srgb: [[f64; 3]; 3],
    hlg: bool,
    curve: [[f64; 3]; 8],
}

impl Hdr {
    pub(super) fn new(profile: &ColorProfile, hlg: bool) -> Self {
        let rec2020 = ColorProfile::new_bt2020();
        Self {
            to_rec2020: profile.transform_matrix(&rec2020).v,
            to_srgb: rec2020.transform_matrix(&ColorProfile::new_srgb()).v,
            hlg,
            curve: rwtmo_curve(),
        }
    }

    fn linearize(&self, v: f64) -> f64 {
        if self.hlg {
            if v <= 0.5 {
                v * v / 3.0
            } else {
                (exp((v - 0.55991073) / 0.17883277) + 0.28466892) / 12.0
            }
        } else {
            let p = powf(v, 32.0 / 2523.0);
            powf(
                (p - 107.0 / 128.0).max(0.0) / (2413.0 / 128.0 - 2392.0 / 128.0 * p),
                8192.0 / 1305.0,
            ) * (10000.0 / 203.0)
        }
    }

    // The browser retains HDR encoded samples in a premultiplied RGBA8 image,
    // then unpremultiplies before the draw-time EOTF. PDF needs the independent
    // straight-color projection, so derive both from the original samples.
    pub(super) fn project_pixel(
        &self,
        rgb: [f64; 3],
        alpha: f64,
    ) -> Result<([u8; 4], [u8; 4]), ResolveError> {
        if !rgb
            .into_iter()
            .chain([alpha])
            .all(|v| (0.0..=1.0).contains(&v))
        {
            return Err(ResolveError::DecodeFailed);
        }
        let a = (alpha * 255.0).round() as u8;
        let source = rgb.map(|v| self.linearize((v * 255.0).round() / 255.0));
        let output = self.project_linear(source)?;
        let mut straight = [0, 0, 0, a];
        for (channel, v) in straight[..3].iter_mut().zip(output) {
            *channel = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        let mut encoded = rgb.map(|v| {
            if a == 0 {
                0.0
            } else {
                (v * alpha * 255.0).round() / f64::from(a)
            }
        });
        encoded = encoded.map(|v| self.linearize(v));
        let output = self.project_linear(encoded)?;
        let mut premultiplied = [0, 0, 0, a];
        for (channel, v) in premultiplied[..3].iter_mut().zip(output) {
            *channel = (v.clamp(0.0, 1.0) * f64::from(a)).round() as u8;
        }
        if a == 0 {
            straight[..3].fill(0);
        }
        Ok((straight, premultiplied))
    }

    fn project_linear(&self, linear: [f64; 3]) -> Result<[f64; 3], ResolveError> {
        let mut rgb = multiply(self.to_rec2020, linear);
        if self.hlg {
            let y = 0.2627 * rgb[0] + 0.6780 * rgb[1] + 0.0593 * rgb[2];
            // SkRasterPipeline's OOTF extends the power with its sign.
            // D50-adapted XYZ primaries can produce negative luminance.
            let scale = powf(y.abs(), 0.2).copysign(y) * (1000.0 / 203.0);
            rgb = rgb.map(|v| v * scale);
        }
        // Check every channel before max(), which otherwise ignores a NaN.
        if !rgb.into_iter().all(f64::is_finite) {
            return Err(ResolveError::DecodeFailed);
        }
        let gain = rwtmo_gain(&self.curve, rgb[0].max(rgb[1]).max(rgb[2]))?;
        let output = multiply(self.to_srgb, rgb.map(|v| v * gain)).map(|v| {
            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * powf(v, 1.0 / 2.4) - 0.055
            }
        });
        if !output.into_iter().all(f64::is_finite) {
            return Err(ResolveError::DecodeFailed);
        }
        Ok(output)
    }
}

fn multiply(matrix: [[f64; 3]; 3], rgb: [f64; 3]) -> [f64; 3] {
    matrix.map(|r| r[0] * rgb[0] + r[1] * rgb[1] + r[2] * rgb[2])
}

// SkHdrAgtm::PopulateUsingRwtmo, SDR alternate: default peak 1000 nits,
// reference white 203 nits, yWhite=0.5, kappa=0.65. Eight Hermite knots in
// log-gain space represent the quadratic Bezier shoulder.
fn rwtmo_curve() -> [[f64; 3]; 8] {
    let xa = 1.0 - 2.0 * 1.65 + 1000.0 / 203.0;
    std::array::from_fn(|i| {
        let t = i as f64 / 7.0;
        let x = 1.0 + t * (1.3 + t * xa);
        let y = 0.5 + t * (0.65 - t * 0.15);
        let slope = (0.65 - 0.3 * t) / (1.3 + 2.0 * xa * t);
        [
            x,
            log2(y / x),
            (x * slope - y) / (std::f64::consts::LN_2 * x * y),
        ]
    })
}

fn rwtmo_gain(curve: &[[f64; 3]; 8], x: f64) -> Result<f64, ResolveError> {
    if !x.is_finite() {
        return Err(ResolveError::DecodeFailed);
    }
    if x <= curve[0][0] {
        return Ok(0.5);
    }
    if x >= curve[7][0] {
        return Ok(1.0 / x);
    }
    // The generated knots are increasing, so an interior finite x has an
    // interval. Preserve a typed failure if that invariant ever stops holding.
    let interval = curve
        .windows(2)
        .find(|p| x < p[1][0])
        .ok_or(ResolveError::DecodeFailed)?;
    let [x0, y0, m0] = interval[0];
    let [x1, y1, m1] = interval[1];
    let h = x1 - x0;
    if !h.is_finite() || h <= 0.0 {
        return Err(ResolveError::DecodeFailed);
    }
    let t = (x - x0) / h;
    let c3 = 2.0 * y0 + m0 * h - 2.0 * y1 + m1 * h;
    let c2 = -3.0 * y0 + 3.0 * y1 - 2.0 * m0 * h - m1 * h;
    let gain = exp2(((c3 * t + c2) * t + m0 * h) * t + y0);
    if !gain.is_finite() || gain <= 0.0 {
        return Err(ResolveError::DecodeFailed);
    }
    Ok(gain)
}

#[cfg(test)]
mod tests {
    use super::{rwtmo_curve, rwtmo_gain, Hdr, ResolveError};
    use moxcms::ColorProfile;

    #[test]
    fn projection_failures_are_typed_and_not_quantized_to_pixels() {
        let mut hdr = Hdr::new(&ColorProfile::new_srgb(), false);
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.1] {
            assert_eq!(
                hdr.project_pixel([value, 0.5, 0.5], 1.0),
                Err(ResolveError::DecodeFailed)
            );
            assert_eq!(
                hdr.project_pixel([0.5; 3], value),
                Err(ResolveError::DecodeFailed)
            );
        }
        hdr.to_rec2020[0][0] = f64::NAN;
        assert_eq!(
            hdr.project_pixel([0.5; 3], 1.0),
            Err(ResolveError::DecodeFailed)
        );
        let hdr = Hdr {
            to_srgb: [[f64::INFINITY; 3]; 3],
            ..Hdr::new(&ColorProfile::new_srgb(), false)
        };
        assert_eq!(
            hdr.project_pixel([0.5; 3], 1.0),
            Err(ResolveError::DecodeFailed)
        );
    }

    #[test]
    fn gain_curve_bounds_and_missing_intervals_are_explicit() {
        let curve = rwtmo_curve();
        assert!(curve.iter().flatten().all(|v| v.is_finite()));
        assert!(curve.windows(2).all(|p| p[0][0] < p[1][0]));
        assert_eq!(rwtmo_gain(&curve, 0.0), Ok(0.5));
        assert_eq!(rwtmo_gain(&curve, 1.0), Ok(0.5));
        for x in [1.1, 2.0, 3.0, 4.0] {
            let y = x * rwtmo_gain(&curve, x).unwrap();
            assert!((0.5..1.0).contains(&y));
        }
        assert_eq!(rwtmo_gain(&curve, 10.0), Ok(0.1));
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(rwtmo_gain(&curve, x), Err(ResolveError::DecodeFailed));
        }
        assert_eq!(
            rwtmo_gain(&[[f64::NAN; 3]; 8], 2.0),
            Err(ResolveError::DecodeFailed)
        );
        let mut broken = curve;
        broken[1][1] = f64::NAN;
        assert_eq!(rwtmo_gain(&broken, 1.1), Err(ResolveError::DecodeFailed));
    }
}
// RWTMO curve construction and evaluation adapted from Skia SkHdrAgtm.cpp:
// Copyright 2025 Google LLC
// Distributed under Skia's BSD-3-Clause license, retained in
// ../../../LICENSE-BUNDLE.txt (Skia HDR tone-mapping adaptation section).
// Source revision: 53348aa333da02b77c4b5797e2de722f5abde7d0.
// See docs/research/png-hdr-gain-map-ownership.md for adaptation provenance.
