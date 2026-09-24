//! Bounded JPEG exact vectors, not a claim of complete codec/profile ownership.
//! Constant 8x8 gray blocks have only a DC coefficient: at quality 100 the
//! quantizer is one, so the authored integer gray values survive exactly.
//! Expected samples and TIFF permutations never come from a JPEG decoder.
use crate::common::codec_fixtures::{jpeg_with_app_segment, jpeg_without_app0};
use crate::common::image_scene::{data_url, image_scene};
use crate::common::raster::decode_png;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const LEVELS: [u8; 6] = [32, 64, 96, 160, 192, 224];
const ORDERS: [[usize; 6]; 8] = [
    [0, 1, 2, 3, 4, 5],
    [2, 1, 0, 5, 4, 3],
    [5, 4, 3, 2, 1, 0],
    [3, 4, 5, 0, 1, 2],
    [0, 3, 1, 4, 2, 5],
    [3, 0, 4, 1, 5, 2],
    [5, 2, 4, 1, 3, 0],
    [2, 5, 1, 4, 0, 3],
];

fn encode(width: u16, height: u16, gray: &[u8], progressive: bool, restart: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut bytes, 100);
    encoder.set_progressive(progressive);
    if restart {
        encoder.set_restart_interval(1);
    }
    encoder
        .encode(gray, width, height, jpeg_encoder::ColorType::Luma)
        .unwrap();
    bytes
}

fn blocks(orientation: usize) -> (usize, usize, Vec<u8>) {
    let columns = if orientation < 5 { 3 } else { 2 };
    let rows = 6 / columns;
    let gray = (0..rows * 8)
        .flat_map(|y| {
            (0..columns * 8).map(move |x| LEVELS[ORDERS[orientation - 1][y / 8 * columns + x / 8]])
        })
        .collect();
    (columns * 8, rows * 8, gray)
}

fn exif(value: u16, big: bool) -> Vec<u8> {
    let short = |v: u16| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let long = |v: u32| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    [
        b"Exif\0\0".as_slice(),
        if big { b"MM" } else { b"II" },
        &short(42),
        &long(8),
        &short(1),
        &short(0x112),
        &short(3),
        &long(1),
        &short(value),
        &short(0),
        &long(0),
    ]
    .concat()
}

fn request(format: OutputFormat) -> RenderRequest {
    RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .build()
        .unwrap()
}

fn missing_scene() -> SceneDocument {
    SceneDocument::parse(br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":24,"height":16,"angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{}}"#).unwrap()
}

fn machine_report(report: &excalidraw_render::CompatibilityReport) -> Value {
    let mut value = serde_json::to_value(report).unwrap();
    for diagnostic in value["diagnostics"].as_array_mut().unwrap() {
        assert!(!diagnostic["message"].as_str().unwrap().is_empty());
        diagnostic.as_object_mut().unwrap().remove("message");
    }
    value
}

fn check(bytes: &[u8], width: usize, height: usize, gray: &[u8]) {
    let expected: Vec<_> = gray.iter().flat_map(|&g| [g, g, g, 255]).collect();
    let scene = image_scene(bytes, "image/jpeg", width as u32, height as u32);
    for format in FORMATS {
        let request = request(format);
        let first = Renderer::new().render(&scene, &request).unwrap();
        let second = Renderer::new().render(&scene, &request).unwrap();
        assert_eq!(first.report(), second.report());
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(
            serde_json::to_value(first.report()).unwrap(),
            json!({"schema_version":1,"diagnostics":[]})
        );
        let artifact = first.artifact().bytes();
        match format {
            OutputFormat::Svg => {
                let text = std::str::from_utf8(artifact).unwrap();
                let document = roxmltree::Document::parse(text).unwrap();
                let images: Vec<_> = document
                    .descendants()
                    .filter(|n| n.has_tag_name("image"))
                    .collect();
                assert_eq!(images.len(), 1);
                assert_eq!(
                    images[0].attribute("href"),
                    Some(data_url("image/jpeg", bytes).as_str())
                );
            }
            OutputFormat::Png => {
                super::png_profile_contract::assert_untagged_png(artifact);
                let png = decode_png(artifact);
                assert_eq!((png.width as usize, png.height as usize), (width, height));
                assert_eq!(png.rgba, expected);
            }
            OutputFormat::Pdf => {
                // This oracle follows page /Im0, its ICC OutputIntent and its
                // attached soft mask; an unrelated image stream cannot pass.
                super::png_profile_contract::assert_pdf_pixels(artifact, width, height, &expected);
                let text = crate::common::pdf_inspect::pdf_text(artifact);
                assert!(text.contains("/SMask "));
                assert!(!text.contains("/DCTDecode"));
            }
            _ => unreachable!("only the three declared formats are tested"),
        }
    }
}

#[test]
fn baseline_and_progressive_dc_blocks_are_exact_opaque_samples() {
    let (width, height, gray) = blocks(1);
    for progressive in [false, true] {
        for restart in [false, true] {
            let bytes = encode(width as u16, height as u16, &gray, progressive, restart);
            for raw in [false, true] {
                let source = if raw {
                    jpeg_without_app0(&bytes)
                } else {
                    bytes.clone()
                };
                check(&source, width, height, &gray);
                // Odd dimensions exercise edge MCU padding and row truncation.
                for level in [0, 64, 128, 192, 255] {
                    let gray = vec![level; 17 * 9];
                    let bytes = encode(17, 9, &gray, progressive, restart);
                    let source = if raw {
                        jpeg_without_app0(&bytes)
                    } else {
                        bytes
                    };
                    check(&source, 17, 9, &gray);
                }
            }
        }
    }
}

#[test]
pub(super) fn all_tiff_orientations_preserve_exact_samples_and_svg_source_octets() {
    let (_, _, gray) = blocks(1);
    for progressive in [false, true] {
        let source = encode(24, 16, &gray, progressive, false);
        for big in [false, true] {
            for orientation in 1..=8 {
                let tagged = jpeg_with_app_segment(&source, 0xe1, &exif(orientation, big));
                let (width, height, expected) = blocks(orientation as usize);
                check(&tagged, width, height, &expected);
            }
        }
    }
}

#[test]
pub(super) fn absent_and_invalid_tiff_orientation_leave_the_source_layout_intact() {
    let (width, height, gray) = blocks(1);
    for progressive in [false, true] {
        let source = encode(width as u16, height as u16, &gray, progressive, false);
        check(&source, width, height, &gray);
        for big in [false, true] {
            let mut payloads: Vec<_> = [0, 9, 255, 256, u16::MAX]
                .into_iter()
                .map(|v| exif(v, big))
                .collect();
            let good = exif(6, big);
            // Complete APP1 framing with invalid TIFF magic, offset, type,
            // count or truncation: metadata is ignored, source remains intact.
            for (range, replacement) in [
                (8..10, vec![0, 0]),
                (10..14, vec![255; 4]),
                (18..20, vec![0, 0]),
                (20..24, vec![0; 4]),
            ] {
                let mut bad = good.clone();
                bad[range].copy_from_slice(&replacement);
                payloads.push(bad);
            }
            payloads.push(good[..24].to_vec());
            for payload in payloads {
                check(
                    &jpeg_with_app_segment(&source, 0xe1, &payload),
                    width,
                    height,
                    &gray,
                );
            }
        }
    }
}

fn malformed_vectors() -> Vec<(&'static str, Vec<u8>)> {
    let source = encode(8, 8, &[64; 64], false, false);
    vec![
        ("missing-eoi", source[..source.len() - 2].to_vec()),
        (
            "short-marker-length",
            b"\xff\xd8\xff\xe0\0\x01\xff\xd9".to_vec(),
        ),
        (
            "segment-past-eof",
            b"\xff\xd8\xff\xe0\xff\xff\xff\xd9".to_vec(),
        ),
        ("no-frame-or-scan", b"\xff\xd8\xff\xd9".to_vec()),
    ]
}

/// Files the reference draws completely: data after the first image's EOI
/// is not read, and libjpeg's `jpeg_read_icc_profile` returns no profile for
/// an inconsistent, incomplete or duplicated sequence, so the image draws
/// untagged.
fn drawn_vectors() -> Vec<(&'static str, Vec<u8>)> {
    let source = encode(8, 8, &[64; 64], false, false);
    let mut vectors = vec![
        (
            "second-image",
            [source.as_slice(), source.as_slice()].concat(),
        ),
        ("trailing-garbage", [source.as_slice(), b"garbage"].concat()),
    ];
    for (name, sequence, count) in [
        ("icc-zero-index", 0, 1),
        ("icc-zero-count", 1, 0),
        ("icc-index-past-count", 2, 1),
        ("icc-missing-part", 1, 2),
    ] {
        let payload = [b"ICC_PROFILE\0".as_slice(), &[sequence, count], b"profile"].concat();
        vectors.push((name, jpeg_with_app_segment(&source, 0xe2, &payload)));
    }
    let one = jpeg_with_app_segment(&source, 0xe2, b"ICC_PROFILE\0\x01\x01profile");
    vectors.push((
        "icc-duplicate",
        jpeg_with_app_segment(&one, 0xe2, b"ICC_PROFILE\0\x01\x01profile"),
    ));
    let one = jpeg_with_app_segment(&source, 0xe2, b"ICC_PROFILE\0\x01\x02profile");
    vectors.push((
        "icc-inconsistent-count",
        jpeg_with_app_segment(&one, 0xe2, b"ICC_PROFILE\0\x02\x03profile"),
    ));
    vectors.push((
        "icc-missing-sequence-fields",
        jpeg_with_app_segment(&source, 0xe2, b"ICC_PROFILE\0"),
    ));
    vectors
}

#[test]
pub(super) fn trailing_data_and_malformed_icc_sequences_draw_the_first_image_untagged() {
    for (_, bytes) in drawn_vectors() {
        check(&bytes, 8, 8, &[64; 64]);
    }
}

#[test]
pub(super) fn malformed_markers_have_exact_recovery_reports() {
    let missing = missing_scene();
    for (name, bytes) in malformed_vectors() {
        let scene = image_scene(&bytes, "image/jpeg", 24, 16);
        for format in FORMATS {
            let first = Renderer::new().render(&scene, &request(format)).unwrap();
            let second = Renderer::new().render(&scene, &request(format)).unwrap();
            assert_eq!(
                first.artifact().bytes(),
                second.artifact().bytes(),
                "{name}"
            );
            assert_eq!(first.report(), second.report(), "{name}");
            let actual = machine_report(first.report());
            let placeholder = format == OutputFormat::Png;
            assert_eq!(
                actual,
                json!({"schema_version":1,"diagnostics":[{
                    "code":"image.decode-failed","severity":"warning","format":format.as_str(),
                    "effects":["visual"],"element_id":"image","json_pointer":"/files/asset/dataURL",
                    "details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,
                    "validated_media_type":"image/jpeg","recovery":if placeholder {"placeholder"} else {"omitted"},
                    "recipe_id":if placeholder {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}
                }]}),
                "{name} {format:?}"
            );
            let control = Renderer::new().render(&missing, &request(format)).unwrap();
            assert_eq!(
                first.artifact().bytes(),
                control.artifact().bytes(),
                "{name}: recovery artifact"
            );
        }
    }
}

#[test]
fn decoded_dimension_and_pixel_boundaries_do_not_apply_to_direct_svg() {
    let (_, _, gray) = blocks(1);
    let bytes = encode(24, 16, &gray, true, true);
    let scene = image_scene(&bytes, "image/jpeg", 24, 16);
    for format in FORMATS {
        for crossing in [false, true] {
            for dimension in [false, true] {
                let configured = if dimension { 24 } else { 384 } - u64::from(crossing);
                let limits = if dimension {
                    ResourceLimits::builder().max_raster_dimension(configured as u32)
                } else {
                    ResourceLimits::builder().max_decoded_pixels_per_asset(configured)
                }
                .build()
                .unwrap();
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .include_background(false)
                    .resource_limits(limits)
                    .build()
                    .unwrap();
                let outcome = Renderer::new().render(&scene, &request).unwrap();
                let repeated = Renderer::new().render(&scene, &request).unwrap();
                assert_eq!(outcome.report(), repeated.report());
                assert_eq!(outcome.artifact().bytes(), repeated.artifact().bytes());
                if !crossing || format == OutputFormat::Svg {
                    assert_eq!(
                        serde_json::to_value(outcome.report()).unwrap(),
                        json!({"schema_version":1,"diagnostics":[]})
                    );
                    let control = Renderer::new()
                        .render(&scene, &self::request(format))
                        .unwrap();
                    assert_eq!(outcome.artifact().bytes(), control.artifact().bytes());
                } else {
                    assert_eq!(
                        machine_report(outcome.report()),
                        json!({"schema_version":1,"diagnostics":[{
                            "code":if dimension {"resource.image-dimensions-exceeded"} else {"resource.image-pixels-exceeded"},
                            "severity":"warning","format":format.as_str(),"effects":["visual"],
                            "element_id":null,"json_pointer":"/files/asset/dataURL","details":{
                            "kind":"resource_limit","field":if dimension {"max_raster_dimension"} else {"max_decoded_pixels_per_asset"},
                            "configured":configured,"value_kind":if dimension {"dimension"} else {"pixels"},
                            "measurement":{"kind":"attempted","value":if dimension {24} else {384}},
                            "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"}
                        }}]})
                    );
                    let control = Renderer::new()
                        .render(&missing_scene(), &self::request(format))
                        .unwrap();
                    assert_eq!(outcome.artifact().bytes(), control.artifact().bytes());
                }
            }
        }
    }
}
