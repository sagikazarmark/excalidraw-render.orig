//! Deterministic valid-window single-scale SSIM.

use std::error::Error;
use std::fmt;

const GAUSSIAN_WORDS: [u64; 121] = [
    0x3eb1be35ac1c34c6,
    0x3ee063548b08c281,
    0x3f036912a05519ac,
    0x3f1d7b5ae8a4fc11,
    0x3f2cb61ec7cbd51a,
    0x3f31ed983c6c179d,
    0x3f2cb61ec7cbd51a,
    0x3f1d7b5ae8a4fc11,
    0x3f036912a05519ac,
    0x3ee063548b08c281,
    0x3eb1be35ac1c34c6,
    0x3ee063548b08c281,
    0x3f0e45e1f47286ac,
    0x3f31ed983c6c179d,
    0x3f4b3afa92035fc1,
    0x3f5a84ce6c9db06d,
    0x3f608f18b8573f7e,
    0x3f5a84ce6c9db06d,
    0x3f4b3afa92035fc1,
    0x3f31ed983c6c179d,
    0x3f0e45e1f47286ac,
    0x3ee063548b08c281,
    0x3f036912a05519ac,
    0x3f31ed983c6c179d,
    0x3f553c0d18eb2ec1,
    0x3f702050fb9dc09d,
    0x3f7f68dcbd7dd57d,
    0x3f839ce92f03301e,
    0x3f7f68dcbd7dd57d,
    0x3f702050fb9dc09d,
    0x3f553c0d18eb2ec1,
    0x3f31ed983c6c179d,
    0x3f036912a05519ac,
    0x3f1d7b5ae8a4fc11,
    0x3f4b3afa92035fc1,
    0x3f702050fb9dc09d,
    0x3f887e5bdea2018b,
    0x3f97da7ee69c2aee,
    0x3f9dca171b0b58b1,
    0x3f97da7ee69c2aee,
    0x3f887e5bdea2018b,
    0x3f702050fb9dc09d,
    0x3f4b3afa92035fc1,
    0x3f1d7b5ae8a4fc11,
    0x3f2cb61ec7cbd51a,
    0x3f5a84ce6c9db06d,
    0x3f7f68dcbd7dd57d,
    0x3f97da7ee69c2aee,
    0x3fa73aea2efb5b99,
    0x3fad02cc3c609d96,
    0x3fa73aea2efb5b99,
    0x3f97da7ee69c2aee,
    0x3f7f68dcbd7dd57d,
    0x3f5a84ce6c9db06d,
    0x3f2cb61ec7cbd51a,
    0x3f31ed983c6c179d,
    0x3f608f18b8573f7e,
    0x3f839ce92f03301e,
    0x3f9dca171b0b58b1,
    0x3fad02cc3c609d96,
    0x3fb21d7958fce8e0,
    0x3fad02cc3c609d96,
    0x3f9dca171b0b58b1,
    0x3f839ce92f03301e,
    0x3f608f18b8573f7e,
    0x3f31ed983c6c179d,
    0x3f2cb61ec7cbd51a,
    0x3f5a84ce6c9db06d,
    0x3f7f68dcbd7dd57d,
    0x3f97da7ee69c2aee,
    0x3fa73aea2efb5b99,
    0x3fad02cc3c609d96,
    0x3fa73aea2efb5b99,
    0x3f97da7ee69c2aee,
    0x3f7f68dcbd7dd57d,
    0x3f5a84ce6c9db06d,
    0x3f2cb61ec7cbd51a,
    0x3f1d7b5ae8a4fc11,
    0x3f4b3afa92035fc1,
    0x3f702050fb9dc09d,
    0x3f887e5bdea2018b,
    0x3f97da7ee69c2aee,
    0x3f9dca171b0b58b1,
    0x3f97da7ee69c2aee,
    0x3f887e5bdea2018b,
    0x3f702050fb9dc09d,
    0x3f4b3afa92035fc1,
    0x3f1d7b5ae8a4fc11,
    0x3f036912a05519ac,
    0x3f31ed983c6c179d,
    0x3f553c0d18eb2ec1,
    0x3f702050fb9dc09d,
    0x3f7f68dcbd7dd57d,
    0x3f839ce92f03301e,
    0x3f7f68dcbd7dd57d,
    0x3f702050fb9dc09d,
    0x3f553c0d18eb2ec1,
    0x3f31ed983c6c179d,
    0x3f036912a05519ac,
    0x3ee063548b08c281,
    0x3f0e45e1f47286ac,
    0x3f31ed983c6c179d,
    0x3f4b3afa92035fc1,
    0x3f5a84ce6c9db06d,
    0x3f608f18b8573f7e,
    0x3f5a84ce6c9db06d,
    0x3f4b3afa92035fc1,
    0x3f31ed983c6c179d,
    0x3f0e45e1f47286ac,
    0x3ee063548b08c281,
    0x3eb1be35ac1c34c6,
    0x3ee063548b08c281,
    0x3f036912a05519ac,
    0x3f1d7b5ae8a4fc11,
    0x3f2cb61ec7cbd51a,
    0x3f31ed983c6c179d,
    0x3f2cb61ec7cbd51a,
    0x3f1d7b5ae8a4fc11,
    0x3f036912a05519ac,
    0x3ee063548b08c281,
    0x3eb1be35ac1c34c6,
];

const C1: f64 = f64::from_bits(0x401a_028f_5c28_f5c3);
const C2: f64 = f64::from_bits(0x404d_42e1_47ae_147b);

pub fn gaussian_kernel_words() -> &'static [u64; 121] {
    &GAUSSIAN_WORDS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SsimError {
    ZeroDimension,
    DimensionsTooLarge,
    RegionTooSmall,
    InvalidBufferLength {
        image: usize,
        expected: usize,
        actual: usize,
    },
    NonFinite,
}

impl fmt::Display for SsimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimension => formatter.write_str("image dimensions must be nonzero"),
            Self::DimensionsTooLarge => formatter.write_str("image dimensions are too large"),
            Self::RegionTooSmall => formatter.write_str("SSIM regions must be at least 11x11"),
            Self::InvalidBufferLength {
                image,
                expected,
                actual,
            } => write!(
                formatter,
                "image {image} must contain {expected} samples, got {actual}"
            ),
            Self::NonFinite => formatter.write_str("SSIM encountered a non-finite value"),
        }
    }
}

impl Error for SsimError {}

pub fn compare_planes(
    first: &[f64],
    second: &[f64],
    width: u32,
    height: u32,
) -> Result<f64, SsimError> {
    if width == 0 || height == 0 {
        return Err(SsimError::ZeroDimension);
    }
    if width < 11 || height < 11 {
        return Err(SsimError::RegionTooSmall);
    }
    let width = usize::try_from(width).map_err(|_| SsimError::DimensionsTooLarge)?;
    let height = usize::try_from(height).map_err(|_| SsimError::DimensionsTooLarge)?;
    let expected = width
        .checked_mul(height)
        .ok_or(SsimError::DimensionsTooLarge)?;
    for (index, plane) in [first, second].into_iter().enumerate() {
        if plane.len() != expected {
            return Err(SsimError::InvalidBufferLength {
                image: index + 1,
                expected,
                actual: plane.len(),
            });
        }
    }

    let mut score_sum = 0.0;
    let mut center_count = 0usize;
    for top in 0..=height - 11 {
        for left in 0..=width - 11 {
            let score = local_score(first, second, width, left, top)?;
            score_sum = finite(score_sum + score)?;
            center_count += 1;
        }
    }
    finite(score_sum / center_count as f64)
}

fn local_score(
    first: &[f64],
    second: &[f64],
    width: usize,
    left: usize,
    top: usize,
) -> Result<f64, SsimError> {
    let mut mean_x = 0.0;
    let mut mean_y = 0.0;
    let mut expected_x_squared = 0.0;
    let mut expected_y_squared = 0.0;
    let mut expected_xy = 0.0;

    for kernel_y in 0..11 {
        for kernel_x in 0..11 {
            let kernel_index = kernel_y * 11 + kernel_x;
            let sample_index = (top + kernel_y) * width + left + kernel_x;
            let weight = f64::from_bits(GAUSSIAN_WORDS[kernel_index]);
            let x = finite(first[sample_index])?;
            let y = finite(second[sample_index])?;

            let weighted_x = finite(weight * x)?;
            mean_x = finite(mean_x + weighted_x)?;
            let weighted_y = finite(weight * y)?;
            mean_y = finite(mean_y + weighted_y)?;
            let weighted_x_squared = finite(weighted_x * x)?;
            expected_x_squared = finite(expected_x_squared + weighted_x_squared)?;
            let weighted_y_squared = finite(weighted_y * y)?;
            expected_y_squared = finite(expected_y_squared + weighted_y_squared)?;
            let weighted_xy = finite(weighted_x * y)?;
            expected_xy = finite(expected_xy + weighted_xy)?;
        }
    }

    let mean_x_squared = finite(mean_x * mean_x)?;
    let variance_x = finite(expected_x_squared - mean_x_squared)?;
    let variance_x = if variance_x < 0.0 { 0.0 } else { variance_x };
    let mean_y_squared = finite(mean_y * mean_y)?;
    let variance_y = finite(expected_y_squared - mean_y_squared)?;
    let variance_y = if variance_y < 0.0 { 0.0 } else { variance_y };
    let mean_product = finite(mean_x * mean_y)?;
    let covariance = finite(expected_xy - mean_product)?;

    let numerator_luminance = finite(2.0 * mean_x)?;
    let numerator_luminance = finite(numerator_luminance * mean_y)?;
    let numerator_luminance = finite(numerator_luminance + C1)?;
    let numerator_structure = finite(2.0 * covariance)?;
    let numerator_structure = finite(numerator_structure + C2)?;
    let numerator = finite(numerator_luminance * numerator_structure)?;

    let denominator_luminance = finite(mean_x_squared + mean_y_squared)?;
    let denominator_luminance = finite(denominator_luminance + C1)?;
    let denominator_structure = finite(variance_x + variance_y)?;
    let denominator_structure = finite(denominator_structure + C2)?;
    let denominator = finite(denominator_luminance * denominator_structure)?;
    finite(numerator / denominator)
}

fn finite(value: f64) -> Result<f64, SsimError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(SsimError::NonFinite)
    }
}
