// Appended inside each extracted module; no production visibility changes.
pub fn parse(value: &str) -> Option<[u8; 4]> {
    parse_css(value).map(CanonicalColor::rgba8)
}

pub fn forward(kind: &str, rgb: [u8; 3]) -> [f64; 3] {
    let c = CanonicalColor::from_rgb8(rgb[0], rgb[1], rgb[2]);
    match kind {
        "xyz" => srgb_to_xyz_d65(c.rgb_f64()),
        "lab" => srgb_to_lab_d50(c.rgb_f64()),
        "oklab" => color_mix_components(ColorMixSpace::Oklab, c),
        _ => panic!("unknown forward operation"),
    }
}

pub fn primitive(kind: &str, v: [f64; 3]) -> [f64; 3] {
    match kind {
        "decode" => v.map(decode_srgb),
        "encode" => v.map(encode_srgb),
        "d50-d65" => adapt_d50_to_d65(v),
        "d65-d50" => adapt_d65_to_d50(v),
        _ => panic!("unknown primitive"),
    }
}
