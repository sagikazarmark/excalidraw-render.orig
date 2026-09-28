//! Independent, source-bound JPEG ICC evidence: 80 cells / 160 vectors from
//! official v0.18.1 and Chromium 149, collected twice in fresh forward/reverse
//! processes. The pinned record binds its collector, authored profiles, base
//! JPEG inputs, browser profile and canonical comparator policy. Test-side APP2
//! reconstruction reads the actual segment bytes without the renderer parser.
//! Browser PNG and browser-rasterized SVG are separate color authorities; SVG
//! source preservation, PNG RGBA8/opacity and PDF extent/opacity are explicit.
//! #188's corrected gray precision and opposite-model precedence are checked
//! against both pinned color authorities, including discriminating untagged
//! controls. The earlier failure measurements belong to the historical source.
//! Recovery grants no tolerance exception.
use crate::common::lower_hex;
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
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
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().expect("PNG buffer fits")];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    bytes.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes,
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => panic!("reference PNG layout"),
    };
    CanonicalRgba8::new(info.width, info.height, rgba).unwrap()
}
fn evidence() -> Value {
    let bytes = include_bytes!("../../../../tests/oracle/jpeg-icc/reference.json");
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        "5879221d5b2eee767e56e8cd94e7bf1fff15db6dd5583bb107a8314e50d4e0e6"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/jpeg-icc-reference/v1");
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(
            lower_hex(&Sha256::digest(bytes)),
            source["sha256"].as_str().unwrap()
        );
    }
    for (bytes, hash) in [
        (
            include_bytes!("png_profile_contract.rs").as_slice(),
            "994389eed1224886cfb0df9e8382d90bcac8b6350d72618409a01a1d108a3661",
        ),
        (
            include_bytes!("../common/pdf_inspect.rs").as_slice(),
            "285cac0d1029d1f7c2485cd0d3cfb7d4be7c8394b320adc837f9eaf49ab5e4b3",
        ),
        (
            include_bytes!("../common/raster.rs").as_slice(),
            "46a6ed5daacc715f1f3c28c2b806b95a04e13206b4bc7f6d13c0ea50c99b8e10",
        ),
    ] {
        assert_eq!(lower_hex(&Sha256::digest(bytes)), hash);
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 160);
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        160
    );
    record
}

#[test]
fn evidence_has_closed_matrix_and_reconstructible_app2_profiles() {
    let record = evidence();
    reconstruct_contract(&record);
}

fn reconstruct_contract(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    let inputs = include_bytes!("../../../../tests/oracle/jpeg-color/inputs.json");
    assert_eq!(
        lower_hex(&Sha256::digest(inputs)),
        "acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258"
    );
    let inputs: Value = serde_json::from_slice(inputs).unwrap();
    let mut cells = BTreeSet::new();
    let mut expected = BTreeSet::new();
    for model in ["gray", "rgb"] {
        for mode in ["baseline", "progressive"] {
            let mut add = |signal: &str, layout: &str, outcome: &str| {
                let cell = format!("{model}-{mode}-{signal}-{layout}");
                cells.insert(cell.clone());
                for pattern in ["ramp", "edges"] {
                    expected.insert((format!("{cell}-{pattern}"), outcome.to_owned()));
                }
            };
            for signal in ["v2-g1", "v2-g2.2", "v4-g1", "v4-g2.2"] {
                for layout in ["single", "ordered", "reverse"] {
                    add(signal, layout, "accepted");
                }
            }
            add("opposite-model", "single", "inspect-model");
            for signal in ["bad-signature", "truncated", "tag-out-of-bounds"] {
                add(signal, "single", "unsupported-format");
            }
            for layout in ["missing", "duplicate", "zero-index", "inconsistent-count"] {
                add("v2-g1", layout, "decode-failed");
            }
        }
    }
    assert_eq!(cells.len(), 80);
    assert_eq!(expected.len(), 160);
    assert_eq!(
        rows.iter()
            .map(|r| r["cell"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        cells
    );
    assert_eq!(
        rows.iter()
            .map(|r| (
                r["id"].as_str().unwrap().to_owned(),
                r["expected_outcome"].as_str().unwrap().to_owned()
            ))
            .collect::<BTreeSet<_>>(),
        expected
    );
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let model = row["source_model"].as_str().unwrap();
        let mode = row["mode"].as_str().unwrap();
        let signal = row["signal"].as_str().unwrap();
        let layout = row["layout"].as_str().unwrap();
        let pattern = row["pattern"].as_str().unwrap();
        assert_eq!(row["cell"], format!("{model}-{mode}-{signal}-{layout}"));
        assert_eq!(id, format!("{model}-{mode}-{signal}-{layout}-{pattern}"));
        assert_eq!(row["base_id"], format!("{model}-1x1-{mode}-{pattern}"));
        let base = inputs["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|base| base["id"] == row["base_id"])
            .unwrap();
        assert_eq!(base["model"], row["source_model"]);
        for key in ["mode", "pattern", "width", "height"] {
            assert_eq!(row[key], base[key], "{id}: {key}");
        }
        let profile = decode(&row["profile_base64"]);
        assert_eq!(
            lower_hex(&Sha256::digest(&profile)),
            row["profile_sha256"].as_str().unwrap()
        );
        let source = decode(&base["jpeg_base64"]);
        assert_eq!(&source[..2], &[0xff, 0xd8]);
        let jpeg = decode(&row["jpeg_base64"]);
        assert_eq!(
            lower_hex(&Sha256::digest(&jpeg)),
            row["jpeg_sha256"].as_str().unwrap()
        );
        assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
        assert_eq!(
            row["scene"]["files"]["asset"]["dataURL"],
            format!(
                "data:image/jpeg;base64,{}",
                row["jpeg_base64"].as_str().unwrap()
            )
        );

        // Inspect only the authored APP2 prefix, retaining wire order and payloads.
        let mut offset = 2;
        let mut parts = Vec::new();
        while jpeg.get(offset..offset + 2) == Some(&[0xff, 0xe2]) {
            let length = usize::from(u16::from_be_bytes([jpeg[offset + 2], jpeg[offset + 3]]));
            assert!(length >= 16, "{id}: APP2 length");
            let end = offset + 2 + length;
            let data = &jpeg[offset + 4..end];
            assert_eq!(&data[..12], b"ICC_PROFILE\0", "{id}");
            parts.push((data[12], data[13], &data[14..]));
            offset = end;
        }
        let expected_parts = authored_parts(&profile, layout);
        assert_eq!(parts, expected_parts, "{id}: APP2 sequence/count/payload");

        // Re-author the full JPEG from the independently pinned base and profile.
        let mut expected_jpeg = source[..2].to_vec();
        for (sequence, total, payload) in expected_parts {
            expected_jpeg.extend_from_slice(&[0xff, 0xe2]);
            expected_jpeg
                .extend_from_slice(&u16::try_from(payload.len() + 16).unwrap().to_be_bytes());
            expected_jpeg.extend_from_slice(b"ICC_PROFILE\0");
            expected_jpeg.extend_from_slice(&[sequence, total]);
            expected_jpeg.extend_from_slice(payload);
        }
        expected_jpeg.extend_from_slice(&source[2..]);
        assert_eq!(
            jpeg, expected_jpeg,
            "{id}: exact source JPEG reconstruction"
        );
        assert_eq!(
            &jpeg[offset..],
            &source[2..],
            "{id}: base JPEG preservation"
        );

        if matches!(layout, "single" | "ordered" | "reverse") {
            parts.sort_by_key(|part| part.0);
            let assembled: Vec<u8> = parts
                .iter()
                .flat_map(|part| part.2.iter().copied())
                .collect();
            assert_eq!(assembled, profile, "{id}: actual APP2 assembly");
        }
        // Both independently collected browser rasters must have the expected
        // natural extent and opaque alpha, including recovery/model controls.
        for key in ["width", "height"] {
            assert_eq!(row["reference"][key], row[key], "{id}: browser {key}");
        }
        for key in ["png", "svg_png"] {
            let reference = raster(&decode(&row["reference"][key]));
            assert_eq!(u64::from(reference.width()), row["width"].as_u64().unwrap());
            assert_eq!(
                u64::from(reference.height()),
                row["height"].as_u64().unwrap()
            );
            assert!(
                reference.rgba().chunks_exact(4).all(|p| p[3] == 255),
                "{id}: {key} opacity"
            );
        }
    }
}

fn authored_parts<'a>(profile: &'a [u8], layout: &str) -> Vec<(u8, u8, &'a [u8])> {
    if layout == "single" {
        return vec![(1, 1, profile)];
    }
    let chunks = [&profile[..73], &profile[73..145], &profile[145..]];
    match layout {
        "ordered" => vec![(1, 3, chunks[0]), (2, 3, chunks[1]), (3, 3, chunks[2])],
        "reverse" => vec![(3, 3, chunks[2]), (2, 3, chunks[1]), (1, 3, chunks[0])],
        "missing" => vec![(1, 3, chunks[0]), (2, 3, chunks[1])],
        "duplicate" => vec![
            (1, 3, chunks[0]),
            (1, 3, chunks[0]),
            (2, 3, chunks[1]),
            (3, 3, chunks[2]),
        ],
        "zero-index" => vec![(0, 1, profile)],
        "inconsistent-count" => vec![(1, 3, chunks[0]), (2, 2, chunks[1]), (3, 3, chunks[2])],
        _ => panic!("unexpected APP2 layout: {layout}"),
    }
}
fn request(format: OutputFormat) -> RenderRequest {
    RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .build()
        .unwrap()
}
fn linear_gray(row: &Value) -> bool {
    row["expected_outcome"] == "accepted"
        && row["source_model"] == "gray"
        && matches!(row["signal"].as_str().unwrap(), "v2-g1" | "v4-g1")
}
fn svg_check(svg: &str, row: &Value) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let w = row["width"].as_u64().unwrap();
    let h = row["height"].as_u64().unwrap();
    svg_extent(doc.root_element(), w, h);
    let images: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), 1);
    let href = images[0]
        .attribute("href")
        .or_else(|| images[0].attribute(("http://www.w3.org/1999/xlink", "href")));
    assert_eq!(href, row["scene"]["files"]["asset"]["dataURL"].as_str());
}

fn svg_extent(root: roxmltree::Node<'_, '_>, w: u64, h: u64) {
    assert!(root.has_tag_name("svg"));
    assert_eq!(root.attribute("width").unwrap().parse::<u64>().unwrap(), w);
    assert_eq!(root.attribute("height").unwrap().parse::<u64>().unwrap(), h);
    assert_eq!(
        root.attribute("viewBox"),
        Some(format!("0 0 {w} {h}").as_str())
    );
}

fn pdf_extent(bytes: &[u8], width: u32, height: u32) {
    let text = crate::common::pdf_inspect::pdf_text(bytes);
    let pages: Vec<_> = text.split("/Type /Page\n").skip(1).collect();
    assert_eq!(pages.len(), 1);
    let page = pages[0].split("endobj").next().unwrap();
    for key in ["/MediaBox [", "/CropBox ["] {
        let bbox = page.split_once(key).unwrap().1.split_once(']').unwrap().0;
        let values: Vec<f64> = bbox
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        assert_eq!(
            values,
            [0.0, 0.0, f64::from(width) * 0.75, f64::from(height) * 0.75]
        );
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}

fn pdf_pixels(bytes: &[u8], width: u32, height: u32) -> CanonicalRgba8 {
    pdf_extent(bytes, width, height);
    CanonicalRgba8::new(
        width,
        height,
        super::png_profile_contract::pdf_rgba(bytes, width as usize, height as usize),
    )
    .unwrap()
}

fn recovery_artifact(bytes: &[u8], format: OutputFormat, row: &Value) {
    // Spec §§8.2, 11.4: PNG paints the official recipe; SVG/PDF omit its paint.
    // The source image still contributes its authored layout bounds, so this
    // is not an empty scene's 1x1 viewport. Padding and background are disabled.
    // These expectations are authored independently of the missing-file control.
    let width = row["width"].as_u64().unwrap();
    let height = row["height"].as_u64().unwrap();
    match format {
        OutputFormat::Png => {
            let actual = crate::common::raster::decode_png(bytes);
            assert_eq!(u64::from(actual.width), width);
            assert_eq!(u64::from(actual.height), height);
            // The official placeholder carries mandatory notices in the
            // application manifest (§12); it still must not carry color tags.
            let mut offset = 8;
            while offset < bytes.len() {
                let length =
                    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                let kind = &bytes[offset + 4..offset + 8];
                assert!(matches!(kind, b"IHDR" | b"IDAT" | b"IEND" | b"iTXt"));
                if kind == b"iTXt" {
                    assert!(bytes[offset + 8..offset + 8 + length]
                        .starts_with(b"excalidraw-render:artifact-manifest\0\0\0\0\0"));
                }
                offset += length + 12;
            }
            assert_eq!(offset, bytes.len());
        }
        OutputFormat::Svg => {
            let doc = roxmltree::Document::parse(std::str::from_utf8(bytes).unwrap()).unwrap();
            svg_extent(doc.root_element(), width, height);
            assert!(!doc.descendants().any(|n| n.has_tag_name("image")));
        }
        OutputFormat::Pdf => {
            pdf_extent(
                bytes,
                u32::try_from(width).unwrap(),
                u32::try_from(height).unwrap(),
            );
            assert!(!crate::common::pdf_inspect::pdf_text(bytes).contains("/Subtype /Image"));
        }
        _ => panic!("unexpected recovery format"),
    }
}
fn check(row: &Value) {
    let id = row["id"].as_str().unwrap();
    assert_eq!(row["reference"]["admitted"], true);
    assert_eq!(
        lower_hex(&Sha256::digest(decode(&row["jpeg_base64"]))),
        row["jpeg_sha256"].as_str().unwrap()
    );
    let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let output = Renderer::new().render(&scene, &request(format)).unwrap();
        let repeat = Renderer::new().render(&scene, &request(format)).unwrap();
        assert_eq!(output.artifact().bytes(), repeat.artifact().bytes());
        assert_eq!(output.report(), repeat.report());
        // Every row draws: a malformed profile or ICC sequence is ignored and
        // the image draws untagged, as the reference draws it.
        let mut report = serde_json::to_value(output.report()).unwrap();
        for d in report["diagnostics"].as_array_mut().unwrap() {
            assert!(!d["message"].as_str().unwrap().is_empty());
            d.as_object_mut().unwrap().remove("message");
        }
        assert_eq!(report, json!({"schema_version":1,"diagnostics":[]}), "{id}");
        if format == OutputFormat::Svg {
            svg_check(std::str::from_utf8(output.artifact().bytes()).unwrap(), row);
            svg_check(row["reference"]["svg"].as_str().unwrap(), row);
            continue;
        }
        let width = row["width"].as_u64().unwrap() as u32;
        let height = row["height"].as_u64().unwrap() as u32;
        let actual = if format == OutputFormat::Png {
            super::png_profile_contract::assert_untagged_png(output.artifact().bytes());
            crate::common::raster::decode_png(output.artifact().bytes()).into_canonical()
        } else {
            pdf_pixels(output.artifact().bytes(), width, height)
        };
        assert_eq!((actual.width(), actual.height()), (width, height));
        assert!(actual.rgba().chunks_exact(4).all(|p| p[3] == 255));
        let reference = raster(&decode(
            &row["reference"][if format == OutputFormat::Png {
                "png"
            } else {
                "svg_png"
            }],
        ));
        assert_eq!((reference.width(), reference.height()), (width, height));
        assert!(reference.rgba().chunks_exact(4).all(|p| p[3] == 255));
        if linear_gray(row) || (row["source_model"] == "gray" && row["signal"] == "opposite-model")
        {
            // Exact neutral anchors read from the pinned browser rasters, not
            // generated by the CMS under test. Canonical tolerance alone can
            // hide a one-code-value quantization error at a neutral point.
            let gray = if row["pattern"] == "ramp" { 93 } else { 169 };
            let anchor = [gray, gray, gray, 255];
            assert_eq!(&reference.rgba()[..4], &anchor, "{id}: reference anchor");
            assert_eq!(&actual.rgba()[..4], &anchor, "{id}: rendered anchor");
        }
        let result = compare(
            &reference,
            &actual,
            VisualClass::GeometryImage,
            &[],
            ComparisonMode::Canonical,
        )
        .unwrap();
        assert!(result.passed(), "{id}: {:?}", result.whole.ssim);
    }
}
#[test]
fn gray_precision_probe_discriminates_8bit_from_16bit() {
    use moxcms::{ColorProfile, DataColorSpace, Layout, ToneReprCurve, TransformOptions};
    use zune_jpeg::zune_core::{
        bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions,
    };
    let record = evidence();
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            r["source_model"] == "gray" && r["signal"] == "v2-g1" && r["layout"] == "single"
        })
        .collect();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let bytes = decode(&row["jpeg_base64"]);
        let expected = raster(&decode(&row["reference"]["png"]));
        let options = DecoderOptions::default()
            .set_use_unsafe(false)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(ColorSpace::Luma);
        let samples =
            zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes.as_slice()), options)
                .decode()
                .unwrap();
        let mut profile = ColorProfile::new_srgb();
        profile.color_space = DataColorSpace::Gray;
        profile.gray_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
        let target = ColorProfile::new_srgb();
        for depth in [8, 16] {
            let rgb = if depth == 8 {
                let t = profile
                    .create_transform_8bit(
                        Layout::Gray,
                        &target,
                        Layout::Rgb,
                        TransformOptions::default(),
                    )
                    .unwrap();
                let mut output = vec![0_u8; samples.len() * 3];
                t.transform(&samples, &mut output).unwrap();
                output
            } else {
                let t = profile
                    .create_transform_16bit(
                        Layout::Gray,
                        &target,
                        Layout::Rgb,
                        TransformOptions::default(),
                    )
                    .unwrap();
                let source: Vec<u16> = samples.iter().map(|&v| u16::from(v) * 257).collect();
                let mut output = vec![0_u16; samples.len() * 3];
                t.transform(&source, &mut output).unwrap();
                output
                    .into_iter()
                    .map(|v| ((u32::from(v) * 255 + 32767) / 65535) as u8)
                    .collect()
            };
            let actual = CanonicalRgba8::new(
                expected.width(),
                expected.height(),
                rgb.chunks_exact(3)
                    .flat_map(|p| [p[0], p[1], p[2], 255])
                    .collect(),
            )
            .unwrap();
            let result = compare(
                &expected,
                &actual,
                VisualClass::GeometryImage,
                &[],
                ComparisonMode::Canonical,
            )
            .unwrap();
            assert_eq!(
                result.passed(),
                depth == 16,
                "{}: depth={depth}, ssim={:?}",
                row["id"],
                result.whole.ssim
            );
        }
    }
}

fn with_profile(jpeg: &[u8], profile: &[u8]) -> Vec<u8> {
    assert_eq!(&jpeg[..2], b"\xff\xd8");
    let mut tagged = jpeg[..2].to_vec();
    tagged.extend_from_slice(b"\xff\xe2");
    tagged.extend_from_slice(&u16::try_from(profile.len() + 16).unwrap().to_be_bytes());
    tagged.extend_from_slice(b"ICC_PROFILE\0\x01\x01");
    tagged.extend_from_slice(profile);
    tagged.extend_from_slice(&jpeg[2..]);
    tagged
}

fn scene_with_jpeg(template: &Value, jpeg: &[u8], width: u32, height: u32) -> SceneDocument {
    let mut scene = template.clone();
    scene["elements"][0]["width"] = json!(width);
    scene["elements"][0]["height"] = json!(height);
    scene["files"]["asset"]["dataURL"] = json!(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    ));
    SceneDocument::parse(&serde_json::to_vec(&scene).unwrap()).unwrap()
}

#[test]
fn gray_profile_precision_handles_small_extents_and_scratch_boundaries() {
    use zune_jpeg::zune_core::{
        bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions,
    };
    let record = evidence();
    // The pinned 33x25 images already cover three full scratch blocks plus 57
    // pixels. These extents exercise no full block, exactly one, and short tails.
    for signal in ["v2-g1", "opposite-model"] {
        let row = record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| {
                r["source_model"] == "gray" && r["signal"] == signal && r["layout"] == "single"
            })
            .unwrap();
        for (width, height) in [(1_u16, 1_u16), (15, 17), (16, 16), (257, 1), (513, 1)] {
            let samples: Vec<u8> = (0..usize::from(width) * usize::from(height))
                .map(|i| [0, 32, 64, 128, 192, 255][i % 6])
                .collect();
            let mut jpeg = Vec::new();
            jpeg_encoder::Encoder::new(&mut jpeg, 100)
                .encode(&samples, width, height, jpeg_encoder::ColorType::Luma)
                .unwrap();
            let options = DecoderOptions::default()
                .set_use_unsafe(false)
                .set_strict_mode(true)
                .jpeg_set_out_colorspace(ColorSpace::Luma);
            let decoded =
                zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(jpeg.as_slice()), options)
                    .decode()
                    .unwrap();
            assert_eq!(decoded.len(), usize::from(width) * usize::from(height));
            // Independent linear-light -> sRGB transfer function, evaluated on
            // decoded JPEG samples so JPEG quantization is not a CMS error.
            // One code value allows ICC matrix/white-point rounding; no CMS or
            // production block traversal is used to generate this expectation.
            let expected: Vec<u8> = decoded
                .iter()
                .map(|&v| {
                    let linear = f64::from(v) / 255.0;
                    let encoded = if linear <= 0.0031308 {
                        12.92 * linear
                    } else {
                        1.055 * libm::pow(linear, 1.0 / 2.4) - 0.055
                    };
                    (encoded * 255.0).round() as u8
                })
                .collect();
            let tagged = with_profile(&jpeg, &decode(&row["profile_base64"]));
            let scene =
                scene_with_jpeg(&row["scene"], &tagged, u32::from(width), u32::from(height));
            for format in [OutputFormat::Png, OutputFormat::Pdf] {
                let output = Renderer::new().render(&scene, &request(format)).unwrap();
                assert_eq!(
                    serde_json::to_value(output.report()).unwrap(),
                    json!({"schema_version":1,"diagnostics":[]})
                );
                let actual = if format == OutputFormat::Png {
                    super::png_profile_contract::assert_untagged_png(output.artifact().bytes());
                    raster(output.artifact().bytes())
                } else {
                    pdf_pixels(
                        output.artifact().bytes(),
                        u32::from(width),
                        u32::from(height),
                    )
                };
                assert_eq!(
                    (actual.width(), actual.height()),
                    (u32::from(width), u32::from(height))
                );
                for (i, (pixel, &gray)) in actual.rgba().chunks_exact(4).zip(&expected).enumerate()
                {
                    assert_eq!(pixel[3], 255);
                    assert!(
                        pixel[..3].iter().all(|v| v.abs_diff(gray) <= 1),
                        "{signal}: {width}x{height}, {format:?}, pixel {i}: {pixel:?} vs {gray}"
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_gray_profiles_are_ignored_on_rgb_sources() {
    // Blink keeps only an RGB profile on a three-component image, and ignores
    // one skcms does not parse: an invalid gray profile draws untagged.
    let record = evidence();
    let row = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "rgb-baseline-opposite-model-single-ramp")
        .unwrap();
    let base = untagged_evidence();
    let source = base["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == row["base_id"])
        .unwrap();
    let profile = decode(&row["profile_base64"]);
    assert_eq!(&profile[144..148], b"kTRC");
    assert_eq!(&profile[20..24], b"XYZ ");
    assert_eq!(&profile[12..16], b"mntr");
    let width = u32::try_from(row["width"].as_u64().unwrap()).unwrap();
    let height = u32::try_from(row["height"].as_u64().unwrap()).unwrap();
    let untagged = scene_with_jpeg(
        &row["scene"],
        &decode(&source["jpeg_base64"]),
        width,
        height,
    );
    for kind in ["missing-trc", "bad-pcs", "device-link"] {
        let mut invalid = profile.clone();
        match kind {
            "missing-trc" => invalid[144..148].copy_from_slice(b"oops"),
            "bad-pcs" => invalid[20..24].copy_from_slice(b"RGB "),
            "device-link" => invalid[12..16].copy_from_slice(b"link"),
            _ => unreachable!(),
        }
        let jpeg = with_profile(&decode(&source["jpeg_base64"]), &invalid);
        let scene = scene_with_jpeg(&row["scene"], &jpeg, width, height);
        // Authored controls without browser exports: the renderer's source
        // preservation and untagged drawing are asserted directly.
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let output = Renderer::new().render(&scene, &request(format)).unwrap();
            let repeat = Renderer::new().render(&scene, &request(format)).unwrap();
            assert_eq!(
                output.artifact().bytes(),
                repeat.artifact().bytes(),
                "{kind}"
            );
            assert_eq!(output.report(), repeat.report(), "{kind}");
            assert_eq!(
                serde_json::to_value(output.report()).unwrap(),
                json!({"schema_version":1,"diagnostics":[]}),
                "{kind}"
            );
            if format == OutputFormat::Svg {
                let svg = std::str::from_utf8(output.artifact().bytes()).unwrap();
                let doc = roxmltree::Document::parse(svg).unwrap();
                svg_extent(doc.root_element(), u64::from(width), u64::from(height));
                let images: Vec<_> = doc
                    .descendants()
                    .filter(|n| n.has_tag_name("image"))
                    .collect();
                assert_eq!(images.len(), 1, "{kind}");
                let href = images[0]
                    .attribute("href")
                    .or_else(|| images[0].attribute(("http://www.w3.org/1999/xlink", "href")));
                let expected = format!(
                    "data:image/jpeg;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&jpeg)
                );
                assert_eq!(href, Some(expected.as_str()), "{kind}: exact SVG source");
            } else {
                let control = Renderer::new().render(&untagged, &request(format)).unwrap();
                assert_eq!(
                    output.artifact().bytes(),
                    control.artifact().bytes(),
                    "{kind}: untagged drawing"
                );
            }
        }
    }
}

#[test]
fn gray_trc_profile_on_ycbcr_preserves_untagged_pixels() {
    use zune_jpeg::zune_core::{bytestream::ZCursor, colorspace::ColorSpace};
    let record = evidence();
    let untagged = untagged_evidence();
    let profile_row = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "rgb-baseline-opposite-model-single-ramp")
        .unwrap();
    let profile = decode(&profile_row["profile_base64"]);
    assert_eq!(&profile[16..20], b"GRAY");
    let rows: Vec<_> = untagged["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["model"] == "ycbcr" && r["sampling"] == json!([1, 1]))
        .collect();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let jpeg = decode(&row["jpeg_base64"]);
        let mut decoder = zune_jpeg::JpegDecoder::new(ZCursor::new(jpeg.as_slice()));
        decoder.decode().unwrap();
        assert_eq!(decoder.info().unwrap().components, 3);
        assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCbCr));
        let width = row["width"].as_u64().unwrap() as u32;
        let height = row["height"].as_u64().unwrap() as u32;
        let base = scene_with_jpeg(&profile_row["scene"], &jpeg, width, height);
        let tagged = scene_with_jpeg(
            &profile_row["scene"],
            &with_profile(&jpeg, &profile),
            width,
            height,
        );
        // Metamorphic regression for three-component routing only. The frozen
        // browser matrix has no gray-tagged YCbCr control, so this does not
        // extend its browser qualification to a new model/profile combination.
        for format in [OutputFormat::Png, OutputFormat::Pdf] {
            let render =
                |scene: &SceneDocument| Renderer::new().render(scene, &request(format)).unwrap();
            let base = render(&base);
            let tagged = render(&tagged);
            assert_eq!(
                serde_json::to_value(tagged.report()).unwrap(),
                json!({"schema_version":1,"diagnostics":[]})
            );
            assert_eq!(
                tagged.artifact().bytes(),
                base.artifact().bytes(),
                "{}: {format:?}",
                row["id"]
            );
        }
    }
}

#[test]
pub(super) fn rgb_and_gamma22_gray_icc_profiles_match_both_reference_authorities() {
    let record = evidence();
    reconstruct_contract(&record);
    multipart_contract(&record);
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["expected_outcome"] == "accepted" && !linear_gray(r))
        .collect();
    assert_eq!(rows.len(), 72);
    for row in rows {
        check(row);
    }
}
#[test]
pub(super) fn malformed_profiles_and_sequences_draw_untagged_as_the_reference() {
    let record = evidence();
    reconstruct_contract(&record);
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            matches!(
                r["expected_outcome"].as_str().unwrap(),
                "unsupported-format" | "decode-failed"
            )
        })
        .collect();
    assert_eq!(rows.len(), 56);
    for row in rows {
        check(row);
    }
}
#[test]
pub(super) fn corrected_gray_precision_and_model_mismatches_match_reference() {
    let record = evidence();
    reconstruct_contract(&record);
    multipart_contract(&record);
    opposite_model_contract(&record);
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            r["expected_outcome"] == "inspect-model"
                || (r["expected_outcome"] == "accepted" && linear_gray(r))
        })
        .collect();
    assert_eq!(rows.len(), 32);
    assert_eq!(
        rows.iter()
            .filter(|r| r["expected_outcome"] == "inspect-model")
            .count(),
        8
    );
    assert_eq!(rows.iter().filter(|r| linear_gray(r)).count(), 24);
    assert_eq!(
        rows.iter()
            .map(|r| r["cell"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        16
    );
    for row in rows {
        check(row);
    }
}
#[test]
pub(super) fn multipart_profiles_preserve_semantics_and_discriminate_ignored_icc() {
    let record = evidence();
    multipart_contract(&record);
}

fn untagged_evidence() -> Value {
    let old = include_bytes!("../../../../tests/oracle/jpeg-color/reference.json");
    assert_eq!(
        lower_hex(&Sha256::digest(old)),
        "8582dd4cbd0e251fcd7eaec51143ba32188f4a3f70f832ee0805432ca1aaca26"
    );
    let old: Value = serde_json::from_slice(old).unwrap();
    assert_eq!(old["schema"], "excalidraw-render/jpeg-color-reference/v1");
    old
}

fn opposite_model_contract(record: &Value) {
    let untagged = untagged_evidence();
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["signal"] == "opposite-model")
        .collect();
    assert_eq!(rows.len(), 8);
    for row in rows {
        let base = untagged["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["id"] == row["base_id"])
            .unwrap();
        for key in ["png", "svg_png"] {
            let tagged = raster(&decode(&row["reference"][key]));
            let base = raster(&decode(&base["reference"][key]));
            if row["source_model"] == "rgb" {
                assert_eq!(tagged, base, "{}: {key}: gray profile ignored", row["id"]);
            } else {
                assert!(
                    !compare(
                        &tagged,
                        &base,
                        VisualClass::GeometryImage,
                        &[],
                        ComparisonMode::Canonical
                    )
                    .unwrap()
                    .passed(),
                    "{}: {key}: RGB profile must be applied to gray",
                    row["id"]
                );
            }
        }
    }
}

fn multipart_contract(record: &Value) {
    let old = untagged_evidence();
    let rows = record["cases"].as_array().unwrap();
    for row in rows.iter().filter(|r| r["expected_outcome"] == "accepted") {
        let single = rows
            .iter()
            .find(|r| {
                r["source_model"] == row["source_model"]
                    && r["mode"] == row["mode"]
                    && r["signal"] == row["signal"]
                    && r["pattern"] == row["pattern"]
                    && r["layout"] == "single"
            })
            .unwrap();
        for key in ["png", "svg_png"] {
            assert_eq!(
                raster(&decode(&row["reference"][key])),
                raster(&decode(&single["reference"][key]))
            );
        }
        for format in [OutputFormat::Png, OutputFormat::Pdf] {
            let render = |r: &Value| {
                let scene =
                    SceneDocument::parse(&serde_json::to_vec(&r["scene"]).unwrap()).unwrap();
                Renderer::new().render(&scene, &request(format)).unwrap()
            };
            assert_eq!(
                render(row).artifact().bytes(),
                render(single).artifact().bytes()
            );
        }
        if row["signal"] == "v2-g1" {
            let base = old["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == row["base_id"])
                .unwrap();
            let tagged = raster(&decode(&row["reference"]["png"]));
            let untagged = raster(&decode(&base["reference"]["png"]));
            assert!(
                !compare(
                    &tagged,
                    &untagged,
                    VisualClass::GeometryImage,
                    &[],
                    ComparisonMode::Canonical
                )
                .unwrap()
                .passed(),
                "ICC precedence is discriminating"
            );
        }
    }
}

#[test]
fn recovery_gates_reject_changed_extents_and_non_rgba_png() {
    let record = evidence();
    let row = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["expected_outcome"] == "decode-failed")
        .unwrap();
    // The recovery artifact: the image's asset is missing.
    let mut missing = row["scene"].clone();
    missing["files"] = json!({});
    let scene = SceneDocument::parse(&serde_json::to_vec(&missing).unwrap()).unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let output = Renderer::new().render(&scene, &request(format)).unwrap();
        let bytes = output.artifact().bytes();
        recovery_artifact(bytes, format, row);
        let mutations = match format {
            OutputFormat::Svg => {
                let original = std::str::from_utf8(bytes).unwrap();
                [
                    original
                        .replacen("width=\"33\"", "width=\"34\"", 1)
                        .into_bytes(),
                    original
                        .replacen("viewBox=\"0 0 33 25\"", "viewBox=\"1 0 33 25\"", 1)
                        .into_bytes(),
                ]
                .to_vec()
            }
            OutputFormat::Pdf => [b"/MediaBox [0 0 ".as_slice(), b"/CropBox [0 0 ".as_slice()]
                .iter()
                .map(|marker| {
                    let mut changed = crate::common::pdf_inspect::inflate_streams(bytes);
                    let at = changed
                        .windows(marker.len())
                        .position(|w| w == *marker)
                        .unwrap();
                    changed[at + marker.len()] = b'9';
                    changed
                })
                .collect(),
            OutputFormat::Png => {
                let original = crate::common::raster::decode_png(bytes);
                // Encode valid PNGs: one has the wrong extent; the other keeps
                // the visible RGB samples but drops the required alpha channel.
                [
                    (original.width + 1, png::ColorType::Rgba),
                    (original.width, png::ColorType::Rgb),
                ]
                .iter()
                .map(|&(width, color)| {
                    let pixels = if color == png::ColorType::Rgb {
                        original
                            .rgba
                            .chunks_exact(4)
                            .flat_map(|p| p[..3].iter().copied())
                            .collect()
                    } else {
                        let mut pixels = original.rgba.clone();
                        pixels.resize((width * original.height * 4) as usize, 0);
                        pixels
                    };
                    let mut changed = Vec::new();
                    {
                        let mut encoder = png::Encoder::new(&mut changed, width, original.height);
                        encoder.set_color(color);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder
                            .write_header()
                            .unwrap()
                            .write_image_data(&pixels)
                            .unwrap();
                    }
                    changed
                })
                .collect()
            }
            _ => panic!("unexpected recovery format"),
        };
        for changed in mutations {
            assert_ne!(changed, bytes);
            assert!(std::panic::catch_unwind(|| recovery_artifact(&changed, format, row)).is_err());
        }
    }
}
