use serde_json::Value;
use sha2::{Digest, Sha256};
pub fn evidence() -> Value {
    let bytes = include_bytes!("../../../../tests/oracle/png-color/stage4-oracle-reference.json");
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        "09d183cbad21aee74f9562055d3ea0872a2105293639f8a9a16dc77f085986cf"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 1968);
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1968
    );
    record
}
/// The supplemental SDR composition record (#85 codec:png:alpha/color-profile):
/// the sRGB chunk, v2 matrix/TRC ICC, SDR/linear/BT.709/Display-P3 and
/// narrow-range cICP crossed with every RGB/gray/palette layout, transparency,
/// interlace and container, plus gray gAMA/cICP/PQ, all with early eXIf.
pub fn supplemental_evidence() -> Value {
    let bytes = include_bytes!(
        "../../../../tests/oracle/png-color/supplemental-composition-reference.json"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        "cd5ea8733c8d2e29d843e464089c69c587574ff6f63db75fced80c2c44c7c469"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 840);
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 840);
    assert!(ids.iter().all(|id| id.starts_with("supplement-cross-")));
    assert!(rows.iter().all(|r| r["group"] == "composition"));
    record
}
pub fn report(row: &Value, format: &str, actual: Value) {
    let case = super::png_stage4::Case {
        id: row["id"].as_str().unwrap().into(),
        bytes: vec![],
        width: row["width"].as_u64().unwrap() as usize,
        height: row["height"].as_u64().unwrap() as usize,
        pixels: vec![],
        outcome: super::png_stage4::Outcome::Accepted(
            row["animation_frames"].as_u64().unwrap() as u32
        ),
    };
    super::png_stage4::report(&case, format, actual);
}

// Also compiled as a module of the library under wasm32, where `common` is
// not in scope, so the helper is local like its siblings'.
fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
