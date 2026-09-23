//! Frozen v1 external-libwebp/browser observations. No codec is invoked to
//! manufacture expectations. Small artifacts retain the canonical comparator's
//! exact projected-view rule. Composition corrects 11 of #191's original 23
//! gaps; the 12 partial-alpha cells retain exact native defect pins, and their
//! only failing views, the PDF black/white composites, pass under approved
//! exact-ceiling tolerance exceptions (spec section 14.4). Synthetic PDF
//! projection failures are not measurements of any PDF viewer.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const REFERENCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/webp-sample/reference.json"
));
const INPUTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/webp-sample/inputs.json"
));
const REFERENCE_HASH: &str = "b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6";
const INPUT_HASH: &str = "21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4";
const OCI: &str = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9";
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
// Fixed by the initial all-72 probe, not selected by runtime pass/fail results.
// Both patterns belong to each cell. Keep this original selector stable.
const QUALIFIED_CELLS: [&str; 13] = [
    "lossless-opaque-static-full",
    "lossless-opaque-anim-full-replace-d1-opaque",
    "lossless-binary-static-full",
    "lossless-binary-anim-full-replace-d1-opaque",
    "lossless-binary-anim-sub-replace-d0-clear",
    "lossless-binary-anim-sub-replace-d1-opaque",
    "lossy-opaque-static-full",
    "lossy-opaque-anim-full-blend-d0-clear",
    "lossy-opaque-anim-full-replace-d1-opaque",
    "lossy-binary-static-full",
    "lossy-binary-anim-full-replace-d1-opaque",
    "lossy-binary-anim-sub-replace-d0-clear",
    "lossy-binary-anim-sub-replace-d1-opaque",
];
const CORRECTED_CELLS: [&str; 11] = [
    "lossless-opaque-anim-full-blend-d0-clear",
    "lossless-opaque-anim-sub-blend-d1-opaque",
    "lossless-opaque-anim-sub-replace-d0-clear",
    "lossless-opaque-anim-sub-replace-d1-opaque",
    "lossless-binary-anim-full-blend-d0-clear",
    "lossless-binary-anim-sub-blend-d1-opaque",
    "lossy-opaque-anim-sub-blend-d1-opaque",
    "lossy-opaque-anim-sub-replace-d0-clear",
    "lossy-opaque-anim-sub-replace-d1-opaque",
    "lossy-binary-anim-full-blend-d0-clear",
    "lossy-binary-anim-sub-blend-d1-opaque",
];
const RESIDUAL_CELLS: [&str; 12] = [
    "lossless-partial-static-full",
    "lossless-partial-anim-full-blend-d0-clear",
    "lossless-partial-anim-full-replace-d1-opaque",
    "lossless-partial-anim-sub-blend-d1-opaque",
    "lossless-partial-anim-sub-replace-d0-clear",
    "lossless-partial-anim-sub-replace-d1-opaque",
    "lossy-partial-static-full",
    "lossy-partial-anim-full-blend-d0-clear",
    "lossy-partial-anim-full-replace-d1-opaque",
    "lossy-partial-anim-sub-blend-d1-opaque",
    "lossy-partial-anim-sub-replace-d0-clear",
    "lossy-partial-anim-sub-replace-d1-opaque",
];
type Configuration = (&'static str, bool, bool, bool, u8, [u8; 4]);
const CONFIGS: [Configuration; 6] = [
    ("static-full", false, false, false, 0, [0; 4]),
    ("anim-full-blend-d0-clear", true, false, true, 0, [0; 4]),
    (
        "anim-full-replace-d1-opaque",
        true,
        false,
        false,
        1,
        [41, 83, 137, 255],
    ),
    (
        "anim-sub-blend-d1-opaque",
        true,
        true,
        true,
        1,
        [41, 83, 137, 255],
    ),
    ("anim-sub-replace-d0-clear", true, true, false, 0, [0; 4]),
    (
        "anim-sub-replace-d1-opaque",
        true,
        true,
        false,
        1,
        [41, 83, 137, 255],
    ),
];
const CONTROLS: [&str; 12] = [
    "riff-size-short",
    "riff-size-long",
    "chunk-length-overflow",
    "vp8x-reserved-flag",
    "vp8x-reserved-byte",
    "anmf-reserved-flag",
    "frame-outside-canvas",
    "intrinsic-frame-mismatch",
    "missing-anim",
    "first-invalid-payload",
    "later-invalid-payload",
    "truncated-later-chunk",
];
const SOURCES: [&str; 20] = [
    "tests/oracle/reference-profiles/revision-1-assets.json",
    "tests/oracle/reference-profiles/revision-1-assets.sha256",
    "tests/oracle/reference-profiles/revision-1.json",
    "tests/oracle/reference-profiles/revision-1.sha256",
    "tests/oracle/webp-sample/inputs.json",
    "tools/reference-oracle/fixtures/profile-smoke.excalidraw",
    "tools/reference-oracle/index.html",
    "tools/reference-oracle/package-lock.json",
    "tools/reference-oracle/package.json",
    "tools/reference-oracle/run-container.sh",
    "tools/reference-oracle/run-webp-sample.sh",
    "tools/reference-oracle/scripts/canonical-json.mjs",
    "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs",
    "tools/reference-oracle/scripts/profile.mjs",
    "tools/reference-oracle/scripts/smoke.mjs",
    "tools/reference-oracle/scripts/webp-encode-inputs.py",
    "tools/reference-oracle/scripts/webp-sample-evidence.mjs",
    "tools/reference-oracle/scripts/webp-sample-fixtures.mjs",
    "tools/reference-oracle/src/browser-entry.js",
    "tools/reference-oracle/vite.config.mjs",
];

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn decode(v: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(v.as_str().unwrap())
        .unwrap()
}
fn encode(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}
fn n(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn rows<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().unwrap()
}
fn by_id<'a>(v: &'a Value, key: &str, id: &str) -> &'a Value {
    rows(v, key).iter().find(|r| r["id"] == id).unwrap()
}
fn packed(v: &Value) -> Vec<u8> {
    let b = decode(&v["base64"]);
    assert_eq!(v["bytes"], b.len());
    assert_eq!(v["sha256"], hash(&b));
    b
}
fn binding(v: &Value, b: &[u8]) {
    assert_eq!(packed(v), b);
}
fn u24(b: &[u8]) -> usize {
    usize::from(b[0]) | (usize::from(b[1]) << 8) | (usize::from(b[2]) << 16)
}
fn put24(b: &mut Vec<u8>, n: usize) {
    assert!(n <= 0xffffff);
    b.extend_from_slice(&(n as u32).to_le_bytes()[..3]);
}
fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = kind.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}
fn riff(body: &[u8]) -> Vec<u8> {
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(body);
    out
}

#[derive(Debug)]
struct Chunk<'a> {
    kind: &'a [u8],
    offset: usize,
    body: &'a [u8],
    end: usize,
}
fn chunks(b: &[u8], start: usize, end: usize) -> Result<Vec<Chunk<'_>>, &'static str> {
    let mut at = start;
    let mut out = vec![];
    if end > b.len() || start > end {
        return Err("bounds");
    }
    while at < end {
        let header = b
            .get(at..at + 8)
            .filter(|_| at + 8 <= end)
            .ok_or("chunk-header")?;
        let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let body_end = at
            .checked_add(8)
            .and_then(|v| v.checked_add(size))
            .ok_or("overflow")?;
        let next = body_end.checked_add(size & 1).ok_or("overflow")?;
        if next > end {
            return Err("chunk-length");
        }
        if size & 1 != 0 && b[body_end] != 0 {
            return Err("padding");
        }
        out.push(Chunk {
            kind: &header[..4],
            offset: at,
            body: &b[at + 8..body_end],
            end: next,
        });
        at = next;
    }
    Ok(out)
}
fn container(b: &[u8]) -> Result<Vec<Chunk<'_>>, &'static str> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return Err("riff");
    }
    if u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize + 8 != b.len() {
        return Err("riff-size");
    }
    chunks(b, 12, b.len())
}
fn layout(b: &[u8], start: usize, end: usize) -> Value {
    json!(chunks(b, start, end).unwrap().iter().map(|c| {
        let mut v = json!({"kind":std::str::from_utf8(c.kind).unwrap(),"offset":c.offset,"payload_offset":c.offset+8,"size":c.body.len(),"padded_end":c.end});
        if c.kind == b"ANMF" { v["children"] = layout(b, c.offset+24, c.offset+8+c.body.len()); }
        v
    }).collect::<Vec<_>>())
}
fn codec_dimensions(kind: &[u8], b: &[u8]) -> Result<(usize, usize), &'static str> {
    if kind == b"VP8L" {
        if b.len() < 5 || b[0] != 0x2f {
            return Err("vp8l-signature");
        }
        let bits = u32::from_le_bytes(b[1..5].try_into().unwrap());
        if bits >> 29 != 0 {
            return Err("vp8l-version");
        }
        Ok((
            (bits & 0x3fff) as usize + 1,
            ((bits >> 14) & 0x3fff) as usize + 1,
        ))
    } else {
        if kind != b"VP8 " || b.len() < 10 || b[0] & 1 != 0 || &b[3..6] != b"\x9d\x01\x2a" {
            return Err("vp8-signature");
        }
        Ok((
            (u16::from_le_bytes(b[6..8].try_into().unwrap()) & 0x3fff) as usize,
            (u16::from_le_bytes(b[8..10].try_into().unwrap()) & 0x3fff) as usize,
        ))
    }
}
// A strict authored dialect, not a claim that every reserved field must be
// rejected by a general WebP reader. No compressed sample decoding here.
fn strict(b: &[u8]) -> Result<(), &'static str> {
    let cs = container(b)?;
    let animated = cs.iter().any(|c| c.kind == b"ANMF");
    if animated && !cs.iter().any(|c| c.kind == b"ANIM" && c.body.len() == 6) {
        return Err("missing-anim");
    }
    for c in &cs {
        if c.kind == b"VP8X" {
            if c.body.len() != 10 || c.body[0] & !0x12 != 0 || c.body[1..4] != [0; 3] {
                return Err("vp8x-reserved");
            }
            if (u24(&c.body[4..7]) + 1, u24(&c.body[7..10]) + 1) != (9, 7) {
                return Err("canvas");
            }
        }
        if c.kind == b"ANMF" {
            if c.body.len() < 16 || c.body[15] & !3 != 0 {
                return Err("anmf-flags");
            }
            let (x, y, w, h) = (
                2 * u24(c.body),
                2 * u24(&c.body[3..]),
                u24(&c.body[6..]) + 1,
                u24(&c.body[9..]) + 1,
            );
            if x + w > 9 || y + h > 7 {
                return Err("frame-bounds");
            }
            let nested = chunks(c.body, 16, c.body.len())?;
            let kinds: Vec<_> = nested.iter().map(|v| v.kind).collect();
            if kinds != [b"VP8L".as_slice()]
                && kinds != [b"VP8 ".as_slice()]
                && kinds != [b"ALPH".as_slice(), b"VP8 ".as_slice()]
            {
                return Err("codec-order");
            }
            let codec = nested.last().ok_or("empty-frame")?;
            if codec_dimensions(codec.kind, codec.body)? != (w, h) {
                return Err("intrinsic-mismatch");
            }
        } else if c.kind == b"VP8L" || c.kind == b"VP8 " {
            codec_dimensions(c.kind, c.body)?;
        }
    }
    Ok(())
}
fn payload(frame: &Value) -> Vec<u8> {
    let b = packed(&frame["encoded"]);
    let cs = container(&b).unwrap();
    let cs: Vec<_> = cs
        .iter()
        .filter(|c| [b"ALPH", b"VP8 ", b"VP8L"].contains(&c.kind.try_into().unwrap()))
        .collect();
    let expected: Vec<&[u8]> = if frame["codec"] == "lossless" {
        vec![b"VP8L"]
    } else if frame["alpha"] == "opaque" {
        vec![b"VP8 "]
    } else {
        vec![b"ALPH", b"VP8 "]
    };
    assert_eq!(cs.iter().map(|c| c.kind).collect::<Vec<_>>(), expected);
    let codec = cs.last().unwrap();
    assert_eq!(
        codec_dimensions(codec.kind, codec.body).unwrap(),
        (n(&frame["width"]), n(&frame["height"]))
    );
    cs.iter()
        .flat_map(|c| b[c.offset..c.end].iter().copied())
        .collect()
}
fn authored(w: usize, h: usize, alpha: &str, pattern: &str, later: bool) -> Vec<u8> {
    let ramp = [0, 1, 17, 63, 127, 128, 191, 254, 255];
    let mut out = vec![];
    for y in 0..h {
        for x in 0..w {
            let rgb = if pattern == "ramp" {
                [
                    ramp[(x + 2 * y) % 9],
                    ramp[(3 * x + y + 2) % 9],
                    ramp[(x + 5 * y + 4) % 9],
                ]
            } else {
                [
                    (if x < 3 { 239 } else { 19 }) + (y % 3) as u8,
                    (if y < 2 { 31 } else { 211 }) + (x % 5) as u8,
                    if x == y { 229 } else { 67 },
                ]
            };
            let a = match alpha {
                "opaque" => 255,
                "binary" => {
                    if (x + 3 * y) % 3 == 0 {
                        0
                    } else {
                        255
                    }
                }
                "partial" => ramp[(x + 3 * y) % 9],
                _ => unreachable!(),
            };
            out.extend_from_slice(&if a == 0 {
                [173, 59, 227]
            } else {
                rgb.map(|v| if later { 255 - v } else { v })
            });
            out.push(a);
        }
    }
    out
}
fn compose(rgba: &[u8], sub: bool, bg: [u8; 4], blend: bool) -> Vec<u8> {
    let (w, h, left, top) = if sub { (5, 3, 2, 2) } else { (9, 7, 0, 0) };
    assert_eq!(rgba.len(), w * h * 4);
    let mut out = bg.repeat(63);
    for y in 0..h {
        for x in 0..w {
            let src = &rgba[(y * w + x) * 4..][..4];
            let dst = &mut out[((y + top) * 9 + x + left) * 4..][..4];
            let a = f64::from(src[3]);
            let ba = if blend { f64::from(bg[3]) } else { 0.0 };
            let oa = a + ba * (255.0 - a) / 255.0;
            for c in 0..3 {
                dst[c] = if oa == 0.0 {
                    0
                } else {
                    ((f64::from(src[c]) * a + f64::from(bg[c]) * ba * (255.0 - a) / 255.0) / oa)
                        .round() as u8
                };
            }
            dst[3] = oa.round() as u8;
        }
    }
    out
}
fn scene(b: &[u8]) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":9,"height":7,"angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"version":1,"versionNonce":1,"updated":1,"isDeleted":false,"groupIds":[],"frameId":null,"boundElements":null,"link":null,"locked":false,"strokeColor":"#000000","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":1,"strokeStyle":"solid","roughness":0,"index":"a0","crop":null,"roundness":null}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/webp","created":1,"dataURL":format!("data:image/webp;base64,{}",encode(b))}}})
}
fn build(first: &[u8], later: &[u8], alpha: &str, codec: &str, config: usize) -> Vec<u8> {
    let (_, animated, sub, blend, disposal, bg) = CONFIGS[config];
    let mut header = vec![
        (if alpha != "opaque" { 16 } else { 0 }) | if animated { 2 } else { 0 },
        0,
        0,
        0,
    ];
    put24(&mut header, 8);
    put24(&mut header, 6);
    let mut body = vec![];
    if animated || (codec == "lossy" && alpha != "opaque") {
        body.extend(chunk(b"VP8X", &header));
    }
    if animated {
        body.extend(chunk(b"ANIM", &[bg[2], bg[1], bg[0], bg[3], 0, 0]));
        for (sub, payload, duration, flags) in [
            (sub, first, 0xffffff, (if blend { 0 } else { 2 }) | disposal),
            (false, later, 100, 2),
        ] {
            let mut f = vec![];
            for v in if sub {
                [1, 1, 4, 2, duration]
            } else {
                [0, 0, 8, 6, duration]
            } {
                put24(&mut f, v);
            }
            f.push(flags);
            f.extend_from_slice(payload);
            body.extend(chunk(b"ANMF", &f));
        }
    } else {
        body.extend_from_slice(first);
    }
    riff(&body)
}
fn sample(v: &Value) -> Vec<u8> {
    let rgba = decode(&v["rgba_base64"]);
    assert_eq!(rgba.len(), n(&v["width"]) * n(&v["height"]) * 4);
    assert!(rgba.chunks_exact(4).all(|p| p[3] != 0 || p[..3] == [0; 3]));
    let decoded = crate::common::raster::decode_png(&decode(&v["png"]));
    assert_eq!(
        (decoded.width as usize, decoded.height as usize),
        (n(&v["width"]), n(&v["height"]))
    );
    assert_eq!(decoded.rgba, rgba);
    if let Some(composites) = v.get("composites") {
        for bg in ["black", "white"] {
            let opaque = sample(&composites[bg]);
            assert!(opaque.chunks_exact(4).all(|p| p[3] == 255));
        }
    }
    rgba
}
fn exact_alpha(actual: &[u8], expected: &[u8]) -> bool {
    actual.len() == expected.len()
        && actual
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .all(|(a, b)| a[3] == b[3] && (a[3] != 0 || a[..3] == [0; 3]))
}
fn canonical(b: &[u8]) -> CanonicalRgba8 {
    CanonicalRgba8::new(9, 7, b.to_vec()).unwrap()
}
fn metric(expected: &[u8], actual: &[u8]) -> Option<String> {
    let r = compare(
        &canonical(expected),
        &canonical(actual),
        VisualClass::GeometryImage,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    (!r.passed()).then(|| {
        format!(
            "changed={}/{} SSIM={:?} {} expected={} actual={}",
            r.whole.changed_pixels,
            r.whole.area,
            r.whole.ssim,
            differences(expected, actual),
            hash(expected),
            hash(actual)
        )
    })
}

fn differences(expected: &[u8], actual: &[u8]) -> String {
    assert_eq!(expected.len(), actual.len());
    let mut alpha = 0;
    let mut rgb = 0;
    let mut maximum = 0;
    for (a, b) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        alpha += usize::from(a[3] != b[3]);
        rgb += usize::from(a[..3] != b[..3] && (a[3] != 0 || b[3] != 0));
        for c in 0..3 {
            maximum = maximum.max(a[c].abs_diff(b[c]));
        }
    }
    format!("alpha_changed={alpha} visible_rgb_changed={rgb} max_rgb_delta={maximum}")
}

fn mutate(base: &[u8], id: &str) -> Vec<u8> {
    let cs = container(base).unwrap();
    let frames: Vec<_> = cs.iter().filter(|c| c.kind == b"ANMF").collect();
    let first = frames[0].offset;
    let later = frames[1].offset;
    let mut b = base.to_vec();
    match id {
        "riff-size-short" => b[4..8].copy_from_slice(&((base.len() - 10) as u32).to_le_bytes()),
        "riff-size-long" => b[4..8].copy_from_slice(&((base.len() - 6) as u32).to_le_bytes()),
        "chunk-length-overflow" => b[first + 4..first + 8].fill(255),
        "vp8x-reserved-flag" => b[20] |= 128,
        "vp8x-reserved-byte" => b[21] = 1,
        "anmf-reserved-flag" => b[first + 23] |= 4,
        "frame-outside-canvas" => b[first + 8..first + 11].copy_from_slice(&[4, 0, 0]),
        "intrinsic-frame-mismatch" => b[first + 14..first + 17].copy_from_slice(&[3, 0, 0]),
        "missing-anim" => {
            let c = cs.iter().find(|c| c.kind == b"ANIM").unwrap();
            b.drain(c.offset..c.end);
            let len = b.len();
            b[4..8].copy_from_slice(&((len - 8) as u32).to_le_bytes());
        }
        "first-invalid-payload" => b[first + 32] = 0,
        "later-invalid-payload" => b[later + 32] = 0,
        "truncated-later-chunk" => b.truncate(base.len() - 7),
        _ => unreachable!(),
    }
    b
}

fn evidence() -> (Value, Value) {
    assert_eq!(hash(REFERENCE), REFERENCE_HASH);
    assert_eq!(hash(INPUTS), INPUT_HASH);
    let record: Value = serde_json::from_slice(REFERENCE).unwrap();
    let inputs: Value = serde_json::from_slice(INPUTS).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/webp-sample-reference/v1"
    );
    assert_eq!(inputs["schema"], "excalidraw-render/webp-sample-inputs/v1");
    assert_eq!(record["inputs_sha256"], INPUT_HASH);
    assert_eq!(inputs["oci"], OCI);
    assert_eq!(record["collection"]["oci"], OCI);
    assert_eq!(record["encoder"], inputs["encoder"]);
    let encoder = &inputs["encoder"];
    assert_eq!(encoder["implementation"], "libwebp");
    assert_eq!(encoder["version"], "1.3.2");
    assert_eq!(encoder["distribution_package"], "1.3.2-0.4build3");
    assert_eq!(encoder["python"], "3.12.3");
    assert_eq!(
        encoder["binaries"],
        json!({"/lib/x86_64-linux-gnu/libwebp.so.7":"9664db51ec65209078abbb8bc7b699888a43ecc2f3c74ce8dd4e497175aa5bf0","/lib/x86_64-linux-gnu/libsharpyuv.so.0":"b2e62799e5dbd783a0671d53be99f7c6489567a5332a1e8f471e6bb250f294dc","/usr/bin/python3":"e1efa562c2cc2e35521a5c9c9b9939921001ff8ca9708a13ef15ace68cc2ccd7"})
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(
        rows(&record, "sources")
            .iter()
            .map(|s| s["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        SOURCES
    );
    for s in rows(&record, "sources") {
        assert_eq!(
            s["sha256"],
            hash(&std::fs::read(root.join(s["path"].as_str().unwrap())).unwrap()),
            "{}",
            s["path"]
        );
    }
    assert_eq!(rows(&inputs, "author_sources").len(), 5);
    assert_eq!(
        rows(&inputs, "author_sources")
            .iter()
            .map(|s| s["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            SOURCES[10],
            SOURCES[12],
            SOURCES[15],
            SOURCES[16],
            SOURCES[17]
        ]
    );
    for s in rows(&inputs, "author_sources") {
        assert!(rows(&record, "sources").contains(s));
    }
    assert_eq!(record["collection"]["renderer"], "official v0.18.1");
    assert_eq!(record["collection"]["chromium"], "149.0.7827.55");
    assert_eq!(
        record["collection"]["passes"],
        json!(["fresh-process-forward", "fresh-process-reverse"])
    );
    assert_eq!(
        record["collection"]["capture"]["first_duration_ms"],
        0xffffff
    );
    assert_eq!(record["collection"]["capture"]["maximum_capture_ms"], 30000);
    assert_eq!(rows(&inputs, "frames").len(), 28);
    let mut frame_ids = BTreeSet::new();
    for f in rows(&inputs, "frames") {
        let id = f["id"].as_str().unwrap();
        assert!(frame_ids.insert(id));
        let original = authored(
            n(&f["width"]),
            n(&f["height"]),
            f["alpha"].as_str().unwrap(),
            f["pattern"].as_str().unwrap(),
            id.contains("-later-"),
        );
        assert_eq!(decode(&f["rgba_base64"]), original);
        let straight = packed(&f["native_decoded_rgba"]);
        assert!(exact_alpha(
            &compose(&straight, f["shape"] == "subrect", [0; 4], false),
            &compose(&original, f["shape"] == "subrect", [0; 4], false)
        ));
        if f["codec"] == "lossless" {
            for (a, b) in straight.chunks_exact(4).zip(original.chunks_exact(4)) {
                if b[3] != 0 {
                    assert_eq!(a, b);
                }
            }
        }
        payload(f);
    }
    assert_eq!(rows(&record, "cases").len(), 72);
    assert_eq!(record["matrix"]["cells"], 36);
    assert_eq!(record["matrix"]["vectors"], 72);
    assert_eq!(record["matrix"]["patterns_per_cell"], 2);
    assert_eq!(record["matrix"]["static_vectors"], 12);
    assert_eq!(record["matrix"]["animated_vectors"], 60);
    assert_eq!(record["matrix"]["controls"], 12);
    assert_eq!(record["matrix"]["total_vectors"], 84);
    assert_eq!(record["matrix"]["configurations"],json!(CONFIGS.iter().map(|(id,animated,sub,blend,disposal,bg)|json!({"id":id,"animated":animated,"shape":if *sub {"subrect"} else {"full"},"blend":blend,"disposal":disposal,"background":bg})).collect::<Vec<_>>()));
    let mut ids = BTreeSet::new();
    let mut cells = BTreeMap::new();
    let mut hashes = BTreeSet::new();
    for codec in ["lossless", "lossy"] {
        for alpha in ["opaque", "partial", "binary"] {
            for (ci, (name, animated, sub, blend, disposal, bg)) in
                CONFIGS.iter().copied().enumerate()
            {
                for pattern in ["ramp", "edges"] {
                    let cell = format!("{codec}-{alpha}-{name}");
                    let id = format!("{cell}-{pattern}");
                    let row = by_id(&record, "cases", &id);
                    assert!(ids.insert(id));
                    *cells.entry(cell.clone()).or_insert(0) += 1;
                    assert_eq!(row["cell"], cell);
                    assert_eq!(row["codec"], codec);
                    assert_eq!(row["alpha"], alpha);
                    assert_eq!(row["pattern"], pattern);
                    let shape = if sub { "subrect" } else { "full" };
                    assert_eq!(
                        row["config"],
                        json!({"id":name,"animated":animated,"shape":shape,"blend":blend,"disposal":disposal,"background":bg})
                    );
                    let first_id = format!("{codec}-{alpha}-{shape}-{pattern}");
                    let later_id = format!("{codec}-later-{pattern}");
                    let first = by_id(&inputs, "frames", &first_id);
                    let later = by_id(&inputs, "frames", &later_id);
                    assert_eq!(row["first_frame_id"], first_id);
                    assert_eq!(
                        row["later_frame_id"],
                        if animated {
                            json!(later_id)
                        } else {
                            Value::Null
                        }
                    );
                    assert_eq!(
                        row["first_rect"],
                        if sub {
                            json!({"left":2,"top":2,"width":5,"height":3})
                        } else {
                            json!({"left":0,"top":0,"width":9,"height":7})
                        }
                    );
                    assert_eq!(row["frame_count"], if animated { 2 } else { 1 });
                    assert_eq!(
                        row["first_duration_ms"],
                        if animated {
                            json!(0xffffff)
                        } else {
                            Value::Null
                        }
                    );
                    let (fp, lp) = (payload(first), payload(later));
                    binding(&row["frame_payload"], &fp);
                    if animated {
                        binding(&row["later_payload"], &lp);
                    } else {
                        assert!(row["later_payload"].is_null());
                    }
                    let wire = build(&fp, &lp, alpha, codec, ci);
                    binding(&row["webp"], &wire);
                    assert!(hashes.insert(hash(&wire)));
                    assert_eq!(strict(&wire), Ok(()));
                    assert_eq!(row["layout"], layout(&wire, 12, wire.len()));
                    assert_eq!(row["scene"], scene(&wire));
                    assert_eq!(row["eligibility"], "valid-first-frame-reference");
                    let original = authored(
                        if sub { 5 } else { 9 },
                        if sub { 3 } else { 7 },
                        alpha,
                        pattern,
                        false,
                    );
                    binding(&row["original_rgba"], &original);
                    let transparent = compose(&original, sub, [0; 4], false);
                    binding(&row["expected_first_transparent"], &transparent);
                    binding(
                        &row["expected_first_anim_background"],
                        &compose(&original, sub, bg, blend),
                    );
                    let later_rgba = authored(9, 7, "opaque", pattern, true);
                    if animated {
                        binding(&row["expected_later"], &later_rgba);
                        assert!(metric(&transparent, &later_rgba).is_some());
                    } else {
                        assert!(row["expected_later"].is_null());
                    }
                    let reference = &row["reference"];
                    assert_eq!(reference["admitted"], true);
                    assert_eq!(reference["capture_under_30_seconds"], true);
                    let d = &reference["diagnostic"];
                    for key in [
                        "standalone_first",
                        "browser_first_transparent",
                        "browser_first_anim_background",
                        "authored_transparent_canvas",
                        "authored_anim_background_canvas",
                        "native_straight_canvas",
                    ] {
                        sample(&d[key]);
                    }
                    if animated {
                        sample(&d["standalone_later"]);
                    }
                    for kind in ["intrinsic", "png", "svg"] {
                        let s = &reference["samples"][kind];
                        let actual = sample(s);
                        assert_eq!((n(&s["width"]), n(&s["height"])), (9, 7));
                        assert!(exact_alpha(&actual, &transparent));
                        assert_eq!(
                            actual,
                            decode(&d["browser_first_transparent"]["rgba_base64"])
                        );
                        assert_eq!(
                            actual,
                            decode(&reference["samples"]["intrinsic"]["rgba_base64"])
                        );
                        for bg in ["black", "white"] {
                            assert_eq!(
                                s["composites"][bg],
                                reference["samples"]["intrinsic"]["composites"][bg]
                            );
                        }
                        if animated {
                            assert!(metric(
                                &actual,
                                &decode(&d["standalone_later"]["rgba_base64"])
                            )
                            .is_some());
                        }
                        if CONFIGS[ci].5[3] != 0 && (sub || (blend && alpha != "opaque")) {
                            assert!(metric(
                                &actual,
                                &decode(&d["browser_first_anim_background"]["rgba_base64"])
                            )
                            .is_some());
                        }
                    }
                    for key in ["png", "svg_png"] {
                        let actual = crate::common::raster::decode_png(&decode(&reference[key]));
                        assert_eq!((actual.width, actual.height), (9, 7));
                        assert!(exact_alpha(&actual.rgba, &transparent));
                    }
                    svg(reference["svg"].as_str().unwrap(), Some(&wire));
                }
            }
        }
    }
    assert_eq!(ids.len(), 72);
    assert_eq!(cells.len(), 36);
    assert!(cells.values().all(|v| *v == 2));
    assert_eq!(hashes.len(), 72);
    // Browser readback is not lossless straight RGB, even when the source was
    // authored through ImageData. Keep both independently observed facts live.
    let partial = by_id(&record, "cases", "lossless-partial-static-full-ramp");
    let authored = packed(&partial["expected_first_transparent"]);
    let d = &partial["reference"]["diagnostic"];
    assert_ne!(
        decode(&partial["reference"]["samples"]["intrinsic"]["rgba_base64"]),
        authored
    );
    assert_ne!(
        decode(&d["authored_transparent_canvas"]["rgba_base64"]),
        authored
    );
    assert_ne!(
        decode(&d["native_straight_canvas"]["rgba_base64"]),
        authored
    );
    let base = packed(
        &by_id(
            &record,
            "cases",
            "lossless-partial-anim-sub-blend-d1-opaque-ramp",
        )["webp"],
    );
    assert_eq!(rows(&record, "controls").len(), 12);
    for id in CONTROLS {
        let c = by_id(&record, "controls", id);
        let bytes = mutate(&base, id);
        binding(&c["webp"], &bytes);
        assert_eq!(c["collected"], false);
        assert_eq!(c["eligibility"], "uncollected-parent-control");
        assert!(strict(&bytes).is_err(), "{id}");
    }
    negative_helpers(&record);
    native_pin_discrimination(&native_defect_pins(), &record);
    composition_discrimination(&record);
    lossless_pdf_gates_are_independent(&record, &inputs);
    (record, inputs)
}

fn svg(text: &str, source: Option<&[u8]>) {
    let doc = roxmltree::Document::parse(text).unwrap();
    let root = doc.root_element();
    for (key, value) in [("width", "9"), ("height", "7"), ("viewBox", "0 0 9 7")] {
        assert_eq!(root.attribute(key), Some(value));
    }
    let images: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), usize::from(source.is_some()));
    if let Some(b) = source {
        let image = images[0];
        let href = image
            .attribute("href")
            .or_else(|| image.attribute(("http://www.w3.org/1999/xlink", "href")))
            .unwrap();
        assert_eq!(href, format!("data:image/webp;base64,{}", encode(b)));
        let extent = if image
            .parent_element()
            .is_some_and(|p| p.has_tag_name("symbol"))
        {
            assert_eq!(image.attribute("width"), Some("100%"));
            assert_eq!(image.attribute("height"), Some("100%"));
            let uses: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name("use"))
                .collect();
            assert_eq!(uses.len(), 1);
            assert_eq!(
                uses[0].attribute("href"),
                Some(
                    format!(
                        "#{}",
                        image.parent_element().unwrap().attribute("id").unwrap()
                    )
                    .as_str()
                )
            );
            uses[0]
        } else {
            image
        };
        assert_eq!(extent.attribute("width"), Some("9"));
        assert_eq!(extent.attribute("height"), Some("7"));
    }
}
fn pdf_extent(b: &[u8]) {
    let text = crate::common::pdf_inspect::pdf_text(b);
    let pages: Vec<_> = text.split("/Type /Page\n").skip(1).collect();
    assert_eq!(pages.len(), 1);
    let page = pages[0].split("endobj").next().unwrap();
    for key in ["/MediaBox [", "/CropBox ["] {
        let nums: Vec<f64> = page
            .split_once(key)
            .unwrap()
            .1
            .split_once(']')
            .unwrap()
            .0
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(nums, [0.0, 0.0, 6.75, 5.25]);
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}
fn render(source: &Value, format: OutputFormat) -> (Vec<u8>, Value) {
    let request = RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .build()
        .unwrap();
    let input = serde_json::to_vec(source).unwrap();
    let once = || {
        Renderer::new()
            .render(&SceneDocument::parse(&input).unwrap(), &request)
            .unwrap()
    };
    let first = once();
    let again = once();
    assert_eq!(first.artifact().bytes(), again.artifact().bytes());
    assert_eq!(
        serde_json::to_value(first.report()).unwrap(),
        serde_json::to_value(again.report()).unwrap()
    );
    let mut report = serde_json::to_value(first.report()).unwrap();
    for d in report["diagnostics"].as_array_mut().unwrap() {
        assert!(!d["message"].as_str().unwrap().is_empty());
        d.as_object_mut().unwrap().remove("message");
    }
    (first.artifact().bytes().to_vec(), report)
}
fn report(format: OutputFormat, malformed: bool, animated: bool) -> Value {
    let ds = if malformed {
        json!([{"code":"image.decode-failed","severity":"warning","format":format.as_str(),"effects":["visual"],"element_id":"image","json_pointer":"/files/asset/dataURL","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/webp","recovery":if format==OutputFormat::Png {"placeholder"} else {"omitted"},"recipe_id":if format==OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}}])
    } else if animated && format != OutputFormat::Svg {
        json!([{"code":"image.animation-flattened","severity":"info","format":format.as_str(),"effects":["visual"],"element_id":"image","json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/webp","recovery":"flattened","recipe_id":null}}])
    } else {
        json!([])
    };
    json!({"schema_version":1,"diagnostics":ds})
}
fn appearance(row: &Value, inputs: &Value, format: OutputFormat, bytes: &[u8]) -> Vec<String> {
    if format == OutputFormat::Svg {
        svg(
            std::str::from_utf8(bytes).unwrap(),
            Some(&packed(&row["webp"])),
        );
        return vec![];
    }
    let actual = if format == OutputFormat::Png {
        super::png_profile_contract::assert_untagged_png(bytes);
        let image = crate::common::png_color_reference::png(bytes);
        assert_eq!((image.width(), image.height()), (9, 7));
        image.rgba().to_vec()
    } else {
        pdf_extent(bytes);
        super::png_profile_contract::pdf_rgba(bytes, 9, 7)
    };
    let expected_alpha = packed(&row["expected_first_transparent"]);
    let mut failures = vec![];
    if !exact_alpha(&actual, &expected_alpha) {
        failures.push(format!(
            "alpha/invisible RGB mismatch {} actual={}",
            differences(&expected_alpha, &actual),
            hash(&actual)
        ));
    }
    if format == OutputFormat::Png {
        let reference = crate::common::raster::decode_png(&decode(&row["reference"]["png"]));
        assert_eq!((reference.width, reference.height), (9, 7));
        if let Some(m) = metric(&reference.rgba, &actual) {
            failures.push(format!("PNG canonical {m}"));
        }
    } else if row["codec"] == "lossless" {
        let frame = by_id(inputs, "frames", row["first_frame_id"].as_str().unwrap());
        let straight = compose(
            &packed(&frame["native_decoded_rgba"]),
            row["config"]["shape"] == "subrect",
            [0; 4],
            false,
        );
        if actual != straight {
            failures.push(format!(
                "PDF exact external straight {} expected={} actual={}",
                differences(&straight, &actual),
                hash(&straight),
                hash(&actual)
            ));
        }
    }
    if format == OutputFormat::Pdf {
        let views = canonical(&actual).views();
        for (bg, projected) in [("black", views.black()), ("white", views.white())] {
            let observed =
                decode(&row["reference"]["samples"]["intrinsic"]["composites"][bg]["rgba_base64"]);
            if let Some(m) = metric(&observed, projected) {
                failures.push(format!("PDF observed {bg} {m}"));
            }
        }
    }
    failures
}

// Structured native observations, never reference expectations. Preserve the
// comparator enum and exact IEEE score bits rather than rounded debug strings.
fn measured_view(name: &str, expected: &[u8], actual: &[u8], exact: bool) -> Value {
    let comparison = compare(
        &canonical(expected),
        &canonical(actual),
        VisualClass::GeometryImage,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    let ssim = match comparison.whole.ssim {
        excalidraw_render_compare::SsimOutcome::Score(score) => {
            json!({"kind":"score","bits":format!("{:016x}",score.to_bits())})
        }
        excalidraw_render_compare::SsimOutcome::SmallArtifactMismatch => {
            json!({"kind":"small-artifact-mismatch"})
        }
        excalidraw_render_compare::SsimOutcome::NonFinite => json!({"kind":"non-finite"}),
    };
    let mut changed = 0;
    let mut visible = 0;
    let mut maximum = 0;
    for (a, b) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        changed += usize::from(a != b);
        visible += usize::from(a[..3] != b[..3] && (a[3] != 0 || b[3] != 0));
        for c in 0..3 {
            maximum = maximum.max(a[c].abs_diff(b[c]));
        }
    }
    json!({"name":name,"requirement":if exact {"exact-external-lossless-straight"} else {"canonical-geometry-image"},
        "expected_rgba_sha256":hash(expected),"actual_rgba_sha256":hash(actual),
        "exact_changed_pixels":changed,"visible_rgb_changed_pixels":visible,"maximum_rgb_delta":maximum,
        "requirement_passed":if exact {expected==actual} else {comparison.passed()},
        "canonical":{"passed":comparison.passed(),"changed_pixels":comparison.whole.changed_pixels,
            "area":comparison.whole.area,"changed_mask_sha256":hash(&comparison.whole.changed_mask.iter().map(|&b|u8::from(b)).collect::<Vec<_>>()),
            "changed_pixels_pass":comparison.whole.changed_pixels_pass,"ssim_pass":comparison.whole.ssim_pass,"ssim":ssim}})
}

fn measurement(
    row: &Value,
    inputs: &Value,
    format: OutputFormat,
    bytes: &[u8],
    actual_report: &Value,
) -> Value {
    assert_eq!(
        actual_report,
        &report(format, false, row["frame_count"] == 2)
    );
    let mut measured = json!({"format":format.as_str(),"dimensions":{"width":9,"height":7},
        "diagnostic_sha256":hash(&serde_json::to_vec(actual_report).unwrap()),"diagnostic":actual_report});
    if format == OutputFormat::Svg {
        svg(
            std::str::from_utf8(bytes).unwrap(),
            Some(&packed(&row["webp"])),
        );
        measured["requirement_passed"] = json!(true);
        measured["source_webp_sha256"] = row["webp"]["sha256"].clone();
        return measured;
    }
    let actual = if format == OutputFormat::Png {
        super::png_profile_contract::assert_untagged_png(bytes);
        let image = crate::common::png_color_reference::png(bytes);
        assert_eq!((image.width(), image.height()), (9, 7));
        image.rgba().to_vec()
    } else {
        pdf_extent(bytes);
        super::png_profile_contract::pdf_rgba(bytes, 9, 7)
    };
    let authored = packed(&row["expected_first_transparent"]);
    let alpha = |b: &[u8]| b.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>();
    let (expected_alpha, actual_alpha) = (alpha(&authored), alpha(&actual));
    measured["actual_rgba_sha256"] = json!(hash(&actual));
    measured["alpha"] = json!({"expected_sha256":hash(&expected_alpha),"actual_sha256":hash(&actual_alpha),
        "mismatched_pixels":expected_alpha.iter().zip(&actual_alpha).filter(|(a,b)|a!=b).count(),
        "nonzero_invisible_pixels":actual.chunks_exact(4).filter(|p|p[3]==0 && p[..3]!=[0;3]).count(),
        "passed":exact_alpha(&actual,&authored)});
    let mut views = if format == OutputFormat::Png {
        let reference = crate::common::raster::decode_png(&decode(&row["reference"]["png"]));
        assert_eq!((reference.width, reference.height), (9, 7));
        vec![measured_view(
            "browser-png",
            &reference.rgba,
            &actual,
            false,
        )]
    } else if row["codec"] == "lossless" {
        let frame = by_id(inputs, "frames", row["first_frame_id"].as_str().unwrap());
        let straight = compose(
            &packed(&frame["native_decoded_rgba"]),
            row["config"]["shape"] == "subrect",
            [0; 4],
            false,
        );
        vec![measured_view(
            "external-lossless-straight",
            &straight,
            &actual,
            true,
        )]
    } else {
        vec![]
    };
    if format == OutputFormat::Pdf {
        let projected = canonical(&actual).views();
        views.extend(
            [("black", projected.black()), ("white", projected.white())]
                .into_iter()
                .map(|(bg, pixels)| {
                    let observed = decode(
                        &row["reference"]["samples"]["intrinsic"]["composites"][bg]["rgba_base64"],
                    );
                    measured_view(bg, &observed, pixels, false)
                }),
        );
    }
    measured["requirement_passed"] = json!(
        measured["alpha"]["passed"] == true
            && views.iter().all(|v| v["requirement_passed"] == true)
    );
    measured["views"] = json!(views);
    measured
}

fn audit_composition_changes(actual: &Value, record: &Value) {
    let before = native_defect_pins();
    assert_eq!(rows(actual, "cases").len(), 46);
    let mut changed_rows = 0;
    let mut changed_formats = 0;
    for row in rows(actual, "cases") {
        let id = row["id"].as_str().unwrap();
        let old = by_id(&before, "cases", id);
        let reference = by_id(record, "cases", id);
        assert_eq!(row["cell"], old["cell"]);
        assert_eq!(row["source_webp_sha256"], old["source_webp_sha256"]);
        let affected = CORRECTED_CELLS.contains(&row["cell"].as_str().unwrap())
            || (reference["alpha"] == "partial" && reference["config"]["blend"] == true);
        assert_eq!(
            row != old,
            affected,
            "{id}: only first-frame composition may change"
        );
        changed_rows += usize::from(row != old);
        for (new, old) in rows(row, "formats").iter().zip(rows(old, "formats")) {
            for key in ["format", "dimensions", "diagnostic", "diagnostic_sha256"] {
                assert_eq!(new[key], old[key], "{id} {key}");
            }
            if new["format"] == "svg" {
                assert_eq!(new, old);
                continue;
            }
            assert_eq!(new != old, affected, "{id}: pixel change scope");
            changed_formats += usize::from(new != old);
            assert_eq!(
                new["alpha"]["expected_sha256"],
                old["alpha"]["expected_sha256"]
            );
            assert_eq!(new["alpha"]["passed"], true);
            for (nv, ov) in rows(new, "views").iter().zip(rows(old, "views")) {
                for key in ["name", "requirement", "expected_rgba_sha256"] {
                    assert_eq!(nv[key], ov[key], "{id} reference view changed");
                }
            }
        }
    }
    assert_eq!((changed_rows, changed_formats), (30, 60));
}

fn composition_pins() -> Value {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/oracle/webp-sample/composition-defect-measurements.json"
    ));
    assert_eq!(
        hash(bytes),
        "6c9f48c8903604c7112085d64fd53325d30975f3552c01a3ace5e50dae5de668"
    );
    serde_json::from_slice(bytes).unwrap()
}

fn assert_composition_defects(actual: &Value, pinned: &Value, record: &Value) {
    assert!(actual==pinned,"#191 composition measurement changed: diagnose improvement or regression, never refresh automatically");
    audit_composition_changes(actual, record);
    let mut passes = [0; 3];
    let mut failures = 0;
    let mut residuals = 0;
    for row in rows(actual, "cases") {
        let corrected = CORRECTED_CELLS.contains(&row["cell"].as_str().unwrap());
        let residual = RESIDUAL_CELLS.contains(&row["cell"].as_str().unwrap());
        assert_ne!(corrected, residual);
        residuals += usize::from(residual);
        assert_eq!(rows(row, "formats").len(), 3);
        for (i, f) in rows(row, "formats").iter().enumerate() {
            assert_eq!(f["format"], FORMATS[i].as_str());
            assert_eq!(f["requirement_passed"], i == 0 || corrected);
            passes[i] += usize::from(f["requirement_passed"] == true);
            if i == 0 {
                continue;
            }
            assert_eq!(f["alpha"]["passed"], true);
            assert_eq!(f["alpha"]["mismatched_pixels"], 0);
            assert_eq!(f["alpha"]["nonzero_invisible_pixels"], 0);
            for view in rows(f, "views") {
                let straight = view["name"] == "external-lossless-straight";
                let passed = corrected || straight;
                assert_eq!(view["requirement_passed"], passed);
                assert_eq!(view["canonical"]["passed"], passed);
                if !passed {
                    failures += 1;
                    assert_eq!(
                        view["canonical"]["ssim"],
                        json!({"kind":"small-artifact-mismatch"})
                    );
                    assert_eq!(view["canonical"]["changed_pixels"], 0);
                    assert_ne!(view["expected_rgba_sha256"], view["actual_rgba_sha256"]);
                }
            }
        }
    }
    assert_eq!(residuals, 24);
    assert_eq!(passes, [46, 22, 22]);
    assert_eq!(failures, 72);
}

fn composition_discrimination(record: &Value) {
    let pinned = composition_pins();
    assert_composition_defects(&pinned, &pinned, record);
    let all: BTreeSet<_> = QUALIFIED_CELLS
        .iter()
        .chain(&CORRECTED_CELLS)
        .chain(&RESIDUAL_CELLS)
        .copied()
        .collect();
    assert_eq!(all.len(), 36);
    assert_eq!(
        all,
        rows(record, "cases")
            .iter()
            .map(|r| r["cell"].as_str().unwrap())
            .collect()
    );
    let residual = rows(&pinned, "cases")
        .iter()
        .position(|r| RESIDUAL_CELLS.contains(&r["cell"].as_str().unwrap()))
        .unwrap();
    let row = by_id(
        record,
        "cases",
        pinned["cases"][residual]["id"].as_str().unwrap(),
    );
    let reference = crate::common::raster::decode_png(&decode(&row["reference"]["png"]));
    let mut improved = pinned.clone();
    improved["cases"][residual]["formats"][1]["views"][0] =
        measured_view("browser-png", &reference.rgba, &reference.rgba, false);
    rejects(|| assert_composition_defects(&improved, &pinned, record));
    rejects(|| assert_composition_defects(&improved, &improved, record));
    for pointer in [
        "/formats/1/actual_rgba_sha256",
        "/formats/1/alpha/mismatched_pixels",
        "/formats/2/diagnostic_sha256",
        "/formats/2/views/0/canonical/ssim/bits",
        "/formats/2/views/1/actual_rgba_sha256",
        "/formats/2/views/1/canonical/changed_pixels",
        "/formats/0/requirement_passed",
    ] {
        let mut wrong = pinned.clone();
        *wrong["cases"][residual].pointer_mut(pointer).unwrap() = Value::Null;
        rejects(|| assert_composition_defects(&wrong, &pinned, record));
    }
    // Old first-frame blending/opaque-canvas behavior must not regress back.
    let before = native_defect_pins();
    let mut regressed = pinned.clone();
    regressed["cases"][0] = before["cases"][0].clone();
    rejects(|| assert_composition_defects(&regressed, &pinned, record));
}

// Projecting away only the additional gates reconstructs the original pin file
// byte-for-byte. This audits every pre-existing field, including actual pixels,
// diagnostics, and reference hashes, without retaining another evidence file.
fn audit_additive_gates(measured: &Value) {
    let mut original = measured.clone();
    original.as_object_mut().unwrap().remove("oracle_amendment");
    let mut added = 0;
    let mut newly_failed = 0;
    for row in original["cases"].as_array_mut().unwrap() {
        let pdf = &mut row["formats"][2];
        if pdf["views"][0]["name"] == "external-lossless-straight" {
            assert_eq!(pdf["views"].as_array().unwrap().len(), 3);
            assert_eq!(pdf["views"][1]["name"], "black");
            assert_eq!(pdf["views"][2]["name"], "white");
            added += 2;
            let previous_pass =
                pdf["alpha"]["passed"] == true && pdf["views"][0]["requirement_passed"] == true;
            newly_failed += usize::from(previous_pass && pdf["requirement_passed"] == false);
            pdf["views"].as_array_mut().unwrap().truncate(1);
            pdf["requirement_passed"] = json!(previous_pass);
        }
    }
    assert_eq!((added, newly_failed), (48, 8));
    let mut bytes = serde_json::to_vec_pretty(&original).unwrap();
    bytes.push(b'\n');
    assert_eq!(
        hash(&bytes),
        "d8b85832ed9b9567f56b03dcbfa1e39d5b5b6be6b988e71bb8ab18eafb54b6f1",
        "existing measurements changed during additive oracle correction"
    );
}

fn native_defect_pins() -> Value {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/oracle/webp-sample/native-defect-measurements.json"
    ));
    assert_eq!(
        hash(bytes),
        "d31850b8a23ed04039de976437a06b9922bbc963ee83f3b6269833d5cc24ba76"
    );
    let pinned = serde_json::from_slice(bytes).unwrap();
    audit_additive_gates(&pinned);
    pinned
}

fn assert_native_defects(actual: &Value, pinned: &Value) {
    assert!(actual==pinned,"#191 native defect changed: improvement or regression requires diagnosis, not a pin refresh");
    audit_additive_gates(actual);
    let mut passes = [0; 3];
    let mut failed_views = 0;
    let mut alpha_failures = 0;
    let mut cells = BTreeMap::new();
    assert_eq!(rows(actual, "cases").len(), 46);
    for row in rows(actual, "cases") {
        *cells.entry(row["cell"].as_str().unwrap()).or_insert(0) += 1;
        assert!(!QUALIFIED_CELLS.contains(&row["cell"].as_str().unwrap()));
        let formats = rows(row, "formats");
        assert_eq!(formats.len(), 3);
        assert_eq!(formats[0]["requirement_passed"], true);
        assert_eq!(
            formats[1]["requirement_passed"], false,
            "#191 PNG improved: requalify this cell"
        );
        for (i, f) in formats.iter().enumerate() {
            assert_eq!(f["format"], FORMATS[i].as_str());
            assert_eq!(f["dimensions"], json!({"width":9,"height":7}));
            passes[i] += usize::from(f["requirement_passed"] == true);
            if i == 0 {
                continue;
            }
            alpha_failures += usize::from(f["alpha"]["passed"] == false);
            for view in rows(f, "views") {
                if view["requirement_passed"] == false {
                    failed_views += 1;
                    assert_eq!(
                        view["canonical"]["passed"], false,
                        "#191 must still fail the canonical comparator"
                    );
                    assert_eq!(
                        view["canonical"]["ssim"],
                        json!({"kind":"small-artifact-mismatch"})
                    );
                    assert_ne!(view["expected_rgba_sha256"], view["actual_rgba_sha256"]);
                } else {
                    assert_eq!(
                        view["canonical"]["passed"], true,
                        "passing companion must remain canonical"
                    );
                }
            }
        }
    }
    assert_eq!(cells.len(), 23);
    assert!(cells.values().all(|&n| n == 2));
    assert_eq!(passes, [46, 0, 0]);
    assert_eq!(failed_views, 144);
    assert_eq!(alpha_failures, 24);
}

fn native_pin_discrimination(pinned: &Value, record: &Value) {
    // A genuine improved comparator result must be rejected even if its alpha,
    // dimensions and source identity are unchanged.
    let first = &pinned["cases"][0];
    let row = by_id(record, "cases", first["id"].as_str().unwrap());
    let reference = crate::common::raster::decode_png(&decode(&row["reference"]["png"]));
    let mut improved = pinned.clone();
    improved["cases"][0]["formats"][1]["views"][0] =
        measured_view("browser-png", &reference.rgba, &reference.rgba, false);
    rejects(|| assert_native_defects(&improved, pinned));
    rejects(|| assert_native_defects(&improved, &improved));
    // Pixel/hash drift must fail even when all failure booleans stay unchanged.
    let mut regressed_pixels = reference.rgba.clone();
    regressed_pixels[..4].copy_from_slice(&[255, 0, 255, 255]);
    let mut regressed = pinned.clone();
    regressed["cases"][0]["formats"][1]["views"][0] =
        measured_view("browser-png", &reference.rgba, &regressed_pixels, false);
    rejects(|| assert_native_defects(&regressed, pinned));
    for pointer in [
        "/cases/0/formats/1/actual_rgba_sha256",
        "/cases/0/formats/1/diagnostic_sha256",
        "/cases/0/formats/1/alpha/mismatched_pixels",
        "/cases/0/formats/1/views/0/expected_rgba_sha256",
        "/cases/0/formats/1/views/0/canonical/changed_pixels",
        "/cases/0/formats/0/dimensions/width",
        "/cases/0/formats/0/requirement_passed",
        "/cases/0/formats/1/diagnostic/diagnostics/0/code",
        "/cases/0/formats/2/views/0/exact_changed_pixels",
        "/cases/0/formats/1/views/0/canonical/ssim/kind",
    ] {
        let mut wrong = pinned.clone();
        *wrong.pointer_mut(pointer).unwrap() = Value::Null;
        rejects(|| assert_native_defects(&wrong, pinned));
    }
    let companion = rows(pinned, "cases")
        .iter()
        .position(|r| r["formats"][2]["views"][0]["requirement_passed"] == true)
        .unwrap();
    let mut wrong_score = pinned.clone();
    wrong_score["cases"][companion]["formats"][2]["views"][0]["canonical"]["ssim"]["bits"] =
        json!("0000000000000000");
    rejects(|| assert_native_defects(&wrong_score, pinned));
}

fn lossless_pdf_gates_are_independent(record: &Value, inputs: &Value) {
    // Exact straight RGB is necessary but insufficient: these twelve real PDF
    // outputs pass it while failing BOTH independent browser-composite gates.
    for config in [
        "static-full",
        "anim-full-blend-d0-clear",
        "anim-full-replace-d1-opaque",
        "anim-sub-blend-d1-opaque",
        "anim-sub-replace-d0-clear",
        "anim-sub-replace-d1-opaque",
    ] {
        for (pattern, changed) in [
            ("ramp", if config.contains("sub") { 10 } else { 37 }),
            ("edges", if config.contains("sub") { 11 } else { 39 }),
        ] {
            let id = format!("lossless-partial-{config}-{pattern}");
            let row = by_id(record, "cases", &id);
            let (bytes, diagnostic) = render(&row["scene"], OutputFormat::Pdf);
            let measured = measurement(row, inputs, OutputFormat::Pdf, &bytes, &diagnostic);
            assert_eq!(measured["alpha"]["passed"], true);
            let views = rows(&measured, "views");
            assert_eq!(views.len(), 3);
            assert_eq!(views[0]["name"], "external-lossless-straight");
            assert_eq!(views[0]["requirement_passed"], true);
            assert_eq!(views[0]["exact_changed_pixels"], 0);
            for (view, name) in views[1..].iter().zip(["black", "white"]) {
                assert_eq!(view["name"], name);
                assert_eq!(view["requirement_passed"], false);
                assert_eq!(
                    view["canonical"]["ssim"],
                    json!({"kind":"small-artifact-mismatch"})
                );
                assert_eq!(view["canonical"]["changed_pixels"], 0);
                assert_eq!(view["exact_changed_pixels"], changed);
                assert_eq!(view["maximum_rgb_delta"], 1);
            }
            assert_eq!(measured["requirement_passed"], false);
            // Exercise both consumers: neither may turn straight success into
            // format success or omit either failing composite.
            let failures = appearance(row, inputs, OutputFormat::Pdf, &bytes);
            assert_eq!(failures.len(), 2);
            assert!(failures[0].starts_with("PDF observed black "));
            assert!(failures[1].starts_with("PDF observed white "));
            rejects(|| {
                assert_eq!(
                    measured["requirement_passed"],
                    views[0]["requirement_passed"]
                )
            });
        }
    }
}
fn rejects(f: impl FnOnce()) {
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err(),
        "negative helper accepted mutation"
    );
}
fn replace(b: &[u8], from: &str, to: &str) -> Vec<u8> {
    let mut out = b.to_vec();
    let at = b
        .windows(from.len())
        .position(|w| w == from.as_bytes())
        .unwrap();
    out.splice(at..at + from.len(), to.bytes());
    out
}
fn negative_helpers(record: &Value) {
    let row = by_id(record, "cases", "lossless-partial-static-full-ramp");
    let expected = packed(&row["expected_first_transparent"]);
    let mut wrong = expected.clone();
    wrong[3] ^= 1;
    assert!(!exact_alpha(&wrong, &expected));
    wrong = expected.clone();
    wrong[0] = 1;
    assert!(!exact_alpha(&wrong, &expected));
    assert!(metric(&expected, &[0, 0, 0, 255].repeat(63)).is_some());
    let limits = VisualClass::GeometryImage.limits(ComparisonMode::Canonical);
    assert_eq!(limits.minimum_ssim(), 0.999);
    assert_eq!(
        limits.changed_pixel_limit(),
        excalidraw_render_compare::ChangedPixelLimit::Fraction {
            numerator: 1,
            denominator: 400
        }
    );
    let s = row["reference"]["svg"].as_str().unwrap();
    let wire = packed(&row["webp"]);
    rejects(|| svg(&s.replacen("width=\"9\"", "width=\"8\"", 1), Some(&wire)));
    let mut bad_wire = wire.clone();
    bad_wire[0] ^= 1;
    rejects(|| svg(s, Some(&bad_wire)));
    rejects(|| svg(s, None));
    let (png, _) = render(&row["scene"], OutputFormat::Png);
    rejects(|| {
        compare(
            &canonical(&expected),
            &CanonicalRgba8::new(7, 9, expected.clone()).unwrap(),
            VisualClass::GeometryImage,
            &[],
            ComparisonMode::Canonical,
        )
        .unwrap();
    });
    for (depth, color, pixels) in [
        (
            png::BitDepth::Eight,
            png::ColorType::Rgb,
            vec![0; 9 * 7 * 3],
        ),
        (
            png::BitDepth::Sixteen,
            png::ColorType::Rgba,
            vec![0; 9 * 7 * 8],
        ),
        (
            png::BitDepth::Eight,
            png::ColorType::Rgba,
            [1, 2, 3, 0].repeat(63),
        ),
    ] {
        let mut output = vec![];
        {
            let mut encoder = png::Encoder::new(&mut output, 9, 7);
            encoder.set_depth(depth);
            encoder.set_color(color);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        rejects(|| {
            crate::common::png_color_reference::png(&output);
        });
    }
    let mut tagged = png.clone();
    let mut tag = vec![];
    tag.extend_from_slice(&1u32.to_be_bytes());
    tag.extend_from_slice(b"sRGB\0");
    tag.extend_from_slice(&crc32fast::hash(b"sRGB\0").to_be_bytes());
    tagged.splice(33..33, tag);
    rejects(|| super::png_profile_contract::assert_untagged_png(&tagged));
    let (pdf, _) = render(&row["scene"], OutputFormat::Pdf);
    rejects(|| {
        pdf_extent(&replace(
            &pdf,
            "/MediaBox [0 0 6.75 5.25]",
            "/MediaBox [0 0 7.75 5.25]",
        ))
    });
    rejects(|| {
        pdf_extent(&replace(
            &pdf,
            "/CropBox [0 0 6.75 5.25]",
            "/CropBox [0 0 7.75 5.25]",
        ))
    });
    rejects(|| {
        super::png_profile_contract::pdf_rgba(&replace(&pdf, "/Width 9", "/Width 8"), 9, 7);
    });
    // Mutate the gray mask separately from the RGB image, using the actual
    // artifact's dictionary (not an assumed object number).
    let gray_at = pdf
        .windows(b"/ColorSpace /DeviceGray".len())
        .position(|w| w == b"/ColorSpace /DeviceGray")
        .unwrap();
    let start = pdf[..gray_at]
        .windows(8)
        .rposition(|w| w == b"/Width 9")
        .unwrap();
    let mut wrong_mask = pdf.clone();
    wrong_mask[start + 7] = b'8';
    rejects(|| {
        super::png_profile_contract::pdf_rgba(&wrong_mask, 9, 7);
    });
    rejects(|| {
        super::png_profile_contract::pdf_rgba(&replace(&pdf, "/SMask ", "/Dummy "), 9, 7)
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .for_each(|(a, b)| assert_eq!(a[3], b[3]));
    });
    rejects(|| {
        super::png_profile_contract::pdf_rgba(
            &replace(&pdf, "/ColorSpace [/ICCBased", "/ColorSpace [/DeviceRGB"),
            9,
            7,
        );
    });
    let complete = report(OutputFormat::Png, false, true);
    for wrong in [
        report(OutputFormat::Png, false, false),
        report(OutputFormat::Pdf, false, true),
        report(OutputFormat::Png, true, false),
    ] {
        rejects(|| assert_eq!(wrong, complete));
    }
    let mut duplicate = complete.clone();
    duplicate["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(complete["diagnostics"][0].clone());
    rejects(|| assert_eq!(duplicate, complete));
}

fn qualified_first_display_partition(cells: &[&str]) {
    let (record, inputs) = evidence();
    let mut failures = vec![];
    let mut pass = 0;
    let selected: Vec<_> = rows(&record, "cases")
        .iter()
        .filter(|row| cells.contains(&row["cell"].as_str().unwrap()))
        .collect();
    assert_eq!(selected.len(), cells.len() * 2);
    let mut format_passes = [0; 3];
    for row in &selected {
        let mut good = true;
        for (i, format) in FORMATS.into_iter().enumerate() {
            let (bytes, actual_report) = render(&row["scene"], format);
            assert_eq!(
                actual_report,
                report(format, false, row["frame_count"] == 2),
                "{} {format:?}",
                row["id"]
            );
            let observations = appearance(row, &inputs, format, &bytes);
            if observations.is_empty() {
                format_passes[i] += 1;
            }
            for failure in observations {
                good = false;
                failures.push(format!("{} {format:?}: {failure}", row["id"]));
            }
        }
        if good {
            pass += 1;
        }
    }
    eprintln!(
        "WebP qualified: complete vectors passed {pass}/{}; SVG/PNG/PDF passes={format_passes:?}; failures:\n{}",
        selected.len(),
        failures.join("\n")
    );
    assert!(
        failures.is_empty(),
        "{} format/color failures",
        failures.len()
    );
}

#[test]
fn qualified_13_cells_first_display_contract() {
    qualified_first_display_partition(&QUALIFIED_CELLS);
}

#[test]
fn all_sample_pngs_match_reference() {
    let (record, inputs) = evidence();
    let mut checked = 0;
    for row in rows(&record, "cases") {
        let (bytes, actual_report) = render(&row["scene"], OutputFormat::Png);
        assert_eq!(
            actual_report,
            report(OutputFormat::Png, false, row["frame_count"] == 2)
        );
        let failures = appearance(row, &inputs, OutputFormat::Png, &bytes);
        assert!(failures.is_empty(), "{}: {failures:?}", row["id"]);
        checked += 1;
    }
    assert_eq!(checked, 72);
}

#[test]
fn nested_webp_png_preserves_draw_time_alpha_quantization() {
    let (record, _) = evidence();
    for row in rows(&record, "cases") {
        let mut nested = row["scene"].clone();
        let href = nested["files"]["asset"]["dataURL"].as_str().unwrap();
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="9" height="7"><image width="9" height="7" href="{href}"/></svg>"#
        );
        nested["files"]["asset"]["dataURL"] = json!(format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg)
        ));
        nested["files"]["asset"]["mimeType"] = json!("image/svg+xml");
        let mut expected = row["scene"].clone();
        let href = format!(
            "data:image/png;base64,{}",
            row["reference"]["png"].as_str().unwrap()
        );
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="9" height="7"><image width="9" height="7" href="{href}"/></svg>"#
        );
        expected["files"]["asset"]["dataURL"] = json!(format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg)
        ));
        expected["files"]["asset"]["mimeType"] = json!("image/svg+xml");
        let (direct, _) = render(&expected, OutputFormat::Png);
        let (indirect, _) = render(&nested, OutputFormat::Png);
        let direct = crate::common::raster::decode_png(&direct);
        let indirect = crate::common::raster::decode_png(&indirect);
        assert_eq!((indirect.width, indirect.height), (9, 7));
        assert_eq!(indirect.rgba, direct.rgba, "{}", row["id"]);
    }
}

#[test]
fn issue_191_partial_12_cells_exact_native_defects() {
    let (record, inputs) = evidence();
    let pinned = composition_pins();
    // Keep the composition checkpoint immutable. Only the PNG projection of
    // partial-alpha rows may improve; SVG and every PDF measurement stay exact.
    assert_composition_defects(&pinned, &pinned, &record);
    let mut corrected = 0;
    for old in rows(&pinned, "cases") {
        let row = by_id(&record, "cases", old["id"].as_str().unwrap());
        for (i, format) in FORMATS.into_iter().enumerate() {
            let (bytes, diagnostic) = render(&row["scene"], format);
            let actual = measurement(row, &inputs, format, &bytes, &diagnostic);
            if format == OutputFormat::Png && row["alpha"] == "partial" {
                assert_eq!(actual["requirement_passed"], true);
                assert!(appearance(row, &inputs, format, &bytes).is_empty());
                assert_eq!(diagnostic, old["formats"][i]["diagnostic"]);
                assert_eq!(actual["alpha"], old["formats"][i]["alpha"]);
                corrected += 1;
            } else {
                assert_eq!(actual, old["formats"][i], "{} {format:?}", row["id"]);
            }
        }
    }
    assert_eq!(corrected, 24);
}

const PDF_EXCEPTIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/webp-sample/pdf-partial-alpha-exceptions.json"
));
const PDF_SAMPLE_VIEWER: &str = "pdf-straight-sample-projection";

/// The approved partial-alpha PDF exceptions. The approval binds the record
/// without its approval object, so any change to a ceiling or reason needs a
/// new approval.
fn approved_pdf_exceptions() -> Value {
    let mut record: Value = serde_json::from_slice(PDF_EXCEPTIONS).unwrap();
    let approval = record["approval"].take();
    let subject = format!("sha256:{}", hash(&serde_json::to_vec(&record).unwrap()));
    assert!(
        approval["approval_url"]
            .as_str()
            .is_some_and(|url| url.starts_with(
                "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-"
            )),
        "partial-alpha PDF exceptions have no recorded approval; subject {subject}"
    );
    assert_eq!(
        approval["approval_subject_sha256"], subject,
        "approval does not bind the exact exception record"
    );
    record
}

fn within_pdf_exception(view: &Value, exception: &Value) -> bool {
    view["canonical"]["ssim"] == json!({"kind":"small-artifact-mismatch"})
        && n(&view["canonical"]["changed_pixels"]) <= n(&exception["max_pixelmatch_changed_pixels"])
        && n(&view["exact_changed_pixels"]) <= n(&exception["max_differing_pixels"])
        && n(&view["maximum_rgb_delta"]) <= n(&exception["max_rgb_delta"])
}

/// The 12 partial-alpha cells: SVG and PNG match the reference outright, and
/// PDF keeps exact alpha and exact lossless straight samples. Each failing
/// black/white composite is held at its approved exact ceilings, and an
/// exception whose view passes the canonical comparator is rejected as dead.
#[test]
fn partial_alpha_cells_match_reference_under_approved_pdf_exceptions() {
    let (record, inputs) = evidence();
    let exceptions = approved_pdf_exceptions();
    let mut unused = BTreeSet::new();
    for exception in rows(&exceptions, "exceptions") {
        assert_eq!(
            [
                &exception["format"],
                &exception["region"],
                &exception["viewer"],
                &exception["ssim_outcome"]
            ],
            [
                &json!("pdf"),
                &json!("whole"),
                &json!(PDF_SAMPLE_VIEWER),
                &json!("small-artifact-mismatch")
            ]
        );
        assert!(RESIDUAL_CELLS.contains(&exception["cell"].as_str().unwrap()));
        assert!(unused.insert((
            exception["vector"].to_string(),
            exception["view"].to_string()
        )));
    }
    let selected: Vec<_> = rows(&record, "cases")
        .iter()
        .filter(|row| RESIDUAL_CELLS.contains(&row["cell"].as_str().unwrap()))
        .collect();
    assert_eq!(selected.len(), RESIDUAL_CELLS.len() * 2);
    for row in selected {
        for format in FORMATS {
            let (bytes, diagnostic) = render(&row["scene"], format);
            let measured = measurement(row, &inputs, format, &bytes, &diagnostic);
            if format != OutputFormat::Pdf {
                assert!(
                    appearance(row, &inputs, format, &bytes).is_empty(),
                    "{} {format:?}",
                    row["id"]
                );
                continue;
            }
            assert_eq!(measured["alpha"]["passed"], true, "{}", row["id"]);
            for view in rows(&measured, "views") {
                let exception = rows(&exceptions, "exceptions")
                    .iter()
                    .find(|e| e["vector"] == row["id"] && e["view"] == view["name"]);
                let Some(exception) = exception else {
                    assert_eq!(
                        view["requirement_passed"], true,
                        "{} {}",
                        row["id"], view["name"]
                    );
                    continue;
                };
                unused.remove(&(row["id"].to_string(), view["name"].to_string()));
                assert_eq!(exception["cell"], row["cell"]);
                assert_eq!(
                    view["canonical"]["passed"], false,
                    "dead exception {} {}: the view passes the canonical comparator",
                    row["id"], view["name"]
                );
                assert!(
                    within_pdf_exception(view, exception),
                    "{} {} exceeds its exception: {view}",
                    row["id"],
                    view["name"]
                );
            }
        }
    }
    assert!(
        unused.is_empty(),
        "exceptions without a measured view: {unused:?}"
    );
}

#[test]
fn pdf_exception_ceilings_are_exact_and_reject_growth() {
    let exceptions: Value = serde_json::from_slice(PDF_EXCEPTIONS).unwrap();
    let exception = &rows(&exceptions, "exceptions")[0];
    let view = json!({"canonical":{"ssim":{"kind":"small-artifact-mismatch"},
        "changed_pixels":exception["max_pixelmatch_changed_pixels"]},
        "exact_changed_pixels":exception["max_differing_pixels"],"maximum_rgb_delta":exception["max_rgb_delta"]});
    assert!(within_pdf_exception(&view, exception));
    for pointer in [
        "/canonical/changed_pixels",
        "/exact_changed_pixels",
        "/maximum_rgb_delta",
    ] {
        let mut grown = view.clone();
        let value = grown.pointer_mut(pointer).unwrap();
        *value = json!(n(value) + 1);
        assert!(!within_pdf_exception(&grown, exception), "{pointer}");
    }
    let mut scored = view.clone();
    scored["canonical"]["ssim"] = json!({"kind":"score","bits":"3fefffffffffffff"});
    assert!(!within_pdf_exception(&scored, exception));
}

#[test]
fn corrected_composition_cells_match_reference() {
    qualified_first_display_partition(&CORRECTED_CELLS);
}

#[test]
fn malformed_10_controls_exact_recovery() {
    // Source-bound rejection characterization, not normative WebP conformance:
    // scan_webp_animation/validate_webp_animation_frame reject ANMF reserved
    // bits, even though the container spec tells readers to ignore them.
    let (record, _) = evidence();
    let selected: Vec<_> = rows(&record, "controls")
        .iter()
        .filter(|c| !c["id"].as_str().unwrap().starts_with("vp8x-reserved-"))
        .collect();
    assert_eq!(selected.len(), 10);
    for c in selected {
        let source = scene(&packed(&c["webp"]));
        for format in FORMATS {
            let (bytes, actual) = render(&source, format);
            assert_eq!(
                actual,
                report(format, true, false),
                "{} {format:?}",
                c["id"]
            );
            let mut missing = source.clone();
            missing["files"] = json!({});
            assert_eq!(
                bytes,
                render(&missing, format).0,
                "{} {format:?} exact recovery",
                c["id"]
            );
            match format {
                OutputFormat::Svg => svg(std::str::from_utf8(&bytes).unwrap(), None),
                OutputFormat::Png => {
                    let image = crate::common::raster::decode_png(&bytes);
                    assert_eq!((image.width, image.height), (9, 7));
                }
                OutputFormat::Pdf => {
                    pdf_extent(&bytes);
                    assert!(
                        !crate::common::pdf_inspect::pdf_text(&bytes).contains("/Subtype /Image")
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn reserved_vp8x_controls_are_accepted_characterization() {
    // These controls are uncollected. The container specification tells readers
    // to ignore reserved fields (Extended File Format, VP8X):
    // https://developers.google.com/speed/webp/docs/riff_container
    // The author's intended_strict is not evidence
    // of a production reject policy. Pin acceptance, not browser color parity.
    let (record, _) = evidence();
    let base = by_id(
        &record,
        "cases",
        "lossless-partial-anim-sub-blend-d1-opaque-ramp",
    );
    for id in ["vp8x-reserved-flag", "vp8x-reserved-byte"] {
        let c = by_id(&record, "controls", id);
        let wire = packed(&c["webp"]);
        for format in FORMATS {
            let (bytes, actual) = render(&scene(&wire), format);
            assert_eq!(actual, report(format, false, true), "{id} {format:?}");
            if format == OutputFormat::Svg {
                svg(std::str::from_utf8(&bytes).unwrap(), Some(&wire));
            } else {
                assert_eq!(
                    bytes,
                    render(&base["scene"], format).0,
                    "{id}: ignored reserved field"
                );
            }
        }
    }
}
