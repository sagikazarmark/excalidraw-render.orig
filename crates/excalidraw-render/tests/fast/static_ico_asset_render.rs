use std::io::Cursor;

use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, RenderRequest, Renderer, ResourceLimits,
    SceneDocument,
};

#[test]
fn ico_selects_largest_area_before_highest_depth() {
    let small_blue = fixture_png(1, 1, [0, 0, 255, 255]);
    let large_red = fixture_png(2, 1, [255, 0, 0, 255]);
    let ico = fixture_ico(&[(1, 1, 32, small_blue), (2, 1, 8, large_red)]);
    let outcome = render_outcome(&image_scene(&ico, 20, 10), OutputFormat::Png);
    let rgba = decoded_png(outcome.artifact().bytes());

    assert_eq!(pixel(&rgba, 40, 15, 15), [255, 0, 0, 255]);
    assert_eq!(pixel(&rgba, 40, 25, 15), [255, 0, 0, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn ico_uses_highest_depth_for_equal_area_and_first_directory_entry_for_exact_ties() {
    let blue = fixture_png(1, 1, [0, 0, 255, 255]);
    let red = fixture_png(1, 1, [255, 0, 0, 255]);
    let green = fixture_png(1, 1, [0, 255, 0, 255]);
    let ico = fixture_ico(&[(1, 1, 8, blue), (1, 1, 32, red), (1, 1, 32, green)]);

    assert_eq!(rendered_center(&ico), [255, 0, 0, 255]);
}

#[test]
fn direct_svg_preserves_validated_ico_octets_with_canonical_mime() {
    let ico = fixture_ico(&[(1, 1, 32, fixture_png(1, 1, [24, 96, 208, 255]))]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&ico);
    let outcome = render_outcome(&image_scene(&ico, 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(svg.contains(&format!("data:image/x-icon;base64,{encoded}")));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn ico_dib_uses_meaningful_alpha_and_legacy_and_mask_fallback() {
    let translucent = fixture_ico(&[(1, 1, 32, fixture_dib32([24, 96, 208, 128], false))]);
    let masked = fixture_ico(&[(1, 1, 32, fixture_dib32([24, 96, 208, 0], true))]);

    assert_eq!(rendered_center(&translucent), [24, 96, 207, 128]);
    assert_eq!(rendered_center(&masked), [0, 0, 0, 0]);
}

#[test]
fn malformed_non_selected_ico_entry_uses_typed_decode_recovery() {
    let valid = fixture_png(2, 1, [255, 0, 0, 255]);
    let mut malformed = fixture_png(1, 1, [0, 0, 255, 255]);
    let last = malformed.len() - 1;
    malformed[last] ^= 1;
    let ico = fixture_ico(&[(2, 1, 32, valid), (1, 1, 8, malformed)]);
    let outcome = render_outcome(&image_scene(&ico, 2, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    let diagnostic = &outcome.report().diagnostics()[0];
    assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_DECODE_FAILED);
    assert!(matches!(
        diagnostic.details(),
        Some(ErrorDetails::Asset { validated_media_type: Some(media_type), .. })
            if media_type == "image/x-icon"
    ));
}

#[test]
fn ico_selected_dimensions_control_decode_resource_limits() {
    let ico = fixture_ico(&[(2, 1, 32, fixture_png(2, 1, [255, 0, 0, 255]))]);
    let limits = ResourceLimits::builder()
        .max_decoded_pixels_per_asset(1)
        .build()
        .expect("resource limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");
    let outcome = Renderer::new()
        .render(&image_scene(&ico, 2, 1), &request)
        .expect("ICO pixel limit has local recovery");

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_IMAGE_PIXELS_EXCEEDED
    );
}

#[test]
fn ico_pdf_and_repeated_artifacts_are_deterministic() {
    let ico = fixture_ico(&[(1, 1, 32, fixture_dib32([24, 96, 208, 128], false))]);
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let scene = image_scene(&ico, 1, 1);
        let first = render_outcome(&scene, format);
        let second = render_outcome(&scene, format);
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(
            serde_json::to_vec(first.report()).expect("report serializes"),
            serde_json::to_vec(second.report()).expect("report serializes")
        );
        if format == OutputFormat::Pdf {
            let pdf = String::from_utf8_lossy(first.artifact().bytes());
            assert!(pdf.contains("/Width 1\n/Height 1"));
            assert!(pdf.contains("/SMask"));
        }
    }
}

#[test]
fn ico_rejects_overlapping_dib_bitfield_masks() {
    let ico = fixture_ico(&[(1, 1, 32, fixture_dib_with_overlapping_masks())]);
    let outcome = render_outcome(&image_scene(&ico, 1, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_DECODE_FAILED
    );
}

fn fixture_png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("PNG header encodes");
    writer
        .write_image_data(&color.repeat((width * height) as usize))
        .expect("PNG pixels encode");
    drop(writer);
    bytes
}

fn fixture_ico(images: &[(u16, u16, u16, Vec<u8>)]) -> Vec<u8> {
    let directory_size = 6 + images.len() * 16;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = directory_size;
    for (width, height, depth, image) in images {
        bytes.push(if *width == 256 { 0 } else { *width as u8 });
        bytes.push(if *height == 256 { 0 } else { *height as u8 });
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&depth.to_le_bytes());
        bytes.extend_from_slice(&(image.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image.len();
    }
    for (_, _, _, image) in images {
        bytes.extend_from_slice(image);
    }
    bytes
}

fn fixture_dib32(pixel: [u8; 4], transparent_mask: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    bytes.extend_from_slice(&[if transparent_mask { 0x80 } else { 0 }, 0, 0, 0]);
    bytes
}

fn fixture_dib_with_overlapping_masks() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&0x00ff_0000_u32.to_le_bytes());
    bytes.extend_from_slice(&0x00ff_0000_u32.to_le_bytes());
    bytes.extend_from_slice(&0x0000_00ff_u32.to_le_bytes());
    bytes.extend_from_slice(&[208, 96, 24, 255]);
    bytes.extend_from_slice(&[0; 4]);
    bytes
}

fn rendered_center(ico: &[u8]) -> [u8; 4] {
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .build()
        .expect("PNG request is valid");
    let outcome = Renderer::new()
        .render(&image_scene(ico, 1, 1), &request)
        .expect("ICO alpha renders");
    pixel(&decoded_png(outcome.artifact().bytes()), 21, 10, 10)
}

fn image_scene(ico: &[u8], width: u32, height: u32) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(ico);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","x":0,"y":0,
                "width":{width},"height":{height},"angle":0,"opacity":100,
                "fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1
            }}],
            "appState":{{}},
            "files":{{"asset":{{
                "id":"asset","mimeType":"image/x-icon",
                "dataURL":"data:image/x-icon;base64,{encoded}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("ICO scene is admitted")
}

fn render_outcome(scene: &SceneDocument, format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("render request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("ICO asset has local recovery")
}

fn decoded_png(bytes: &[u8]) -> Vec<u8> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    rgba
}

fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    rgba[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel has four channels")
}
