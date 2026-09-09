use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, RenderRequest, Renderer, ResourceLimits,
};

use crate::common::image_scene::image_scene;
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request};

#[test]
fn direct_svg_preserves_validated_bmp_octets_in_a_canonical_data_url() {
    let bmp = fixture_bmp24(1, 1, &[[24, 96, 208]]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bmp);
    let outcome = render(&image_scene(&bmp, "image/bmp", 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"data:image/bmp;base64,{encoded}\" preserveAspectRatio=\"none\" width=\"1\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_bottom_up_bmp_rows_as_canonical_rgba() {
    let bmp = fixture_bmp24(1, 2, &[[255, 0, 0], [0, 0, 255]]);
    let outcome = render(&image_scene(&bmp, "image/bmp", 10, 20), OutputFormat::Png);
    let png = decode_png(outcome.artifact().bytes());

    assert_eq!(png.pixel(15, 14), [255, 0, 0, 255]);
    assert_eq!(png.pixel(15, 25), [0, 0, 255, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn legacy_bi_rgb_32_ignores_reserved_alpha_while_alpha_bitfields_preserve_it() {
    let legacy = fixture_bmp32([24, 96, 208, 0], false);
    let bitfields = fixture_bmp32([24, 96, 208, 128], true);

    assert_eq!(rendered_center(&legacy), [24, 96, 208, 255]);
    assert_eq!(rendered_center(&bitfields), [24, 96, 207, 128]);
}

#[test]
fn bmp_pdf_embeds_the_canonical_selected_surface() {
    let bmp = fixture_bmp32([24, 96, 208, 128], true);
    let outcome = render(&image_scene(&bmp, "image/bmp", 1, 1), OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/Width 1\n/Height 1"));
    assert!(pdf.contains("/SMask"));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn malformed_bmp_uses_typed_decode_recovery() {
    let mut bmp = fixture_bmp24(1, 1, &[[24, 96, 208]]);
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
    let bmp = fixture_bmp24(2, 1, &[[255, 0, 0], [0, 0, 255]]);
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

    let outcome = Renderer::new()
        .render(&image_scene(&bmp, "image/bmp", 2, 1), &request)
        .expect("direct SVG validates BMP without pixel derivation");
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn rle8_delta_pixels_use_palette_zero_and_validation_work_is_prepaid() {
    let bmp = fixture_bmp_rle8();
    let outcome = render(&image_scene(&bmp, "image/bmp", 20, 10), OutputFormat::Png);
    let png = decode_png(outcome.artifact().bytes());
    assert_eq!(png.pixel(14, 15), [255, 0, 0, 255]);
    assert_eq!(png.pixel(25, 15), [0, 0, 255, 255]);

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
        let outcome = Renderer::new()
            .render(&image_scene(&bmp, "image/bmp", 2, 1), &request)
            .expect("RLE validation has local recovery");
        assert_eq!(outcome.report().diagnostics().len(), expected);
    }
}

#[test]
fn alphabitfields_requires_storage_for_the_alpha_mask() {
    let mut bmp = fixture_bmp32([24, 96, 208, 128], true);
    bmp[30..34].copy_from_slice(&6_u32.to_le_bytes());
    bmp.drain(66..70);
    let length = bmp.len() as u32;
    bmp[2..6].copy_from_slice(&length.to_le_bytes());
    bmp[10..14].copy_from_slice(&66_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&52_u32.to_le_bytes());

    let outcome = render(&image_scene(&bmp, "image/bmp", 1, 1), OutputFormat::Svg);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_DECODE_FAILED
    );
}

fn fixture_bmp24(width: u32, height: u32, pixels: &[[u8; 3]]) -> Vec<u8> {
    let row_bytes = width * 3;
    let stride = row_bytes.div_ceil(4) * 4;
    let pixel_bytes = stride * height;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(54 + pixel_bytes).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&(width as i32).to_le_bytes());
    bytes.extend_from_slice(&(height as i32).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&24_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&pixel_bytes.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    for y in (0..height as usize).rev() {
        for [red, green, blue] in &pixels[y * width as usize..(y + 1) * width as usize] {
            bytes.extend_from_slice(&[*blue, *green, *red]);
        }
        bytes.resize(bytes.len() + (stride - row_bytes) as usize, 0);
    }
    bytes
}

fn fixture_bmp32(pixel: [u8; 4], alpha_bitfields: bool) -> Vec<u8> {
    let dib_size = if alpha_bitfields { 56_u32 } else { 40 };
    let pixel_offset = 14 + dib_size;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(pixel_offset + 4).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&pixel_offset.to_le_bytes());
    bytes.extend_from_slice(&dib_size.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&(if alpha_bitfields { 3_u32 } else { 0 }).to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    if alpha_bitfields {
        bytes.extend_from_slice(&0x00ff_0000_u32.to_le_bytes());
        bytes.extend_from_slice(&0x0000_ff00_u32.to_le_bytes());
        bytes.extend_from_slice(&0x0000_00ff_u32.to_le_bytes());
        bytes.extend_from_slice(&0xff00_0000_u32.to_le_bytes());
    }
    bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    bytes
}

fn fixture_bmp_rle8() -> Vec<u8> {
    let encoded = [0, 2, 1, 0, 1, 1, 0, 1];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(62_u32 + encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&62_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    bytes.extend_from_slice(&encoded);
    bytes
}

fn rendered_center(bmp: &[u8]) -> [u8; 4] {
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .build()
        .expect("PNG request is valid");
    let outcome = render_with_request(&image_scene(bmp, "image/bmp", 1, 1), &request);
    decode_png(outcome.artifact().bytes()).pixel(10, 10)
}
