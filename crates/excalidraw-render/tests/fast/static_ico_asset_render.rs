//! ICO assets end to end: the octets a direct SVG passes through under the
//! canonical `image/x-icon`, the selected entry the PNG target projects, the
//! PDF embedding with the DIB's alpha as a soft mask and the selected entry's
//! pixel accounting. Entry selection, DIB alpha and the malformed entries are
//! tested at the asset cache's own seam in `src/render/asset/tests.rs`.

use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, ResourceLimits};

use crate::common::codec_fixtures::{dib32, ico, solid_png};
use crate::common::image_scene::{data_url, image_scene};
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request};

#[test]
fn direct_svg_preserves_validated_ico_octets_with_canonical_mime() {
    let ico = ico(&[(1, 1, 32, solid_png(1, 1, [24, 96, 208, 255]))]);
    let data_url = data_url("image/x-icon", &ico);

    let outcome = render(&image_scene(&ico, "image/x-icon", 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(svg.contains(&data_url));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn png_projects_the_selected_largest_ico_entry() {
    let small_blue = solid_png(1, 1, [0, 0, 255, 255]);
    let large_red = solid_png(2, 1, [255, 0, 0, 255]);
    let ico = ico(&[(1, 1, 32, small_blue), (2, 1, 8, large_red)]);

    let outcome = render(
        &image_scene(&ico, "image/x-icon", 20, 10),
        OutputFormat::Png,
    );
    let png = decode_png(outcome.artifact().bytes());

    assert_eq!(png.pixel(15, 15), [255, 0, 0, 255]);
    assert_eq!(png.pixel(25, 15), [255, 0, 0, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn ico_embeds_in_pdf_as_an_image_with_its_alpha_mask() {
    let ico = ico(&[(1, 1, 32, dib32([24, 96, 208, 128], false))]);

    let outcome = render(&image_scene(&ico, "image/x-icon", 1, 1), OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/Width 1\n/Height 1"));
    assert!(pdf.contains("/SMask"));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn ico_selected_dimensions_control_decode_resource_limits() {
    let ico = ico(&[(2, 1, 32, solid_png(2, 1, [255, 0, 0, 255]))]);
    let limits = ResourceLimits::builder()
        .max_decoded_pixels_per_asset(1)
        .build()
        .expect("resource limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let outcome = render_with_request(&image_scene(&ico, "image/x-icon", 2, 1), &request);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_IMAGE_PIXELS_EXCEEDED
    );
}
