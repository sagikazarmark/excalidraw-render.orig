//! Canvas image resampling against the pinned reference (#198). The record
//! (`tests/oracle/image-resample/reference.json`, collected by
//! `tools/reference-oracle/run-image-resample.sh`) draws seeded images at
//! other than their natural size, with no padding and no background: strong
//! and mild minification, magnification, anisotropic scales, fractional sizes
//! and offsets, crops, opacity, and one image drawn at several sizes in one
//! scene. Every PNG must equal the reference exactly.
use excalidraw_render::OutputFormat;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::webp_sample_contract::render;
use crate::common::lower_hex;

const RECORD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/image-resample/reference.json"
));

fn hash(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn evidence() -> Value {
    assert_eq!(
        hash(RECORD),
        "38dd62100a314ebf79863c6deb2685a0d9a59e3e3958ee5b6f7970247983073e"
    );
    let record: Value = serde_json::from_slice(RECORD).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/image-resample-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], hash(&bytes), "{}", source["path"]);
    }
    assert_eq!(record["cases"].as_array().unwrap().len(), 73);
    record
}

fn scene(row: &Value, image: &str) -> Value {
    let elements: Vec<Value> = row["elements"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, e)| {
            json!({"id":format!("image{index}"),"type":"image","x":e["x"],"y":e["y"],"width":e["width"],
                "height":e["height"],"opacity":e.get("opacity").cloned().unwrap_or(json!(100)),
                "crop":e.get("crop").cloned().unwrap_or(Value::Null),"fileId":"asset","status":"saved",
                "scale":[1,1],"updated":1,"index":format!("a{index}")})
        })
        .collect();
    json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/png","created":1,
            "dataURL":format!("data:image/png;base64,{image}")}}})
}

/// Judges the rows of `groups`; returns how many.
fn check(groups: &[&str]) -> usize {
    use base64::Engine as _;
    let record = evidence();
    let mut failures = Vec::new();
    let mut checked = 0;
    for row in record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| groups.contains(&r["group"].as_str().unwrap()))
    {
        let id = row["id"].as_str().unwrap();
        let image = record["images"][row["image"].as_str().unwrap()]
            .as_str()
            .unwrap();
        let (bytes, report) = render(&scene(row, image), OutputFormat::Png);
        assert_eq!(report["diagnostics"], json!([]), "{id}: {report}");
        let expected = crate::common::raster::decode_png(
            &base64::engine::general_purpose::STANDARD
                .decode(row["reference"]["png"].as_str().unwrap())
                .unwrap(),
        );
        let actual = crate::common::raster::decode_png(&bytes);
        if (actual.width, actual.height) != (expected.width, expected.height) {
            failures.push(format!(
                "{id}: size {}x{} != {}x{}",
                actual.width, actual.height, expected.width, expected.height
            ));
        } else if actual.rgba != expected.rgba {
            let differing = actual
                .rgba
                .chunks_exact(4)
                .zip(expected.rgba.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            failures.push(format!("{id}: {differing} differing pixels"));
        }
        checked += 1;
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    checked
}

/// Every image drawn at other than its natural size resamples as the pinned
/// canvas does: pre-scaled levels, their construction and the final draw.
#[test]
fn raster_images_resample_exactly_as_the_reference_canvas() {
    let groups = [
        "minify",
        "natural",
        "magnify",
        "anisotropic",
        "fractional",
        "crop",
        "opacity",
        "multi",
    ];
    assert_eq!(check(&groups), 73);
}
