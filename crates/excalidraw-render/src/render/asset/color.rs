//! Normalization of decoded pixels: the color management that brings a
//! raster's declared ICC profile, cICP tags, gAMA/cHRM chunks or linear
//! encoding to sRGB with `moxcms`, and the final pass that zeroes invisible
//! pixels' color and derives the premultiplied copy. The PNG, JPEG, WebP, BMP
//! and ICO decoders finish their RGBA here; GIF composites its indexed frame
//! straight into both buffers itself.

use std::io::Cursor;
use std::sync::Arc;

use moxcms::{
    Chromaticity, CicpColorPrimaries, CicpProfile, ColorPrimaries, ColorProfile, Layout,
    MatrixCoefficients, ToneReprCurve, TransferCharacteristics, TransformOptions, XyY, Xyzd,
};

use super::png::PngSemantics;
use super::{try_copy, DecodedRaster, ResolveError};

pub(super) enum ColorNormalization {
    None,
    LinearSrgb,
    Profile(Box<ColorProfile>),
}

pub(super) const LINEAR_TO_SRGB: [u8; 256] = [
    0, 13, 22, 28, 34, 38, 42, 46, 50, 53, 56, 59, 61, 64, 66, 69, 71, 73, 75, 77, 79, 81, 83, 85,
    86, 88, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104, 105, 106, 108, 109, 110, 112, 113, 114, 115,
    117, 118, 119, 120, 121, 122, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
    137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 148, 149, 150, 151, 152, 153, 154,
    155, 155, 156, 157, 158, 159, 159, 160, 161, 162, 163, 163, 164, 165, 166, 167, 167, 168, 169,
    170, 170, 171, 172, 173, 173, 174, 175, 175, 176, 177, 178, 178, 179, 180, 180, 181, 182, 182,
    183, 184, 185, 185, 186, 187, 187, 188, 189, 189, 190, 190, 191, 192, 192, 193, 194, 194, 195,
    196, 196, 197, 197, 198, 199, 199, 200, 200, 201, 202, 202, 203, 203, 204, 205, 205, 206, 206,
    207, 208, 208, 209, 209, 210, 210, 211, 212, 212, 213, 213, 214, 214, 215, 215, 216, 216, 217,
    218, 218, 219, 219, 220, 220, 221, 221, 222, 222, 223, 223, 224, 224, 225, 226, 226, 227, 227,
    228, 228, 229, 229, 230, 230, 231, 231, 232, 232, 233, 233, 234, 234, 235, 235, 236, 236, 237,
    237, 238, 238, 238, 239, 239, 240, 240, 241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246,
    246, 246, 247, 247, 248, 248, 249, 249, 250, 250, 251, 251, 251, 252, 252, 253, 253, 254, 254,
    255, 255,
];

pub(super) fn normalize_rgba(
    rgba: &mut [u8],
    color_normalization: ColorNormalization,
) -> Result<(), ResolveError> {
    match color_normalization {
        ColorNormalization::None => Ok(()),
        ColorNormalization::LinearSrgb => {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                for channel in &mut pixel[..3] {
                    *channel = LINEAR_TO_SRGB[usize::from(*channel)];
                }
            }
            Ok(())
        }
        ColorNormalization::Profile(profile) => {
            let srgb = ColorProfile::new_srgb();
            let transform = profile
                .create_transform_16bit(
                    Layout::Rgba,
                    &srgb,
                    Layout::Rgba,
                    TransformOptions::default(),
                )
                .map_err(|_| ResolveError::Unsupported)?;
            // The 8-bit executor's 4096-entry output curve loses near-black
            // samples even in identity transforms. Use its higher-resolution
            // path and quantize once, with bounded scratch independent of size.
            let mut source = [0_u16; 1024];
            let mut output = [0_u16; 1024];
            for block in rgba.chunks_mut(source.len()) {
                for (target, value) in source.iter_mut().zip(block.iter()) {
                    *target = u16::from(*value) * 257;
                }
                transform
                    .transform(&source[..block.len()], &mut output[..block.len()])
                    .map_err(|_| ResolveError::Unsupported)?;
                for (target, value) in block.iter_mut().zip(output.iter()) {
                    *target = ((u32::from(*value) * 255 + 32767) / 65535) as u8;
                }
            }
            Ok(())
        }
    }
}

pub(super) fn finish_rgba(mut rgba: Vec<u8>) -> Result<DecodedRaster, ResolveError> {
    for pixel in rgba.as_chunks_mut::<4>().0 {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
    let mut premultiplied_rgba = try_copy(&rgba)?;
    for pixel in premultiplied_rgba.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}

pub(super) fn prepare_png_color_normalization(
    bytes: &[u8],
    semantics: &PngSemantics,
    workspace_bytes: u64,
) -> Result<ColorNormalization, ResolveError> {
    // The pinned SkPngRustCodec cannot construct a narrow-range RGB space and
    // continues with the lower-priority metadata instead.
    if let Some(cicp) = semantics.cicp.filter(|cicp| cicp[3] != 0) {
        if cicp == [1, 13, 0, 1] {
            return Ok(ColorNormalization::None);
        }
        if cicp == [1, 8, 0, 1] {
            return Ok(ColorNormalization::LinearSrgb);
        }
        return cicp_color_profile(cicp)
            .map(Box::new)
            .map(ColorNormalization::Profile);
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(workspace_bytes).map_err(|_| ResolveError::Memory)?,
    });
    let reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    png_color_profile(reader.info())
}

fn cicp_color_profile(cicp: [u8; 4]) -> Result<ColorProfile, ResolveError> {
    let color_primaries =
        CicpColorPrimaries::try_from(cicp[0]).map_err(|_| ResolveError::Unsupported)?;
    let transfer_characteristics =
        TransferCharacteristics::try_from(cicp[1]).map_err(|_| ResolveError::Unsupported)?;
    let matrix_coefficients =
        MatrixCoefficients::try_from(cicp[2]).map_err(|_| ResolveError::Unsupported)?;
    if !matches!(matrix_coefficients, MatrixCoefficients::Identity)
        || cicp[3] > 1
        || matches!(
            color_primaries,
            CicpColorPrimaries::Reserved | CicpColorPrimaries::Unspecified
        )
        || matches!(
            transfer_characteristics,
            TransferCharacteristics::Reserved | TransferCharacteristics::Unspecified
        )
    {
        return Err(ResolveError::Unsupported);
    }
    let mut profile = ColorProfile::new_from_cicp(CicpProfile {
        color_primaries,
        transfer_characteristics,
        matrix_coefficients,
        full_range: cicp[3] == 1,
    });
    if matches!(color_primaries, CicpColorPrimaries::Xyz) {
        // The generic xy -> XYZ constructor normalizes each primary to Y=1,
        // collapsing the zero-luminance X/Z axes. Preserve the XYZ basis and
        // its equal-energy white, then let the CMS adapt it to the D50 PCS.
        let signaling = profile.cicp;
        profile.update_rgb_colorimetry_triplet(
            XyY::new(1.0 / 3.0, 1.0 / 3.0, 1.0),
            Xyzd::new(1.0, 0.0, 0.0),
            Xyzd::new(0.0, 1.0, 0.0),
            Xyzd::new(0.0, 0.0, 1.0),
        );
        profile.cicp = signaling;
    }
    // SkNamedTransferFn follows H.273's reference EOTF (BT.1886 gamma 2.4),
    // rather than inverting the camera OETF for these display-referred codes.
    if matches!(cicp[1], 1 | 6 | 11 | 14 | 15) {
        profile.cicp = None;
        let curve = ToneReprCurve::Parametric(vec![2.4]);
        profile.red_trc = Some(curve.clone());
        profile.green_trc = Some(curve.clone());
        profile.blue_trc = Some(curve);
    }
    Ok(profile)
}

fn png_color_profile(info: &png::Info<'_>) -> Result<ColorNormalization, ResolveError> {
    if let Some(profile) = info.icc_profile.as_deref() {
        return ColorProfile::new_from_slice(profile)
            .map(Box::new)
            .map(ColorNormalization::Profile)
            .map_err(|_| ResolveError::Unsupported);
    }
    if info.srgb.is_some() {
        return Ok(ColorNormalization::None);
    }
    // The reference intentionally does not guess a transfer function from
    // chromaticities alone (SkPngRustCodec::CreateColorProfile).
    if info.source_gamma.is_none() {
        return Ok(ColorNormalization::None);
    }
    if info.source_chromaticities.is_none()
        && info.source_gamma.is_some_and(|gamma| {
            let neutral = gamma.into_value() * 2.2;
            neutral > 0.95 && neutral < 1.05
        })
    {
        return Ok(ColorNormalization::None);
    }
    if info.source_chromaticities.is_none()
        && info
            .source_gamma
            .is_some_and(|gamma| gamma.into_scaled() == 100_000)
    {
        return Ok(ColorNormalization::LinearSrgb);
    }

    let mut profile = ColorProfile::new_srgb();
    profile.cicp = None;
    if let Some(chromaticities) = info.source_chromaticities {
        let valid = [
            chromaticities.white,
            chromaticities.red,
            chromaticities.green,
            chromaticities.blue,
        ]
        .into_iter()
        .all(|(x, y)| u64::from(x.into_scaled()) + u64::from(y.into_scaled()) <= 100_000);
        if !valid {
            return Err(ResolveError::Unsupported);
        }
        let value = |scaled: png::ScaledFloat| f64::from(scaled.into_scaled()) / 100_000.0;
        profile.update_rgb_colorimetry(
            XyY::new(
                value(chromaticities.white.0),
                value(chromaticities.white.1),
                1.0,
            ),
            ColorPrimaries {
                red: Chromaticity {
                    x: value(chromaticities.red.0) as f32,
                    y: value(chromaticities.red.1) as f32,
                },
                green: Chromaticity {
                    x: value(chromaticities.green.0) as f32,
                    y: value(chromaticities.green.1) as f32,
                },
                blue: Chromaticity {
                    x: value(chromaticities.blue.0) as f32,
                    y: value(chromaticities.blue.1) as f32,
                },
            },
        );
    }
    if let Some(gamma) = info.source_gamma {
        let gamma = gamma.into_value();
        if gamma <= 0.0 {
            return Err(ResolveError::Unsupported);
        }
        // curve_from_gamma stores an ICC u8Fixed8 exponent, which saturates
        // above 255.996 and rounds tiny exponents to zero. gAMA has a much
        // wider domain; retain the actual power in a parametric curve.
        let curve = ToneReprCurve::Parametric(vec![1.0 / gamma]);
        profile.red_trc = Some(curve.clone());
        profile.green_trc = Some(curve.clone());
        profile.blue_trc = Some(curve);
    }
    Ok(ColorNormalization::Profile(Box::new(profile)))
}
