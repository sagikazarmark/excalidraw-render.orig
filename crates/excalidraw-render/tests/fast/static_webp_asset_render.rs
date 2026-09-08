use std::io::Cursor;

use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, Recovery, RenderRequest, Renderer, ResourceLimits,
    SceneDocument,
};
use image_webp::{ColorType, WebPEncoder};

#[test]
fn direct_svg_preserves_validated_lossless_webp_octets_in_a_canonical_data_url() {
    let webp = fixture_webp(1, 1, &[[24, 96, 208, 128]]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&webp);
    let outcome = render_outcome(&webp_scene(&webp, 1, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"data:image/webp;base64,{encoded}\" preserveAspectRatio=\"none\" width=\"1\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_lossless_webp_alpha_as_canonical_rgba() {
    let webp = fixture_webp(2, 1, &[[24, 96, 208, 128], [200, 100, 50, 0]]);
    let scene = webp_scene(&webp, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .build()
        .expect("PNG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("WebP alpha renders");
    let rgba = decoded_png(outcome.artifact().bytes());

    let translucent = pixel(&rgba, 40, 14, 15);
    assert!(translucent[0].abs_diff(24) <= 1, "red: {translucent:?}");
    assert!(translucent[1].abs_diff(96) <= 1, "green: {translucent:?}");
    assert!(translucent[2].abs_diff(208) <= 1, "blue: {translucent:?}");
    assert!(translucent[3].abs_diff(128) <= 1, "alpha: {translucent:?}");
    assert_eq!(pixel(&rgba, 40, 25, 15), [0, 0, 0, 0]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn png_and_pdf_flatten_animated_webp_per_rendered_occurrence() {
    let first = fixture_webp(1, 1, &[[255, 0, 0, 255]]);
    let second = fixture_webp(1, 1, &[[0, 0, 255, 255]]);
    let webp = animated_webp(1, 1, &[(0, 0, first), (0, 0, second)]);
    let scene = two_webp_scene(&webp);

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render_outcome(&scene, format);
        assert_eq!(outcome.report().diagnostics().len(), 2);
        for diagnostic in outcome.report().diagnostics() {
            assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_ANIMATION_FLATTENED);
            assert!(matches!(
                diagnostic.details(),
                Some(ErrorDetails::Asset {
                    validated_media_type: Some(media_type),
                    recovery: Recovery::Flattened,
                    ..
                }) if media_type == "image/webp"
            ));
        }
        if format == OutputFormat::Png {
            let rgba = decoded_png(outcome.artifact().bytes());
            assert_eq!(pixel(&rgba, 23, 10, 10), [255, 0, 0, 255]);
            assert_eq!(pixel(&rgba, 23, 12, 10), [255, 0, 0, 255]);
        }
    }

    let svg = render_outcome(&scene, OutputFormat::Svg);
    assert!(svg.report().diagnostics().is_empty());
}

#[test]
fn animation_validation_work_uses_additional_webp_frame_rectangles() {
    let first = fixture_webp(2, 2, &[[255, 0, 0, 255]; 4]);
    let second = fixture_webp(1, 1, &[[0, 0, 255, 255]]);
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
        let outcome = Renderer::new()
            .render(&webp_scene(&webp, 2, 2), &request)
            .expect("WebP validation has local recovery");
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
fn lossy_webp_decodes_to_opaque_canonical_srgb() {
    let webp = lossy_red_webp();
    let outcome = render_outcome(&webp_scene(&webp, 2, 2), OutputFormat::Png);
    let center = pixel(&decoded_png(outcome.artifact().bytes()), 22, 11, 11);

    assert_eq!(center, [255, 0, 2, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn static_and_animated_webp_artifacts_and_reports_are_deterministic() {
    let static_webp = fixture_webp(1, 1, &[[24, 96, 208, 128]]);
    let animated = animated_webp(
        1,
        1,
        &[
            (0, 0, fixture_webp(1, 1, &[[255, 0, 0, 255]])),
            (0, 0, fixture_webp(1, 1, &[[0, 0, 255, 255]])),
        ],
    );

    for webp in [&static_webp, &animated] {
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let scene = webp_scene(webp, 1, 1);
            let first = render_outcome(&scene, format);
            let second = render_outcome(&scene, format);
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(
                serde_json::to_vec(first.report()).expect("report serializes"),
                serde_json::to_vec(second.report()).expect("report serializes")
            );
        }
    }
}

#[test]
fn png_and_pdf_apply_webp_exif_orientation_while_svg_preserves_source_bytes() {
    let pixels = [
        [255, 0, 0, 255],
        [255, 0, 0, 255],
        [0, 0, 255, 255],
        [0, 0, 255, 255],
    ];
    let webp = fixture_webp_with_metadata(4, 1, &pixels, None, Some(exif_orientation_bytes(6)));
    let encoded = base64::engine::general_purpose::STANDARD.encode(&webp);
    let scene = webp_scene(&webp, 10, 20);

    let svg = render_outcome(&scene, OutputFormat::Svg);
    assert!(std::str::from_utf8(svg.artifact().bytes())
        .expect("SVG is UTF-8")
        .contains(&format!("data:image/webp;base64,{encoded}")));

    let png = render_outcome(&scene, OutputFormat::Png);
    let rgba = decoded_png(png.artifact().bytes());
    let top = pixel(&rgba, 30, 15, 14);
    let bottom = pixel(&rgba, 30, 15, 25);
    assert!(top[0] > top[2], "top pixel: {top:?}");
    assert!(bottom[2] > bottom[0], "bottom pixel: {bottom:?}");

    let pdf = render_outcome(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/Width 1\n/Height 4"));
}

#[test]
fn webp_icc_profile_takes_precedence_over_default_srgb_interpretation() {
    let pixels = [[128, 128, 128, 255]];
    let untagged = fixture_webp(1, 1, &pixels);
    let tagged = fixture_webp_with_metadata(1, 1, &pixels, Some(linear_rgb_icc_profile()), None);
    let untagged = rendered_center(&untagged);
    let normalized = rendered_center(&tagged);

    assert!(untagged[0].abs_diff(128) <= 1, "untagged: {untagged:?}");
    assert!(
        normalized[0].abs_diff(188) <= 2,
        "ICC pixel: {normalized:?}"
    );
    assert_eq!(normalized[0], normalized[1]);
    assert_eq!(normalized[1], normalized[2]);
}

#[test]
fn malformed_and_truncated_webp_use_typed_decode_recovery() {
    let mut truncated = fixture_webp(1, 1, &[[24, 96, 208, 255]]);
    truncated.truncate(truncated.len() - 2);
    let malformed = b"RIFF\x04\0\0\0WEBP".to_vec();

    for webp in [truncated, malformed] {
        let outcome = render_outcome(&webp_scene(&webp, 1, 1), OutputFormat::Svg);
        assert_eq!(outcome.report().diagnostics().len(), 1);
        assert_eq!(
            outcome.report().diagnostics()[0].code(),
            DiagnosticCode::IMAGE_DECODE_FAILED
        );
        assert!(matches!(
            outcome.report().diagnostics()[0].details(),
            Some(ErrorDetails::Asset {
                validated_media_type: Some(media_type),
                ..
            }) if media_type == "image/webp"
        ));
    }
}

#[test]
fn malformed_later_webp_frames_and_alpha_flags_use_decode_recovery() {
    let first = fixture_webp(1, 1, &[[255, 0, 0, 255]]);
    let second = fixture_webp(1, 1, &[[0, 0, 255, 128]]);
    let animated = animated_webp(1, 1, &[(0, 0, first), (0, 0, second)]);
    let second_frame = animated
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == b"ANMF")
        .nth(1)
        .map(|(offset, _)| offset + 8)
        .expect("fixture has a second animation frame");

    let mut outside_canvas = animated.clone();
    outside_canvas[second_frame] = 1;
    let mut reserved_frame_flag = animated.clone();
    reserved_frame_flag[second_frame + 15] |= 0x80;
    let mut missing_alpha_flag = animated;
    missing_alpha_flag[20] &= !0x10;

    for webp in [outside_canvas, reserved_frame_flag, missing_alpha_flag] {
        let outcome = render_outcome(&webp_scene(&webp, 1, 1), OutputFormat::Svg);
        assert_eq!(outcome.report().diagnostics().len(), 1);
        assert_eq!(
            outcome.report().diagnostics()[0].code(),
            DiagnosticCode::IMAGE_DECODE_FAILED
        );
    }
}

#[test]
fn animated_webp_first_frame_alpha_is_canonicalized() {
    let first = fixture_webp(1, 1, &[[24, 96, 208, 128]]);
    let second = fixture_webp(1, 1, &[[0, 0, 255, 255]]);
    let webp = animated_webp(1, 1, &[(0, 0, first), (0, 0, second)]);
    let scene = webp_scene(&webp, 10, 10);
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .build()
        .expect("PNG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("animated WebP alpha renders");

    assert_eq!(
        pixel(&decoded_png(outcome.artifact().bytes()), 30, 15, 15),
        [24, 96, 207, 128]
    );
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_ANIMATION_FLATTENED
    );
}

#[test]
fn direct_svg_validates_webp_without_charging_decoded_pixels() {
    let webp = fixture_webp(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
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
        .render(&webp_scene(&webp, 2, 1), &request)
        .expect("direct SVG validates compressed WebP without pixel derivation");

    assert!(outcome.report().diagnostics().is_empty());
}

fn fixture_webp(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let rgba = pixels.iter().flatten().copied().collect::<Vec<_>>();
    let mut bytes = Vec::new();
    WebPEncoder::new(&mut bytes)
        .encode(&rgba, width, height, ColorType::Rgba8)
        .expect("WebP fixture encodes");
    bytes
}

fn fixture_webp_with_metadata(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
) -> Vec<u8> {
    let rgba = pixels.iter().flatten().copied().collect::<Vec<_>>();
    let mut bytes = Vec::new();
    let mut encoder = WebPEncoder::new(&mut bytes);
    if let Some(icc) = icc {
        encoder.set_icc_profile(icc);
    }
    if let Some(exif) = exif {
        encoder.set_exif_metadata(exif);
    }
    encoder
        .encode(&rgba, width, height, ColorType::Rgba8)
        .expect("WebP fixture with metadata encodes");
    bytes
}

fn lossy_red_webp() -> Vec<u8> {
    vec![
        0x52, 0x49, 0x46, 0x46, 0x3c, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x20, 0x30, 0x00, 0x00, 0x00, 0xd0, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x02, 0x00, 0x02, 0x00,
        0x02, 0x00, 0x34, 0x25, 0xa0, 0x02, 0x74, 0xba, 0x01, 0xf8, 0x00, 0x03, 0xb0, 0x00, 0xfe,
        0xf0, 0xc4, 0x0b, 0xff, 0x20, 0xb9, 0x61, 0x75, 0xc8, 0xd7, 0xff, 0x20, 0x3f, 0xe4, 0x07,
        0xfc, 0x80, 0xff, 0xf8, 0xf2, 0x00, 0x00, 0x00,
    ]
}

fn exif_orientation_bytes(orientation: u16) -> Vec<u8> {
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());
    exif
}

fn linear_rgb_icc_profile() -> Vec<u8> {
    const PROFILE_SIZE: usize = 312;
    const DATA_OFFSET: u32 = 216;
    const XYZ_SIZE: u32 = 20;
    const CURVE_OFFSET: u32 = DATA_OFFSET + 4 * XYZ_SIZE;
    const TAGS: [(&[u8; 4], u32, u32); 7] = [
        (b"rXYZ", DATA_OFFSET, XYZ_SIZE),
        (b"gXYZ", DATA_OFFSET + XYZ_SIZE, XYZ_SIZE),
        (b"bXYZ", DATA_OFFSET + 2 * XYZ_SIZE, XYZ_SIZE),
        (b"wtpt", DATA_OFFSET + 3 * XYZ_SIZE, XYZ_SIZE),
        (b"rTRC", CURVE_OFFSET, 14),
        (b"gTRC", CURVE_OFFSET, 14),
        (b"bTRC", CURVE_OFFSET, 14),
    ];
    let mut profile = vec![0; 128];
    profile[0..4].copy_from_slice(&(PROFILE_SIZE as u32).to_be_bytes());
    profile[8..12].copy_from_slice(b"\x02\x10\0\0");
    profile[12..16].copy_from_slice(b"mntr");
    profile[16..20].copy_from_slice(b"RGB ");
    profile[20..24].copy_from_slice(b"XYZ ");
    profile[36..40].copy_from_slice(b"acsp");
    profile[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
    profile.extend_from_slice(&(TAGS.len() as u32).to_be_bytes());
    for (signature, offset, size) in TAGS {
        profile.extend_from_slice(signature);
        profile.extend_from_slice(&offset.to_be_bytes());
        profile.extend_from_slice(&size.to_be_bytes());
    }
    append_xyz(&mut profile, [0x0000_6fa3, 0x0000_38f6, 0x0000_0391]);
    append_xyz(&mut profile, [0x0000_6294, 0x0000_b785, 0x0000_18dc]);
    append_xyz(&mut profile, [0x0000_24a1, 0x0000_0f85, 0x0000_b6d4]);
    append_xyz(&mut profile, [0x0000_f6d6, 0x0001_0000, 0x0000_d32d]);
    profile.extend_from_slice(b"curv\0\0\0\0\0\0\0\x01\x01\0\0\0");
    assert_eq!(profile.len(), PROFILE_SIZE);
    profile
}

fn append_xyz(profile: &mut Vec<u8>, values: [u32; 3]) {
    profile.extend_from_slice(b"XYZ \0\0\0\0");
    for value in values {
        profile.extend_from_slice(&value.to_be_bytes());
    }
}

fn rendered_center(webp: &[u8]) -> [u8; 4] {
    let outcome = render_outcome(&webp_scene(webp, 1, 1), OutputFormat::Png);
    pixel(&decoded_png(outcome.artifact().bytes()), 21, 10, 10)
}

fn animated_webp(width: u32, height: u32, frames: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    let has_alpha = frames.iter().any(|(_, _, frame)| {
        image_webp::WebPDecoder::new(Cursor::new(frame))
            .expect("frame WebP decodes")
            .has_alpha()
    });
    append_webp_chunk(
        &mut body,
        *b"VP8X",
        &[
            vec![0x02 | if has_alpha { 0x10 } else { 0 }, 0, 0, 0],
            le_u24(width - 1).to_vec(),
            le_u24(height - 1).to_vec(),
        ]
        .concat(),
    );
    append_webp_chunk(&mut body, *b"ANIM", &[0, 0, 0, 0, 0, 0]);
    for (x, y, frame) in frames {
        let frame_chunk = webp_image_chunk(frame);
        let frame_decoder =
            image_webp::WebPDecoder::new(Cursor::new(frame)).expect("frame WebP decodes");
        let (frame_width, frame_height) = frame_decoder.dimensions();
        let mut payload = Vec::new();
        payload.extend_from_slice(&le_u24(x / 2));
        payload.extend_from_slice(&le_u24(y / 2));
        payload.extend_from_slice(&le_u24(frame_width - 1));
        payload.extend_from_slice(&le_u24(frame_height - 1));
        payload.extend_from_slice(&le_u24(100));
        payload.push(0x02);
        payload.extend_from_slice(frame_chunk);
        append_webp_chunk(&mut body, *b"ANMF", &payload);
    }
    let mut webp = b"RIFF".to_vec();
    webp.extend_from_slice(
        &u32::try_from(body.len() + 4)
            .expect("WebP body fits u32")
            .to_le_bytes(),
    );
    webp.extend_from_slice(b"WEBP");
    webp.extend_from_slice(&body);
    webp
}

fn webp_image_chunk(webp: &[u8]) -> &[u8] {
    let mut offset = 12;
    while offset + 8 <= webp.len() {
        let length = u32::from_le_bytes(webp[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let end = offset + 8 + length + (length & 1);
        if matches!(&webp[offset..offset + 4], b"VP8 " | b"VP8L") {
            return &webp[offset..end];
        }
        offset = end;
    }
    panic!("fixture has a WebP image chunk")
}

fn append_webp_chunk(output: &mut Vec<u8>, kind: [u8; 4], payload: &[u8]) {
    output.extend_from_slice(&kind);
    output.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("chunk length fits u32")
            .to_le_bytes(),
    );
    output.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        output.push(0);
    }
}

fn le_u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

fn webp_scene(webp: &[u8], width: u32, height: u32) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(webp);
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
                "id":"asset","mimeType":"image/webp",
                "dataURL":"data:image/webp;base64,{encoded}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("WebP scene is admitted")
}

fn two_webp_scene(webp: &[u8]) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(webp);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"two","type":"image","x":2,"y":0,"width":1,"height":1,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},"files":{{
                "one":{{"id":"one","mimeType":"image/webp","dataURL":"data:image/webp;base64,{encoded}","created":1}},
                "two":{{"id":"two","mimeType":"image/webp","dataURL":"data:image/webp;base64,{encoded}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("two-WebP scene is admitted")
}

fn render_outcome(scene: &SceneDocument, format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("render request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("WebP asset has local recovery")
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
