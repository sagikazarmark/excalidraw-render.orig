//! Independently authored byte-format vectors, also consumed in browser WASM.
//! No production parser, decoder, or inventory supplies expected pixels/reports.
use base64::Engine as _;
use serde_json::{json, Value};

pub type Chunk = ([u8; 4], Vec<u8>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted(u32),
    DecodeFailed,
    Unsupported,
}
pub struct Case {
    pub id: String,
    pub bytes: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub outcome: Outcome,
}
pub const PIXELS: [[u8; 4]; 6] = [
    [255, 0, 0, 255],
    [0, 255, 0, 85],
    [0, 0, 255, 255],
    [255, 255, 0, 170],
    [0, 255, 255, 255],
    [0, 0, 0, 0],
];
pub fn chunk(bytes: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&kind);
    bytes.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(&kind);
    crc.update(data);
    bytes.extend_from_slice(&crc.finalize().to_be_bytes());
}
pub fn encode(chunks: &[Chunk]) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in chunks {
        chunk(&mut bytes, *kind, data);
    }
    bytes
}
pub fn header(w: u32, h: u32, color: u8, depth: u8, interlace: bool) -> Vec<u8> {
    [
        w.to_be_bytes().as_slice(),
        h.to_be_bytes().as_slice(),
        &[depth, color, 0, 0, u8::from(interlace)],
    ]
    .concat()
}
pub fn compressed(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}
pub fn base() -> Vec<Chunk> {
    vec![
        (*b"IHDR", header(3, 2, 6, 8, false)),
        (
            *b"IDAT",
            compressed(
                &[
                    &[0][..],
                    PIXELS[..3].as_flattened(),
                    &[0],
                    PIXELS[3..].as_flattened(),
                ]
                .concat(),
            ),
        ),
        (*b"IEND", vec![]),
    ]
}
pub fn case(id: impl Into<String>, chunks: &[Chunk], outcome: Outcome) -> Case {
    Case {
        id: id.into(),
        bytes: encode(chunks),
        width: 3,
        height: 2,
        pixels: PIXELS.as_flattened().to_vec(),
        outcome,
    }
}
pub fn tiff(value: u16, big: bool) -> Vec<u8> {
    let short = |v: u16| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let long = |v: u32| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    [
        if big { &b"MM"[..] } else { &b"II"[..] },
        &short(42),
        &long(8),
        &short(1),
        &short(0x112),
        &short(3),
        &long(1),
        &short(value),
        &short(0),
        &long(0),
    ]
    .concat()
}
pub fn orientations() -> Vec<Case> {
    // Rows of labelled asymmetric 3x2 source, written out as TIFF specifies.
    let permutations = [
        [0, 1, 2, 3, 4, 5],
        [2, 1, 0, 5, 4, 3],
        [5, 4, 3, 2, 1, 0],
        [3, 4, 5, 0, 1, 2],
        [0, 3, 1, 4, 2, 5],
        [3, 0, 4, 1, 5, 2],
        [5, 2, 4, 1, 3, 0],
        [2, 5, 1, 4, 0, 3],
    ];
    let mut cases = Vec::new();
    for value in 1..=8 {
        for big in [false, true] {
            for after in [false, true] {
                let mut chunks = base();
                chunks.insert(if after { 2 } else { 1 }, (*b"eXIf", tiff(value, big)));
                let mut row = case(
                    format!("orientation-{value}-{big}-{after}"),
                    &chunks,
                    Outcome::Accepted(1),
                );
                if value >= 5 {
                    row.width = 2;
                    row.height = 3;
                }
                row.pixels = permutations[value as usize - 1]
                    .iter()
                    .flat_map(|&i| PIXELS[i])
                    .collect();
                cases.push(row);
            }
        }
    }
    cases
}
pub fn invalid_orientation() -> Vec<Case> {
    let mut cases = vec![case("orientation-absent", &base(), Outcome::Accepted(1))];
    for big in [false, true] {
        let valid = tiff(6, big);
        let mut payloads: Vec<(String, Vec<u8>)> = (0..22)
            .map(|n| (format!("truncated-{n}"), valid[..n].to_vec()))
            .collect();
        for value in [0, 9, 255, 256, 65535] {
            payloads.push((format!("value-{value}"), tiff(value, big)));
        }
        for (name, range, bytes) in [
            ("byte-order", 0..2, vec![0, 0]),
            ("magic", 2..4, vec![0, 0]),
            ("offset", 4..8, vec![255; 4]),
            ("tag", 10..12, vec![0, 0]),
            ("type", 12..14, vec![0, 0]),
            ("count-zero", 14..18, vec![0; 4]),
            ("count-many", 14..18, vec![255; 4]),
        ] {
            let mut p = valid.clone();
            p[range].copy_from_slice(&bytes);
            payloads.push((name.into(), p));
        }
        payloads.push((
            "jpeg-prefix".into(),
            [b"Exif\0\0".as_slice(), &valid].concat(),
        ));
        // Ignore later IFDs and stop at the first orientation tag, even invalid.
        let mut later = tiff(0, big);
        later.extend_from_slice(&valid);
        payloads.push(("later-ifd".into(), later));
        for (name, p) in payloads {
            for after in [false, true] {
                let mut chunks = base();
                chunks.insert(if after { 2 } else { 1 }, (*b"eXIf", p.clone()));
                cases.push(case(
                    format!("exif-{name}-{big}-{after}"),
                    &chunks,
                    Outcome::Accepted(1),
                ));
            }
        }
        for value in [0, 6] {
            let mut chunks = base();
            chunks.insert(1, (*b"eXIf", tiff(value, big)));
            chunks.insert(3, (*b"eXIf", valid.clone()));
            cases.push(case(
                format!("exif-duplicate-{value}-{big}"),
                &chunks,
                Outcome::DecodeFailed,
            ));
        }
    }
    cases
}
pub fn ihdr_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for color in 0..=255 {
        for depth in 0..=255 {
            if matches!(
                (color, depth),
                (0, 1 | 2 | 4 | 8 | 16) | (2 | 4 | 6, 8 | 16) | (3, 1 | 2 | 4 | 8)
            ) {
                continue;
            }
            let mut chunks = base();
            chunks[0].1[8] = depth;
            chunks[0].1[9] = color;
            cases.push(case(
                format!("ihdr-{color}-{depth}"),
                &chunks,
                Outcome::DecodeFailed,
            ));
        }
    }
    for index in [10, 11, 12] {
        for value in 1..=255 {
            if index == 12 && value == 1 {
                continue;
            }
            let mut chunks = base();
            chunks[0].1[index] = value;
            cases.push(case(
                format!("ihdr-method-{index}-{value}"),
                &chunks,
                Outcome::DecodeFailed,
            ));
        }
    }
    for index in [0, 4] {
        let mut c = base();
        c[0].1[index..index + 4].fill(0);
        cases.push(case(
            format!("ihdr-zero-{index}"),
            &c,
            Outcome::DecodeFailed,
        ));
    }
    for length in [0, 12, 14] {
        let mut c = base();
        c[0].1.resize(length, 0);
        cases.push(case(
            format!("ihdr-length-{length}"),
            &c,
            Outcome::DecodeFailed,
        ));
    }
    let mut c = base();
    c.insert(1, c[0].clone());
    cases.push(case("ihdr-duplicate", &c, Outcome::DecodeFailed));
    let mut c = base();
    c.insert(0, (*b"tEXt", b"key\0value".to_vec()));
    cases.push(case("ihdr-misplaced", &c, Outcome::DecodeFailed));
    cases
}
pub fn chunk_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (kind, accepted) in [
        (*b"vpAg", true),
        (*b"VpAg", false),
        (*b"vpag", false),
        (*b"vp1g", false),
    ] {
        let mut c = base();
        c.insert(1, (kind, vec![1, 2, 3]));
        cases.push(case(
            format!("chunk-{kind:?}"),
            &c,
            if accepted {
                Outcome::Accepted(1)
            } else {
                Outcome::DecodeFailed
            },
        ));
    }
    for index in 0..3 {
        let mut row = case(format!("crc-{index}"), &base(), Outcome::DecodeFailed);
        let at = 8
            + base()[..index]
                .iter()
                .map(|(_, d)| d.len() + 12)
                .sum::<usize>()
            + 8
            + base()[index].1.len();
        row.bytes[at] ^= 1;
        cases.push(row);
    }
    let bytes = encode(&base());
    for length in 8..bytes.len() {
        let mut row = case(
            format!("truncated-{length}"),
            &base(),
            Outcome::DecodeFailed,
        );
        row.bytes.truncate(length);
        cases.push(row);
    }
    for length in [0x7fff_ffff_u32, 0x8000_0000, u32::MAX] {
        let mut row = case(format!("length-{length}"), &base(), Outcome::DecodeFailed);
        row.bytes[33..37].copy_from_slice(&length.to_be_bytes());
        cases.push(row);
    }
    let mut c = base();
    c.push((*b"IEND", vec![]));
    cases.push(case("iend-duplicate", &c, Outcome::DecodeFailed));
    let mut c = base();
    c[2].1.push(0);
    cases.push(case("iend-data", &c, Outcome::DecodeFailed));
    let mut row = case("trailing", &base(), Outcome::DecodeFailed);
    row.bytes.push(0);
    cases.push(row);
    cases
}
pub fn scanline_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let raw = [
        &[0][..],
        PIXELS[..3].as_flattened(),
        &[0],
        PIXELS[3..].as_flattened(),
    ]
    .concat();
    for filter in 5..=255 {
        let mut r = raw.clone();
        r[0] = filter;
        let mut c = base();
        c[1].1 = compressed(&r);
        cases.push(case(format!("filter-{filter}"), &c, Outcome::DecodeFailed));
    }
    for length in [0, 1, 12, 13, 24, 25, 27, 52] {
        let mut r = raw.clone();
        r.resize(length, 0);
        let mut c = base();
        c[1].1 = compressed(&r);
        cases.push(case(
            format!("rows-length-{length}"),
            &c,
            Outcome::DecodeFailed,
        ));
    }
    for at in [0, 1, 3, base()[1].1.len() - 1] {
        let mut c = base();
        c[1].1[at] ^= 0xff;
        cases.push(case(format!("zlib-{at}"), &c, Outcome::DecodeFailed));
    }
    for interlaced in [false, true] {
        let mut c = base();
        c[0].1[12] = u8::from(interlaced);
        if interlaced {
            // 3x2: passes 1,4,6,7 contain 1,1,1,3 pixels respectively.
            c[1].1 = compressed(
                &[
                    &[0][..],
                    &PIXELS[0],
                    &[0],
                    &PIXELS[2],
                    &[0],
                    &PIXELS[1],
                    &[0],
                    PIXELS[3..].as_flattened(),
                ]
                .concat(),
            );
        }
        cases.push(case(
            format!("scanlines-control-{interlaced}"),
            &c,
            Outcome::Accepted(1),
        ));
        if interlaced {
            let mut short = c.clone();
            short[0].1[4..8].copy_from_slice(&3_u32.to_be_bytes());
            cases.push(case("adam7-pass-extents", &short, Outcome::DecodeFailed));
        }
        let data = c[1].1.clone();
        let mid = data.len() / 2;
        c[1].1 = data[..mid].to_vec();
        c.insert(2, (*b"IDAT", data[mid..].to_vec()));
        cases.push(case(
            format!("idat-split-{interlaced}"),
            &c,
            Outcome::Accepted(1),
        ));
        c.insert(2, (*b"tEXt", b"key\0value".to_vec()));
        cases.push(case(
            format!("idat-nonconsecutive-{interlaced}"),
            &c,
            Outcome::DecodeFailed,
        ));
    }
    let mut c = base();
    c.remove(1);
    cases.push(case("idat-missing", &c, Outcome::DecodeFailed));
    cases
}
pub fn scene(row: &Case) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":row.width,"height":row.height,"fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(&row.bytes)),"created":1}}})
}
pub fn palette_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let model = |color| {
        let pixels: Vec<[u8; 4]> = if color == 2 {
            PIXELS.iter().map(|p| [p[0], p[1], p[2], 255]).collect()
        } else {
            [85, 170, 255, 0, 85, 170].map(|g| [g, g, g, 255]).to_vec()
        };
        let mut raw = Vec::new();
        for row in pixels.as_chunks::<3>().0 {
            raw.push(0);
            for p in row {
                match color {
                    0 => raw.push(p[0]),
                    2 => raw.extend_from_slice(&p[..3]),
                    4 => raw.extend_from_slice(&[p[0], p[3]]),
                    _ => unreachable!(),
                }
            }
        }
        (
            vec![
                (*b"IHDR", header(3, 2, color, 8, false)),
                (*b"IDAT", compressed(&raw)),
                (*b"IEND", vec![]),
            ],
            pixels.as_flattened().to_vec(),
        )
    };
    for color in [0, 2, 4] {
        let (chunks, pixels) = model(color);
        let mut row = case(
            format!("palette-model-control-{color}"),
            &chunks,
            Outcome::Accepted(1),
        );
        row.pixels = pixels;
        cases.push(row);
    }
    for color in [2, 6] {
        let mut c = base();
        if color == 2 {
            c[0].1[9] = 2;
            c[1].1 = compressed(
                &[
                    &[0][..],
                    &[255, 0, 0, 0, 255, 0, 0, 0, 255],
                    &[0],
                    &[255, 255, 0, 0, 255, 255, 0, 0, 0],
                ]
                .concat(),
            );
        }
        for count in [1, 256] {
            let mut c = c.clone();
            c.insert(1, (*b"PLTE", vec![73; count * 3]));
            let mut r = case(
                format!("suggested-{color}-{count}"),
                &c,
                Outcome::Accepted(1),
            );
            if color == 2 {
                for p in r.pixels.as_chunks_mut::<4>().0 {
                    p[3] = 255;
                }
            }
            cases.push(r);
            if color == 2 {
                let mut invalid = c.clone();
                invalid.insert(1, (*b"tRNS", vec![0; 6]));
                cases.push(case(
                    format!("truecolor-trns-before-plte-{count}"),
                    &invalid,
                    Outcome::DecodeFailed,
                ));
            }
        }
    }
    for depth in [1, 2, 4, 8] {
        for count in [1, 1_usize << depth] {
            let mut palette = vec![17; count * 3];
            palette[..3].copy_from_slice(&[255, 0, 0]);
            let c = vec![
                (*b"IHDR", header(3, 2, 3, depth, false)),
                (*b"PLTE", palette),
                (
                    *b"IDAT",
                    compressed(&vec![0; 2 * (1 + (3 * depth as usize).div_ceil(8))]),
                ),
                (*b"IEND", vec![]),
            ];
            let mut r = case(format!("palette-{depth}-{count}"), &c, Outcome::Accepted(1));
            r.pixels = [255, 0, 0, 255].repeat(6);
            cases.push(r);
        }
    }
    for (name, kind, data) in [
        ("empty", *b"PLTE", vec![]),
        ("oversized", *b"PLTE", vec![0; 771]),
        ("unaligned", *b"PLTE", vec![0; 4]),
        ("rgba-trns", *b"tRNS", vec![0; 6]),
    ] {
        let mut c = base();
        c.insert(1, (kind, data));
        cases.push(case(name, &c, Outcome::DecodeFailed));
    }
    for color in [0, 4] {
        let mut c = model(color).0;
        c.insert(1, (*b"PLTE", vec![0; 3]));
        cases.push(case(
            format!("forbidden-plte-{color}"),
            &c,
            Outcome::DecodeFailed,
        ));
    }
    let indexed = vec![
        (*b"IHDR", header(3, 2, 3, 8, false)),
        (*b"PLTE", vec![255, 0, 0]),
        (*b"IDAT", compressed(&[0, 0, 0, 0, 0, 0, 0, 0])),
        (*b"IEND", vec![]),
    ];
    for mutation in 0..10 {
        let mut c = indexed.clone();
        match mutation {
            0 => {
                c.remove(1);
            }
            1 => c.insert(2, c[1].clone()),
            2 => {
                let p = c.remove(1);
                c.insert(2, p);
            }
            3 => c[2].1 = compressed(&[0, 1, 0, 0, 0, 0, 0, 0]),
            4 => c.insert(1, (*b"tRNS", vec![0])),
            5 => {
                c.insert(2, (*b"tRNS", vec![0]));
                c.insert(3, (*b"tRNS", vec![255]));
            }
            6 => c.insert(3, (*b"tRNS", vec![0])),
            7 => c.insert(2, (*b"tRNS", vec![])),
            8 => c.insert(2, (*b"tRNS", vec![0, 255])),
            9 => {
                c[0].1[8] = 1;
                c[1].1 = vec![0; 9];
                c[2].1 = compressed(&[0, 0, 0, 0]);
            }
            _ => unreachable!(),
        }
        cases.push(case(
            format!("palette-malformed-{mutation}"),
            &c,
            Outcome::DecodeFailed,
        ));
    }
    for (color, size) in [(0, 2), (2, 6)] {
        for length in 0..=8 {
            if length == size {
                continue;
            }
            let mut c = model(color).0;
            c.insert(1, (*b"tRNS", vec![0; length]));
            cases.push(case(
                format!("trns-{color}-{length}"),
                &c,
                Outcome::DecodeFailed,
            ));
        }
    }
    let mut c = model(4).0;
    c.insert(1, (*b"tRNS", vec![0; 2]));
    cases.push(case("gray-alpha-trns", &c, Outcome::DecodeFailed));
    cases
}
pub fn control(sequence: u32, w: u32, h: u32, x: u32, y: u32, dispose: u8, blend: u8) -> Vec<u8> {
    [
        sequence.to_be_bytes().as_slice(),
        &w.to_be_bytes(),
        &h.to_be_bytes(),
        &x.to_be_bytes(),
        &y.to_be_bytes(),
        &[0, 1, 0, 30, dispose, blend],
    ]
    .concat()
}
pub fn animation(
    separate: bool,
    frames: u32,
    subframe: bool,
    dispose: u8,
    blend: u8,
) -> Vec<Chunk> {
    let mut c = base();
    c.pop();
    let data = c.pop().unwrap().1;
    c.push((
        *b"acTL",
        [frames.to_be_bytes(), 0_u32.to_be_bytes()].concat(),
    ));
    if separate {
        c.push((
            *b"IDAT",
            compressed(&[
                0, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 0, 255, 0, 255, 255, 255,
                0, 255, 255, 255, 0, 255, 255,
            ]),
        ));
    }
    let mut seq = 0;
    for frame in 0..frames {
        let small = subframe && (separate || frame > 0);
        c.push((
            *b"fcTL",
            control(
                seq,
                if small { 1 } else { 3 },
                if small { 1 } else { 2 },
                u32::from(small),
                u32::from(small),
                dispose,
                blend,
            ),
        ));
        seq += 1;
        let data = if small {
            compressed(&[0, 0, 255, 0, 85])
        } else if frame == 0 {
            data.clone()
        } else {
            compressed(&[
                0, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 0, 255, 0, 255, 255, 255,
                0, 255, 255, 255, 0, 255, 255,
            ])
        };
        if !separate && frame == 0 {
            c.push((*b"IDAT", data));
        } else {
            c.push((*b"fdAT", [seq.to_be_bytes().as_slice(), &data].concat()));
            seq += 1;
        }
    }
    c.push((*b"IEND", vec![]));
    c
}
pub fn apng_controls() -> Vec<Case> {
    let mut cases = Vec::new();
    for separate in [false, true] {
        for frames in [1, 2, 3] {
            for subframe in [false, true] {
                for dispose in 0..=2 {
                    for blend in 0..=1 {
                        let mut r = case(
                            format!("apng-{separate}-{frames}-{subframe}-{dispose}-{blend}"),
                            &animation(separate, frames, subframe, dispose, blend),
                            Outcome::Accepted(frames),
                        );
                        if separate && subframe {
                            r.pixels = vec![0; 24];
                            r.pixels[16..20].copy_from_slice(&[0, 255, 0, 85]);
                        }
                        cases.push(r);
                    }
                }
            }
        }
    }
    for separate in [false, true] {
        let mut c = animation(separate, 2, false, 0, 0);
        c[1].1[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        for (_, data) in c.iter_mut().filter(|(kind, _)| kind == b"fcTL") {
            data[20..24].fill(0);
        }
        cases.push(case(
            format!("apng-zero-delay-denominator-{separate}"),
            &c,
            Outcome::Accepted(2),
        ));
    }
    cases
}
pub fn apng_malformed() -> Vec<Case> {
    let mut cases = Vec::new();
    for separate in [false, true] {
        let original = animation(separate, 2, false, 0, 0);
        let first = original.iter().position(|(k, _)| k == b"fcTL").unwrap();
        let later = original.iter().rposition(|(k, _)| k == b"fcTL").unwrap();
        let data = original.iter().rposition(|(k, _)| k == b"fdAT").unwrap();
        if !separate {
            for (at, value) in [(4, 2_u32), (8, 1), (12, 1), (16, 1)] {
                let mut c = original.clone();
                c[first].1[at..at + 4].copy_from_slice(&value.to_be_bytes());
                cases.push(case(
                    format!("included-default-rectangle-{at}"),
                    &c,
                    Outcome::DecodeFailed,
                ));
            }
        }
        for mutation in 0..21 {
            let mut c = original.clone();
            match mutation {
                0 => c[1].1[..4].copy_from_slice(&0_u32.to_be_bytes()),
                1 => c[1].1[..4].copy_from_slice(&1_u32.to_be_bytes()),
                2 => c[1].1[..4].copy_from_slice(&3_u32.to_be_bytes()),
                3 => {
                    c.remove(1);
                }
                4 => c.insert(2, c[1].clone()),
                5 => {
                    let a = c.remove(1);
                    c.insert(data, a);
                }
                6 => c[first].1[..4].copy_from_slice(&1_u32.to_be_bytes()),
                7 => c[later].1[..4].copy_from_slice(&0_u32.to_be_bytes()),
                8 => c[data].1[..4].copy_from_slice(&99_u32.to_be_bytes()),
                9 => {
                    c.remove(first);
                }
                10 => {
                    c.remove(later);
                }
                11 => {
                    c.remove(data);
                }
                12 => c.insert(later, c[later].clone()),
                13 => c[later].1.truncate(25),
                14 => c[data].1.truncate(3),
                15 => c[data].1[4] ^= 255,
                16 => {
                    let n = c[data].1.len();
                    c[data].1[n - 1] ^= 255;
                }
                17 => c[later].1[4..8].copy_from_slice(&2_u32.to_be_bytes()),
                18 => c[later].1[8..12].copy_from_slice(&1_u32.to_be_bytes()),
                19 => c[later].1[12..16].copy_from_slice(&u32::MAX.to_be_bytes()),
                20 => c[later].1[4..8].fill(0),
                _ => unreachable!(),
            }
            cases.push(case(
                format!("apng-malformed-{separate}-{mutation}"),
                &c,
                Outcome::DecodeFailed,
            ));
        }
        for frame in [first, later] {
            for (index, start) in [(24, 3), (25, 2)] {
                for value in start..=255 {
                    let mut c = original.clone();
                    c[frame].1[index] = value;
                    cases.push(case(
                        format!("apng-control-invalid-{separate}-{frame}-{index}-{value}"),
                        &c,
                        Outcome::DecodeFailed,
                    ));
                }
            }
        }
        let mut c = original.clone();
        let payload = c[data].1.clone();
        let mid = (payload.len() - 4) / 2 + 4;
        let seq = u32::from_be_bytes(payload[..4].try_into().unwrap());
        c[data].1 = payload[..mid].to_vec();
        c.insert(
            data + 1,
            (
                *b"fdAT",
                [(seq + 1).to_be_bytes().as_slice(), &payload[mid..]].concat(),
            ),
        );
        cases.push(case(
            format!("apng-split-fdat-{separate}"),
            &c,
            Outcome::Accepted(2),
        ));
        c.insert(data + 1, (*b"tEXt", b"key\0value".to_vec()));
        cases.push(case(
            format!("apng-interspersed-ancillary-fdat-{separate}"),
            &c,
            Outcome::Accepted(2),
        ));
    }
    cases
}

/// Explicit crossing of each legal sample layout, interlace, all orientations
/// and each container. The caller supplies independently qualified metadata;
/// linear-sRGB anchors 0/85/170/255 -> 0/156/213/255 are authored here.
pub fn composition(metadata: &[Chunk], linear: bool) -> Vec<Case> {
    composition_vectors(metadata, linear, None)
}
pub fn hdr_composition(transfer: u8) -> Vec<Case> {
    assert!(matches!(transfer, 16 | 18));
    composition_vectors(
        &[(*b"cICP", vec![1, transfer, 0, 1])],
        false,
        Some(transfer),
    )
}
fn composition_vectors(metadata: &[Chunk], linear: bool, hdr: Option<u8>) -> Vec<Case> {
    let mut cases = Vec::new();
    for (color, depth) in [
        (0, 1),
        (0, 2),
        (0, 4),
        (0, 8),
        (0, 16),
        (2, 8),
        (2, 16),
        (3, 1),
        (3, 2),
        (3, 4),
        (3, 8),
        (4, 8),
        (4, 16),
        (6, 8),
        (6, 16),
    ] {
        for interlace in [false, true] {
            let maximum = if depth == 16 {
                65535
            } else {
                (1_u32 << depth) - 1
            };
            let mut samples = Vec::new();
            let mut expected = Vec::new();
            let palette = if hdr.is_some() {
                [[64; 3], [128; 3]]
            } else {
                [[85, 170, 255], [255, 85, 170]]
            };
            for i in 0..6 {
                let gray = if depth == 1 || (hdr.is_some() && depth < 8) {
                    if i % 2 == 0 {
                        0
                    } else {
                        255
                    }
                } else if hdr.is_some() {
                    [0, 64, 128, 255, 64, 128][i]
                } else {
                    [0, 85, 170, 255, 85, 170][i]
                };
                let a = if i == 5 && matches!(color, 4 | 6) {
                    0
                } else {
                    255
                };
                let rgb = match color {
                    0 | 4 => [gray; 3],
                    3 => palette[i % 2],
                    _ if hdr.is_some() => [gray; 3],
                    _ => [[85, 170, 255], [255, 85, 170], [170, 255, 85]][i % 3],
                };
                let values = match color {
                    0 => vec![u32::from(gray) * maximum / 255],
                    3 => vec![(i % 2) as u32],
                    2 => rgb.map(|v| u32::from(v) * maximum / 255).to_vec(),
                    4 => vec![
                        u32::from(gray) * maximum / 255,
                        u32::from(a) * maximum / 255,
                    ],
                    6 => [rgb[0], rgb[1], rgb[2], a]
                        .map(|v| u32::from(v) * maximum / 255)
                        .to_vec(),
                    _ => unreachable!(),
                };
                samples.push(values);
                let convert = |v| {
                    if let Some(transfer) = hdr {
                        match (transfer, v) {
                            (_, 0) => 0,
                            (_, 255) => 255,
                            (16, 64) => 30,
                            (16, 128) => 132,
                            (18, 64) => 43,
                            (18, 128) => 99,
                            _ => unreachable!(),
                        }
                    } else if linear {
                        match v {
                            0 => 0,
                            85 => 156,
                            170 => 213,
                            255 => 255,
                            _ => unreachable!(),
                        }
                    } else {
                        v
                    }
                };
                expected.push(if a == 0 {
                    [0; 4]
                } else {
                    [convert(rgb[0]), convert(rgb[1]), convert(rgb[2]), a]
                });
            }
            let mut raw = Vec::new();
            let passes = if interlace {
                &[
                    (0, 0, 8, 8),
                    (4, 0, 8, 8),
                    (0, 4, 4, 8),
                    (2, 0, 4, 4),
                    (0, 2, 2, 4),
                    (1, 0, 2, 2),
                    (0, 1, 1, 2),
                ][..]
            } else {
                &[(0, 0, 1, 1)][..]
            };
            for &(x0, y0, dx, dy) in passes {
                if x0 >= 3 || y0 >= 2 {
                    continue;
                }
                for y in (y0..2).step_by(dy) {
                    raw.push(0);
                    let values: Vec<_> = (x0..3)
                        .step_by(dx)
                        .flat_map(|x| samples[y * 3 + x].clone())
                        .collect();
                    if depth < 8 {
                        for values in values.chunks(8 / depth as usize) {
                            let mut byte = 0;
                            for (i, v) in values.iter().enumerate() {
                                byte |= (*v as u8) << (8 - depth as usize * (i + 1));
                            }
                            raw.push(byte);
                        }
                    } else {
                        for v in values {
                            if depth == 16 {
                                raw.extend_from_slice(&(v as u16).to_be_bytes());
                            } else {
                                raw.push(v as u8);
                            }
                        }
                    }
                }
            }
            for container in 0..3 {
                for orientation in 1..=8 {
                    let mut c = vec![(*b"IHDR", header(3, 2, color, depth, interlace))];
                    c.extend_from_slice(metadata);
                    c.push((*b"eXIf", tiff(orientation, true)));
                    if color == 3 {
                        c.push((*b"PLTE", palette.as_flattened().to_vec()));
                    }
                    if container == 0 {
                        c.push((*b"IDAT", compressed(&raw)));
                    } else {
                        c.push((
                            *b"acTL",
                            [2_u32.to_be_bytes(), 0_u32.to_be_bytes()].concat(),
                        ));
                        // A different default/later frame in the same layout.
                        let mut other = raw.clone();
                        for b in &mut other {
                            if *b != 0 {
                                *b = 0;
                            }
                        }
                        let mut seq = 0;
                        if container == 2 {
                            c.push((*b"IDAT", compressed(&other)));
                        }
                        for frame in 0..2 {
                            c.push((*b"fcTL", control(seq, 3, 2, 0, 0, 2, 1)));
                            seq += 1;
                            let data = compressed(if frame == 0 { &raw } else { &other });
                            if container == 1 && frame == 0 {
                                c.push((*b"IDAT", data));
                            } else {
                                c.push((*b"fdAT", [seq.to_be_bytes().as_slice(), &data].concat()));
                                seq += 1;
                            }
                        }
                    }
                    c.push((*b"IEND", vec![]));
                    let mut row=case(format!("composition-{color}-{depth}-{interlace}-{container}-{orientation}-{linear}"),&c,Outcome::Accepted(if container==0 {1} else {2}));
                    let orders = [
                        [0, 1, 2, 3, 4, 5],
                        [2, 1, 0, 5, 4, 3],
                        [5, 4, 3, 2, 1, 0],
                        [3, 4, 5, 0, 1, 2],
                        [0, 3, 1, 4, 2, 5],
                        [3, 0, 4, 1, 5, 2],
                        [5, 2, 4, 1, 3, 0],
                        [2, 5, 1, 4, 0, 3],
                    ];
                    row.pixels = orders[orientation as usize - 1]
                        .iter()
                        .flat_map(|&i| expected[i])
                        .collect();
                    if orientation >= 5 {
                        row.width = 2;
                        row.height = 3;
                    }
                    cases.push(row);
                }
            }
        }
    }
    cases
}

pub fn invalid_cicp() -> Vec<Case> {
    let mut cases = Vec::new();
    for field in 0..4 {
        for value in 0..=255 {
            let valid = match field {
                0 => matches!(value, 1 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 22),
                1 => matches!(
                    value,
                    1 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18
                ),
                2 => value == 0,
                3 => value <= 1,
                _ => unreachable!(),
            };
            if valid {
                continue;
            }
            let mut data = vec![1, 13, 0, 1];
            data[field] = value;
            let mut c = base();
            c.insert(1, (*b"cICP", data));
            cases.push(case(
                format!("invalid-cicp-{field}-{value}"),
                &c,
                Outcome::Unsupported,
            ));
        }
    }
    cases
}

pub fn large_streams() -> Vec<Case> {
    let mut cases = Vec::new();
    for (width, height) in [(37, 33), (1, 1500)] {
        for split in [false, true] {
            let mut raw = Vec::new();
            for _ in 0..height {
                raw.push(0);
                raw.extend_from_slice(&[255, 85, 170, 255].repeat(width));
            }
            let data = compressed(&raw);
            let mut c = vec![(*b"IHDR", header(width as u32, height as u32, 6, 8, false))];
            for part in data.chunks(if split { 1 } else { data.len() }) {
                c.push((*b"IDAT", part.to_vec()));
            }
            c.push((*b"IEND", vec![]));
            let mut row = case(
                format!("large-stream-{width}-{height}-{split}"),
                &c,
                Outcome::Accepted(1),
            );
            row.width = width;
            row.height = height;
            row.pixels = [255, 85, 170, 255].repeat(width * height);
            cases.push(row);
        }
    }
    cases
}
pub fn report(row: &Case, format: &str, mut actual: Value) {
    for d in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(!d["message"].as_str().unwrap().is_empty());
        d.as_object_mut().unwrap().remove("message");
    }
    let recovery = match row.outcome {
        Outcome::DecodeFailed => Some("decode-failed"),
        Outcome::Unsupported if format != "svg" => Some("unsupported-format"),
        _ => None,
    };
    let diagnostics = if let Some(code) = recovery {
        json!([{"code":format!("image.{code}"),"severity":"warning","format":format,"effects":["visual"],"element_id":"image","json_pointer":"/files/asset/dataURL","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":if code=="decode-failed" {json!("image/png")} else {Value::Null},"recovery":if format=="png" {"placeholder"} else {"omitted"},"recipe_id":if format=="png" {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}}])
    } else if matches!(row.outcome,Outcome::Accepted(n) if n>1) && format != "svg" {
        json!([{"code":"image.animation-flattened","severity":"info","format":format,"effects":["visual"],"element_id":"image","json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/png","recovery":"flattened","recipe_id":null}}])
    } else {
        json!([])
    };
    assert_eq!(
        actual,
        json!({"schema_version":1,"diagnostics":diagnostics}),
        "{} {format}",
        row.id
    );
}
