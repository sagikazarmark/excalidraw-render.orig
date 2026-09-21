//! Independent, source-bound JPEG ICC evidence: 80 cells / 160 vectors from
//! official v0.18.1 and Chromium 149, collected twice in fresh forward/reverse
//! processes. The pinned record binds its collector, authored profiles, base
//! JPEG inputs, browser profile and canonical comparator policy. Test-side APP2
//! reconstruction reads the actual segment bytes without the renderer parser.
//! Browser PNG and browser-rasterized SVG are separate color authorities; SVG
//! source preservation, PNG RGBA8/opacity and PDF extent/opacity are explicit.
//! #188 remains unqualified: 24 gray gamma-1 vectors retain 48 exact PNG/PDF
//! failure pins, and eight opposite-model vectors assert reported recovery or
//! acceptance only, never color correctness. Recovery grants no tolerance
//! exception. This new test source remains unbound until ownership is recorded.
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
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
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
        format!("{:x}", Sha256::digest(bytes)),
        "7ffd57eaa8ddfe4a7d311c5a55c57b84a001463864ce6bd235613faab85e76d5"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/jpeg-icc-reference/v1");
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            source["sha256"].as_str().unwrap()
        );
    }
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
        format!("{:x}", Sha256::digest(inputs)),
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
            format!("{:x}", Sha256::digest(&profile)),
            row["profile_sha256"].as_str().unwrap()
        );
        let source = decode(&base["jpeg_base64"]);
        assert_eq!(&source[..2], &[0xff, 0xd8]);
        let jpeg = decode(&row["jpeg_base64"]);
        assert_eq!(
            format!("{:x}", Sha256::digest(&jpeg)),
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
fn gray_failure(row: &Value) -> bool {
    row["source_model"] == "gray" && matches!(row["signal"].as_str().unwrap(), "v2-g1" | "v4-g1")
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
        format!("{:x}", Sha256::digest(decode(&row["jpeg_base64"]))),
        row["jpeg_sha256"].as_str().unwrap()
    );
    let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let output = Renderer::new().render(&scene, &request(format)).unwrap();
        let repeat = Renderer::new().render(&scene, &request(format)).unwrap();
        assert_eq!(output.artifact().bytes(), repeat.artifact().bytes());
        assert_eq!(output.report(), repeat.report());
        let outcome = row["expected_outcome"].as_str().unwrap();
        let recovery = if outcome == "decode-failed"
            || (outcome == "unsupported-format" && format != OutputFormat::Svg)
        {
            Some(outcome)
        } else if outcome == "inspect-model"
            && row["source_model"] == "rgb"
            && format != OutputFormat::Svg
        {
            Some("decode-failed")
        } else {
            None
        };
        let mut report = serde_json::to_value(output.report()).unwrap();
        for d in report["diagnostics"].as_array_mut().unwrap() {
            assert!(!d["message"].as_str().unwrap().is_empty());
            d.as_object_mut().unwrap().remove("message");
        }
        if let Some(code) = recovery {
            assert_eq!(
                report,
                json!({"schema_version":1,"diagnostics":[{
                    "code":format!("image.{code}"),"severity":"warning","format":format.as_str(),"effects":["visual"],
                    "element_id":"image","json_pointer":"/files/asset/dataURL","details":{"kind":"asset","asset_id":"asset",
                    "claimed_media_type":null,"validated_media_type":if code == "decode-failed" {json!("image/jpeg")} else {Value::Null},
                    "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"},
                    "recipe_id":if format == OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}
                }]}),
                "{id}"
            );
            let mut missing = row["scene"].clone();
            missing["files"] = json!({});
            let missing = SceneDocument::parse(&serde_json::to_vec(&missing).unwrap()).unwrap();
            let control = Renderer::new().render(&missing, &request(format)).unwrap();
            assert_eq!(
                output.artifact().bytes(),
                control.artifact().bytes(),
                "{id}"
            );
            recovery_artifact(output.artifact().bytes(), format, row);
            continue;
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
        if outcome == "inspect-model" {
            continue;
        } // #188, no color qualification claimed.
        let reference = raster(&decode(
            &row["reference"][if format == OutputFormat::Png {
                "png"
            } else {
                "svg_png"
            }],
        ));
        assert_eq!((reference.width(), reference.height()), (width, height));
        assert!(reference.rgba().chunks_exact(4).all(|p| p[3] == 255));
        let result = compare(
            &reference,
            &actual,
            VisualClass::GeometryImage,
            &[],
            ComparisonMode::Canonical,
        )
        .unwrap();
        if gray_failure(row) {
            assert!(!result.passed(), "#188 pin stale: {id}");
            assert_eq!(result.whole.changed_pixels, 0);
            let SsimOutcome::Score(score) = result.whole.ssim else {
                panic!("score")
            };
            assert_eq!(
                score.to_bits(),
                if row["pattern"] == "ramp" {
                    0x3fefeeb8ab8d6588
                } else {
                    0x3feff7526521e20c
                },
                "{id}"
            );
        } else {
            assert!(result.passed(), "{id}: {:?}", result.whole.ssim);
        }
    }
}
#[test]
fn rgb_and_gamma22_gray_icc_profiles_match_both_reference_authorities() {
    let record = evidence();
    reconstruct_contract(&record);
    multipart_contract(&record);
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["expected_outcome"] == "accepted" && !gray_failure(r))
        .collect();
    assert_eq!(rows.len(), 72);
    for row in rows {
        check(row);
    }
}
#[test]
fn malformed_profiles_and_sequences_have_exact_recovery_and_source_preservation() {
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
fn issue_188_gray_precision_and_model_mismatches_remain_unqualified() {
    let record = evidence();
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            r["expected_outcome"] == "inspect-model"
                || (r["expected_outcome"] == "accepted" && gray_failure(r))
        })
        .collect();
    assert_eq!(rows.len(), 32);
    assert_eq!(
        rows.iter()
            .filter(|r| r["expected_outcome"] == "inspect-model")
            .count(),
        8
    );
    assert_eq!(rows.iter().filter(|r| gray_failure(r)).count() * 2, 48);
    for row in rows {
        check(row);
    }
}
#[test]
fn multipart_profiles_preserve_semantics_and_discriminate_ignored_icc() {
    let record = evidence();
    multipart_contract(&record);
}

fn multipart_contract(record: &Value) {
    let old = include_bytes!("../../../../tests/oracle/jpeg-color/reference.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(old)),
        "9b5ab60a18e83b52c582ccb38728b71633b11b592ffe540bab2c796857694638"
    );
    let old: Value = serde_json::from_slice(old).unwrap();
    assert_eq!(old["schema"], "excalidraw-render/jpeg-color-reference/v1");
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
    let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
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
                    let mut changed = bytes.to_vec();
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
