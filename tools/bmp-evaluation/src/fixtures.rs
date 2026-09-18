pub struct Case {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub fn u16_at(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub fn u32_at(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

// Palette zero is deliberately nonblack. Palette bytes are BGR, reserved=0.
const PALETTE: [[u8; 3]; 3] = [[30, 20, 10], [60, 50, 240], [90, 200, 70]];

pub fn bmp(
    dib: usize,
    width: u32,
    height: i32,
    bits: u16,
    compression: u32,
    masks: &[u32],
    pixels: &[u8],
) -> Vec<u8> {
    let external = if dib == 40 { masks.len() * 4 } else { 0 };
    let palette_entries = if bits <= 8 {
        if dib == 12 {
            1 << bits
        } else {
            2
        }
    } else {
        0
    };
    let palette_stride = if dib == 12 { 3 } else { 4 };
    let offset = 14 + dib + external + palette_entries * palette_stride;
    let mut bytes = vec![0; offset + pixels.len()];
    bytes[..2].copy_from_slice(b"BM");
    let size = bytes.len() as u32;
    u32_at(&mut bytes, 2, size);
    u32_at(&mut bytes, 10, offset as u32);
    u32_at(&mut bytes, 14, dib as u32);
    if dib == 12 {
        u16_at(&mut bytes, 18, width as u16);
        u16_at(&mut bytes, 20, height as u16);
        u16_at(&mut bytes, 22, 1);
        u16_at(&mut bytes, 24, bits);
    } else {
        u32_at(&mut bytes, 18, width);
        u32_at(&mut bytes, 22, height as u32);
        u16_at(&mut bytes, 26, 1);
        u16_at(&mut bytes, 28, bits);
        u32_at(&mut bytes, 30, compression);
        u32_at(&mut bytes, 34, pixels.len() as u32);
        u32_at(&mut bytes, 46, palette_entries as u32);
    }
    for (i, mask) in masks.iter().enumerate() {
        u32_at(&mut bytes, 54 + i * 4, *mask);
    }
    for i in 0..palette_entries {
        let start = 14 + dib + external + i * palette_stride;
        bytes[start..start + 3].copy_from_slice(&PALETTE[i % PALETTE.len()]);
    }
    bytes[offset..].copy_from_slice(pixels);
    bytes
}

pub fn bmps() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut add = |name: &str, bytes| {
        cases.push(Case {
            name: name.into(),
            bytes,
        })
    };
    let rgb = [30, 20, 10, 0, 60, 50, 240, 0];
    for dib in [12, 40, 52, 56, 108, 124] {
        add(
            &format!("header-{dib}-rgb24"),
            bmp(dib, 1, 2, 24, 0, &[], &rgb),
        );
    }
    add("top-down-rgb24", bmp(40, 1, -2, 24, 0, &[], &rgb));
    for (bits, data) in [(1, 0x40), (4, 0x01), (8, 0)] {
        let row = if bits == 8 {
            [0, 1, 0, 0]
        } else {
            [data, 0, 0, 0]
        };
        add(
            &format!("palette-{bits}"),
            bmp(40, 2, 1, bits, 0, &[], &row),
        );
        add(
            &format!("core-palette-{bits}"),
            bmp(12, 2, 1, bits, 0, &[], &row),
        );
    }
    add(
        "palette-2-outside-profile",
        bmp(40, 2, 1, 2, 0, &[], &[0x10, 0, 0, 0]),
    );
    add("palette-hole-8", bmp(40, 1, 1, 8, 0, &[], &[2, 0, 0, 0]));
    add("palette-hole-4", bmp(40, 1, 1, 4, 0, &[], &[0x20, 0, 0, 0]));
    add(
        "rgb32-unused-alpha",
        bmp(40, 1, 1, 32, 0, &[], &[30, 20, 10, 128]),
    );
    add(
        "rgb16-555",
        bmp(40, 2, 1, 16, 0, &[], &[0x10, 0x42, 0xff, 0x7f]),
    );
    add(
        "bitfields-565",
        bmp(
            40,
            2,
            1,
            16,
            3,
            &[0xf800, 0x7e0, 0x1f],
            &[0x10, 0x84, 0xff, 0xff],
        ),
    );
    let rgba = [0xff0000, 0xff00, 0xff, 0xff000000];
    for dib in [56, 108, 124] {
        for compression in [0, 3, 6] {
            add(
                &format!("alpha-header-{dib}-compression-{compression}"),
                bmp(dib, 1, 1, 32, compression, &rgba, &[30, 20, 10, 128]),
            );
        }
    }
    add(
        "alpha-external-code-6",
        bmp(40, 1, 1, 32, 6, &rgba, &[30, 20, 10, 128]),
    );
    add("alpha-zero", bmp(56, 1, 1, 32, 3, &rgba, &[30, 20, 10, 0]));
    add(
        "bitfields-10-bit-rounding",
        bmp(
            56,
            1,
            1,
            32,
            3,
            &[0x3ff00000, 0xffc00, 0x3ff, 0xc0000000],
            &0xc0000003u32.to_le_bytes(),
        ),
    );
    add(
        "bitfields-overlap",
        bmp(
            40,
            1,
            1,
            16,
            3,
            &[0x7c00, 0x7c00, 0x1f],
            &[0xff, 0x7f, 0, 0],
        ),
    );
    add(
        "bitfields-noncontiguous",
        bmp(40, 1, 1, 16, 3, &[0x7400, 0x3e0, 0x1f], &[0xff, 0x7f, 0, 0]),
    );
    add(
        "bitfields-outside-word",
        bmp(
            40,
            1,
            1,
            16,
            3,
            &[0x10000, 0x3e0, 0x1f],
            &[0xff, 0x7f, 0, 0],
        ),
    );
    add(
        "bitfields-zero-red",
        bmp(40, 1, 1, 16, 3, &[0, 0x3e0, 0x1f], &[0xff, 0x7f, 0, 0]),
    );
    for (bits, compression, run, literal) in [
        (8, 1, 1, vec![0, 3, 1, 0, 1, 0]),
        (4, 2, 0x11, vec![0, 3, 0x10, 0x10]),
    ] {
        let mut rle = |name: &str, data: &[u8]| {
            add(
                &format!("rle{bits}-{name}"),
                bmp(40, 3, 2, bits, compression, &[], data),
            )
        };
        rle("complete", &[3, run, 0, 0, 3, run, 0, 1]);
        rle("early-eob", &[1, run, 0, 1]);
        rle("short-eol", &[1, run, 0, 0, 3, run, 0, 1]);
        rle("delta-x", &[0, 2, 1, 0, 2, run, 0, 1]);
        rle("delta-xy", &[1, run, 0, 2, 1, 1, 1, run, 0, 1]);
        rle("delta-x-boundary", &[0, 2, 3, 0, 0, 1]);
        rle("delta-x-overrun", &[0, 2, 4, 0, 0, 1]);
        rle("delta-y-overrun", &[0, 2, 0, 2, 0, 1]);
        rle("encoded-overrun", &[4, run, 0, 1]);
        rle("palette-hole", &[1, if bits == 8 { 2 } else { 0x22 }, 0, 1]);
        let mut data = literal.clone();
        data.extend_from_slice(&[0, 1]);
        rle("absolute", &data);
        data[1] = 4;
        rle("absolute-overrun", &data);
        rle("missing-eob", &[3, run, 0, 0, 3, run, 0, 0]);
        rle("truncated-command", &[0, 2, 1]);
        rle("truncated-literal", &literal[..literal.len() - 1]);
        add(
            &format!("rle{bits}-top-down"),
            bmp(40, 3, -2, bits, compression, &[], &[3, run, 0, 1]),
        );
    }
    let base = bmp(40, 1, 1, 24, 0, &[], &rgb[..4]);
    for (name, offset, value) in [
        ("declared-size-mismatch", 2, 1),
        ("declared-size-zero", 2, 0),
        ("reserved-nonzero", 6, 1),
        ("pixel-offset-in-header", 10, 14),
        ("pixel-offset-past-end", 10, 999),
        ("unknown-header", 14, 41),
        ("zero-width", 18, 0),
        ("negative-width", 18, u32::MAX),
        ("zero-height", 22, 0),
        ("minimum-height", 22, i32::MIN as u32),
        ("unsupported-compression", 30, 4),
    ] {
        let mut bytes = base.clone();
        u32_at(&mut bytes, offset, value);
        add(name, bytes);
    }
    for length in [13, 25, 53, 54, 57] {
        let mut bytes = base[..length].to_vec();
        u32_at(&mut bytes, 2, length as u32);
        add(&format!("truncated-at-{length}"), bytes);
    }
    let mut profile = bmp(124, 1, 1, 24, 0, &[], &rgb[..4]);
    u32_at(&mut profile, 14 + 56, 0x4d424544); // PROFILE_EMBEDDED
    u32_at(&mut profile, 14 + 112, 128);
    u32_at(&mut profile, 14 + 116, 4);
    profile.extend_from_slice(b"bad!");
    let size = profile.len() as u32;
    u32_at(&mut profile, 2, size);
    add("v5-invalid-embedded-profile-ignored", profile);
    cases
}

fn exif(big: bool, entries: &[(u16, u16, u32, u16)]) -> Vec<u8> {
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
    let mut bytes = Vec::from(if big { *b"MM" } else { *b"II" });
    bytes.extend(short(42));
    bytes.extend(long(8));
    bytes.extend(short(entries.len() as u16));
    for &(tag, format, count, value) in entries {
        bytes.extend(short(tag));
        bytes.extend(short(format));
        bytes.extend(long(count));
        bytes.extend(short(value));
        bytes.extend([0, 0]);
    }
    bytes.extend(long(0));
    bytes
}

pub fn exifs() -> Vec<Case> {
    let mut cases = Vec::new();
    for big in [false, true] {
        let mut add = |name: &str, bytes| {
            cases.push(Case {
                name: format!("{}-{name}", if big { "be" } else { "le" }),
                bytes,
            })
        };
        for value in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 255, 256, 65535] {
            add(
                &format!("value-{value}"),
                exif(big, &[(0x112, 3, 1, value)]),
            );
        }
        for (name, entries) in [
            ("absent", vec![]),
            (
                "unrelated-before",
                vec![(0x111, 4, 100, 0), (0x112, 3, 1, 6)],
            ),
            ("duplicate-valid", vec![(0x112, 3, 1, 6), (0x112, 3, 1, 8)]),
            (
                "invalid-value-first",
                vec![(0x112, 3, 1, 9), (0x112, 3, 1, 8)],
            ),
            ("wrong-type-only", vec![(0x112, 4, 1, 6)]),
            ("wrong-type-first", vec![(0x112, 4, 1, 6), (0x112, 3, 1, 8)]),
            (
                "wrong-count-first",
                vec![(0x112, 3, 2, 6), (0x112, 3, 1, 8)],
            ),
            ("zero-count-first", vec![(0x112, 3, 0, 6), (0x112, 3, 1, 8)]),
        ] {
            add(name, exif(big, &entries));
        }
        let base = exif(big, &[(0x112, 3, 1, 6)]);
        for length in [0, 2, 4, 7, 9, 10, 19, 20, 21, 22] {
            add(&format!("truncated-{length}"), base[..length].to_vec());
        }
        let mut bytes = base.clone();
        bytes[2] = 0xff;
        add("wrong-magic", bytes);
        let mut bytes = base.clone();
        bytes[4..8].fill(0xff);
        add("ifd-outside-chunk", bytes);
        let mut bytes = b"Exif\0\0".to_vec();
        bytes.extend(&base);
        add("jpeg-prefix-not-tiff", bytes);
        // Neither reader traverses the next IFD.
        let mut bytes = exif(big, &[]);
        let next = if big {
            14u32.to_be_bytes()
        } else {
            14u32.to_le_bytes()
        };
        bytes[10..14].copy_from_slice(&next);
        bytes.extend(&base[8..]);
        add("orientation-in-second-ifd", bytes);
    }
    cases
}
