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
        lower_hex(&Sha256::digest(REFERENCE)),
        "0ce509fd2a143945a9eeecfcd45683584e1562a538bb8211cc531db48d1ce53e"
    );
    let record: Value = serde_json::from_slice(REFERENCE).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/png-color-reference/v1");
    assert_eq!(record["visual_class"], "geometry-image");
    assert_eq!(record["cases"].as_array().unwrap().len(), 984);
    let rows = record["cases"].as_array().unwrap();
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), rows.len());
    let mut expected = std::collections::BTreeSet::new();
    for p in [1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 22] {
        for t in [1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18] {
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
    remaining_transfer_discrimination(&record);
    hdr_transfer_discrimination(&record);
    record
}

/// Valid stage-2 signaling which the pinned decoder ignores. These source
/// fields have no lower metadata, so they must match the untagged control.
pub fn ignored_cicp_equivalent<'a>(record: &'a Value, row: &Value) -> Option<&'a Value> {
    if row["group"] != "cicp" {
        return None;
    }
    let cicp = &row["tags"][0]["data"];
    let transfer = cicp[1].as_u64().unwrap();
    if !matches!(transfer, 9 | 10 | 12) && !(matches!(transfer, 7 | 16 | 17 | 18) && cicp[3] == 0) {
        return None;
    }
    let id = format!("untagged-c6-d{}", row["layout"]["depth"]);
    Some(
        record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap(),
    )
}

fn remaining_transfer_discrimination(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    let image = |id: &str, kind: &str| {
        let row = rows.iter().find(|r| r["id"] == id).unwrap();
        png(&base64(&row["reference"][kind]))
    };
    let mut count = 0;
    for row in rows {
        if let Some(control) = ignored_cicp_equivalent(record, row) {
            count += 1;
            for kind in ["png", "opaque_png"] {
                assert_eq!(
                    image(row["id"].as_str().unwrap(), kind),
                    image(control["id"].as_str().unwrap(), kind),
                    "{} {kind} ignored signaling",
                    row["id"]
                );
            }
        }
    }
    assert_eq!(count, 220);
    for (transfer, neutral) in [(7, 141), (17, 118)] {
        for depth in [8, 16] {
            let right = format!("cicp-p1-t{transfer}-r1-c6-d{depth}");
            // Independently calculated EOTF -> sRGB at the authored neutral
            // (9,6) sample: SMPTE240's piecewise curve, and (52.37/48)*V^2.6.
            let opaque = image(&right, "opaque_png");
            assert_eq!(
                &opaque.rgba()[99 * 4..100 * 4],
                &[neutral, neutral, neutral, 255]
            );
            for wrong in [1, 4, 5, 8, 13, if transfer == 7 { 17 } else { 7 }] {
                // Cinema and BT.1886 fields fall inside the canonical budget;
                // their distinct neutral values are pinned exactly above and
                // on facade output below instead of tightening that budget.
                if transfer == 17 && wrong == 1 {
                    continue;
                }
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

fn hdr_transfer_discrimination(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    for (transfer, neutral) in [(16, 132), (18, 99)] {
        for depth in [8, 16] {
            let id = format!("cicp-p1-t{transfer}-r1-c6-d{depth}");
            let row = rows.iter().find(|r| r["id"] == id).unwrap();
            let opaque = png(&base64(&row["reference"]["opaque_png"]));
            assert_eq!(
                &opaque.rgba()[99 * 4..100 * 4],
                &[neutral, neutral, neutral, 255]
            );
            for wrong in [1, 8, 13, if transfer == 16 { 18 } else { 16 }] {
                let other = format!("cicp-p1-t{wrong}-r1-c6-d{depth}");
                let other = rows.iter().find(|r| r["id"] == other).unwrap();
                assert!(
                    !compare(
                        &png(&base64(&row["reference"]["png"])),
                        &png(&base64(&other["reference"]["png"])),
                        VisualClass::GeometryImage,
                        &[],
                        ComparisonMode::Canonical
                    )
                    .unwrap()
                    .passed(),
                    "{id} does not distinguish transfer {wrong}"
                );
            }
        }
    }
}

pub fn gain_map_evidence() -> Value {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/oracle/png-color/gain-map-reference.json"
    ));
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        "c68882cdbaa86ca3c03b7911ec93408c183f3d0073fb7c7f1ecbc74484b0d5b2"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/png-color-reference/v1");
    assert_eq!(record["visual_class"], "geometry-image");
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 108);
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 108);
    let names = [
        "absent",
        "sdr-black",
        "sdr-white",
        "sdr-checker",
        "hdr-black",
        "hdr-white",
        "hdr-checker",
        "multichannel",
        "alternate-space",
        "metadata-only",
        "payload-only",
        "empty-metadata",
        "short-metadata",
        "future-version",
        "missing-inner-metadata",
        "truncated-inner-metadata",
        "zero-denominator",
        "zero-gamma",
        "duplicate-metadata",
        "duplicate-payload",
        "empty-payload",
        "non-png-payload",
        "truncated-payload",
        "bad-inner-crc",
        "huge-inner-dimensions",
        "recursive-payload",
        "split-payload",
        "late-payload",
        "bad-outer-crc",
        "pq",
        "hlg",
        "pq-over-all",
        "hlg-over-all",
        "pq-narrow-icc",
        "hlg-narrow-legacy",
        "icc-over-srgb",
        "srgb-over-legacy",
        "chrm-legacy",
        "invalid-cicp",
        "invalid-icc",
        "duplicate-cicp",
        "invalid-legacy",
    ];
    let expected: std::collections::BTreeSet<_> = names
        .iter()
        .flat_map(|name| [8, 16].map(|d| format!("gain-{name}-c6-d{d}")))
        .collect();
    let actual: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|r| r["group"] == "gain-map")
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(actual, expected);
    let colors = evidence();
    for row in rows {
        if row["group"] == "gain-map-controls" {
            let original = colors["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == row["id"])
                .unwrap();
            let mut restored = row.clone();
            restored["group"] = original["group"].clone();
            assert_eq!(
                &restored, original,
                "supplement changed its independent base control"
            );
        }
        if let Some(control) = gain_map_equivalent(&record, row) {
            for kind in ["png", "opaque_png"] {
                assert_eq!(
                    row["reference"][kind], control["reference"][kind],
                    "{} {kind} base selection",
                    row["id"]
                );
            }
        }
    }
    // With an HDR base and max gain 16, a white gain map would divide linear
    // neutral 128 by 16 for SDR (~31). The observed 128 rules that path out;
    // the selected Rust PNG decoder ignores gmAP/gdAT, even for an HDR base.
    for depth in [8, 16] {
        for name in ["sdr-white", "hdr-white", "hdr-black", "hdr-checker"] {
            let id = format!("gain-{name}-c6-d{depth}");
            let row = rows.iter().find(|r| r["id"] == id).unwrap();
            let image = png(&base64(&row["reference"]["opaque_png"]));
            assert_eq!(&image.rgba()[99 * 4..100 * 4], &[128, 128, 128, 255]);
        }
    }
    record
}

pub fn gain_map_equivalent<'a>(record: &'a Value, row: &Value) -> Option<&'a Value> {
    if row["group"] != "gain-map" || row["expected_outcome"] != "accepted" {
        return None;
    }
    Some(
        record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == row["control_id"])
            .unwrap(),
    )
}

/// The supplemental HDR peak-luminance record (#85 codec:png:hdr-gain-map):
/// cLLI/mDCV/gain-map chunks inserted into unchanged main-record PQ/HLG bases
/// and SDR controls. Reference-side, every row equals its declared equivalent
/// exactly, and the distinct peak classes of one base are pairwise distinct.
pub fn hdr_metadata_evidence() -> Value {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/oracle/png-color/hdr-metadata-reference.json"
    ));
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        "65d3136c2c279a6d56ae71d7bfea11c63553e95effd0372c54505703f9a246f8"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/png-color-reference/v1");
    assert_eq!(record["visual_class"], "geometry-image");
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 170);
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), rows.len());
    let colors = evidence();
    let by_id = |id: &Value| rows.iter().find(|r| &r["id"] == id).unwrap();
    for row in rows {
        assert_eq!(row["reference"]["admitted"], true, "{}", row["id"]);
        if row["group"] == "hdr-metadata-controls" {
            let original = colors["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == row["id"])
                .unwrap();
            let mut restored = row.clone();
            restored["group"] = original["group"].clone();
            assert_eq!(
                &restored, original,
                "supplement changed its independent base control"
            );
        } else if !row["equivalent_id"].is_null() {
            let equivalent = by_id(&row["equivalent_id"]);
            for kind in ["png", "opaque_png"] {
                assert_eq!(
                    row["reference"][kind], equivalent["reference"][kind],
                    "{} {kind} equals {}",
                    row["id"], row["equivalent_id"]
                );
            }
        }
    }
    // Each HDR base's distinct peak classes, and its default control, differ
    // pairwise in the reference: the peak is observable, not assumed.
    let distinct = [
        "clli-203",
        "clli-500",
        "clli-1234.5678",
        "clli-4000",
        "clli-10000",
    ];
    for base in ["cicp-p9-t16-r1", "cicp-p9-t18-r1", "cicp-p1-t16-r1"] {
        for depth in [8, 16] {
            let control = format!("{base}-c6-d{depth}");
            let mut seen = std::collections::BTreeSet::new();
            for id in std::iter::once(control)
                .chain(distinct.map(|v| format!("hdr-{base}-{v}-c6-d{depth}")))
            {
                let row = by_id(&json!(id));
                assert!(
                    seen.insert(row["reference"]["opaque_png"].as_str().unwrap().to_owned()),
                    "{id} does not discriminate its peak class"
                );
            }
        }
    }
    record
}

/// A supplemental HDR metadata row's exact equivalent: its declared
/// equivalent peak class, or its base control.
pub fn hdr_metadata_equivalent<'a>(record: &'a Value, row: &Value) -> Option<&'a Value> {
    if row["group"] != "hdr-metadata" || row["equivalent_id"].is_null() {
        return None;
    }
    Some(
        record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == row["equivalent_id"])
            .unwrap(),
    )
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
    let mut data = vec![0; reader.output_buffer_size().expect("PNG buffer fits")];
    let info = reader.next_frame(&mut data).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    data.truncate(info.buffer_size());
    assert!(data
        .as_chunks::<4>()
        .0
        .iter()
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
    if row["group"] == "cicp" && matches!(row["tags"][0]["data"][1].as_u64().unwrap(), 9 | 10 | 12)
    {
        return true;
    }
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
    if row["group"] == "cicp" {
        let cicp = &row["tags"][0]["data"];
        if cicp[0] == 1 && cicp[3] == 1 {
            if let Some(neutral) = match cicp[1].as_u64().unwrap() {
                7 => Some(141),
                17 => Some(118),
                16 => Some(132),
                18 => Some(99),
                _ => None,
            } {
                assert_eq!(
                    &actual.rgba()[99 * 4..100 * 4],
                    &[neutral, neutral, neutral, 255],
                    "{} exact transfer anchor",
                    row["id"]
                );
            }
        }
    }
    let expected = png(&base64(
        &row["reference"][if pdf { "opaque_png" } else { "png" }],
    ));
    let mut expected_bytes = expected.rgba().to_vec();
    for ((actual_pixel, expected_pixel), alpha) in actual
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
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

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
