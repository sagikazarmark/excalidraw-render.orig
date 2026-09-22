use jpeg_encoder::{ColorType, Encoder, SamplingFactor};
use zune_jpeg::{
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
    JpegDecoder,
};
fn main() {
    if std::env::args().any(|a| a == "malformed") {
        malformed();
        return;
    }
    if std::env::args().any(|a| a == "matrix") {
        matrix();
        return;
    }
    let mut failures = 0;
    for sf in [
        SamplingFactor::F_4_2,
        SamplingFactor::F_1_4,
        SamplingFactor::F_2_4,
    ] {
        for restart in [0, 1, 2, 3, 7] {
            for progressive in [false, true] {
                for h in [1, 7, 8, 9, 15, 16, 17, 24, 25, 31, 32, 33] {
                    for w in [1, 8, 9, 16, 17, 33] {
                        let mut bytes = Vec::new();
                        let mut e = Encoder::new(&mut bytes, 100);
                        e.set_sampling_factor(sf);
                        e.set_restart_interval(restart);
                        e.set_progressive(progressive);
                        e.encode(
                            &[96, 128, 128].repeat(w as usize * h as usize),
                            w,
                            h,
                            ColorType::Ycbcr,
                        )
                        .unwrap();
                        let options = DecoderOptions::default()
                            .set_use_unsafe(false)
                            .set_strict_mode(true)
                            .jpeg_set_out_colorspace(ColorSpace::RGB);
                        let p = JpegDecoder::new_with_options(ZCursor::new(&bytes), options)
                            .decode()
                            .unwrap();
                        let bad = p.chunks_exact(3).filter(|p| *p != [96, 96, 96]).count();
                        if bad != 0 {
                            failures += 1;
                            println!("{sf:?} {w}x{h} restart={restart} progressive={progressive}: {bad} wrong");
                        }
                    }
                }
            }
        }
    }
    assert_eq!(failures, 0, "neutral size/sampling cells failed");
}

fn malformed() {
    for sf in [
        SamplingFactor::F_4_2,
        SamplingFactor::F_1_4,
        SamplingFactor::F_2_4,
    ] {
        let mut bytes = Vec::new();
        let mut e = Encoder::new(&mut bytes, 100);
        e.set_sampling_factor(sf);
        e.encode(&[96, 128, 128].repeat(81), 9, 9, ColorType::Ycbcr)
            .unwrap();
        let sos: Vec<_> = bytes
            .windows(2)
            .enumerate()
            .filter(|(_, p)| *p == [255, 218])
            .map(|(i, _)| i)
            .collect();
        for (scan, start) in sos.iter().enumerate() {
            let length = u16::from_be_bytes([bytes[start + 2], bytes[start + 3]]) as usize;
            let entropy = start + 2 + length;
            let end = sos.get(scan + 1).copied().unwrap_or(bytes.len() - 2);
            for keep in 0..end - entropy {
                let mut bad = bytes[..entropy + keep].to_vec();
                bad.extend_from_slice(&bytes[end..]);
                let opts = DecoderOptions::default()
                    .set_use_unsafe(false)
                    .set_strict_mode(true)
                    .jpeg_set_out_colorspace(ColorSpace::RGB);
                let original =
                    stable_jpeg::JpegDecoder::new_with_options(ZCursor::new(&bad), opts).decode();
                print!(
                    "original={} ",
                    if original.is_ok() {
                        "accepted"
                    } else {
                        "rejected"
                    }
                );
                let result = std::panic::catch_unwind(|| {
                    JpegDecoder::new_with_options(ZCursor::new(&bad), opts).decode()
                });
                match result {
                    Ok(Ok(p)) => println!(
                        "{sf:?} scan={scan} keep={keep}/{} accepted wrong={}",
                        end - entropy,
                        p.chunks_exact(3).filter(|p| *p != [96, 96, 96]).count()
                    ),
                    Ok(Err(_)) => {
                        println!("{sf:?} scan={scan} keep={keep}/{} rejected", end - entropy)
                    }
                    Err(_) => panic!("panic on malformed scan"),
                }
            }
        }
    }
}

fn matrix() {
    use base64::Engine as _;
    use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
    let root = std::env::var("JPEG187_REPOSITORY").expect("JPEG187_REPOSITORY");
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("{root}/tests/oracle/jpeg-color/reference.json")).unwrap(),
    )
    .unwrap();
    let mut failures = 0;
    for row in record["cases"].as_array().unwrap() {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(row["jpeg_base64"].as_str().unwrap())
            .unwrap();
        let space = match row["model"].as_str().unwrap() {
            "cmyk" => ColorSpace::CMYK,
            "ycck" => ColorSpace::YCCK,
            _ => ColorSpace::RGB,
        };
        let opts = DecoderOptions::default()
            .set_use_unsafe(false)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(space);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&bytes), opts);
        let raw = decoder.decode().unwrap();
        let actual: Vec<u8> = if matches!(space, ColorSpace::CMYK | ColorSpace::YCCK) {
            raw.chunks_exact(4)
                .flat_map(|p| {
                    let c = if space == ColorSpace::YCCK {
                        let y = i32::from(p[0]);
                        let cb = i32::from(p[1]) - 128;
                        let cr = i32::from(p[2]) - 128;
                        [
                            y + ((91881 * cr + 32768) >> 16),
                            y + ((-22554 * cb - 46802 * cr + 32768) >> 16),
                            y + ((116130 * cb + 32768) >> 16),
                        ]
                        .map(|v| (255 - v.clamp(0, 255)) as u8)
                    } else {
                        [p[0], p[1], p[2]]
                    };
                    [
                        (u32::from(c[0]) * u32::from(p[3]) / 255) as u8,
                        (u32::from(c[1]) * u32::from(p[3]) / 255) as u8,
                        (u32::from(c[2]) * u32::from(p[3]) / 255) as u8,
                        255,
                    ]
                })
                .collect()
        } else {
            raw.chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect()
        };
        for authority in ["png", "svg_png"] {
            let png = base64::engine::general_purpose::STANDARD
                .decode(row["reference"][authority].as_str().unwrap())
                .unwrap();
            let mut reader = png::Decoder::new(std::io::Cursor::new(png))
                .read_info()
                .unwrap();
            let mut rgba = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut rgba).unwrap();
            rgba.truncate(info.buffer_size());
            let expected = CanonicalRgba8::new(info.width, info.height, rgba).unwrap();
            let actual = CanonicalRgba8::new(info.width, info.height, actual.clone()).unwrap();
            let result = compare(
                &expected,
                &actual,
                VisualClass::GeometryImage,
                &[],
                ComparisonMode::Canonical,
            )
            .unwrap();
            if !result.passed() {
                failures += 1;
                println!(
                    "{} changed={} ssim={:?}",
                    row["id"], result.whole.changed_pixels, result.whole.ssim
                );
            }
        }
    }
    println!("matrix failures={failures}");
    assert_eq!(failures, 0);
}
