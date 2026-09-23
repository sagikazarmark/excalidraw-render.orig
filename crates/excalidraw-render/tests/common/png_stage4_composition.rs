use serde_json::Value;
use sha2::{Digest, Sha256};
pub fn evidence() -> Value {
    let bytes = include_bytes!("../../../../tests/oracle/png-color/stage4-oracle-reference.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "9de893992bef08f966c0bb149f0cd61f3c20d6f3c2e701ded5706b39c60953f3"
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
        format!("{:x}", Sha256::digest(bytes)),
        "58f2b7381540dc926a1538b4de276bbd30c532ba0b93761fd768145e4e15d4bb"
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
