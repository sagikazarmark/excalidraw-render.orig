use zune_jpeg::{
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
    JpegDecoder,
};
fn segment(out: &mut Vec<u8>, marker: u8, data: &[u8]) {
    out.extend([255, marker]);
    out.extend(((data.len() + 2) as u16).to_be_bytes());
    out.extend(data);
}
fn entropy(out: &mut Vec<u8>, bits: &mut Vec<bool>) {
    while bits.len() % 8 != 0 {
        bits.push(true);
    }
    for b in bits.chunks(8) {
        let value = b.iter().fold(0, |v, b| v * 2 + u8::from(*b));
        out.push(value);
        if value == 255 {
            out.push(0);
        }
    }
    bits.clear();
}
fn push(bits: &mut Vec<bool>, value: u32, n: u32) {
    for i in (0..n).rev() {
        bits.push(value & (1 << i) != 0);
    }
}
pub fn source(
    h: usize,
    v: usize,
    width: usize,
    height: usize,
    groups: &[Vec<usize>],
    restart: usize,
) -> Vec<u8> {
    let mut out = vec![255, 216];
    let mut qt = vec![0];
    qt.extend([8; 64]);
    segment(&mut out, 219, &qt);
    let mut frame = vec![8];
    frame.extend((height as u16).to_be_bytes());
    frame.extend((width as u16).to_be_bytes());
    frame.push(3);
    for (id, hs, vs) in [(1, h, v), (2, 1, 1), (3, 1, 1)] {
        frame.extend([id, ((hs << 4) | vs) as u8, 0]);
    }
    segment(&mut out, 192, &frame);
    let mut dc = vec![0];
    let mut lengths = [0; 16];
    lengths[3] = 9;
    dc.extend(lengths);
    dc.extend(0..=8);
    segment(&mut out, 196, &dc);
    let mut ac = vec![16, 1];
    ac.extend([0; 15]);
    ac.push(0);
    segment(&mut out, 196, &ac);
    if restart != 0 {
        segment(&mut out, 221, &(restart as u16).to_be_bytes());
    }
    for group in groups {
        let mut sos = vec![group.len() as u8];
        for id in group {
            sos.extend([*id as u8, 0]);
        }
        sos.extend([0, 63, 0]);
        segment(&mut out, 218, &sos);
        let multi = group.len() > 1;
        let id = group[0];
        let hs = if id == 1 { h } else { 1 };
        let vs = if id == 1 { v } else { 1 };
        let cols = if multi {
            width.div_ceil(8 * h)
        } else {
            (width * hs).div_ceil(8 * h)
        };
        let rows = if multi {
            height.div_ceil(8 * v)
        } else {
            (height * vs).div_ceil(8 * v)
        };
        let mut predictions = [0i32; 3];
        let mut bits = vec![];
        let mut mcu = 0;
        let mut rst = 0;
        for y in 0..rows {
            for x in 0..cols {
                for id in group {
                    let hs = if *id == 1 { h } else { 1 };
                    let vs = if *id == 1 { v } else { 1 };
                    for dy in 0..if multi { vs } else { 1 } {
                        for dx in 0..if multi { hs } else { 1 } {
                            let bx = if multi { x * hs + dx } else { x };
                            let by = if multi { y * vs + dy } else { y };
                            let sample = if *id == 1 {
                                64 + ((bx * 29 + by * 37) % 128) as i32
                            } else {
                                128
                            };
                            let coefficient = sample - 128;
                            let diff = coefficient - predictions[*id - 1];
                            predictions[*id - 1] = coefficient;
                            let size = 32 - diff.unsigned_abs().leading_zeros();
                            push(&mut bits, size, 4);
                            if size != 0 {
                                let value = if diff < 0 {
                                    ((1 << size) - 1) as i32 + diff
                                } else {
                                    diff
                                };
                                push(&mut bits, value as u32, size);
                            }
                            push(&mut bits, 0, 1);
                        }
                    }
                }
                mcu += 1;
                if restart != 0 && mcu % restart == 0 && mcu < rows * cols {
                    entropy(&mut out, &mut bits);
                    out.extend([255, 208 + rst]);
                    rst = (rst + 1) % 8;
                    predictions = [0; 3];
                }
            }
        }
        entropy(&mut out, &mut bits);
    }
    out.extend([255, 217]);
    out
}
pub fn run() {
    let mut count = 0;
    for (h, v) in [(1, 1), (2, 2), (4, 2), (1, 4), (2, 4)] {
        for (w, ht) in [(9, 9), (33, 25), (48, 32)] {
            for groups in [
                vec![vec![1], vec![2], vec![3]],
                vec![vec![1], vec![2, 3]],
                vec![vec![1, 2], vec![3]],
                vec![vec![3], vec![2, 1]],
                vec![vec![1, 2, 3]],
            ] {
                for restart in [0, 1, 3] {
                    let bytes = source(h, v, w, ht, &groups, restart);
                    if let Ok(path) = std::env::var("JPEG187_MIXED_EXPORT") {
                        let directory = std::path::Path::new(&path);
                        std::fs::create_dir_all(directory).unwrap();
                        let expected: Vec<u8> = (0..ht)
                            .flat_map(|y| {
                                (0..w).flat_map(move |x| {
                                    [64 + (((x / 8) * 29 + (y / 8) * 37) % 128) as u8; 3]
                                })
                            })
                            .collect();
                        let write = |name: String, data: &[u8]| {
                            use std::io::Write;
                            let mut f = std::fs::OpenOptions::new()
                                .create_new(true)
                                .write(true)
                                .open(directory.join(name))
                                .unwrap();
                            f.write_all(data).unwrap();
                        };
                        write(format!("{count:03}.jpg"), &bytes);
                        write(format!("{count:03}.rgb"), &expected);
                    }
                    let options = DecoderOptions::default()
                        .set_use_unsafe(false)
                        .set_strict_mode(true)
                        .jpeg_set_out_colorspace(ColorSpace::RGB);
                    let result = std::panic::catch_unwind(|| {
                        JpegDecoder::new_with_options(ZCursor::new(&bytes), options).decode()
                    });
                    let actual = result.expect("panic").unwrap_or_else(|e| {
                        panic!("{h}x{v} {w}x{ht} {groups:?} restart{restart}: {e:?}")
                    });
                    let expected: Vec<u8> = (0..ht)
                        .flat_map(|y| {
                            (0..w).flat_map(move |x| {
                                [64 + (((x / 8) * 29 + (y / 8) * 37) % 128) as u8; 3]
                            })
                        })
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "{h}x{v} {w}x{ht} {groups:?} restart{restart}"
                    );
                    count += 1;
                }
            }
        }
    }
    println!("{count} independently authored nonuniform mixed-scan controls pass");
    let mut rejected = 0;
    for groups in [
        vec![vec![1], vec![2, 3]],
        vec![vec![1, 2], vec![3]],
        vec![vec![3], vec![2, 1]],
    ] {
        let bytes = source(2, 2, 33, 25, &groups, 1);
        let restarts: Vec<_> = bytes
            .windows(2)
            .enumerate()
            .filter(|(_, b)| b[0] == 255 && (208..216).contains(&b[1]))
            .map(|(i, _)| i)
            .collect();
        for at in restarts {
            for mode in 0..3 {
                let mut bad = bytes.clone();
                if mode == 0 {
                    bad.drain(at..at + 2);
                } else if mode == 1 {
                    bad[at + 1] = 208 + (bad[at + 1] - 208 + 1) % 8;
                } else {
                    bad[at + 1] = 217;
                }
                let options = DecoderOptions::default()
                    .set_use_unsafe(false)
                    .set_strict_mode(true)
                    .jpeg_set_out_colorspace(ColorSpace::RGB);
                let result = std::panic::catch_unwind(|| {
                    JpegDecoder::new_with_options(ZCursor::new(&bad), options).decode()
                });
                assert!(
                    matches!(result, Ok(Err(_))),
                    "restart mutation accepted groups={groups:?} at={at} mode={mode}"
                );
                rejected += 1;
            }
        }
        // Legal inter-scan metadata must not change the decoded picture.
        let second = bytes
            .windows(2)
            .enumerate()
            .filter(|(_, b)| *b == [255, 218])
            .nth(1)
            .unwrap()
            .0;
        let mut changed = bytes[..second].to_vec();
        segment(&mut changed, 254, b"inter-scan comment");
        segment(&mut changed, 225, b"non-EXIF APP1");
        let mut q = vec![0];
        q.extend([8; 64]);
        segment(&mut changed, 219, &q);
        let mut dc = vec![0];
        let mut lengths = [0; 16];
        lengths[3] = 9;
        dc.extend(lengths);
        dc.extend(0..=8);
        segment(&mut changed, 196, &dc);
        segment(&mut changed, 221, &1_u16.to_be_bytes());
        changed.extend_from_slice(&bytes[second..]);
        let options = DecoderOptions::default()
            .set_use_unsafe(false)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(ColorSpace::RGB);
        let actual = JpegDecoder::new_with_options(ZCursor::new(&changed), options)
            .decode()
            .unwrap();
        let expected = JpegDecoder::new_with_options(ZCursor::new(&bytes), options)
            .decode()
            .unwrap();
        assert_eq!(actual, expected);
    }
    println!("{rejected} restart mutations rejected; 3 mixed inter-scan metadata cases pass");
    let mut scans = 0;
    for groups in [
        vec![vec![1], vec![2, 3]],
        vec![vec![1, 2], vec![3]],
        vec![vec![3], vec![2, 1]],
    ] {
        let source = source(2, 2, 33, 25, &groups, 0);
        let second = source
            .windows(2)
            .enumerate()
            .filter(|(_, b)| *b == [255, 218])
            .nth(1)
            .unwrap()
            .0;
        let mut missing = source[..second].to_vec();
        missing.extend([255, 217]);
        let mut duplicate = source.clone();
        duplicate[second + 5] = groups[0][0] as u8;
        let mut extra = source[..second].to_vec();
        extra.extend([0, 0]);
        extra.extend_from_slice(&source[second..]);
        for bad in [missing, duplicate, extra] {
            for strict in [false, true] {
                let options = DecoderOptions::default()
                    .set_use_unsafe(false)
                    .set_strict_mode(strict)
                    .jpeg_set_out_colorspace(ColorSpace::RGB);
                let result = std::panic::catch_unwind(|| {
                    JpegDecoder::new_with_options(ZCursor::new(&bad), options).decode()
                });
                assert!(result.is_ok(), "malformed input panicked strict={strict}");
                if strict {
                    assert!(
                        result.unwrap().is_err(),
                        "malformed scan inventory accepted"
                    );
                    scans += 1;
                }
            }
        }
    }
    println!("{scans} missing/duplicate/extra-entropy scans rejected strictly; permissive runs do not panic");
    let mut terminal = 0;
    for groups in [vec![vec![1], vec![2, 3]], vec![vec![1, 2, 3]]] {
        for restart in [0, 1, 3] {
            let valid = source(2, 2, 33, 25, &groups, restart);
            let sos = valid
                .windows(2)
                .enumerate()
                .find(|(_, b)| *b == [255, 218])
                .unwrap()
                .0;
            let start = sos + 2 + u16::from_be_bytes([valid[sos + 2], valid[sos + 3]]) as usize;
            let mut bads = vec![valid[..start].to_vec(), valid[..valid.len() - 2].to_vec()];
            let mut extra = valid[..valid.len() - 2].to_vec();
            extra.extend([0, 0, 255, 217]);
            bads.push(extra);
            let mut rst = valid[..valid.len() - 2].to_vec();
            rst.extend([255, 215, 255, 217]);
            bads.push(rst);
            let mut dup = valid[..valid.len() - 2].to_vec();
            dup.extend_from_slice(&valid[sos..]);
            bads.push(dup);
            for bad in bads {
                for strict in [false, true] {
                    let options = DecoderOptions::default()
                        .set_use_unsafe(false)
                        .set_strict_mode(strict)
                        .jpeg_set_out_colorspace(ColorSpace::RGB);
                    let result = std::panic::catch_unwind(|| {
                        JpegDecoder::new_with_options(ZCursor::new(&bad), options).decode()
                    });
                    assert!(result.is_ok(), "terminal mutation panic");
                    if strict {
                        assert!(
                            result.unwrap().is_err(),
                            "terminal mutation accepted {groups:?} restart={restart}"
                        );
                        terminal += 1;
                    }
                }
            }
        }
    }
    // Grayscale with valid one-bit DC-zero and EOB codes: zero-filled physical
    // EOF would otherwise manufacture a perfectly neutral block.
    let mut gray = vec![255, 216];
    let mut qt = vec![0];
    qt.extend([8; 64]);
    segment(&mut gray, 219, &qt);
    segment(&mut gray, 192, &[8, 0, 8, 0, 8, 1, 1, 17, 0]);
    for class in [0, 16] {
        let mut table = vec![class, 1];
        table.extend([0; 15]);
        table.push(0);
        segment(&mut gray, 196, &table);
    }
    segment(&mut gray, 218, &[1, 1, 0, 0, 63, 0]);
    let options = DecoderOptions::default()
        .set_use_unsafe(false)
        .set_strict_mode(true)
        .jpeg_set_out_colorspace(ColorSpace::RGB);
    assert!(JpegDecoder::new_with_options(ZCursor::new(&gray), options)
        .decode()
        .is_err());
    terminal += 1;
    let mut padded = gray.clone();
    padded.extend([0, 255, 217]);
    assert!(
        JpegDecoder::new_with_options(ZCursor::new(&padded), options)
            .decode()
            .is_err()
    );
    terminal += 1;
    gray.extend([63, 255, 217]);
    assert_eq!(
        JpegDecoder::new_with_options(ZCursor::new(&gray), options)
            .decode()
            .unwrap(),
        vec![128; 192]
    );
    println!("{terminal} strict terminal/physical-EOF/padding cases rejected; valid minimal grayscale passes");
}
