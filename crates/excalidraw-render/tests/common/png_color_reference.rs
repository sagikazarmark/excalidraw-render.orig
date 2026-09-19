//! Portable evidence gates shared by native contracts and the browser worker.
use base64::Engine as _;
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const REFERENCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/png-color/reference.json"
));
pub fn evidence() -> Value {
    assert_eq!(
        format!("{:x}", Sha256::digest(REFERENCE)),
        "30a754e614199e6fdb325613af4802ba489f386ce899e5503641489730371555"
    );
    let record: Value = serde_json::from_slice(REFERENCE).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/png-color-reference/v1");
    assert_eq!(record["visual_class"], "geometry-image");
    assert_eq!(record["cases"].as_array().unwrap().len(), 676);
    let rows = record["cases"].as_array().unwrap();
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), rows.len());
    let mut expected = std::collections::BTreeSet::new();
    for p in [1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 22] {
        for t in [1, 4, 5, 6, 8, 11, 13, 14, 15] {
            for r in [0, 1] {
                for depth in [8, 16] {
                    expected.insert(format!("cicp-p{p}-t{t}-r{r}-c6-d{depth}"));
                }
            }
        }
    }
    let actual: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|r| r["group"] == "cicp")
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(actual, expected);
    transfer_equivalence_and_discrimination(&record);
    record
}

/// The four newly qualified H.273 codes select the same reference EOTF as
/// transfer 1. Keep this equivalence exact on both reference and facade output.
pub fn bt1886_equivalent<'a>(record: &'a Value, row: &Value) -> Option<&'a Value> {
    if row["group"] != "cicp" {
        return None;
    }
    let cicp = &row["tags"][0]["data"];
    if !matches!(cicp[1].as_u64().unwrap(), 6 | 11 | 14 | 15) {
        return None;
    }
    let id = format!(
        "cicp-p{}-t1-r{}-c6-d{}",
        cicp[0], cicp[3], row["layout"]["depth"]
    );
    Some(
        record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap(),
    )
}

fn transfer_equivalence_and_discrimination(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    let image = |id: &str, kind: &str| {
        let row = rows.iter().find(|r| r["id"] == id).unwrap();
        png(&base64(&row["reference"][kind]))
    };
    let mut count = 0;
    for row in rows {
        let Some(equivalent) = bt1886_equivalent(record, row) else {
            continue;
        };
        count += 1;
        for kind in ["png", "opaque_png"] {
            assert_eq!(
                row["reference"][kind], equivalent["reference"][kind],
                "{} {kind} BT.1886",
                row["id"]
            );
            if row["tags"][0]["data"][3] == 0 {
                let id = format!("untagged-c6-d{}", row["layout"]["depth"]);
                assert_eq!(
                    image(row["id"].as_str().unwrap(), kind),
                    image(&id, kind),
                    "{} range fallback",
                    row["id"]
                );
            }
        }
    }
    assert_eq!(count, 176);
    for transfer in [6, 11, 14, 15] {
        for depth in [8, 16] {
            let right = format!("cicp-p1-t{transfer}-r1-c6-d{depth}");
            // The authored (9,6) neutral sample distinguishes BT.1886 from
            // sRGB identity (128), linear (188), and the inverse camera OETF.
            let opaque = image(&right, "opaque_png");
            assert_eq!(&opaque.rgba()[99 * 4..100 * 4], &[121, 121, 121, 255]);
            for wrong in [4, 5, 8, 13] {
                let wrong = format!("cicp-p1-t{wrong}-r1-c6-d{depth}");
                let result = compare(
                    &image(&right, "png"),
                    &image(&wrong, "png"),
                    VisualClass::GeometryImage,
                    &[],
                    ComparisonMode::Canonical,
                )
                .unwrap();
                assert!(!result.passed(), "{right} does not discriminate {wrong}");
            }
        }
    }
}
pub fn base64(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
pub fn png(bytes: &[u8]) -> CanonicalRgba8 {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut data = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut data).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    data.truncate(info.buffer_size());
    assert!(data
        .chunks_exact(4)
        .all(|p| p[3] != 0 || p[..3] == [0, 0, 0]));
    CanonicalRgba8::new(info.width, info.height, data).unwrap()
}
pub fn recovery<'a>(row: &'a Value, format: &str) -> Option<&'a str> {
    match row["expected_outcome"].as_str().unwrap() {
        "accepted" => None,
        "unsupported-format" if format == "svg" => None,
        code => Some(code),
    }
}
pub fn report(row: &Value, format: &str, mut actual: Value) {
    for diagnostic in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(!diagnostic["message"].as_str().unwrap().is_empty());
        diagnostic.as_object_mut().unwrap().remove("message");
    }
    let diagnostics = recovery(row, format).map_or_else(|| json!([]), |code| json!([{
        "code":format!("image.{code}"), "severity":"warning", "format":format,
        "effects":["visual"],
        "element_id":"image", "json_pointer":"/files/asset/dataURL",
        "details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,
            "validated_media_type":if code == "decode-failed" {json!("image/png")} else {Value::Null},
            "recovery":if format == "png" {"placeholder"} else {"omitted"},
            "recipe_id":if format == "png" {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}
    }]));
    assert_eq!(
        actual,
        json!({"schema_version":1,"diagnostics":diagnostics}),
        "{} {format}",
        row["id"]
    );
}
fn identity(row: &Value) -> bool {
    let signal = row["signal"].as_str().unwrap();
    matches!(
        signal,
        "untagged"
            | "srgb-0"
            | "srgb-1"
            | "srgb-2"
            | "srgb-3"
            | "srgb-empty"
            | "srgb-long"
            | "gama-43182"
            | "gama-45455"
            | "gama-47727"
            | "srgb-over-gama"
            | "gama-after-srgb"
            | "cicp-over-all"
            | "srgb-over-chrm"
            | "cicp-over-chrm"
            | "cicp-p1-t13-r1"
    ) || signal.ends_with("-default")
        || signal.ends_with("-r0")
}
pub fn appearance(row: &Value, actual: CanonicalRgba8, pdf: bool) {
    assert_eq!(
        (actual.width(), actual.height()),
        (
            row["width"].as_u64().unwrap() as u32,
            row["height"].as_u64().unwrap() as u32
        )
    );
    assert_eq!(row["reference"]["admitted"], true);
    let expected = png(&base64(
        &row["reference"][if pdf { "opaque_png" } else { "png" }],
    ));
    let mut expected_bytes = expected.rgba().to_vec();
    for ((actual_pixel, expected_pixel), alpha) in actual
        .rgba()
        .chunks_exact(4)
        .zip(expected_bytes.chunks_exact_mut(4))
        .zip(row["alpha"].as_array().unwrap())
    {
        let alpha = alpha.as_u64().unwrap() as u8;
        assert_eq!(actual_pixel[3], alpha, "{} alpha", row["id"]);
        if pdf {
            expected_pixel[3] = alpha;
        } else {
            assert_eq!(expected_pixel[3], alpha);
        }
        if alpha == 0 {
            assert_eq!(&actual_pixel[..3], &[0, 0, 0]);
            expected_pixel[..3].fill(0);
        }
    }
    let expected =
        CanonicalRgba8::new(expected.width(), expected.height(), expected_bytes).unwrap();
    if identity(row) {
        assert_eq!(
            actual.rgba(),
            expected.rgba(),
            "{} exact identity",
            row["id"]
        );
    } else {
        let result = compare(
            &expected,
            &actual,
            VisualClass::GeometryImage,
            &[],
            ComparisonMode::Canonical,
        )
        .unwrap();
        assert!(result.passed(), "{} {pdf}: {:?}", row["id"], result.whole);
    }
}
