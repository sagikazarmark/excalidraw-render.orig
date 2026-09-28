//! GIF assets end to end: the octets a direct SVG passes through, the pixels
//! the PNG target projects over a transparent canvas, the PDF embedding of the
//! flattened first frame and the animation accounting. Decoder behaviour
//! (palettes, disposal, interlacing, the malformed blocks) is tested at the
//! asset cache's own seam in `src/render/asset/tests.rs`.

use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, ResourceLimits};

use crate::common::codec_fixtures::{gif, gif_frame, gif_with_screen, RED_BLUE_PALETTE};
use crate::common::image_scene::{data_url, image_scene};
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request, render_without_background};
use crate::common::svg::{image_symbol, image_use};

#[test]
fn direct_svg_preserves_validated_gif_octets_in_a_canonical_data_url() {
    let gif = gif(&[gif_frame(&[0, 1], 2, 1, &RED_BLUE_PALETTE, None)]);
    let data_url = data_url("image/gif", &gif);

    let outcome = render(&image_scene(&gif, "image/gif", 2, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&image_symbol(0, &data_url)));
    assert!(svg.contains(&image_use(0, "2", "1", "")));
}

#[test]
fn png_projects_gif_palette_transparency_and_frame_offset_on_a_transparent_canvas() {
    let mut first = gif_frame(&[1], 1, 1, &[0, 0, 255, 255, 0, 0], Some(0));
    first.left = 1;
    let gif = gif_with_screen(2, 1, &[], None, &[first]);

    let outcome =
        render_without_background(&image_scene(&gif, "image/gif", 20, 10), OutputFormat::Png);
    let png = decode_png(outcome.artifact().bytes());

    assert_eq!(png.pixel(14, 15), [0, 0, 0, 0]);
    assert_eq!(png.pixel(25, 15), [255, 0, 0, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn pdf_embeds_the_flattened_first_gif_frame_with_a_soft_mask() {
    let gif = gif(&[
        gif_frame(&[0, 1], 2, 1, &RED_BLUE_PALETTE, None),
        gif_frame(&[1, 0], 2, 1, &RED_BLUE_PALETTE, None),
    ]);

    let outcome = render(&image_scene(&gif, "image/gif", 2, 1), OutputFormat::Pdf);
    let pdf = pdf_text(outcome.artifact().bytes());

    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/Width 2\n/Height 1"));
    assert!(pdf.contains("/SMask "));
    assert!(pdf.contains("/x0 Do"));
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        [DiagnosticCode::IMAGE_ANIMATION_FLATTENED]
    );
}

#[test]
fn animation_validation_work_uses_additional_frame_rectangles() {
    let mut second = gif_frame(&[1], 1, 1, &RED_BLUE_PALETTE, None);
    second.left = 1;
    second.top = 1;
    let gif = gif_with_screen(
        2,
        2,
        &[],
        None,
        &[
            gif_frame(&[0, 0, 0, 0], 2, 2, &RED_BLUE_PALETTE, None),
            second,
        ],
    );
    let payload_bytes = u64::try_from(gif.len()).expect("fixture length fits u64");

    for (limit, expected) in [(payload_bytes + 4, 0), (payload_bytes + 3, 1)] {
        let limits = ResourceLimits::builder()
            .max_asset_work_units(limit)
            .build()
            .expect("asset work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");
        let outcome = render_with_request(&image_scene(&gif, "image/gif", 2, 2), &request);
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
fn flattening_is_not_reported_when_png_local_recovery_omits_the_occurrence() {
    let gif = gif(&[
        gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None),
        gif_frame(&[1], 1, 1, &RED_BLUE_PALETTE, None),
    ]);
    let limits = ResourceLimits::builder()
        .max_element_work_units(31)
        .build()
        .expect("element work limit is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let outcome = render_with_request(&image_scene(&gif, "image/gif", 1, 1), &request);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
    );
}
