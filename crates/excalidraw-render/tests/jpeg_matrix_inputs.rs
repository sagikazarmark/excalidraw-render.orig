//! Authored JPEG color/sampling inputs, independent of the production decoder.
//! Reconstructs the frozen inputs byte-for-byte without writing reference data.
use base64::Engine as _;
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};
use serde_json::json;
use sha2::{Digest, Sha256};

const FACTORS: [(u8, u8, SamplingFactor); 8] = [
    (1, 1, SamplingFactor::F_1_1),
    (2, 1, SamplingFactor::F_2_1),
    (1, 2, SamplingFactor::F_1_2),
    (2, 2, SamplingFactor::F_2_2),
    (4, 1, SamplingFactor::F_4_1),
    (4, 2, SamplingFactor::F_4_2),
    (1, 4, SamplingFactor::F_1_4),
    (2, 4, SamplingFactor::F_2_4),
];

fn pixels(pattern: &str, width: usize, height: usize) -> Vec<[u8; 3]> {
    const COLORS: [[u8; 3]; 6] = [
        [231, 23, 57],
        [19, 211, 83],
        [41, 67, 229],
        [219, 197, 31],
        [37, 203, 217],
        [183, 43, 193],
    ];
    (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                if pattern == "edges" {
                    COLORS[(x / 7 + 2 * (y / 5) + usize::from(x == width - 1)) % 6]
                } else {
                    [
                        (17 + 213 * x / (width - 1)) as u8,
                        (29 + 191 * y / (height - 1)) as u8,
                        (43 + 167 * (x + y) / (width + height - 2)) as u8,
                    ]
                }
            })
        })
        .collect()
}

// The encoder has no raw-RGB mode. Feed its YCbCr entry point RGB channel
// samples unchanged, remove JFIF, identify components as R/G/B in SOF/SOS,
// and add Adobe transform 0. Entropy-coded component samples stay unchanged.
fn rgb_markers(source: &[u8]) -> Vec<u8> {
    let mut result = b"\xff\xd8\xff\xee\0\x0eAdobe\0\x64\0\0\0\0\0".to_vec();
    let mut offset = 2;
    while offset < source.len() {
        assert_eq!(source[offset], 255);
        let marker = source[offset + 1];
        if marker == 0xd9 {
            result.extend_from_slice(&source[offset..]);
            break;
        }
        let length = u16::from_be_bytes([source[offset + 2], source[offset + 3]]) as usize;
        let end = offset + 2 + length;
        let mut segment = source[offset..end].to_vec();
        if matches!(marker, 0xc0..=0xc2) {
            assert_eq!(segment[9], 3);
            for (i, name) in b"RGB".iter().enumerate() {
                segment[10 + 3 * i] = *name;
            }
        }
        if marker == 0xda {
            for i in 0..segment[4] as usize {
                let id = segment[5 + 2 * i];
                segment[5 + 2 * i] = b"RGB"[id as usize];
            }
        }
        if marker != 0xe0 {
            result.extend_from_slice(&segment);
        }
        offset = end;
        if marker == 0xda {
            let start = offset;
            while offset + 1 < source.len() {
                if source[offset] == 255
                    && source[offset + 1] != 0
                    && !matches!(source[offset + 1], 0xd0..=0xd7)
                {
                    break;
                }
                offset += 1;
            }
            result.extend_from_slice(&source[start..offset]);
        }
    }
    result
}

#[test]
fn authored_jpeg_inputs_reproduce_exactly() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for model in ["gray", "rgb", "ycbcr", "cmyk", "ycck"] {
        let factors = if matches!(model, "ycbcr" | "ycck") {
            &FACTORS[..]
        } else {
            &FACTORS[..1]
        };
        for &(h, v, factor) in factors {
            for progressive in [false, true] {
                for pattern in ["ramp", "edges"] {
                    let (width, height) = if pattern == "ramp" {
                        (33, 25)
                    } else {
                        (48, 32)
                    };
                    let authored = pixels(pattern, width, height);
                    let samples: Vec<_> = authored
                        .iter()
                        .enumerate()
                        .flat_map(|(i, &[r, g, b])| {
                            match model {
                                "gray" => {
                                    vec![((u16::from(r) + u16::from(g) + u16::from(b)) / 3) as u8]
                                }
                                "rgb" | "ycbcr" => vec![r, g, b],
                                // Vary K independently to expose omitted/inverted black.
                                _ => vec![
                                    255 - r,
                                    255 - g,
                                    255 - b,
                                    ((i % width + 3 * (i / width)) % 97) as u8,
                                ],
                            }
                        })
                        .collect();
                    let mut bytes = Vec::new();
                    let mut encoder = Encoder::new(&mut bytes, 95);
                    encoder.set_sampling_factor(factor);
                    encoder.set_progressive(progressive);
                    encoder.encode(
                        &samples,
                        width as u16,
                        height as u16,
                        match model {
                            "gray" => ColorType::Luma,
                            "rgb" => ColorType::Ycbcr,
                            "ycbcr" => ColorType::Rgb,
                            "cmyk" => ColorType::Cmyk,
                            "ycck" => ColorType::CmykAsYcck,
                            _ => unreachable!(),
                        },
                    )?;
                    if model == "rgb" {
                        bytes = rgb_markers(&bytes);
                    }
                    let mode = if progressive {
                        "progressive"
                    } else {
                        "baseline"
                    };
                    let cell = format!("{model}-{h}x{v}-{mode}");
                    cases.push(json!({"id":format!("{cell}-{pattern}"),"cell":cell,"model":model,
                        "sampling":[h,v],"mode":mode,"pattern":pattern,"width":width,"height":height,
                        "jpeg_base64":base64::engine::general_purpose::STANDARD.encode(&bytes),
                        "jpeg_sha256":format!("{:x}",Sha256::digest(&bytes)),
                        "authored_samples_sha256":format!("{:x}",Sha256::digest(&samples))}));
                }
            }
        }
    }
    assert_eq!(cases.len(), 76);
    let record = json!({"schema":"excalidraw-render/jpeg-matrix-inputs/v1",
        "encoder":"jpeg-encoder 0.6.1","quality":95,"license":"project-authored",
        "scope":"untagged 8-bit DCT, no EXIF/ICC/MPF, two patterns per model/sampling/scan cell",
        "cases":cases});
    let mut bytes = serde_json::to_vec_pretty(&record)?;
    bytes.push(b'\n');
    assert_eq!(
        bytes,
        include_bytes!("../../../tests/oracle/jpeg-color/inputs.json")
    );
    Ok(())
}
