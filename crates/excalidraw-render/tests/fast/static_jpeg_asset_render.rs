//! JPEG assets end to end: the octets a direct SVG passes through, the pixels
//! the PNG target projects, the lossless PDF embedding, the session's claim
//! diagnostics and the SVG target's pixel accounting. Decoder behaviour (EXIF,
//! ICC, gain-map containers, the malformed streams) is tested at the asset
//! cache's own seam in `src/render/asset/tests.rs`.

use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, RenderRequest, ResourceLimits,
};

use crate::common::codec_fixtures::{jpeg_rgb, JFIF_BLUE};
use crate::common::image_scene::{data_url, image_scene, image_scene_from_data_url};
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request};

#[test]
fn direct_svg_preserves_validated_jfif_octets_in_a_canonical_data_url() {
    let jpeg = jpeg_rgb(1, 1, &[JFIF_BLUE]);
    let data_url = data_url("image/jpeg", &jpeg);

    let outcome = render(&image_scene(&jpeg, "image/jpeg", 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"{data_url}\" preserveAspectRatio=\"none\" width=\"1\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_jpeg_as_opaque_canonical_srgb() {
    let scene = image_scene(&jpeg_rgb(1, 1, &[JFIF_BLUE]), "image/jpeg", 1, 1);

    let outcome = render(&scene, OutputFormat::Png);
    let decoded = decode_png(outcome.artifact().bytes());

    assert_eq!((decoded.width, decoded.height), (21, 21));
    let center = decoded.pixel(10, 10);
    assert!(center[0].abs_diff(24) <= 3, "red channel: {}", center[0]);
    assert!(center[1].abs_diff(96) <= 3, "green channel: {}", center[1]);
    assert!(center[2].abs_diff(208) <= 3, "blue channel: {}", center[2]);
    assert_eq!(center[3], 255);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn pdf_embeds_normalized_jpeg_losslessly_with_a_soft_mask_and_no_fallback() {
    let scene = image_scene(&jpeg_rgb(1, 1, &[JFIF_BLUE]), "image/jpeg", 1, 1);

    let outcome = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/ColorSpace /CS1"));
    assert!(pdf.contains("/ColorSpace /DeviceGray"));
    assert!(pdf.contains("/SMask "));
    assert!(pdf.contains("/Im0 Do"));
    assert!(!pdf.contains("/DCTDecode"));
    assert!(!pdf.contains("pdf.raster-fallback"));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn sniffed_jpeg_type_wins_over_jfif_and_png_claims() {
    let jpeg = jpeg_rgb(1, 1, &[JFIF_BLUE]);
    let jfif_url = data_url("image/jfif", &jpeg);
    let scene = image_scene_from_data_url(&jfif_url, "image/png", 1, 1);

    let outcome = render(&scene, OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 2);
    for diagnostic in outcome.report().diagnostics() {
        assert_eq!(diagnostic.code(), DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH);
        assert!(matches!(
            diagnostic.details(),
            Some(ErrorDetails::Asset {
                validated_media_type: Some(validated),
                ..
            }) if validated == "image/jpeg"
        ));
    }
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&data_url("image/jpeg", &jpeg)));
}

#[test]
fn direct_svg_validates_jpeg_without_charging_decoded_pixels() {
    let jpeg = jpeg_rgb(2, 1, &[[255, 0, 0], [0, 0, 255]]);
    let scene = image_scene(&jpeg, "image/jpeg", 2, 1);
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

    let outcome = render_with_request(&scene, &request);

    assert!(outcome.report().diagnostics().is_empty());
    assert!(std::str::from_utf8(outcome.artifact().bytes())
        .expect("SVG is UTF-8")
        .contains("<image "));
}
