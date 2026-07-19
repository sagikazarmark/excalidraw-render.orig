use excalidraw_render_compare::{
    compare, compare_with_limits, CanonicalRgba8, ChangedPixelLimit, ComparisonError,
    ComparisonLimits, ComparisonMode, Crop, SsimOutcome, VisualClass,
};

fn solid(width: u32, height: u32, rgba: [u8; 4]) -> CanonicalRgba8 {
    let mut pixels = Vec::new();
    for _ in 0..width * height {
        pixels.extend_from_slice(&rgba);
    }
    CanonicalRgba8::new(width, height, pixels).unwrap()
}

#[test]
fn visual_limits_pass_at_exact_integer_and_binary64_equality() {
    let limits = VisualClass::GeometryImage.limits(ComparisonMode::Canonical);
    assert!(limits.changed_pixels_pass(1, 400));
    assert!(!limits.changed_pixels_pass(2, 400));
    assert_eq!(
        limits.changed_pixel_limit(),
        ChangedPixelLimit::Fraction {
            numerator: 1,
            denominator: 400,
        }
    );
    assert_eq!(limits.minimum_ssim().to_bits(), 0x3fef_f7ce_d916_872b);
    assert!(limits.ssim_pass(limits.minimum_ssim()));
    assert!(!limits.ssim_pass(f64::from_bits(limits.minimum_ssim().to_bits() - 1)));
    assert!(!limits.ssim_pass(f64::NAN));
    assert!(!limits.ssim_pass(f64::INFINITY));

    let rust_typography = VisualClass::Typography.limits(ComparisonMode::PureRustSvg);
    assert_eq!(
        rust_typography.changed_pixel_limit(),
        ChangedPixelLimit::Fraction {
            numerator: 3,
            denominator: 100,
        }
    );
    assert_eq!(
        rust_typography.minimum_ssim().to_bits(),
        0x3fef_ae14_7ae1_47ae
    );
    assert_eq!(
        VisualClass::Typography
            .limits(ComparisonMode::PdfJs)
            .minimum_ssim()
            .to_bits(),
        0x3fef_ae14_7ae1_47ae
    );
}

#[test]
fn approved_tolerance_limits_use_exact_counts_with_no_fractional_margin() {
    let limits = ComparisonLimits::tolerance_exception(7, 0.75).unwrap();
    assert_eq!(limits.changed_pixel_limit(), ChangedPixelLimit::Count(7));
    assert!(limits.changed_pixels_pass(7, 1));
    assert!(!limits.changed_pixels_pass(8, u64::MAX));
    assert_eq!(
        ComparisonLimits::tolerance_exception(0, f64::NAN),
        Err(ComparisonError::InvalidSsimFloor)
    );

    let reference = solid(11, 11, [0, 0, 0, 255]);
    let mut candidate_pixels = reference.rgba().to_vec();
    candidate_pixels[..4].copy_from_slice(&[255, 255, 255, 255]);
    let candidate = CanonicalRgba8::new(11, 11, candidate_pixels).unwrap();
    let report = compare_with_limits(
        &reference,
        &candidate,
        ComparisonLimits::tolerance_exception(1, -1.0).unwrap(),
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    assert!(report.passed());
}

#[test]
fn comparator_unions_views_and_evaluates_whole_and_crops_independently() {
    let reference = solid(22, 11, [0, 0, 0, 255]);
    let mut candidate_pixels = reference.rgba().to_vec();
    for (x, alpha) in [(0usize, 128), (15, 64)] {
        let index = (5 * 22 + x) * 4;
        candidate_pixels[index..index + 4].copy_from_slice(&[255, 0, 0, alpha]);
    }
    let candidate = CanonicalRgba8::new(22, 11, candidate_pixels).unwrap();
    let crop = Crop::new("right", 11, 0, 11, 11, VisualClass::Typography).unwrap();

    let report = compare(
        &reference,
        &candidate,
        VisualClass::Typography,
        &[crop],
        ComparisonMode::Canonical,
    )
    .unwrap();

    assert_eq!(report.whole.changed_pixels, 2);
    assert_eq!(
        report.whole.changed_mask.iter().filter(|bit| **bit).count(),
        2
    );
    assert_eq!(report.crops.len(), 1);
    assert_eq!(report.crops[0].id, "right");
    assert_eq!(report.crops[0].region.changed_pixels, 1);
    assert_eq!(report.crops[0].region.area, 121);
}

#[test]
fn small_complete_artifacts_require_exact_projected_identity_for_ssim() {
    let reference = solid(2, 2, [20, 40, 60, 128]);
    let identical = compare(
        &reference,
        &reference,
        VisualClass::Exact,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    assert_eq!(identical.whole.ssim, SsimOutcome::Score(1.0));
    assert!(identical.passed());

    let candidate = solid(2, 2, [20, 40, 60, 127]);
    let changed = compare(
        &reference,
        &candidate,
        VisualClass::Exact,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    assert_eq!(changed.whole.ssim, SsimOutcome::SmallArtifactMismatch);
    assert!(!changed.whole.ssim_pass);
    assert!(!changed.passed());
}

#[test]
fn alpha_projection_can_be_the_exact_minimum_ssim_view() {
    let reference = solid(11, 11, [200, 100, 50, 128]);
    let candidate = solid(11, 11, [200, 100, 50, 127]);
    let report = compare(
        &reference,
        &candidate,
        VisualClass::Typography,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();

    let SsimOutcome::Score(score) = report.whole.ssim else {
        panic!("11x11 canonical rasters must produce an SSIM score");
    };
    assert_eq!(score.to_bits(), 0x3fef_ffbf_82cd_499b);
}

#[test]
fn comparator_rejects_dimension_mismatch_and_invalid_crops() {
    let reference = solid(11, 11, [0, 0, 0, 255]);
    let other_size = solid(12, 11, [0, 0, 0, 255]);
    assert_eq!(
        compare(
            &reference,
            &other_size,
            VisualClass::Exact,
            &[],
            ComparisonMode::Canonical,
        ),
        Err(ComparisonError::DimensionMismatch {
            reference: (11, 11),
            candidate: (12, 11),
        })
    );

    assert_eq!(
        Crop::new("small", 0, 0, 10, 11, VisualClass::Exact),
        Err(ComparisonError::CropTooSmall {
            id: "small".to_owned(),
            width: 10,
            height: 11,
        })
    );

    let outside = Crop::new("outside", 1, 0, 11, 11, VisualClass::Exact).unwrap();
    assert_eq!(
        compare(
            &reference,
            &reference,
            VisualClass::Exact,
            &[outside],
            ComparisonMode::Canonical,
        ),
        Err(ComparisonError::CropOutOfBounds {
            id: "outside".to_owned(),
            artifact: (11, 11),
        })
    );
}

#[test]
fn crops_become_pixelmatch_image_boundaries_before_aa_detection() {
    let mut reference_pixels = Vec::new();
    let mut candidate_pixels = Vec::new();
    for y in 0..11 {
        for x in 0..13 {
            let reference_value = match x {
                0 | 1 => 0,
                2 => 128,
                3 if y == 5 => 255,
                3 => 200,
                _ => 255,
            };
            let candidate_value = if x == 2 && y == 5 {
                200
            } else {
                reference_value
            };
            reference_pixels.extend_from_slice(&[
                reference_value,
                reference_value,
                reference_value,
                255,
            ]);
            candidate_pixels.extend_from_slice(&[
                candidate_value,
                candidate_value,
                candidate_value,
                255,
            ]);
        }
    }
    let reference = CanonicalRgba8::new(13, 11, reference_pixels).unwrap();
    let candidate = CanonicalRgba8::new(13, 11, candidate_pixels).unwrap();
    let crop = Crop::new("right-edge", 2, 0, 11, 11, VisualClass::Typography).unwrap();

    let report = compare(
        &reference,
        &candidate,
        VisualClass::Typography,
        &[crop],
        ComparisonMode::Canonical,
    )
    .unwrap();

    assert_eq!(report.whole.changed_pixels, 0);
    assert_eq!(report.crops[0].region.changed_pixels, 1);
}
