use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::pixelmatch::{self, Options};
use crate::ssim::{self, SsimError};
use crate::{CanonicalRgba8, ComparisonViews};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonMode {
    Canonical,
    PureRustSvg,
    PdfJs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualClass {
    Exact,
    GeometryImage,
    Typography,
    MixedRepresentative,
}

impl VisualClass {
    pub fn limits(self, mode: ComparisonMode) -> ComparisonLimits {
        if self == Self::Typography
            && matches!(mode, ComparisonMode::PureRustSvg | ComparisonMode::PdfJs)
        {
            return ComparisonLimits::fraction(3, 100, f64::from_bits(0x3fef_ae14_7ae1_47ae));
        }
        match self {
            Self::Exact => ComparisonLimits::fraction(0, 1, f64::from_bits(0x3ff0_0000_0000_0000)),
            Self::GeometryImage => {
                ComparisonLimits::fraction(1, 400, f64::from_bits(0x3fef_f7ce_d916_872b))
            }
            Self::Typography => {
                ComparisonLimits::fraction(1, 40, f64::from_bits(0x3fef_d70a_3d70_a3d7))
            }
            Self::MixedRepresentative => {
                ComparisonLimits::fraction(1, 100, f64::from_bits(0x3fef_e76c_8b43_9581))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangedPixelLimit {
    Fraction { numerator: u64, denominator: u64 },
    Count(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComparisonLimits {
    changed_pixels: ChangedPixelLimit,
    minimum_ssim: f64,
}

impl ComparisonLimits {
    const fn fraction(numerator: u64, denominator: u64, minimum_ssim: f64) -> Self {
        Self {
            changed_pixels: ChangedPixelLimit::Fraction {
                numerator,
                denominator,
            },
            minimum_ssim,
        }
    }

    /// Creates the exact no-margin limits from an approved tolerance exception.
    pub fn tolerance_exception(
        maximum_changed_pixels: u64,
        minimum_ssim: f64,
    ) -> Result<Self, ComparisonError> {
        if !minimum_ssim.is_finite() {
            return Err(ComparisonError::InvalidSsimFloor);
        }
        Ok(Self {
            changed_pixels: ChangedPixelLimit::Count(maximum_changed_pixels),
            minimum_ssim,
        })
    }

    pub fn changed_pixel_limit(self) -> ChangedPixelLimit {
        self.changed_pixels
    }

    pub fn minimum_ssim(self) -> f64 {
        self.minimum_ssim
    }

    pub fn changed_pixels_pass(self, changed_pixels: u64, area: u64) -> bool {
        match self.changed_pixels {
            ChangedPixelLimit::Fraction {
                numerator,
                denominator,
            } => {
                u128::from(changed_pixels) * u128::from(denominator)
                    <= u128::from(area) * u128::from(numerator)
            }
            ChangedPixelLimit::Count(maximum) => changed_pixels <= maximum,
        }
    }

    pub fn ssim_pass(self, score: f64) -> bool {
        score.is_finite() && score >= self.minimum_ssim
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RegionLimits {
    Class(VisualClass),
    Explicit(ComparisonLimits),
}

impl RegionLimits {
    fn resolve(self, mode: ComparisonMode) -> ComparisonLimits {
        match self {
            Self::Class(class) => class.limits(mode),
            Self::Explicit(limits) => limits,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Crop {
    id: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    limits: RegionLimits,
}

impl Crop {
    pub fn new(
        id: impl Into<String>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        class: VisualClass,
    ) -> Result<Self, ComparisonError> {
        Self::with_region_limits(id.into(), x, y, width, height, RegionLimits::Class(class))
    }

    /// Creates a crop governed by one approved, exact tolerance exception.
    pub fn with_limits(
        id: impl Into<String>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        limits: ComparisonLimits,
    ) -> Result<Self, ComparisonError> {
        Self::with_region_limits(
            id.into(),
            x,
            y,
            width,
            height,
            RegionLimits::Explicit(limits),
        )
    }

    fn with_region_limits(
        id: String,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        limits: RegionLimits,
    ) -> Result<Self, ComparisonError> {
        if id.is_empty() {
            return Err(ComparisonError::EmptyCropId);
        }
        if width < 11 || height < 11 {
            return Err(ComparisonError::CropTooSmall { id, width, height });
        }
        Ok(Self {
            id,
            x,
            y,
            width,
            height,
            limits,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SsimOutcome {
    Score(f64),
    SmallArtifactMismatch,
    NonFinite,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegionReport {
    pub width: u32,
    pub height: u32,
    pub area: u64,
    pub changed_pixels: u64,
    pub changed_mask: Vec<bool>,
    pub ssim: SsimOutcome,
    pub changed_pixels_pass: bool,
    pub ssim_pass: bool,
}

impl RegionReport {
    pub fn passed(&self) -> bool {
        self.changed_pixels_pass && self.ssim_pass
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CropReport {
    pub id: String,
    pub region: RegionReport,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonReport {
    pub whole: RegionReport,
    pub crops: Vec<CropReport>,
}

impl ComparisonReport {
    pub fn passed(&self) -> bool {
        self.whole.passed() && self.crops.iter().all(|crop| crop.region.passed())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComparisonError {
    DimensionMismatch {
        reference: (u32, u32),
        candidate: (u32, u32),
    },
    EmptyCropId,
    DuplicateCropId(String),
    CropTooSmall {
        id: String,
        width: u32,
        height: u32,
    },
    CropOutOfBounds {
        id: String,
        artifact: (u32, u32),
    },
    InvalidSsimFloor,
    MetricInvariant(&'static str),
}

impl fmt::Display for ComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch {
                reference,
                candidate,
            } => write!(
                formatter,
                "raster dimensions differ: reference {reference:?}, candidate {candidate:?}"
            ),
            Self::EmptyCropId => formatter.write_str("crop identity must not be empty"),
            Self::DuplicateCropId(id) => write!(formatter, "crop identity {id:?} is duplicated"),
            Self::CropTooSmall { id, width, height } => write!(
                formatter,
                "crop {id:?} must be at least 11x11, got {width}x{height}"
            ),
            Self::CropOutOfBounds { id, artifact } => write!(
                formatter,
                "crop {id:?} is outside artifact dimensions {artifact:?}"
            ),
            Self::InvalidSsimFloor => {
                formatter.write_str("SSIM floor must be a finite binary64 value")
            }
            Self::MetricInvariant(message) => formatter.write_str(message),
        }
    }
}

impl Error for ComparisonError {}

pub fn compare(
    reference: &CanonicalRgba8,
    candidate: &CanonicalRgba8,
    whole_class: VisualClass,
    crops: &[Crop],
    mode: ComparisonMode,
) -> Result<ComparisonReport, ComparisonError> {
    compare_internal(
        reference,
        candidate,
        RegionLimits::Class(whole_class),
        crops,
        mode,
    )
}

/// Compares a whole artifact under one approved, exact tolerance exception.
pub fn compare_with_limits(
    reference: &CanonicalRgba8,
    candidate: &CanonicalRgba8,
    whole_limits: ComparisonLimits,
    crops: &[Crop],
    mode: ComparisonMode,
) -> Result<ComparisonReport, ComparisonError> {
    compare_internal(
        reference,
        candidate,
        RegionLimits::Explicit(whole_limits),
        crops,
        mode,
    )
}

fn compare_internal(
    reference: &CanonicalRgba8,
    candidate: &CanonicalRgba8,
    whole_limits: RegionLimits,
    crops: &[Crop],
    mode: ComparisonMode,
) -> Result<ComparisonReport, ComparisonError> {
    if (reference.width(), reference.height()) != (candidate.width(), candidate.height()) {
        return Err(ComparisonError::DimensionMismatch {
            reference: (reference.width(), reference.height()),
            candidate: (candidate.width(), candidate.height()),
        });
    }
    validate_crops(reference, crops)?;

    let whole = compare_region(reference, candidate, whole_limits.resolve(mode))?;
    let crop_reports = crops
        .iter()
        .map(|crop| {
            let reference = extract_crop(reference, crop)?;
            let candidate = extract_crop(candidate, crop)?;
            Ok(CropReport {
                id: crop.id.clone(),
                region: compare_region(&reference, &candidate, crop.limits.resolve(mode))?,
            })
        })
        .collect::<Result<Vec<_>, ComparisonError>>()?;
    Ok(ComparisonReport {
        whole,
        crops: crop_reports,
    })
}

fn validate_crops(raster: &CanonicalRgba8, crops: &[Crop]) -> Result<(), ComparisonError> {
    let mut identities = BTreeSet::new();
    for crop in crops {
        if !identities.insert(&crop.id) {
            return Err(ComparisonError::DuplicateCropId(crop.id.clone()));
        }
        let right = crop.x.checked_add(crop.width);
        let bottom = crop.y.checked_add(crop.height);
        if right.is_none()
            || bottom.is_none()
            || right > Some(raster.width())
            || bottom > Some(raster.height())
        {
            return Err(ComparisonError::CropOutOfBounds {
                id: crop.id.clone(),
                artifact: (raster.width(), raster.height()),
            });
        }
    }
    Ok(())
}

fn extract_crop(raster: &CanonicalRgba8, crop: &Crop) -> Result<CanonicalRgba8, ComparisonError> {
    let row_bytes = usize::try_from(crop.width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or(ComparisonError::MetricInvariant(
            "crop dimensions are too large",
        ))?;
    let crop_height = usize::try_from(crop.height)
        .map_err(|_| ComparisonError::MetricInvariant("crop dimensions are too large"))?;
    let mut rgba = Vec::with_capacity(row_bytes.checked_mul(crop_height).ok_or(
        ComparisonError::MetricInvariant("crop dimensions are too large"),
    )?);
    for y in crop.y..crop.y + crop.height {
        let pixel = u64::from(y) * u64::from(raster.width()) + u64::from(crop.x);
        let start = usize::try_from(pixel * 4)
            .map_err(|_| ComparisonError::MetricInvariant("crop offset is too large"))?;
        rgba.extend_from_slice(&raster.rgba()[start..start + row_bytes]);
    }
    CanonicalRgba8::new(crop.width, crop.height, rgba)
        .map_err(|_| ComparisonError::MetricInvariant("validated crop could not be constructed"))
}

fn compare_region(
    reference: &CanonicalRgba8,
    candidate: &CanonicalRgba8,
    limits: ComparisonLimits,
) -> Result<RegionReport, ComparisonError> {
    let reference_views = reference.views();
    let candidate_views = candidate.views();
    let area = u64::from(reference.width()) * u64::from(reference.height());
    let pixel_count = usize::try_from(area)
        .map_err(|_| ComparisonError::MetricInvariant("region dimensions are too large"))?;
    let mut changed_mask = vec![false; pixel_count];
    for (first, second) in view_pairs(&reference_views, &candidate_views) {
        let result = pixelmatch::compare(
            first,
            second,
            reference.width(),
            reference.height(),
            Options::default(),
        )
        .map_err(|_| ComparisonError::MetricInvariant("valid raster failed Pixelmatch"))?;
        for (union, changed) in changed_mask.iter_mut().zip(result.changed_mask) {
            *union |= changed;
        }
    }

    let changed_pixels = changed_mask.iter().filter(|changed| **changed).count() as u64;
    let ssim = compare_ssim(
        &reference_views,
        &candidate_views,
        reference.width(),
        reference.height(),
    )?;
    let ssim_pass = match ssim {
        SsimOutcome::Score(score) => limits.ssim_pass(score),
        SsimOutcome::SmallArtifactMismatch | SsimOutcome::NonFinite => false,
    };
    Ok(RegionReport {
        width: reference.width(),
        height: reference.height(),
        area,
        changed_pixels,
        changed_mask,
        ssim,
        changed_pixels_pass: limits.changed_pixels_pass(changed_pixels, area),
        ssim_pass,
    })
}

fn compare_ssim(
    reference: &ComparisonViews,
    candidate: &ComparisonViews,
    width: u32,
    height: u32,
) -> Result<SsimOutcome, ComparisonError> {
    if width < 11 || height < 11 {
        return Ok(
            if view_pairs(reference, candidate)
                .into_iter()
                .all(|(first, second)| first == second)
            {
                SsimOutcome::Score(1.0)
            } else {
                SsimOutcome::SmallArtifactMismatch
            },
        );
    }

    let reference_planes = ssim_planes(reference);
    let candidate_planes = ssim_planes(candidate);
    let mut minimum = f64::INFINITY;
    for (first, second) in reference_planes.iter().zip(&candidate_planes) {
        match ssim::compare_planes(first, second, width, height) {
            Ok(score) => {
                if score < minimum {
                    minimum = score;
                }
            }
            Err(SsimError::NonFinite) => return Ok(SsimOutcome::NonFinite),
            Err(_) => return Err(ComparisonError::MetricInvariant("valid raster failed SSIM")),
        }
    }
    Ok(SsimOutcome::Score(minimum))
}

fn view_pairs<'a>(
    reference: &'a ComparisonViews,
    candidate: &'a ComparisonViews,
) -> [(&'a [u8], &'a [u8]); 3] {
    [
        (reference.alpha(), candidate.alpha()),
        (reference.black(), candidate.black()),
        (reference.white(), candidate.white()),
    ]
}

fn ssim_planes(views: &ComparisonViews) -> [Vec<f64>; 3] {
    [
        views
            .alpha()
            .chunks_exact(4)
            .map(|pixel| f64::from(pixel[0]))
            .collect(),
        luma_plane(views.black()),
        luma_plane(views.white()),
    ]
}

fn luma_plane(rgba: &[u8]) -> Vec<f64> {
    let red_coefficient = f64::from_bits(0x3fd3_2119_cb58_2d94);
    let green_coefficient = f64::from_bits(0x3fe2_c59c_7c77_f5b0);
    let blue_coefficient = f64::from_bits(0x3fbd_4eb5_19d2_b9f7);
    rgba.chunks_exact(4)
        .map(|pixel| {
            let luma = red_coefficient * f64::from(pixel[0]);
            let luma = luma + green_coefficient * f64::from(pixel[1]);
            luma + blue_coefficient * f64::from(pixel[2])
        })
        .collect()
}
