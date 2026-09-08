use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, RenderRequest, Renderer, ResourceLimits,
    SceneDocument,
};
use std::io::Cursor;

#[test]
fn direct_svg_preserves_validated_jfif_octets_in_a_canonical_data_url() {
    let jpeg = fixture_jpeg(1, 1, &[[24, 96, 208]]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&jpeg);
    let scene = image_scene(
        &format!("data:image/jpeg;base64,{encoded}"),
        "image/jpeg",
        1,
        1,
    );
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("a referenced JFIF renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"data:image/jpeg;base64,{encoded}\" preserveAspectRatio=\"none\" width=\"1\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_jpeg_as_opaque_canonical_srgb() {
    let jpeg = fixture_jpeg(1, 1, &[[24, 96, 208]]);
    let scene = jpeg_scene(&jpeg, 1, 1);

    let outcome = render_outcome(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(outcome.artifact().bytes()));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut pixels).expect("PNG frame decodes");

    assert_eq!((output.width, output.height), (21, 21));
    let center = &pixels[(10 * 21 + 10) * 4..(10 * 21 + 10) * 4 + 4];
    assert!(center[0].abs_diff(24) <= 3, "red channel: {}", center[0]);
    assert!(center[1].abs_diff(96) <= 3, "green channel: {}", center[1]);
    assert!(center[2].abs_diff(208) <= 3, "blue channel: {}", center[2]);
    assert_eq!(center[3], 255);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn pdf_embeds_normalized_jpeg_losslessly_with_a_soft_mask_and_no_fallback() {
    let scene = jpeg_scene(&fixture_jpeg(1, 1, &[[24, 96, 208]]), 1, 1);

    let outcome = render_outcome(&scene, OutputFormat::Pdf);
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
    let jpeg = fixture_jpeg(1, 1, &[[24, 96, 208]]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(jpeg);
    let scene = image_scene(
        &format!("data:image/jfif;base64,{encoded}"),
        "image/png",
        1,
        1,
    );

    let outcome = render_outcome(&scene, OutputFormat::Svg);

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
    assert!(svg.contains(&format!("data:image/jpeg;base64,{encoded}")));
}

#[test]
fn png_and_pdf_apply_jpeg_exif_orientation_while_svg_preserves_source_bytes() {
    let source = fixture_jpeg(
        8,
        4,
        &[
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [240, 20, 20],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
            [20, 20, 240],
        ],
    );
    let oriented = jpeg_with_app1_exif(&source, 6);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&oriented);
    let scene = image_scene(
        &format!("data:image/jpeg;base64,{encoded}"),
        "image/jpeg",
        10,
        20,
    );

    let svg_outcome = render_outcome(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&format!("data:image/jpeg;base64,{encoded}")));

    let png_outcome = render_outcome(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(png_outcome.artifact().bytes()));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut rgba).expect("PNG frame decodes");
    assert_eq!((output.width, output.height), (30, 40));
    let top = pixel(&rgba, 30, 15, 14);
    let bottom = pixel(&rgba, 30, 15, 25);
    assert!(top[0] > top[2], "top pixel: {top:?}");
    assert!(bottom[2] > bottom[0], "bottom pixel: {bottom:?}");

    let pdf_outcome = render_outcome(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf_outcome.artifact().bytes());
    assert!(pdf.contains("/Width 4\n/Height 8"));
}

#[test]
fn direct_svg_validates_jpeg_without_charging_decoded_pixels() {
    let jpeg = fixture_jpeg(2, 1, &[[255, 0, 0], [0, 0, 255]]);
    let scene = jpeg_scene(&jpeg, 2, 1);
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
        .render(&scene, &request)
        .expect("direct SVG validates compressed JPEG without pixel derivation");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(std::str::from_utf8(outcome.artifact().bytes())
        .expect("SVG is UTF-8")
        .contains("<image "));
}

#[test]
fn malformed_and_truncated_jpegs_use_typed_decode_recovery() {
    let mut truncated = fixture_jpeg(1, 1, &[[24, 96, 208]]);
    truncated.truncate(truncated.len() - 2);
    let malformed_segment = b"\xff\xd8\xff\xe0\0\x01\xff\xd9".to_vec();

    for jpeg in [truncated, malformed_segment] {
        let outcome = render_outcome(&jpeg_scene(&jpeg, 1, 1), OutputFormat::Svg);
        assert_eq!(outcome.report().diagnostics().len(), 1);
        assert_eq!(
            outcome.report().diagnostics()[0].code(),
            DiagnosticCode::IMAGE_DECODE_FAILED
        );
        assert!(!std::str::from_utf8(outcome.artifact().bytes())
            .expect("SVG is UTF-8")
            .contains("<image "));
    }
}

#[test]
fn standard_sdr_projection_uses_the_base_of_a_jpeg_gain_map_container() {
    let base = fixture_jpeg(1, 1, &[[24, 96, 208]]);
    let gain_map = fixture_jpeg(1, 1, &[[255, 255, 255]]);
    let xmp = b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta hdrgm:Version=\"1.0\"/>";
    let container_base = jpeg_with_app_segment(&base, 0xe2, b"MPF\0fixture-index");
    let mut container = jpeg_with_app_segment(&container_base, 0xe1, xmp);
    container.extend_from_slice(&gain_map);

    let base_pixel = rendered_center(&base);
    let container_pixel = rendered_center(&container);

    assert_eq!(container_pixel, base_pixel);
}

#[test]
fn unqualified_concatenated_jpegs_are_rejected_as_malformed_containers() {
    let mut concatenated = fixture_jpeg(1, 1, &[[24, 96, 208]]);
    concatenated.extend_from_slice(&fixture_jpeg(1, 1, &[[255, 255, 255]]));

    let outcome = render_outcome(&jpeg_scene(&concatenated, 1, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_DECODE_FAILED
    );
}

#[test]
fn jpeg_icc_profile_takes_precedence_over_default_srgb_interpretation() {
    let source = fixture_jpeg(1, 1, &[[128, 128, 128]]);
    let tagged = jpeg_with_icc_profile(&source, &linear_rgb_icc_profile());

    let untagged = rendered_center(&source);
    let normalized = rendered_center(&tagged);

    assert!(
        untagged[0].abs_diff(128) <= 2,
        "untagged pixel: {untagged:?}"
    );
    assert!(
        normalized[0].abs_diff(188) <= 2,
        "ICC pixel: {normalized:?}"
    );
    assert_eq!(normalized[0], normalized[1]);
    assert_eq!(normalized[1], normalized[2]);
}

#[test]
fn malformed_multipart_jpeg_icc_metadata_uses_decode_recovery() {
    let source = fixture_jpeg(1, 1, &[[128, 128, 128]]);
    let profile = linear_rgb_icc_profile();
    let mut missing_part = Vec::new();
    missing_part.extend_from_slice(b"ICC_PROFILE\0\x01\x02");
    missing_part.extend_from_slice(&profile);
    let missing_part = jpeg_with_app_segment(&source, 0xe2, &missing_part);

    let mut duplicate_part = Vec::new();
    duplicate_part.extend_from_slice(b"ICC_PROFILE\0\x01\x01");
    duplicate_part.extend_from_slice(&profile);
    let duplicate = jpeg_with_app_segment(&source, 0xe2, &duplicate_part);
    let duplicate = jpeg_with_app_segment(&duplicate, 0xe2, &duplicate_part);

    for malformed in [missing_part, duplicate] {
        let outcome = render_outcome(&jpeg_scene(&malformed, 1, 1), OutputFormat::Svg);
        assert_eq!(outcome.report().diagnostics().len(), 1);
        assert_eq!(
            outcome.report().diagnostics()[0].code(),
            DiagnosticCode::IMAGE_DECODE_FAILED
        );
    }
}

#[test]
fn baseline_progressive_grayscale_and_raw_jpeg_streams_decode() {
    let baseline = fixture_jpeg(1, 1, &[[64, 64, 64]]);
    let mut progressive = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut progressive, 100);
    encoder.set_progressive(true);
    encoder
        .encode(&[64], 1, 1, jpeg_encoder::ColorType::Luma)
        .expect("progressive grayscale JPEG encodes");
    let raw = jpeg_without_app0(&baseline);

    for jpeg in [baseline, progressive, raw] {
        let pixel = rendered_center(&jpeg);
        assert!(pixel[0].abs_diff(64) <= 2, "decoded pixel: {pixel:?}");
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
        assert_eq!(pixel[3], 255);
    }
}

#[test]
fn cmyk_jpeg_decodes_to_opaque_canonical_srgb() {
    let mut cmyk = Vec::new();
    jpeg_encoder::Encoder::new(&mut cmyk, 100)
        .encode(&[255, 0, 0, 0], 1, 1, jpeg_encoder::ColorType::Cmyk)
        .expect("CMYK JPEG encodes");

    let outcome = render_outcome(&jpeg_scene(&cmyk, 1, 1), OutputFormat::Png);
    let pixel = decoded_png_pixel(outcome.artifact().bytes(), 21, 10, 10);

    assert_eq!(pixel[3], 255);
    assert!(outcome.report().diagnostics().is_empty());
}

fn fixture_jpeg(width: u16, height: u16, pixels: &[[u8; 3]]) -> Vec<u8> {
    assert_eq!(pixels.len(), usize::from(width) * usize::from(height));
    let mut bytes = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut bytes, 100);
    let rgb: Vec<u8> = pixels.iter().flatten().copied().collect();
    encoder
        .encode(&rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .expect("fixture JPEG encodes");
    bytes
}

fn jpeg_with_app1_exif(source: &[u8], orientation: u16) -> Vec<u8> {
    let mut exif = Vec::new();
    exif.extend_from_slice(b"Exif\0\0II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());

    jpeg_with_app_segment(source, 0xe1, &exif)
}

fn jpeg_with_icc_profile(source: &[u8], profile: &[u8]) -> Vec<u8> {
    let mut segment = Vec::new();
    segment.extend_from_slice(b"ICC_PROFILE\0\x01\x01");
    segment.extend_from_slice(profile);
    jpeg_with_app_segment(source, 0xe2, &segment)
}

fn jpeg_with_app_segment(source: &[u8], marker: u8, segment: &[u8]) -> Vec<u8> {
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&source[..2]);
    jpeg.extend_from_slice(&[0xff, marker]);
    jpeg.extend_from_slice(
        &u16::try_from(segment.len() + 2)
            .expect("APP segment length fits u16")
            .to_be_bytes(),
    );
    jpeg.extend_from_slice(segment);
    jpeg.extend_from_slice(&source[2..]);
    jpeg
}

fn jpeg_without_app0(source: &[u8]) -> Vec<u8> {
    assert_eq!(&source[..4], b"\xff\xd8\xff\xe0");
    let length = usize::from(u16::from_be_bytes([source[4], source[5]]));
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&source[..2]);
    jpeg.extend_from_slice(&source[4 + length..]);
    jpeg
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

fn rendered_center(jpeg: &[u8]) -> [u8; 4] {
    let outcome = render_outcome(&jpeg_scene(jpeg, 1, 1), OutputFormat::Png);
    decoded_png_pixel(outcome.artifact().bytes(), 21, 10, 10)
}

fn decoded_png_pixel(png: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    pixel(&rgba, width, x, y)
}

fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    rgba[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel has four channels")
}

fn image_scene(data_url: &str, media_type: &str, width: u32, height: u32) -> SceneDocument {
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
                "id":"asset","mimeType":"{media_type}",
                "dataURL":"{data_url}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

fn jpeg_scene(jpeg: &[u8], width: u32, height: u32) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(jpeg);
    image_scene(
        &format!("data:image/jpeg;base64,{encoded}"),
        "image/jpeg",
        width,
        height,
    )
}

fn render_outcome(scene: &SceneDocument, format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("render request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("JPEG-family asset renders")
}
