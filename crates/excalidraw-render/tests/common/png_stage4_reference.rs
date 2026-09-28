use serde_json::Value;
use sha2::{Digest, Sha256};
pub fn evidence() -> Value {
    let bytes = include_bytes!("../../../../tests/oracle/png-color/stage4-reference.json");
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        "7dc27033eaad05f06d1d259f55f0fdbd700b750f0218c1e74602ebdd5bcac174"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(record["cases"].as_array().unwrap().len(), 186);
    assert_eq!(record["visual_class"], "geometry-image");
    let rows = record["cases"].as_array().unwrap();
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 186);
    for row in rows {
        if row["expected_outcome"] == "accepted" {
            let image = super::png_color_reference::png(&super::png_color_reference::base64(
                &row["reference"]["png"],
            ));
            anchor(row, image.rgba());
        }
    }
    record
}

pub fn anchor(row: &Value, rgba: &[u8]) {
    if row["layout"]["depth"] != 8 {
        return;
    }
    let id = row["signal"].as_str().unwrap();
    // At (9,6), the authored source is neutral 128 and opaque. Independent
    // EOTF anchors: linear128 ->188; gamma2.2 ->129; half-linear128 ->137.
    let expected = if id.starts_with("gray-v") {
        Some(if id.ends_with("g1") { 188 } else { 129 })
    } else if id.starts_with("lut-mft1-") {
        Some(188)
    } else if id.starts_with("lut-mft2-") || id.starts_with("lut-mAB-") || id == "lut-a2b-over-mpet"
    {
        Some(137)
    } else if id == "lut-only-a2b2" {
        Some(128)
    } else {
        None
    };
    if let Some(value) = expected {
        assert_eq!(
            &rgba[396..400],
            &[value, value, value, 255],
            "{} semantic anchor",
            row["id"]
        );
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
