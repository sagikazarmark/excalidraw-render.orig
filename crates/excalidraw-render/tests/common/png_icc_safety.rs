//! Authored malformed headers and variable TRC/irrelevant-metadata fixtures.
use super::png_stage4::{self as v, Case, Outcome};
use base64::Engine as _;
use serde_json::{json, Value};
pub fn headers() -> Vec<Case> {
    let evidence = super::png_stage4_reference::evidence();
    let mut profiles = Vec::new();
    for id in [
        "lut-mft1-2-intent0-c6-d8",
        "lut-mft2-2-intent0-c6-d8",
        "lut-mAB-2-intent0-c6-d8",
        "gray-v4-g1-c4-d8",
    ] {
        let row = evidence["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let data: Vec<u8> = row["tags"][0]["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        profiles.push((
            id,
            miniz_oxide::inflate::decompress_to_vec_zlib(&data[10..]).unwrap(),
        ));
    }
    profiles.push(("matrix-v2", super::codec_fixtures::linear_rgb_icc_profile()));
    let mut rows = Vec::new();
    for (name, p) in profiles {
        for (field, at, word) in [
            ("signature", 36, 0_u32),
            ("version-zero", 8, 0),
            ("version-one", 8, 0x01000000),
            ("version-five", 8, 0x05000000),
            ("version-max", 8, u32::MAX),
            ("intent-four", 64, 4),
            ("intent-byte", 64, 255),
            ("intent-max", 64, u32::MAX),
            ("class", 12, 0),
            ("model", 16, 0),
            ("pcs", 20, 0),
        ] {
            let mut p = p.clone();
            p[at..at + 4].copy_from_slice(&word.to_be_bytes());
            let mut c = v::base();
            c.insert(
                1,
                (
                    *b"iCCP",
                    [b"authored\0\0".as_slice(), &v::compressed(&p)].concat(),
                ),
            );
            rows.push(v::case(
                format!("header-{name}-{field}"),
                &c,
                Outcome::Unsupported,
            ));
        }
    }
    rows
}

pub fn sampled_profile(gray: bool, version: u8, count: usize, metadata: bool) -> Vec<u8> {
    let base = super::codec_fixtures::linear_rgb_icc_profile();
    let mut head = base[..128].to_vec();
    head[8] = version;
    if gray {
        head[16..20].copy_from_slice(b"GRAY");
    }
    let mut curve = b"curv\0\0\0\0".to_vec();
    curve.extend((count as u32).to_be_bytes());
    for i in 0..count {
        curve.extend(
            ((((i as u64) * 65535 + (count - 1) as u64 / 2) / (count - 1) as u64) as u16)
                .to_be_bytes(),
        );
    }
    let mut tags: Vec<([u8; 4], Vec<u8>)> = if gray {
        vec![(*b"wtpt", base[276..296].to_vec()), (*b"kTRC", curve)]
    } else {
        vec![
            (*b"rXYZ", base[216..236].to_vec()),
            (*b"gXYZ", base[236..256].to_vec()),
            (*b"bXYZ", base[256..276].to_vec()),
            (*b"wtpt", base[276..296].to_vec()),
            (*b"rTRC", curve.clone()),
            (*b"gTRC", curve.clone()),
            (*b"bTRC", curve),
        ]
    };
    if metadata {
        let mut description = b"desc\0\0\0\0".to_vec();
        description.extend(32769_u32.to_be_bytes());
        description.extend(vec![b'A'; 32768]);
        description.push(0);
        description.extend([0; 78]);
        tags.push((*b"desc", description));
        let mut localized = b"mluc\0\0\0\0".to_vec();
        localized.extend(64_u32.to_be_bytes());
        localized.extend(12_u32.to_be_bytes());
        for i in 0..64 {
            localized.extend(b"enUS");
            localized.extend(512_u32.to_be_bytes());
            localized.extend((16 + 64 * 12 + i * 512_u32).to_be_bytes());
        }
        for _ in 0..(64 * 256) {
            localized.extend([0, b'A']);
        }
        tags.push((*b"dmdd", localized));
    }
    head.extend((tags.len() as u32).to_be_bytes());
    let mut offset = 132 + tags.len() * 12;
    let mut bodies = Vec::new();
    for (name, data) in tags {
        head.extend(name);
        head.extend((offset as u32).to_be_bytes());
        head.extend((data.len() as u32).to_be_bytes());
        offset += data.len().div_ceil(4) * 4;
        bodies.extend(&data);
        bodies.resize(bodies.len().div_ceil(4) * 4, 0);
    }
    head.extend(bodies);
    let size = head.len() as u32;
    head[..4].copy_from_slice(&size.to_be_bytes());
    head
}
pub fn with_profile(png: &[u8], profile: &[u8]) -> Vec<u8> {
    let mut chunks = Vec::new();
    let mut at = 8;
    while at < png.len() {
        let n = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let k = png[at + 4..at + 8].try_into().unwrap();
        if &k != b"iCCP" {
            chunks.push((k, png[at + 8..at + 8 + n].to_vec()));
        }
        at += 12 + n;
    }
    chunks.insert(
        1,
        (
            *b"iCCP",
            [b"authored\0\0".as_slice(), &v::compressed(profile)].concat(),
        ),
    );
    v::encode(&chunks)
}
pub fn sampled_evidence() -> Value {
    let mut evidence = super::png_color_reference::evidence();
    let gray = super::png_stage4_reference::evidence();
    let mut rows = Vec::new();
    for is_gray in [false, true] {
        let originals = if is_gray { &gray } else { &evidence };
        for depth in [8, 16] {
            let id = if is_gray {
                format!("gray-v2-g1-c4-d{depth}")
            } else {
                format!("icc-v2-gamma1-c6-d{depth}")
            };
            let original = originals["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap();
            for version in [2, 4] {
                for count in [2, 257, 4097, 40000] {
                    for metadata in [false, true] {
                        let mut row = original.clone();
                        let p = sampled_profile(is_gray, version, count, metadata);
                        let png =
                            with_profile(&super::png_color_reference::base64(&row["png"]), &p);
                        row["png"] = json!(base64::engine::general_purpose::STANDARD.encode(&png));
                        row["scene"]["files"]["asset"]["dataURL"] = json!(format!(
                            "data:image/png;base64,{}",
                            row["png"].as_str().unwrap()
                        ));
                        row["id"] = json!(format!(
                            "sampled-{is_gray}-v{version}-d{depth}-n{count}-meta{metadata}"
                        ));
                        row["group"] = json!("icc-sampled");
                        row.as_object_mut().unwrap().remove("opaque_png");
                        row["equivalence"]=json!("Uniform sampled identity TRCs encode the same linear RGB/gray transform as the independently collected gamma-1 profile; descriptions/localizations do not affect pixels.");
                        rows.push(row);
                    }
                }
            }
        }
    }
    evidence["cases"] = json!(rows);
    evidence
}
