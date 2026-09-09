//! WebP assets end to end: the octets a direct SVG passes through, the
//! straight alpha the PNG target projects, the PDF embedding with its soft mask
//! and the animation and pixel accounting. Decoder behaviour (lossy streams,
//! EXIF, ICC, animation frames, the malformed containers) is tested at the
//! asset cache's own seam in `src/render/asset/tests.rs`.

use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, ResourceLimits};

use crate::common::codec_fixtures::{animated_webp, webp_rgba};
use crate::common::image_scene::{data_url, image_scene};
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request, render_without_background};

#[test]
fn direct_svg_preserves_validated_lossless_webp_octets_in_a_canonical_data_url() {
    let webp = webp_rgba(1, 1, &[[24, 96, 208, 128]]);
    let data_url = data_url("image/webp", &webp);

    let outcome = render(&image_scene(&webp, "image/webp", 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"{data_url}\" preserveAspectRatio=\"none\" width=\"1\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_lossless_webp_alpha_as_canonical_rgba() {
    let webp = webp_rgba(2, 1, &[[24, 96, 208, 128], [200, 100, 50, 0]]);

    let outcome =
        render_without_background(&image_scene(&webp, "image/webp", 20, 10), OutputFormat::Png);
    let decoded = decode_png(outcome.artifact().bytes());

    let translucent = decoded.pixel(14, 15);
    assert!(translucent[0].abs_diff(24) <= 1, "red: {translucent:?}");
    assert!(translucent[1].abs_diff(96) <= 1, "green: {translucent:?}");
    assert!(translucent[2].abs_diff(208) <= 1, "blue: {translucent:?}");
    assert!(translucent[3].abs_diff(128) <= 1, "alpha: {translucent:?}");
    assert_eq!(decoded.pixel(25, 15), [0, 0, 0, 0]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn pdf_embeds_lossless_webp_with_its_alpha_as_a_soft_mask() {
    let webp = webp_rgba(2, 1, &[[24, 96, 208, 128], [200, 100, 50, 0]]);

    let outcome = render(&image_scene(&webp, "image/webp", 2, 1), OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/Width 2\n/Height 1"));
    assert!(pdf.contains("/SMask "));
    assert!(pdf.contains("/Im0 Do"));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn animation_validation_work_uses_additional_webp_frame_rectangles() {
    let first = webp_rgba(2, 2, &[[255, 0, 0, 255]; 4]);
    let second = webp_rgba(1, 1, &[[0, 0, 255, 255]]);
    let webp = animated_webp(2, 2, &[(0, 0, first), (1, 1, second)]);
    let payload_bytes = u64::try_from(webp.len()).expect("fixture length fits u64");

    for (limit, expected) in [(payload_bytes + 4, 0), (payload_bytes + 3, 1)] {
        let limits = ResourceLimits::builder()
            .max_asset_work_units(limit)
            .build()
            .expect("asset work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");
        let outcome = render_with_request(&image_scene(&webp, "image/webp", 2, 2), &request);
        assert_eq!(outcome.report().diagnostics().len(), expected);
        if expected == 1 {
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED
            );
        }
    }
}

#[test]
fn direct_svg_validates_webp_without_charging_decoded_pixels() {
    let webp = webp_rgba(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let limits = ResourceLimits::builder()
        .max_raster_dimension(1)
        .max_decoded_pixels_per_asset(1)
        .max_total_decoded_pixels(1)
        .build()
        .expect("resource limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let outcome = render_with_request(&image_scene(&webp, "image/webp", 2, 1), &request);

    assert!(outcome.report().diagnostics().is_empty());
}
