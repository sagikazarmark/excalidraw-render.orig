//! Format-neutral visual comparison for conformance tests.

use std::error::Error;
use std::fmt;

pub mod pixelmatch;
pub mod ssim;

mod comparison;

pub use comparison::{
    compare, compare_with_limits, ChangedPixelLimit, ComparisonError, ComparisonLimits,
    ComparisonMode, ComparisonReport, Crop, CropReport, RegionReport, SsimOutcome, VisualClass,
};

/// A tightly packed, top-left, row-major, straight-alpha sRGB RGBA8 raster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRgba8 {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl CanonicalRgba8 {
    /// Validates the raster layout and clears meaningless RGB below zero alpha.
    pub fn new(width: u32, height: u32, mut rgba: Vec<u8>) -> Result<Self, RasterError> {
        if width == 0 || height == 0 {
            return Err(RasterError::ZeroDimension);
        }
        let expected = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(width, height)| width.checked_mul(height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(RasterError::DimensionsTooLarge)?;
        if rgba.len() != expected {
            return Err(RasterError::InvalidBufferLength {
                expected,
                actual: rgba.len(),
            });
        }

        for pixel in rgba.as_chunks_mut::<4>().0 {
            if pixel[3] == 0 {
                pixel[..3].fill(0);
            }
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Produces the protocol's three complete opaque comparison inputs.
    pub fn views(&self) -> ComparisonViews {
        let mut alpha = Vec::with_capacity(self.rgba.len());
        let mut black = Vec::with_capacity(self.rgba.len());
        let mut white = Vec::with_capacity(self.rgba.len());

        for pixel in self.rgba.as_chunks::<4>().0 {
            let [red, green, blue, opacity] =
                [pixel[0], pixel[1], pixel[2], pixel[3]].map(u32::from);
            let projected_black = [red, green, blue].map(|channel| {
                u8::try_from((channel * opacity + 127) / 255).expect("projection is RGBA8")
            });
            let projected_white = [red, green, blue].map(|channel| {
                u8::try_from((channel * opacity + 255 * (255 - opacity) + 127) / 255)
                    .expect("projection is RGBA8")
            });
            let opacity = u8::try_from(opacity).expect("source opacity is RGBA8");

            alpha.extend_from_slice(&[opacity, opacity, opacity, 255]);
            black.extend_from_slice(&[
                projected_black[0],
                projected_black[1],
                projected_black[2],
                255,
            ]);
            white.extend_from_slice(&[
                projected_white[0],
                projected_white[1],
                projected_white[2],
                255,
            ]);
        }

        ComparisonViews {
            alpha,
            black,
            white,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonViews {
    alpha: Vec<u8>,
    black: Vec<u8>,
    white: Vec<u8>,
}

impl ComparisonViews {
    pub fn alpha(&self) -> &[u8] {
        &self.alpha
    }

    pub fn black(&self) -> &[u8] {
        &self.black
    }

    pub fn white(&self) -> &[u8] {
        &self.white
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RasterError {
    ZeroDimension,
    DimensionsTooLarge,
    InvalidBufferLength { expected: usize, actual: usize },
}

impl fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimension => formatter.write_str("raster dimensions must be nonzero"),
            Self::DimensionsTooLarge => formatter.write_str("raster dimensions are too large"),
            Self::InvalidBufferLength { expected, actual } => write!(
                formatter,
                "RGBA8 buffer length must be {expected} bytes, got {actual}"
            ),
        }
    }
}

impl Error for RasterError {}
