//! BMP assets end to end: the octets a direct SVG passes through, the pixels
//! the PNG target projects, the PDF embedding, the session's typed decode
//! recovery under the sniffed media type and the validation accounting.
//! Decoder behaviour (row order, the alpha readings, RLE8, the malformed
//! headers) is tested at the asset cache's own seam in
//! `src/render/asset/tests.rs`.

use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, RenderRequest, ResourceLimits,
};

use crate::common::codec_fixtures::{bmp24, bmp32, bmp_rle8};
use crate::common::image_scene::{data_url, image_scene};
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request};
use crate::common::svg::{image_symbol, image_use};

#[test]
fn direct_svg_preserves_validated_bmp_octets_in_a_canonical_data_url() {
    let bmp = bmp24(1, 1, &[[24, 96, 208]]);
    let data_url = data_url("image/bmp", &bmp);

    let outcome = render(&image_scene(&bmp, "image/bmp", 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&image_symbol(0, &data_url)));
    assert!(svg.contains(&image_use(0, "1", "1", "")));
}

#[test]
fn png_projects_bottom_up_bmp_rows_as_canonical_rgba() {
    let bmp = bmp24(1, 2, &[[255, 0, 0], [0, 0, 255]]);

    let outcome = render(&image_scene(&bmp, "image/bmp", 10, 20), OutputFormat::Png);
    let png = decode_png(outcome.artifact().bytes());

    assert_eq!(png.pixel(15, 14), [255, 0, 0, 255]);
    assert_eq!(png.pixel(15, 25), [0, 0, 255, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn bmp_pdf_embeds_the_canonical_selected_surface() {
    let bmp = bmp32([24, 96, 208, 128], true);

    let outcome = render(&image_scene(&bmp, "image/bmp", 1, 1), OutputFormat::Pdf);
    let pdf = pdf_text(outcome.artifact().bytes());

    assert!(pdf.contains("/Width 1\n/Height 1"));
    assert!(pdf.contains("/SMask"));
    assert!(outcome.report().diagnostics().is_empty());
}

/// The session reports a typed decode failure under the media type the asset
/// cache sniffed, not the one the record claimed, and reports nothing else.
#[test]
fn malformed_bmp_uses_typed_decode_recovery() {
    let mut bmp = bmp24(1, 1, &[[24, 96, 208]]);
    bmp.truncate(bmp.len() - 1);

    let outcome = render(&image_scene(&bmp, "image/png", 1, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    let diagnostic = &outcome.report().diagnostics()[0];
    assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_DECODE_FAILED);
    assert!(matches!(
        diagnostic.details(),
        Some(ErrorDetails::Asset { validated_media_type: Some(media_type), .. })
            if media_type == "image/bmp"
    ));
}

#[test]
fn direct_svg_validates_bmp_without_charging_decoded_pixels() {
    let bmp = bmp24(2, 1, &[[255, 0, 0], [0, 0, 255]]);
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

    let outcome = render_with_request(&image_scene(&bmp, "image/bmp", 2, 1), &request);

    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn rle8_validation_work_is_prepaid_at_eight_units_over_the_payload() {
    let bmp = bmp_rle8();
    let required = u64::try_from(bmp.len()).expect("fixture length fits u64") + 8;

    for (limit, expected) in [(required, 0), (required - 1, 1)] {
        let limits = ResourceLimits::builder()
            .max_asset_work_units(limit)
            .build()
            .expect("asset work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");
        let outcome = render_with_request(&image_scene(&bmp, "image/bmp", 2, 1), &request);
        assert_eq!(outcome.report().diagnostics().len(), expected);
        if expected == 1 {
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED
            );
        }
    }
}
