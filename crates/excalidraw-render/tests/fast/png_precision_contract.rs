//! Pinned official direct-PNG evidence and independently authored straight PDF
//! samples distinguish low-byte precision, full-width keys and alpha rounding.
use crate::common::lower_hex;
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const REFERENCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/png-precision/reference.json"
));
fn decode(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
fn reference() -> Value {
    assert_eq!(
        lower_hex(&Sha256::digest(REFERENCE)),
        "8400494ec5f6e744b00e175db6931aff33c2a659dc57c3b3ab62646da0cf83e5"
    );
    let evidence: Value = serde_json::from_slice(REFERENCE).unwrap();
    assert_eq!(
        evidence["schema"],
        "excalidraw-render/png-precision-reference/v1"
    );
    let sources = evidence["sources"].as_array().unwrap();
    let paths = [
        "tests/oracle/png-precision/inputs.json",
        "tests/oracle/reference-profiles/revision-1.json",
        "tools/reference-oracle/scripts/png-precision-evidence.mjs",
        "tools/reference-oracle/run-png-precision.sh",
    ];
    assert_eq!(sources.len(), paths.len());
    for (source, path) in sources.iter().zip(paths) {
        assert_eq!(source["path"], path);
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path),
        )
        .unwrap();
        assert_eq!(source["sha256"], lower_hex(&Sha256::digest(bytes)));
    }
    assert_eq!(evidence["cases"].as_array().unwrap().len(), 30);
    evidence
}
fn check_group(group: &str, count: usize) {
    let evidence = reference();
    let rows: Vec<_> = evidence["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["obligation"] == group)
        .collect();
    assert_eq!(rows.len(), count);
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
        let source = row["scene"]["files"]["asset"]["dataURL"].as_str().unwrap();
        let source_bytes = base64::engine::general_purpose::STANDARD
            .decode(source.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        assert_eq!(
            row["source_sha256"],
            lower_hex(&Sha256::digest(&source_bytes))
        );
        let width = row["width"].as_u64().unwrap() as usize;
        let height = row["height"].as_u64().unwrap() as usize;
        let straight = decode(&row["straight_rgba"]);
        assert_eq!(straight.len(), width * height * 4);
        let expected_png = crate::common::raster::decode_png(&decode(&row["png_base64"]));
        assert_eq!(
            (expected_png.width as usize, expected_png.height as usize),
            (width, height)
        );
        for format in [OutputFormat::Svg, OutputFormat::Pdf, OutputFormat::Png] {
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .include_background(row["options"]["exportBackground"].as_bool().unwrap())
                .build()
                .unwrap();
            let first = Renderer::new().render(&scene, &request).unwrap();
            let second = Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(
                first.artifact().bytes(),
                second.artifact().bytes(),
                "{id} {format:?}"
            );
            assert_eq!(first.report(), second.report());
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
            assert_eq!(
                serde_json::to_value(first.report()).unwrap(),
                json!({"schema_version":1,"diagnostics":[]}),
                "{id} {format:?}"
            );
            match format {
                OutputFormat::Svg => {
                    let xml = roxmltree::Document::parse(
                        std::str::from_utf8(first.artifact().bytes()).unwrap(),
                    )
                    .unwrap();
                    let image = xml.descendants().find(|n| n.has_tag_name("image")).unwrap();
                    assert_eq!(image.attribute("href").unwrap(), source, "{id}");
                }
                OutputFormat::Pdf => super::png_profile_contract::assert_pdf_pixels(
                    first.artifact().bytes(),
                    width,
                    height,
                    &straight,
                ),
                OutputFormat::Png => {
                    super::png_profile_contract::assert_untagged_png(first.artifact().bytes());
                    let actual = crate::common::raster::decode_png(first.artifact().bytes());
                    assert_eq!(
                        (actual.width, actual.height),
                        (expected_png.width, expected_png.height),
                        "{id}"
                    );
                    let differences: Vec<_> = actual
                        .rgba
                        .chunks_exact(4)
                        .zip(expected_png.rgba.chunks_exact(4))
                        .enumerate()
                        .filter(|(_, (a, b))| a != b)
                        .take(8)
                        .collect();
                    assert!(
                        differences.is_empty(),
                        "{id}: first differing pixels (actual, reference): {differences:?}"
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
#[test]
fn sixteen_bit_precision_matches_pinned_reference() {
    check_group("precision", 13);
}
#[test]
fn sixteen_bit_transparency_keys_compare_before_reduction() {
    check_group("transparency-key", 8);
}
#[test]
fn alpha_rounding_and_compositing_match_pinned_reference() {
    check_group("alpha", 9);
}

fn render_strip(pixels: &[[u8; 4]], background: Option<&str>) -> Vec<u8> {
    let png = crate::common::codec_fixtures::png_rgba(pixels.len() as u32, 1, pixels);
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    let scene = SceneDocument::parse(&serde_json::to_vec(&json!({
        "type":"excalidraw", "version":2,
        "elements":[{"id":"image","type":"image","width":pixels.len(),"height":1,"fileId":"asset","status":"saved","scale":[1,1],"updated":1}],
        "appState":{"viewBackgroundColor":background.unwrap_or("#ffffff")}, "files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":data,"created":1}}
    })).unwrap()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Png)
        .padding(0.0)
        .include_background(background.is_some())
        .build()
        .unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    crate::common::raster::decode_png(outcome.artifact().bytes()).rgba
}

#[test]
fn canvas_unpremultiplication_matches_float32_rounding() {
    // Pixels (3, 6) and (1, 10) from the exhaustive reference grid. Their blue
    // fractions are 5/6 and 3/10: exact integer half-tie rounding alone cannot
    // reproduce the canvas's normalization/unpremultiplication/store sequence.
    assert_eq!(
        render_strip(&[[3, 252, 219, 6], [1, 254, 73, 10]], None),
        [0, 255, 212, 6, 0, 255, 77, 10]
    );
}

#[test]
fn canvas_low_alpha_blit_matches_byte_source_over() {
    assert_eq!(
        render_strip(&[[0, 255, 0, 1]], Some("#17395b")),
        [22, 57, 90, 255]
    );
}
