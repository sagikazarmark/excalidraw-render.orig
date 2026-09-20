//! Review regressions and independently authored one-channel LUT programs.
use super::png_stage4::{self as v, Case, Outcome};
use base64::Engine as _;
use serde_json::{json, Value};
pub fn adam7() -> Vec<Case> {
    let mut rows = Vec::new();
    for selected in [false, true] {
        for bad in [false, true] {
            let mut chunks = vec![
                (*b"IHDR", v::header(3, 2, 3, 8, true)),
                (*b"PLTE", vec![255, 0, 0, 0, 255, 0]),
                (
                    *b"acTL",
                    [
                        if selected { 1_u32 } else { 2 }.to_be_bytes(),
                        0_u32.to_be_bytes(),
                    ]
                    .concat(),
                ),
            ];
            if !selected {
                chunks.push((*b"fcTL", v::control(0, 3, 2, 0, 0, 0, 0)));
            }
            chunks.push((*b"IDAT", v::compressed(&[0; 10])));
            chunks.push((*b"fcTL", v::control(u32::from(!selected), 1, 2, 1, 0, 0, 0)));
            chunks.push((
                *b"fdAT",
                [
                    (1 + u32::from(!selected)).to_be_bytes().as_slice(),
                    &v::compressed(&[0, 0, 0, if bad { 2 } else { 1 }]),
                ]
                .concat(),
            ));
            chunks.push((*b"IEND", vec![]));
            let mut r = v::case(
                format!("narrow-adam7-{selected}-{bad}"),
                &chunks,
                if bad {
                    Outcome::DecodeFailed
                } else {
                    Outcome::Accepted(if selected { 1 } else { 2 })
                },
            );
            r.pixels = if selected {
                vec![
                    0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0,
                ]
            } else {
                [255, 0, 0, 255].repeat(6)
            };
            rows.push(r);
        }
    }
    rows
}
pub fn icc_extents() -> Vec<Case> {
    [132_u32, 144, 215, 216, 296, 308, 312]
        .into_iter()
        .map(|size| {
            let mut p = super::codec_fixtures::linear_rgb_icc_profile();
            p[..4].copy_from_slice(&size.to_be_bytes());
            let mut c = v::base();
            c.insert(
                1,
                (
                    *b"iCCP",
                    [b"authored\0\0".as_slice(), &v::compressed(&p)].concat(),
                ),
            );
            v::case(
                format!("icc-declared-{size}"),
                &c,
                if size == 312 {
                    Outcome::Accepted(1)
                } else {
                    Outcome::Unsupported
                },
            )
        })
        .collect()
}
pub fn lower_keys() -> Vec<Case> {
    let mut rows = Vec::new();
    for (color, depth) in [(0, 1), (0, 2), (0, 4), (0, 8), (2, 8)] {
        for channel in 0..if color == 0 { 1 } else { 3 } {
            let maximum = (1_u16 << depth) - 1;
            for key in [0, maximum, maximum + 1, 0x100, 0xffff] {
                let mut data = vec![0; if color == 0 { 2 } else { 6 }];
                data[channel * 2..channel * 2 + 2].copy_from_slice(&key.to_be_bytes());
                let raw = vec![
                    0;
                    if color == 0 {
                        2 * (1 + (3 * depth as usize).div_ceil(8))
                    } else {
                        20
                    }
                ];
                let c = vec![
                    (*b"IHDR", v::header(3, 2, color, depth, false)),
                    (*b"tRNS", data),
                    (*b"IDAT", v::compressed(&raw)),
                    (*b"IEND", vec![]),
                ];
                let mut r = v::case(
                    format!("key-{color}-{depth}-{channel}-{key}"),
                    &c,
                    if key > maximum {
                        Outcome::DecodeFailed
                    } else {
                        Outcome::Accepted(1)
                    },
                );
                r.pixels = if key == 0 {
                    vec![0; 24]
                } else {
                    [0, 0, 0, 255].repeat(6)
                };
                rows.push(r);
            }
        }
    }
    rows
}

fn gray_lut(kind: usize, version: u8) -> Vec<u8> {
    let mut lut = vec![
        0;
        if kind == 0 {
            48
        } else if kind == 1 {
            52
        } else {
            32
        }
    ];
    lut[..4].copy_from_slice([b"mft1", b"mft2", b"mAB "][kind]);
    lut[8] = 1;
    lut[9] = 3;
    if kind < 2 {
        lut[10] = 2;
        for i in [12, 28, 44] {
            lut[i..i + 4].copy_from_slice(&65536_u32.to_be_bytes());
        }
        if kind == 0 {
            lut.extend(0..=255);
            lut.extend([0, 0, 0, 246, 255, 210]);
            for _ in 0..3 {
                lut.extend(0..=255);
            }
        } else {
            lut[48..52].copy_from_slice(&[0, 2, 0, 2]);
            lut.extend([0, 0, 255, 255]);
            for p in [0_u16, 0, 0, 63189, 65535, 54060] {
                lut.extend(p.to_be_bytes());
            }
            for _ in 0..3 {
                lut.extend([0, 0, 255, 255]);
            }
        }
    } else {
        lut[12..16].copy_from_slice(&44_u32.to_be_bytes());
        lut[24..28].copy_from_slice(&80_u32.to_be_bytes());
        lut[28..32].copy_from_slice(&32_u32.to_be_bytes());
        for _ in 0..4 {
            lut.extend(b"curv\0\0\0\0\0\0\0\0");
        }
        let mut grid = [0; 20];
        grid[0] = 2;
        grid[16] = 2;
        lut.extend(grid);
        for p in [0_u16, 0, 0, 63189, 65535, 54060] {
            lut.extend(p.to_be_bytes());
        }
    }
    let mut profile = super::codec_fixtures::linear_rgb_icc_profile()[..128].to_vec();
    profile[8] = version;
    profile[16..20].copy_from_slice(b"GRAY");
    profile.extend(2_u32.to_be_bytes());
    profile.extend(b"wtpt");
    profile.extend(156_u32.to_be_bytes());
    profile.extend(20_u32.to_be_bytes());
    profile.extend(b"A2B0");
    profile.extend(176_u32.to_be_bytes());
    profile.extend((lut.len() as u32).to_be_bytes());
    profile.extend(b"XYZ \0\0\0\0");
    for x in [0xf6d6_u32, 0x10000, 0xd32d] {
        profile.extend(x.to_be_bytes());
    }
    profile.extend(lut);
    profile.resize(profile.len().div_ceil(4) * 4, 0);
    let size = profile.len() as u32;
    profile[..4].copy_from_slice(&size.to_be_bytes());
    profile
}
pub fn gray_lut_equivalence() -> Value {
    let mut evidence = super::png_stage4_reference::evidence();
    let originals = evidence["cases"].as_array().unwrap();
    let mut rows = Vec::new();
    for depth in [8, 16] {
        let original = originals
            .iter()
            .find(|r| r["id"] == format!("gray-v2-g1-c4-d{depth}"))
            .unwrap();
        for version in [2, 4] {
            for kind in 0..3 {
                let mut row = original.clone();
                let source = super::png_color_reference::base64(&row["png"]);
                let mut chunks = Vec::new();
                let mut at = 8;
                while at < source.len() {
                    let n = u32::from_be_bytes(source[at..at + 4].try_into().unwrap()) as usize;
                    let k = source[at + 4..at + 8].try_into().unwrap();
                    if &k != b"iCCP" {
                        chunks.push((k, source[at + 8..at + 8 + n].to_vec()));
                    }
                    at += n + 12;
                }
                let p = gray_lut(kind, version);
                let iccp = [b"authored\0\0".as_slice(), &v::compressed(&p)].concat();
                chunks.insert(1, (*b"iCCP", iccp.clone()));
                let png = base64::engine::general_purpose::STANDARD.encode(v::encode(&chunks));
                row["tags"] = json!([{"kind":"iCCP","data":iccp,"after":false}]);
                row.as_object_mut().unwrap().remove("opaque_png");
                row["png"] = json!(png);
                row["scene"]["files"]["asset"]["dataURL"] =
                    json!(format!("data:image/png;base64,{png}"));
                row["id"] = json!(format!("gray-lut-v{version}-kind{kind}-d{depth}"));
                row["group"] = json!("icc-gray-lut");
                row["equivalence"]=json!("One-dimensional affine XYZ table along D50 white is linear gray under the pinned normalized-XYZ LUT interpretation; compare the independently collected gamma-1 gray field, with unchanged samples and alpha.");
                rows.push(row);
            }
        }
    }
    evidence["cases"] = json!(rows);
    evidence
}

pub fn late_scene(row: &Value) -> Value {
    let source = super::png_color_reference::base64(&row["png"]);
    let mut chunks = Vec::new();
    let mut exif = None;
    let mut at = 8;
    while at < source.len() {
        let n = u32::from_be_bytes(source[at..at + 4].try_into().unwrap()) as usize;
        let k = source[at + 4..at + 8].try_into().unwrap();
        let c = (k, source[at + 8..at + 8 + n].to_vec());
        if &k == b"eXIf" {
            exif = Some(c);
        } else {
            chunks.push(c);
        }
        at += n + 12;
    }
    chunks.insert(chunks.len() - 1, exif.unwrap());
    let mut scene = row["scene"].clone();
    scene["files"]["asset"]["dataURL"] = json!(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(v::encode(&chunks))
    ));
    scene
}
