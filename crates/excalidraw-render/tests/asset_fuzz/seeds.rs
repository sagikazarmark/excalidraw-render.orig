use super::{data_url, Seed};
use crate::codec_fixtures as f;
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub(super) fn core() -> Vec<Seed> {
    let rgba = [[255, 0, 0, 255], [0, 0, 255, 128]];
    let png = f::png_rgba(2, 1, &rgba);
    let webp = f::webp_rgba(2, 1, &rgba);
    let jpeg = f::jpeg_rgb(2, 1, &[[255, 0, 0], [0, 0, 255]]);
    let mut seeds = vec![
        Seed::new("png-rgba", "image/png", png.clone(), true),
        Seed::new("apng", "image/png", f::apng(&rgba, &rgba), true),
        Seed::new("jpeg-rgb", "image/jpeg", jpeg, true),
        Seed::new("jpeg-progressive-gray", "image/jpeg", f::progressive_gray_jpeg(96), true),
        Seed::new("jpeg-cmyk", "image/jpeg", f::cmyk_jpeg([0, 255, 0, 0]), true),
        Seed::new("gif", "image/gif", f::gif(&[f::gif_frame(&[0, 1], 2, 1, &f::RED_BLUE_PALETTE, Some(1))]), true),
        Seed::new("webp-lossless", "image/webp", webp, true),
        Seed::new("webp-lossy", "image/webp", f::lossy_red_webp(), true),
        Seed::new("bmp-rgb", "image/bmp", f::bmp24(2, 1, &[[255, 0, 0], [0, 0, 255]]), true),
        Seed::new("bmp-alpha", "image/bmp", f::bmp32(rgba[1], true), true),
        Seed::new("bmp-rle", "image/bmp", f::bmp_rle8(), true),
        Seed::new("ico-png", "image/x-icon", f::ico(&[(2, 1, 32, png)]), true),
        Seed::new("ico-dib", "image/x-icon", f::ico(&[(1, 1, 32, f::dib32(rgba[1], false))]), true),
        Seed::new("svg-vector", "image/svg+xml", br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>"#.to_vec(), true),
    ];
    seeds.extend(dib_profiles());
    seeds
}

/// One-pixel DIBs authored from the container layout, independently of the
/// production BMP/ICO decoder. Include packed palettes and both bitfield forms.
fn dib_profiles() -> Vec<Seed> {
    let mut seeds = Vec::new();
    for bits in [1_u16, 4, 8, 16, 24, 32] {
        for bitfields in [false, true] {
            if bitfields && !matches!(bits, 16 | 32) {
                continue;
            }
            let mut dib = Vec::new();
            dib.extend_from_slice(&40_u32.to_le_bytes());
            dib.extend_from_slice(&1_i32.to_le_bytes());
            dib.extend_from_slice(&1_i32.to_le_bytes());
            dib.extend_from_slice(&1_u16.to_le_bytes());
            dib.extend_from_slice(&bits.to_le_bytes());
            dib.extend_from_slice(&(if bitfields { 3_u32 } else { 0 }).to_le_bytes());
            dib.extend_from_slice(&4_u32.to_le_bytes());
            dib.extend_from_slice(&[0; 16]);
            if bitfields {
                for mask in if bits == 16 {
                    [0xf800_u32, 0x07e0, 0x001f]
                } else {
                    [0xff0000, 0xff00, 0xff]
                } {
                    dib.extend_from_slice(&mask.to_le_bytes());
                }
            }
            if bits <= 8 {
                for _ in 0..1 << bits {
                    dib.extend_from_slice(&[0, 0, 255, 0]);
                }
            }
            let offset = 14 + dib.len();
            dib.extend_from_slice(&match bits {
                16 => {
                    if bitfields {
                        [0, 0xf8, 0, 0]
                    } else {
                        [0, 0x7c, 0, 0]
                    }
                }
                24 | 32 => [0, 0, 255, 255],
                _ => [0; 4],
            });
            let mut bmp = b"BM".to_vec();
            bmp.extend_from_slice(&(14 + dib.len() as u32).to_le_bytes());
            bmp.extend_from_slice(&[0; 4]);
            bmp.extend_from_slice(&(offset as u32).to_le_bytes());
            bmp.extend_from_slice(&dib);
            seeds.push(Seed::new(
                format!("bmp-{bits}-bitfields-{bitfields}"),
                "image/bmp",
                bmp,
                true,
            ));
            dib[8..12].copy_from_slice(&2_i32.to_le_bytes());
            dib.extend_from_slice(&[0; 4]); // AND mask
            seeds.push(Seed::new(
                format!("ico-{bits}-bitfields-{bitfields}"),
                "image/x-icon",
                f::ico(&[(1, 1, bits, dib)]),
                bits != 16 || bitfields,
            ));
        }
    }
    let mut rle4 = f::bmp_rle8();
    rle4[28..30].copy_from_slice(&4_u16.to_le_bytes());
    rle4[30..34].copy_from_slice(&2_u32.to_le_bytes());
    rle4[67] = 0x10; // encoded run's first high-nibble palette index
    seeds.push(Seed::new("bmp-rle4", "image/bmp", rle4, true));
    seeds
}

pub(super) fn expanded() -> Vec<Seed> {
    let mut seeds = core();
    let pixels = [
        [24, 96, 208, 0],
        [200, 100, 50, 128],
        [255, 0, 0, 255],
        [0, 255, 0, 255],
    ];
    let png = f::png_rgba(2, 2, &pixels);
    let jpeg = f::jpeg_rgb(2, 2, &pixels.map(|p| [p[0], p[1], p[2]]));
    let icc = f::linear_rgb_icc_profile();
    for orientation in 1..=8 {
        seeds.push(Seed::new(
            format!("jpeg-exif-{orientation}"),
            "image/jpeg",
            f::jpeg_with_app1_exif(&jpeg, orientation),
            true,
        ));
        seeds.push(Seed::new(
            format!("webp-exif-{orientation}"),
            "image/webp",
            f::webp_with_metadata(
                2,
                2,
                &pixels,
                None,
                Some(f::exif_orientation_tiff(orientation)),
            ),
            true,
        ));
    }
    seeds.push(Seed::new(
        "jpeg-icc",
        "image/jpeg",
        f::jpeg_with_icc_profile(&jpeg, &icc),
        true,
    ));
    seeds.push(Seed::new(
        "webp-icc",
        "image/webp",
        f::webp_with_metadata(2, 2, &pixels, Some(icc.clone()), None),
        true,
    ));
    seeds.push(Seed::new(
        "png-icc",
        "image/png",
        crate::png_icc_safety::with_profile(&png, &icc),
        true,
    ));
    seeds.push(Seed::new(
        "apng-separate-default",
        "image/png",
        f::apng_with_separate_default(false),
        true,
    ));
    for dispose in [
        gif::DisposalMethod::Any,
        gif::DisposalMethod::Keep,
        gif::DisposalMethod::Background,
        gif::DisposalMethod::Previous,
    ] {
        let mut first = f::gif_frame(&[0, 1], 2, 1, &f::RED_BLUE_PALETTE, Some(1));
        first.dispose = dispose;
        first.interlaced = true;
        seeds.push(Seed::new(
            format!("gif-{dispose:?}"),
            "image/gif",
            f::gif(&[
                first,
                f::gif_frame(&[1, 0], 2, 1, &f::RED_BLUE_PALETTE, None),
            ]),
            true,
        ));
    }
    seeds.push(Seed::new(
        "webp-animation",
        "image/webp",
        f::animated_webp(
            2,
            2,
            &[
                (0, 0, f::webp_rgba(2, 2, &pixels)),
                (0, 0, f::lossy_red_webp()),
            ],
        ),
        true,
    ));
    seeds.push(Seed::new(
        "ico-selection",
        "image/x-icon",
        f::ico(&[(1, 1, 32, f::dib32(pixels[1], true)), (2, 2, 32, png)]),
        true,
    ));
    seeds.push(Seed::new(
        "bmp-reserved-alpha",
        "image/bmp",
        f::bmp32(pixels[0], false),
        true,
    ));
    // Byte-authored vectors cover every legal PNG channel/depth and Adam7,
    // transparency keys, palette, frame controls, HDR and malformed streams.
    for rows in [
        crate::png_stage4::orientations(),
        crate::png_stage4::palette_cases(),
        crate::png_stage4::scanline_cases(),
        crate::png_stage4::chunk_cases(),
        crate::png_stage4::apng_controls(),
        crate::png_stage4::apng_malformed(),
        crate::png_stage4::hdr_composition(16),
        crate::png_stage4::hdr_composition(18),
        crate::png_icc_safety::headers(),
    ] {
        for row in rows {
            seeds.push(Seed::new(
                format!("png-{}", row.id),
                "image/png",
                row.bytes,
                false,
            ));
        }
    }
    // The exhaustive 65,536 header-pair contract already has its own gate;
    // mutation seeds retain legal-range pairs plus byte-extreme controls.
    for row in crate::png_stage4::ihdr_cases() {
        if row.bytes[24] <= 16 && row.bytes[25] <= 6 || row.bytes[24] == 255 && row.bytes[25] == 255
        {
            seeds.push(Seed::new(
                format!("png-{}", row.id),
                "image/png",
                row.bytes,
                false,
            ));
        }
    }
    // Reuse the authored sampling/color matrix, not the renderer's encoder.
    let matrix: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/oracle/jpeg-color/inputs.json"
    )))
    .unwrap();
    for row in matrix["cases"].as_array().unwrap() {
        seeds.push(Seed::new(
            format!("jpeg-matrix-{}", row["id"].as_str().unwrap()),
            "image/jpeg",
            STANDARD
                .decode(row["jpeg_base64"].as_str().unwrap())
                .unwrap(),
            true,
        ));
    }
    // Profile/frame evidence carries the original source scene, allowing the
    // fuzz population to follow the admitted matrices without copying blobs.
    for (name, mime, bytes) in [
        (
            "png-color",
            "image/png",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/png-color/reference.json"
            ))
            .as_slice(),
        ),
        (
            "png-icc",
            "image/png",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/png-color/stage4-reference.json"
            ))
            .as_slice(),
        ),
        (
            "png-gain",
            "image/png",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/png-color/gain-map-reference.json"
            ))
            .as_slice(),
        ),
        (
            "jpeg-icc",
            "image/jpeg",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/jpeg-icc/reference.json"
            ))
            .as_slice(),
        ),
        (
            "jpeg-gain",
            "image/jpeg",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/jpeg-gain-map/reference.json"
            ))
            .as_slice(),
        ),
        (
            "jpeg-exif",
            "image/jpeg",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/jpeg-exif/reference.json"
            ))
            .as_slice(),
        ),
        (
            "gif-frame",
            "image/gif",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/gif-first-frame/reference.json"
            ))
            .as_slice(),
        ),
        (
            "gif-control",
            "image/gif",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/gif-control/reference.json"
            ))
            .as_slice(),
        ),
        (
            "gif-lzw",
            "image/gif",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/gif-lzw/reference.json"
            ))
            .as_slice(),
        ),
        (
            "webp-sample",
            "image/webp",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/oracle/webp-sample/reference.json"
            ))
            .as_slice(),
        ),
    ] {
        let record: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        for row in record["cases"].as_array().unwrap() {
            let files = row["scene"]["files"].as_object().unwrap();
            assert_eq!(files.len(), 1);
            let url = files.values().next().unwrap()["dataURL"].as_str().unwrap();
            let (_, encoded) = url.split_once(";base64,").unwrap();
            seeds.push(Seed::new(
                format!("{name}-{}", row["id"].as_str().unwrap()),
                mime,
                STANDARD.decode(encoded).unwrap(),
                row["expected_outcome"] == "accepted",
            ));
        }
    }
    seeds.extend(svg());
    seeds
}

pub(super) fn svg() -> Vec<Seed> {
    let mut seeds = Vec::new();
    let bodies = [
        (
            "xml-active",
            r#"<script>bad()</script><foreignObject/><rect onload="bad()"/><animate attributeName="x"/><set attributeName="fill"/>"#,
        ),
        (
            "css-urls",
            r#"<style>@import url(https://fuzz.invalid/a); rect{fill:url('https://fuzz.invalid/b');stroke:red}</style><rect width="8" height="8" style="filter:url(https://fuzz.invalid/c)"/>"#,
        ),
        (
            "css-escapes",
            r#"<style>rect{fill:u\72l(https://fuzz.invalid/a);stroke:var(--x);--x:url(file:///fuzz.invalid)}</style><rect width="8" height="8"/>"#,
        ),
        (
            "references",
            r##"<defs><linearGradient id="a" href="#b"/><linearGradient id="b" href="#a"/><pattern id="p" width="1" height="1"><use href="#p"/></pattern></defs><rect width="8" height="8" fill="url(#a)"/><use href="#missing"/>"##,
        ),
        (
            "use-cycle",
            r##"<g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g><use href="#a"/>"##,
        ),
        (
            "unsafe-links",
            r#"<image href="file:///fuzz.invalid"/><image href="https://fuzz.invalid/a"/><a href="javascript:bad()"><rect width="8" height="8"/></a>"#,
        ),
        (
            "malformed-data",
            r#"<image width="8" height="8" href="data:image/png;base64,%%%"/><image href="data:image/svg+xml,%3Csvg%3E"/>"#,
        ),
        (
            "namespaces",
            r#"<x:script xmlns:x="http://www.w3.org/2000/svg"/><image xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="https://fuzz.invalid/a"/>"#,
        ),
        (
            "filter",
            r##"<defs><filter id="f"><feGaussianBlur stdDeviation="1"/><feColorMatrix type="saturate" values="0.5"/></filter></defs><rect width="8" height="8" fill="red" filter="url(#f)"/>"##,
        ),
        (
            "mask-pattern",
            r##"<defs><mask id="m"><rect width="8" height="8" fill="white"/></mask><pattern id="p" patternUnits="userSpaceOnUse" width="2" height="2"><circle r="1" fill="red"/></pattern></defs><rect width="8" height="8" fill="url(#p)" mask="url(#m)"/>"##,
        ),
        (
            "numeric",
            r#"<path d="M1e300 0 Q-1e300 1e-300 8 8" transform="matrix(1e300 0 0 1e300 0 0)"/><rect width="NaN" height="Infinity"/>"#,
        ),
    ];
    for (id, body) in bodies {
        seeds.push(Seed::new(
            format!("svg-{id}"),
            "image/svg+xml",
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{body}</svg>"#)
                .into_bytes(),
            false,
        ));
    }
    for (id, bytes) in [
        ("entity", br#"<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///fuzz.invalid">]><svg xmlns="http://www.w3.org/2000/svg">&x;</svg>"#.to_vec()),
        ("malformed", b"<svg><g></svg>\xff".to_vec()),
        ("xml-depth", format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{}{}</svg>", "<g>".repeat(512), "</g>".repeat(512)).into_bytes()),
        ("node-expansion", format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{}</svg>", "<rect width=\"1\" height=\"1\"/>".repeat(300)).into_bytes()),
    ] {
        seeds.push(Seed::new(format!("svg-{id}"), "image/svg+xml", bytes, false));
    }
    let child = data_url("image/svg+xml", &seeds[0].bytes);
    seeds.push(Seed::new("svg-recursive-sanitizer", "image/svg+xml", format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image width="8" height="8" href="{child}"/></svg>"#).into_bytes(), false));
    seeds
}
