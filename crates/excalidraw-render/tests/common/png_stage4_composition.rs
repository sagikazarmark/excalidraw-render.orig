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
