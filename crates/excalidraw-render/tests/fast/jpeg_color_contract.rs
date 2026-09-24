//! Independent JPEG reference gates. #187 color corrections pass canonical
//! budgets; six sampling cells retain exact known-defect measurements.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn decode(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}

fn raster(bytes: &[u8]) -> CanonicalRgba8 {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    bytes.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes,
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => panic!("unexpected reference PNG color type"),
    };
    CanonicalRgba8::new(info.width, info.height, rgba).unwrap()
}

fn evidence() -> Value {
    for (bytes, hash) in [
        (
            include_bytes!("png_profile_contract.rs").as_slice(),
            "44a5b9b4fba24704191b32f53008467605e6afa67f3fad02a3937ea53aba64c5",
        ),
        (
            include_bytes!("../common/pdf_inspect.rs").as_slice(),
            "2e46ed33e8decbb629d47dd1a45f43781c102a4ceba5990620b33edc25bab074",
        ),
        (
            include_bytes!("../common/raster.rs").as_slice(),
            "4f6bc5a85213b1268b8536c242687d58c3cdb81423c6643b6cd1c44e9d06d303",
        ),
    ] {
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), hash);
    }
    let bytes = include_bytes!("../../../../tests/oracle/jpeg-color/reference.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "9b5ab60a18e83b52c582ccb38728b71633b11b592ffe540bab2c796857694638"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/jpeg-color-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            source["sha256"].as_str().unwrap()
        );
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 76);
    let mut expected = BTreeSet::new();
    for model in ["gray", "rgb", "ycbcr", "cmyk", "ycck"] {
        for (h, v) in [
            (1, 1),
            (2, 1),
            (1, 2),
            (2, 2),
            (4, 1),
            (4, 2),
            (1, 4),
            (2, 4),
        ] {
            if !matches!(model, "ycbcr" | "ycck") && (h, v) != (1, 1) {
                continue;
            }
            for mode in ["baseline", "progressive"] {
                for pattern in ["ramp", "edges"] {
                    expected.insert(format!("{model}-{h}x{v}-{mode}-{pattern}"));
                }
            }
        }
    }
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        expected
    );
    record
}

#[test]
fn jpeg_headers_independently_confirm_color_sampling_and_scan_axes() {
    let record = evidence();
    for row in record["cases"].as_array().unwrap() {
        let bytes = decode(&row["jpeg_base64"]);
        let mut offset = 2;
        let mut frames = 0;
        let mut scans = Vec::new();
        let mut adobe = None;
        let mut jfif = false;
        assert_eq!(&bytes[..2], b"\xff\xd8");
        while offset < bytes.len() {
            assert_eq!(bytes[offset], 255);
            let marker = bytes[offset + 1];
            offset += 2;
            if marker == 0xd9 {
                assert_eq!(offset, bytes.len());
                break;
            }
            let length = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
            let segment = &bytes[offset + 2..offset + length];
            offset += length;
            if matches!(marker, 0xc0 | 0xc2) {
                frames += 1;
                assert_eq!(marker == 0xc2, row["mode"] == "progressive");
                assert_eq!(segment[0], 8);
                assert_eq!(
                    u16::from_be_bytes([segment[1], segment[2]]) as u64,
                    row["height"].as_u64().unwrap()
                );
                assert_eq!(
                    u16::from_be_bytes([segment[3], segment[4]]) as u64,
                    row["width"].as_u64().unwrap()
                );
                let model = row["model"].as_str().unwrap();
                let components = match model {
                    "gray" => 1,
                    "rgb" | "ycbcr" => 3,
                    _ => 4,
                };
                assert_eq!(segment[5], components);
                let factor = ((row["sampling"][0].as_u64().unwrap() << 4)
                    | row["sampling"][1].as_u64().unwrap()) as u8;
                for (i, c) in segment[6..].chunks_exact(3).enumerate() {
                    assert_eq!(c[0], if model == "rgb" { b"RGB"[i] } else { i as u8 });
                    assert_eq!(
                        c[1],
                        if i == 0 || (model == "ycck" && i == 3) {
                            factor
                        } else {
                            0x11
                        }
                    );
                }
            }
            if marker == 0xe0 {
                jfif = segment.starts_with(b"JFIF\0");
            }
            if marker == 0xee {
                assert!(segment.starts_with(b"Adobe"));
                adobe = Some(segment[11]);
            }
            assert!(
                !matches!(marker, 0xe1 | 0xe2),
                "no EXIF/ICC/MPF in this matrix"
            );
            if marker == 0xda {
                scans.push(segment[0]);
                if row["mode"] == "baseline" {
                    assert_eq!(&segment[segment.len() - 3..], &[0, 63, 0]);
                }
                while offset + 1 < bytes.len() {
                    if bytes[offset] == 255
                        && bytes[offset + 1] != 0
                        && !matches!(bytes[offset + 1], 0xd0..=0xd7)
                    {
                        break;
                    }
                    offset += 1;
                }
            }
        }
        assert_eq!(frames, 1);
        assert!(!scans.is_empty());
        if row["mode"] == "progressive" {
            assert!(scans.len() > 1);
        }
        match row["model"].as_str().unwrap() {
            "rgb" => {
                assert_eq!(adobe, Some(0));
                assert!(!jfif);
            }
            "cmyk" => assert_eq!(adobe, Some(0)),
            "ycck" => assert_eq!(adobe, Some(2)),
            _ => {
                assert!(jfif);
                assert_eq!(adobe, None);
            }
        }
        // Encoder chooses non-interleaved baseline scans for these factors.
        if row["mode"] == "baseline"
            && matches!(
                (
                    row["sampling"][0].as_u64().unwrap(),
                    row["sampling"][1].as_u64().unwrap()
                ),
                (4, 2) | (1, 4) | (2, 4)
            )
        {
            assert!(scans.len() > 1);
            assert!(scans.iter().all(|&n| n == 1));
        }
    }
}

fn svg_extent(svg: &str, width: u32, height: u32) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let root = document.root_element();
    assert!(root.has_tag_name("svg"));
    for (attribute, expected) in [("width", width), ("height", height)] {
        assert_eq!(
            root.attribute(attribute).unwrap().parse::<f64>().unwrap(),
            f64::from(expected)
        );
    }
    let view_box: Vec<f64> = root
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(view_box, [0.0, 0.0, f64::from(width), f64::from(height)]);
}

fn pdf_extent(pdf: &[u8], width: u32, height: u32) {
    let text = crate::common::pdf_inspect::pdf_text(pdf);
    let pages: Vec<_> = text.split("/Type /Page\n").skip(1).collect();
    assert_eq!(pages.len(), 1);
    let page = pages[0].split("endobj").next().unwrap();
    assert!(!page.contains("/Rotate"));
    assert!(!page.contains("/UserUnit"));
    for key in ["/MediaBox [", "/CropBox ["] {
        let bbox = page.split_once(key).unwrap().1.split_once(']').unwrap().0;
        let numbers: Vec<f64> = bbox
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        assert_eq!(
            numbers,
            [0.0, 0.0, f64::from(width) * 0.75, f64::from(height) * 0.75]
        );
    }
}

#[test]
fn extent_gates_reject_artifacts_with_unchanged_image_payloads() {
    let record = evidence();
    let row = &record["cases"][0];
    let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .include_background(false)
            .build()
            .unwrap();
        let output = Renderer::new().render(&scene, &request).unwrap();
        if format == OutputFormat::Svg {
            let original = std::str::from_utf8(output.artifact().bytes()).unwrap();
            svg_extent(original, 33, 25);
            for changed in [
                original.replacen("width=\"33\"", "width=\"34\"", 1),
                original.replacen("viewBox=\"0 0 33 25\"", "viewBox=\"1 0 33 25\"", 1),
            ] {
                assert_ne!(changed, original);
                assert!(std::panic::catch_unwind(|| svg_extent(&changed, 33, 25)).is_err());
            }
        } else {
            pdf_extent(output.artifact().bytes(), 33, 25);
            let original = crate::common::pdf_inspect::inflate_streams(output.artifact().bytes());
            let position = original
                .windows(b"/MediaBox [0 0 ".len())
                .position(|w| w == b"/MediaBox [0 0 ")
                .unwrap();
            let mut changed = original.clone();
            changed[position + b"/MediaBox [0 0 ".len()] = b'9';
            assert!(std::panic::catch_unwind(|| pdf_extent(&changed, 33, 25)).is_err());
        }
    }
}

fn sampling_defect(id: &str) -> Option<(u64, u64)> {
    // Six baseline sampling cells are still unqualified under #187. These
    // exact measurements are not error ceilings or approved exceptions.
    Some(match id {
        "ycbcr-4x2-baseline-ramp" => (814, 0x3fcc5b0a24de3b74),
        "ycbcr-4x2-baseline-edges" => (1507, 0x3fbd6cebb4036331),
        "ycbcr-1x4-baseline-ramp" => (822, 0x3fc4cc85cad8760a),
        "ycbcr-1x4-baseline-edges" => (1536, 0x3fbe499a5f7bd0d1),
        "ycbcr-2x4-baseline-ramp" => (821, 0x3fc49786fb86165f),
        "ycbcr-2x4-baseline-edges" => (1521, 0x3fbee9df3bb84b69),
        "ycck-4x2-baseline-ramp" => (825, 0x3f41f75c931c33e4),
        "ycck-4x2-baseline-edges" => (1536, 0x3f1d786c5dc23a31),
        "ycck-1x4-baseline-ramp" => (825, 0x3f41f6edcda7062a),
        "ycck-1x4-baseline-edges" => (1536, 0x3f1bd5d3252e3748),
        "ycck-2x4-baseline-ramp" => (825, 0x3f41f1dc1ac80141),
        "ycck-2x4-baseline-edges" => (1536, 0x3f1cc3d0228479b7),
        _ => return None,
    })
}

fn check(row: &Value) -> usize {
    let id = row["id"].as_str().unwrap();
    assert_eq!(row["reference"]["admitted"], true, "{id}");
    let jpeg = decode(&row["jpeg_base64"]);
    assert_eq!(
        format!("{:x}", Sha256::digest(&jpeg)),
        row["jpeg_sha256"].as_str().unwrap()
    );
    let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
    let (width, height) = (
        row["width"].as_u64().unwrap() as u32,
        row["height"].as_u64().unwrap() as u32,
    );
    let mut pins = 0;
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .include_background(false)
            .build()
            .unwrap();
        let output = Renderer::new().render(&scene, &request).unwrap();
        let repeated = Renderer::new().render(&scene, &request).unwrap();
        assert_eq!(
            output.artifact().bytes(),
            repeated.artifact().bytes(),
            "{id}"
        );
        assert_eq!(output.report(), repeated.report(), "{id}");
        assert_eq!(
            serde_json::to_value(output.report()).unwrap(),
            json!({"schema_version":1,"diagnostics":[]}),
            "{id} {format:?}"
        );
        if format == OutputFormat::Svg {
            let expected = format!(
                "data:image/jpeg;base64,{}",
                row["jpeg_base64"].as_str().unwrap()
            );
            for svg in [
                std::str::from_utf8(output.artifact().bytes()).unwrap(),
                row["reference"]["svg"].as_str().unwrap(),
            ] {
                svg_extent(svg, width, height);
                let document = roxmltree::Document::parse(svg).unwrap();
                let images: Vec<_> = document
                    .descendants()
                    .filter(|n| n.has_tag_name("image"))
                    .collect();
                assert_eq!(images.len(), 1, "{id}");
                let href = images[0]
                    .attribute("href")
                    .or_else(|| images[0].attribute(("http://www.w3.org/1999/xlink", "href")));
                assert_eq!(href, Some(expected.as_str()), "{id}");
            }
            continue;
        }
        let pixels = if format == OutputFormat::Png {
            super::png_profile_contract::assert_untagged_png(output.artifact().bytes());
            crate::common::raster::decode_png(output.artifact().bytes()).into_canonical()
        } else {
            pdf_extent(output.artifact().bytes(), width, height);
            CanonicalRgba8::new(
                width,
                height,
                super::png_profile_contract::pdf_rgba(
                    output.artifact().bytes(),
                    width as usize,
                    height as usize,
                ),
            )
            .unwrap()
        };
        let expected = raster(&decode(
            &row["reference"][if format == OutputFormat::Png {
                "png"
            } else {
                "svg_png"
            }],
        ));
        assert_eq!((pixels.width(), pixels.height()), (width, height));
        assert_eq!((expected.width(), expected.height()), (width, height));
        assert!(pixels.rgba().chunks_exact(4).all(|p| p[3] == 255));
        assert!(expected.rgba().chunks_exact(4).all(|p| p[3] == 255));
        let result = compare(
            &expected,
            &pixels,
            VisualClass::GeometryImage,
            &[],
            ComparisonMode::Canonical,
        )
        .unwrap();
        if let Some((changed, bits)) = sampling_defect(id) {
            assert!(!result.passed(), "#187 sampling pin became stale: {id}");
            assert_eq!(result.whole.changed_pixels, changed, "{id}");
            let SsimOutcome::Score(score) = result.whole.ssim else {
                panic!("SSIM expected")
            };
            assert_eq!(
                score.to_bits(),
                bits,
                "#187 sampling measurement moved: {id}"
            );
            pins += 1;
        } else {
            assert!(
                result.passed(),
                "JPEG {id} {format:?}: changed={} SSIM={:?}",
                result.whole.changed_pixels,
                result.whole.ssim
            );
        }
    }
    pins
}

#[test]
pub(super) fn qualified_gray_rgb_and_ycbcr_cells_pass_both_reference_authorities() {
    let record = evidence();
    // Retain the existing exact owner selector; its scope now also covers
    // the 15 repaired CMYK/YCCK color cells. Sampling cells stay separate.
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| sampling_defect(r["id"].as_str().unwrap()).is_none())
        .collect();
    assert_eq!(rows.len(), 64);
    for row in rows {
        assert_eq!(check(row), 0);
    }
}

#[test]
fn issue_187_baseline_sampling_cells_remain_explicit_defects() {
    let record = evidence();
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| sampling_defect(r["id"].as_str().unwrap()).is_some())
        .collect();
    assert_eq!(rows.len(), 12);
    assert_eq!(rows.into_iter().map(check).sum::<usize>(), 24);
}

#[test]
pub(super) fn cmyk_component_probe_isolates_reference_truncation_from_decoder_rounding() {
    use zune_jpeg::zune_core::{
        bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions,
    };
    let record = evidence();
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["model"] == "cmyk")
        .collect();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let bytes = decode(&row["jpeg_base64"]);
        let options = DecoderOptions::default()
            .set_use_unsafe(false)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(ColorSpace::CMYK);
        let mut decoder =
            zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes.as_slice()), options);
        let raw = decoder.decode().unwrap();
        let expected = raster(&decode(&row["reference"]["png"]));
        for rounding in [false, true] {
            let pixels = raw
                .chunks_exact(4)
                .flat_map(|p| {
                    let f = |c| {
                        ((u32::from(c) * u32::from(p[3]) + if rounding { 127 } else { 0 }) / 255)
                            as u8
                    };
                    [f(p[0]), f(p[1]), f(p[2]), 255]
                })
                .collect();
            let pixels = CanonicalRgba8::new(expected.width(), expected.height(), pixels).unwrap();
            let result = compare(
                &expected,
                &pixels,
                VisualClass::GeometryImage,
                &[],
                ComparisonMode::Canonical,
            )
            .unwrap();
            assert_eq!(
                result.passed(),
                !rounding || row["pattern"] == "edges",
                "{} rounding={rounding}",
                row["id"]
            );
        }
    }
}

#[test]
pub(super) fn ycck_unconverted_components_with_integer_color_conversion_match_reference() {
    use zune_jpeg::zune_core::{
        bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions,
    };
    let record = evidence();
    for row in record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["model"] == "ycck" && r["mode"] == "progressive")
    {
        let bytes = decode(&row["jpeg_base64"]);
        let options = DecoderOptions::default()
            .set_use_unsafe(false)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(ColorSpace::YCCK);
        let mut decoder =
            zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes.as_slice()), options);
        let raw = decoder.decode().unwrap();
        let expected = raster(&decode(&row["reference"]["png"]));
        let rgba = raw
            .chunks_exact(4)
            .flat_map(|p| {
                let y = i32::from(p[0]);
                let cb = i32::from(p[1]) - 128;
                let cr = i32::from(p[2]) - 128;
                let cmy = [
                    y + ((91881 * cr + 32768) >> 16),
                    y + ((-22554 * cb - 46802 * cr + 32768) >> 16),
                    y + ((116130 * cb + 32768) >> 16),
                ];
                let rgb =
                    cmy.map(|v| ((255 - v.clamp(0, 255)) as u32 * u32::from(p[3]) / 255) as u8);
                [rgb[0], rgb[1], rgb[2], 255]
            })
            .collect();
        let actual = CanonicalRgba8::new(expected.width(), expected.height(), rgba).unwrap();
        let result = compare(
            &expected,
            &actual,
            VisualClass::GeometryImage,
            &[],
            ComparisonMode::Canonical,
        )
        .unwrap();
        assert!(result.passed(), "{} {:?}", row["id"], result.whole.ssim);
    }
}

#[test]
fn provisional_adobe_color_space_does_not_override_three_component_rgb() {
    let record = evidence();
    for row in record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["model"] == "rgb")
    {
        let mut bytes = decode(&row["jpeg_base64"]);
        let mut offset = 2;
        while offset < bytes.len() {
            assert_eq!(bytes[offset], 255);
            let marker = bytes[offset + 1];
            if marker == 0xd9 {
                break;
            }
            let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
            if matches!(marker, 0xc0 | 0xc2) {
                for i in 0..3 {
                    bytes[offset + 10 + i * 3] = i as u8;
                }
            }
            if marker == 0xda {
                for i in 0..bytes[offset + 4] as usize {
                    let at = offset + 5 + i * 2;
                    bytes[at] = b"RGB".iter().position(|&c| c == bytes[at]).unwrap() as u8;
                }
            }
            offset += 2 + length;
            if marker == 0xda {
                while offset + 1 < bytes.len() {
                    if bytes[offset] == 255
                        && bytes[offset + 1] != 0
                        && !matches!(bytes[offset + 1], 0xd0..=0xd7)
                    {
                        break;
                    }
                    offset += 1;
                }
            }
        }
        let changed = crate::common::image_scene::image_scene(
            &bytes,
            "image/jpeg",
            row["width"].as_u64().unwrap() as u32,
            row["height"].as_u64().unwrap() as u32,
        );
        let original = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
        for format in [OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap();
            let expected = Renderer::new().render(&original, &request).unwrap();
            let actual = Renderer::new().render(&changed, &request).unwrap();
            assert_eq!(actual.report(), expected.report());
            assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        }
    }
}

#[test]
fn issue_187_minimized_noninterleaved_neutral_image_still_exposes_missing_rows() {
    for sampling in [
        jpeg_encoder::SamplingFactor::F_4_2,
        jpeg_encoder::SamplingFactor::F_1_4,
        jpeg_encoder::SamplingFactor::F_2_4,
    ] {
        for (width, height) in [(1_u16, 1_u16), (9, 9), (17, 17), (33, 25)] {
            // Constant Y=96, Cb=Cr=128: no color rounding or AC coefficients
            // can explain missing rows/chroma starvation in sequential scans.
            let samples = [96, 128, 128].repeat(usize::from(width) * usize::from(height));
            let mut bytes = Vec::new();
            let mut encoder = jpeg_encoder::Encoder::new(&mut bytes, 100);
            encoder.set_sampling_factor(sampling);
            encoder
                .encode(&samples, width, height, jpeg_encoder::ColorType::Ycbcr)
                .unwrap();
            let scene = crate::common::image_scene::image_scene(
                &bytes,
                "image/jpeg",
                u32::from(width),
                u32::from(height),
            );
            for format in [OutputFormat::Png, OutputFormat::Pdf] {
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .include_background(false)
                    .build()
                    .unwrap();
                let output = Renderer::new().render(&scene, &request).unwrap();
                assert_eq!(
                    serde_json::to_value(output.report()).unwrap(),
                    json!({"schema_version":1,"diagnostics":[]})
                );
                let actual = if format == OutputFormat::Png {
                    crate::common::raster::decode_png(output.artifact().bytes()).rgba
                } else {
                    super::png_profile_contract::pdf_rgba(
                        output.artifact().bytes(),
                        width as usize,
                        height as usize,
                    )
                };
                let wrong = actual
                    .chunks_exact(4)
                    .filter(|p| *p != [96, 96, 96, 255])
                    .count();
                let expected_wrong = match width {
                    1 => 0,
                    9 => 9,
                    17 if sampling == jpeg_encoder::SamplingFactor::F_4_2 => 153,
                    17 => 289,
                    33 => 825,
                    _ => unreachable!(),
                };
                assert_eq!(
                    wrong, expected_wrong,
                    "#187 neutral measurement changed: {sampling:?} {width}x{height} {format:?}"
                );
            }
        }
    }
}
