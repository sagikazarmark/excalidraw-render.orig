//! Closed 32-cell first-display characterization and 20 strict recovery controls.
//! Duplicate/orphan GCE owners are explicitly NON-NORMATIVE, bounded recovery
//! characterizations. Empty Plain Text uses measured browser RGBA, leaving the
//! author's consuming-GCE hypothesis intact. No later composite is qualified by
//! native first-frame output. Every selector runs all evidence/negative gates.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const SIGNALS: [&str; 12] = [
    "no-gce",
    "transparent",
    "opaque",
    "transparent-comment",
    "transparent-application",
    "transparent-plain-text",
    "transparent-plain-text-opaque",
    "duplicate-transparent-opaque",
    "duplicate-opaque-transparent",
    "duplicate-index0-index1",
    "user-input-long-delay",
    "trailing-gce",
];
const LATER: [&str; 4] = [
    "later-no-gce",
    "later-transparent",
    "later-opaque",
    "first-transparent-later-no-gce",
];
const MALFORMED: [(&str, &str); 10] = [
    ("bad-palette-index", "palette-index"),
    ("undefined-lzw", "literal-lzw"),
    ("truncated-gce", "truncated-gce"),
    ("out-of-bounds", "out-of-bounds"),
    ("truncated-subblocks", "truncated-subblocks"),
    ("invalid-end", "invalid-end"),
    ("minimum-code-size-1", "minimum-code-size"),
    ("early-eoi", "pixel-count"),
    ("missing-eoi", "missing-eoi"),
    ("truncated-palette", "truncated-palette"),
];
const BASE: &str = "two-frame-local-later-opaque-pattern1";
const SOURCES: [&str; 18] = [
    "tests/oracle/reference-profiles/revision-1-assets.json",
    "tests/oracle/reference-profiles/revision-1-assets.sha256",
    "tests/oracle/reference-profiles/revision-1.json",
    "tests/oracle/reference-profiles/revision-1.sha256",
    "tools/reference-oracle/fixtures/profile-smoke.excalidraw",
    "tools/reference-oracle/index.html",
    "tools/reference-oracle/package-lock.json",
    "tools/reference-oracle/package.json",
    "tools/reference-oracle/run-container.sh",
    "tools/reference-oracle/run-gif-control.sh",
    "tools/reference-oracle/scripts/canonical-json.mjs",
    "tools/reference-oracle/scripts/gif-control-evidence.mjs",
    "tools/reference-oracle/scripts/gif-control-fixtures.mjs",
    "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs",
    "tools/reference-oracle/scripts/profile.mjs",
    "tools/reference-oracle/scripts/smoke.mjs",
    "tools/reference-oracle/src/browser-entry.js",
    "tools/reference-oracle/vite.config.mjs",
];
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn encode(bytes: &[u8]) -> Value {
    json!(base64::engine::general_purpose::STANDARD.encode(bytes))
}
fn decode(v: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(v.as_str().unwrap())
        .unwrap()
}
fn n(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn by_id<'a>(record: &'a Value, key: &str, id: &str) -> &'a Value {
    record[key]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap()
}
fn require(ok: bool, error: &'static str) -> Result<(), &'static str> {
    if ok {
        Ok(())
    } else {
        Err(error)
    }
}
fn rgba_record(bytes: &[u8]) -> Value {
    json!({"rgba_base64":encode(bytes),"rgba_sha256":hash(bytes)})
}

// Independent bounded wire reader. The dialect deliberately requires a clear
// before every literal; it does not borrow a GIF codec or the authored arrays.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, size: usize, error: &'static str) -> Result<&[u8], &'static str> {
        let result = self.bytes.get(self.at..self.at + size).ok_or(error)?;
        self.at += size;
        Ok(result)
    }
    fn byte(&mut self, error: &'static str) -> Result<u8, &'static str> {
        Ok(self.take(1, error)?[0])
    }
    fn blocks(&mut self) -> Result<Vec<u8>, &'static str> {
        let mut result = Vec::new();
        loop {
            let size = usize::from(self.byte("truncated-subblocks")?);
            if size == 0 {
                return Ok(result);
            }
            require(result.len() + size <= 1024, "subblock-bound")?;
            result.extend(self.take(size, "truncated-subblocks")?);
        }
    }
}
fn short(bytes: &[u8]) -> usize {
    usize::from(u16::from_le_bytes(bytes[..2].try_into().unwrap()))
}
fn literals(data: &[u8], count: usize, entries: usize) -> Result<Vec<u8>, &'static str> {
    let mut bit = 0;
    let mut indices = Vec::new();
    let mut want_clear = true;
    loop {
        require(bit + 3 <= data.len() * 8, "missing-eoi")?;
        let code = (0..3).fold(0, |code, j| {
            code | (((data[(bit + j) / 8] >> ((bit + j) % 8)) & 1) << j)
        });
        bit += 3;
        if code == 5 {
            require(indices.len() == count, "pixel-count")?;
            require(data.len() * 8 - bit < 8, "lzw-padding")?;
            require(
                (bit..data.len() * 8).all(|i| data[i / 8] & (1 << (i % 8)) == 0),
                "lzw-padding",
            )?;
            return Ok(indices);
        }
        if want_clear {
            require(code == 4, "literal-lzw")?;
        } else {
            require(code < 4, "literal-lzw")?;
            require(usize::from(code) < entries, "palette-index")?;
            require(indices.len() < 1024, "pixel-bound")?;
            indices.push(code);
        }
        want_clear = !want_clear;
    }
}
fn inspect(bytes: &[u8]) -> Value {
    let mut r = Reader { bytes, at: 0 };
    let mut frames = Vec::new();
    let mut blocks = Vec::new();
    let mut policy = Vec::new();
    let mut pending = Value::Null;
    let result = (|| -> Result<(), &'static str> {
        require(bytes.len() <= 1024, "input-bound")?;
        require(r.take(6, "truncated-container")? == b"GIF89a", "header")?;
        let screen = r.take(7, "truncated-container")?;
        let (width, height, flags) = (short(screen), short(&screen[2..]), screen[4]);
        require(
            width > 0 && height > 0 && width * height <= 63,
            "screen-bound",
        )?;
        let global = if flags & 128 != 0 {
            r.take(3 * (2 << (flags & 7)), "truncated-palette")?
                .to_vec()
        } else {
            vec![]
        };
        loop {
            let at = r.at;
            match r.byte("invalid-end")? {
                0x3b => {
                    require(r.at == bytes.len(), "invalid-end")?;
                    if !pending.is_null() {
                        policy.push("orphan-gce");
                    }
                    return Ok(());
                }
                0x21 => {
                    let label = r.byte("truncated-container")?;
                    if label == 0xf9 {
                        require(r.byte("truncated-gce")? == 4, "gce-size")?;
                        let fields = r.take(5, "truncated-gce")?;
                        require(fields[4] == 0, "gce-terminator")?;
                        if !pending.is_null() {
                            policy.push("duplicate-gce");
                        }
                        pending = json!({"transparent":if fields[0]&1 != 0 {json!(fields[3])} else {Value::Null},
                            "delay_cs":short(&fields[1..]),"user_input":fields[0]&2 != 0,"disposal":(fields[0]>>2)&7});
                        let mut block = pending.clone();
                        block["kind"] = json!("gce");
                        block["offset"] = json!(at);
                        blocks.push(block);
                    } else {
                        require([1, 0xfe, 0xff].contains(&label), "extension-label")?;
                        if label != 0xfe {
                            let size = usize::from(r.byte("truncated-container")?);
                            require(size == if label == 1 { 12 } else { 11 }, "extension-size")?;
                            r.take(size, "truncated-container")?;
                        }
                        let data = r.blocks()?;
                        blocks.push(json!({"kind":match label {1=>"plain-text",0xfe=>"comment",_=>"application"},
                            "offset":at,"payload_bytes":data.len(),"consumed_gce":if label == 1 {pending.clone()} else {Value::Null}}));
                        if label == 1 {
                            pending = Value::Null;
                        }
                    }
                }
                0x2c => {
                    let desc = r.take(9, "truncated-container")?;
                    let (x, y, w, h, flags) = (
                        short(desc),
                        short(&desc[2..]),
                        short(&desc[4..]),
                        short(&desc[6..]),
                        desc[8],
                    );
                    require(
                        w > 0 && h > 0 && x + w <= width && y + h <= height,
                        "out-of-bounds",
                    )?;
                    let palette = if flags & 128 != 0 {
                        r.take(3 * (2 << (flags & 7)), "truncated-palette")?
                            .to_vec()
                    } else {
                        global.clone()
                    };
                    let minimum = r.at;
                    require(r.byte("truncated-container")? == 2, "minimum-code-size")?;
                    let data = r.blocks()?;
                    let indices = literals(&data, w * h, palette.len() / 3)?;
                    frames.push(json!({"offset":at,"end_offset":r.at,"minimum_offset":minimum,"width":w,"height":h,
                        "palette_sha256":hash(&palette),"indices_base64":encode(&indices),"gce":pending,
                        "effective_transparent":pending["transparent"]}));
                    blocks.push(json!({"kind":"image","offset":at}));
                    pending = Value::Null;
                }
                _ => return Err("invalid-end"),
            }
        }
    })();
    let mut value = json!({"valid":result.is_ok(),"completed_images":frames.len(),"frames":frames,"blocks":blocks,"policy":policy});
    if let Err(error) = result {
        value["error"] = json!(error);
        value["offset"] = json!(r.at);
    }
    value
}

fn pack(codes: &[u8]) -> Vec<u8> {
    // Accumulator packing contrasts with the bit-at-a-time reader above.
    let (mut acc, mut bits) = (0_u32, 0);
    let mut out = Vec::new();
    for &code in codes {
        assert!(code < 8);
        acc |= u32::from(code) << bits;
        bits += 3;
        while bits >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
    if bits != 0 {
        out.push(acc as u8);
    }
    out
}
fn subblocks(data: &[u8]) -> Vec<u8> {
    assert!(data.len() < 256);
    [vec![data.len() as u8], data.to_vec(), vec![0]].concat()
}
fn gce(index: Option<u8>, delay: u16, user: bool) -> Vec<u8> {
    vec![
        0x21,
        0xf9,
        4,
        4 | u8::from(index.is_some()) | (u8::from(user) << 1),
        delay as u8,
        (delay >> 8) as u8,
        index.unwrap_or(0),
        0,
    ]
}
fn palette(pattern: usize) -> Vec<u8> {
    match pattern {
        1 => vec![23, 91, 167, 229, 47, 83],
        2 => vec![181, 113, 29, 41, 197, 139],
        _ => unreachable!(),
    }
}
fn indices(pattern: usize) -> Vec<u8> {
    (0..63)
        .map(|i| {
            if pattern == 1 {
                ((i % 9 + i / 9) % 2) as u8
            } else {
                ((i % 9 * 3 + i / 9 * 5 + i / 3) % 2) as u8
            }
        })
        .collect()
}
fn pixels(indices: &[u8], palette: &[u8], transparent: Option<u8>) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|&index| {
            if Some(index) == transparent {
                [0; 4]
            } else {
                let at = usize::from(index) * 3;
                [palette[at], palette[at + 1], palette[at + 2], 255]
            }
        })
        .collect()
}
fn image(indices: &[u8], palette: &[u8], local: bool, w: u8, h: u8, eoi: bool) -> Vec<u8> {
    let mut out = vec![0x2c, 0, 0, 0, 0, w, 0, h, 0, if local { 128 } else { 0 }];
    if local {
        out.extend(palette);
    }
    let mut codes: Vec<_> = indices.iter().flat_map(|&i| [4, i]).collect();
    if eoi {
        codes.push(5);
    }
    out.push(2);
    out.extend(subblocks(&pack(&codes)));
    out
}
#[derive(Clone, Copy)]
struct Vector<'a> {
    scope: &'a str,
    signal: &'a str,
    pattern: usize,
    later: bool,
}
impl Vector<'_> {
    fn cell(self) -> String {
        format!(
            "{}-{}-{}",
            if self.later { "two-frame" } else { "static" },
            self.scope,
            self.signal
        )
    }
    fn id(self) -> String {
        format!("{}-pattern{}", self.cell(), self.pattern)
    }
    fn transparency(self, later: bool) -> Option<u8> {
        if self.later {
            return if (later && self.signal == "later-transparent")
                || (!later && self.signal == "first-transparent-later-no-gce")
            {
                Some(0)
            } else {
                None
            };
        }
        match self.signal {
            "transparent"
            | "transparent-comment"
            | "transparent-application"
            | "duplicate-opaque-transparent"
            | "user-input-long-delay" => Some(0),
            "duplicate-index0-index1" => Some(1),
            _ => None,
        }
    }
    fn global(self) -> Vec<u8> {
        if self.scope == "global" {
            palette(self.pattern)
        } else {
            vec![7, 19, 31, 241, 223, 199]
        }
    }
    fn wire(self) -> Vec<u8> {
        let p = palette(self.pattern);
        let i = indices(self.pattern);
        let t = gce(Some(0), 0, false);
        let o = gce(None, 0, false);
        let pt = vec![0x21, 1, 12, 0, 0, 0, 0, 9, 0, 7, 0, 1, 1, 1, 0, 0];
        let mut out = b"GIF89a\x09\x00\x07\x00\xf0\x00\x00".to_vec();
        out.extend(self.global());
        if self.later {
            out.extend(gce(self.transparency(false), 65535, false));
        } else {
            out.extend(match self.signal {
                "no-gce" | "trailing-gce" => vec![],
                "transparent" => t,
                "opaque" => o,
                "transparent-comment" => [t, vec![0x21, 0xfe, 3, 71, 67, 69, 0]].concat(),
                "transparent-application" => [
                    t,
                    b"\x21\xff\x0bGCEPROBE001\x02\x21\xf9\x01\x2c\x00".to_vec(),
                ]
                .concat(),
                "transparent-plain-text" => [t, pt].concat(),
                "transparent-plain-text-opaque" => [t, pt, o].concat(),
                "duplicate-transparent-opaque" => [t, o].concat(),
                "duplicate-opaque-transparent" => [o, t].concat(),
                "duplicate-index0-index1" => [t, gce(Some(1), 0, false)].concat(),
                "user-input-long-delay" => gce(Some(0), 65535, true),
                _ => unreachable!(),
            });
        }
        out.extend(image(&i, &p, self.scope == "local", 9, 7, true));
        if self.later {
            match self.signal {
                "later-transparent" => out.extend(gce(Some(0), 0, false)),
                "later-opaque" => out.extend(gce(None, 0, false)),
                _ => (),
            }
            out.extend(image(
                &i.iter().map(|v| 1 - v).collect::<Vec<_>>(),
                &p,
                self.scope == "local",
                9,
                7,
                true,
            ));
        } else if self.signal == "trailing-gce" {
            out.extend(gce(Some(0), 0, false));
        }
        out.push(0x3b);
        out
    }
}
fn scene(encoded: &Value) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,
        "width":9,"height":7,"angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],
        "seed":1,"version":1,"versionNonce":1,"updated":1,"isDeleted":false,"groupIds":[],"frameId":null,
        "boundElements":null,"link":null,"locked":false,"strokeColor":"#000000","backgroundColor":"transparent",
        "fillStyle":"solid","strokeWidth":1,"strokeStyle":"solid","roughness":0,"index":"a0","crop":null,"roundness":null}],
        "appState":{},"files":{"asset":{"id":"asset","mimeType":"image/gif","created":1,
        "dataURL":format!("data:image/gif;base64,{}",encoded.as_str().unwrap())}}})
}
fn binding(row: &Value, bytes: &[u8]) {
    assert_eq!(decode(&row["gif_base64"]), bytes);
    assert_eq!(row["gif_sha256"], hash(bytes));
    assert_eq!(row["gif_bytes"], bytes.len());
    assert_eq!(row["audit"], inspect(bytes));
}
fn qualify_row(record: &Value, v: Vector<'_>) {
    let row = by_id(record, "cases", &v.id());
    assert_eq!(row["cell"], v.cell());
    assert_eq!(row["signal"], v.signal);
    assert_eq!(row["group"], if v.later { "two-frame" } else { "static" });
    assert_eq!(row["palette_scope"], v.scope);
    assert_eq!(row["pattern"], v.pattern);
    assert_eq!((n(&row["width"]), n(&row["height"])), (9, 7));
    assert_eq!(row["image_count"], if v.later { 2 } else { 1 });
    assert_eq!(
        row["first_delay_cs"],
        if v.later || v.signal == "user-input-long-delay" {
            65535
        } else {
            0
        }
    );
    assert_eq!(row["eligibility"], "collected-well-framed-probe");
    let policy = if v.signal.starts_with("duplicate-") {
        json!(["duplicate-gce"])
    } else if v.signal == "trailing-gce" {
        json!(["orphan-gce"])
    } else {
        json!([])
    };
    assert_eq!(row["scope_policy"], policy);
    assert_eq!(
        row["scope_status"],
        if policy == json!([]) {
            "spec-valid"
        } else {
            "well-framed-nonconforming-scope-probe"
        }
    );
    assert_eq!(row["expectation_basis"],"GIF89a scope hypothesis; duplicate GCE uses complete last-GCE replacement (non-normative); transparent initial canvas");
    let p = palette(v.pattern);
    let triples = |p: &[u8]| json!(p.chunks_exact(3).collect::<Vec<_>>());
    assert_eq!(row["palette"], triples(&p));
    assert_eq!(row["global_palette"], triples(&v.global()));
    let bytes = v.wire();
    binding(row, &bytes);
    assert_eq!(row["audit"]["valid"], true);
    assert_eq!(row["audit"]["policy"], policy);
    let first_indices = indices(v.pattern);
    assert_eq!(
        first_indices.iter().filter(|&&i| i == 0).count(),
        if v.pattern == 1 { 32 } else { 42 }
    );
    let first = pixels(&first_indices, &p, v.transparency(false));
    let mut display = first.clone();
    let mut frames = Vec::new();
    for frame in 0..if v.later { 2 } else { 1 } {
        let i: Vec<_> = first_indices
            .iter()
            .map(|&i| if frame == 0 { i } else { 1 - i })
            .collect();
        let transparent = v.transparency(frame == 1);
        let patch = pixels(&i, &p, transparent);
        frames.push(json!({"indices_base64":encode(&i),"indices_sha256":hash(&i),"transparent_index":transparent,"expected_patch":rgba_record(&patch)}));
        let audited = &row["audit"]["frames"][frame];
        assert_eq!(audited["indices_base64"], encode(&i));
        assert_eq!(audited["palette_sha256"], hash(&p));
        assert_eq!(audited["effective_transparent"], json!(transparent));
        if frame == 1 {
            for (dst, src) in display.chunks_exact_mut(4).zip(patch.chunks_exact(4)) {
                if src[3] != 0 {
                    dst.copy_from_slice(src);
                }
            }
        }
    }
    assert_eq!(row["frames"], json!(frames));
    assert_eq!(row["expected_first"], rgba_record(&first));
    assert_eq!(
        row["expected_later_display"],
        if v.later {
            assert_ne!(display, first);
            rgba_record(&display)
        } else {
            Value::Null
        }
    );
    assert_eq!(row["scene"], scene(&row["gif_base64"]));
    // Independent selection of the browser sibling, never rewriting expected_first.
    let differs = v.signal == "transparent-plain-text";
    let actual = if differs {
        let sibling = by_id(
            record,
            "cases",
            &Vector {
                signal: "transparent",
                ..v
            }
            .id(),
        );
        let expected = pixels(&first_indices, &p, Some(0));
        assert_eq!(decode(&sibling["expected_first"]["rgba_base64"]), expected);
        assert_ne!(expected, first);
        expected
    } else {
        first.clone()
    };
    let reference = &row["reference"];
    assert_eq!(reference["admitted"], true);
    assert_eq!(reference["capture_under_30_seconds"], true);
    assert_eq!((n(&reference["width"]), n(&reference["height"])), (9, 7));
    svg_check(reference["svg"].as_str().unwrap(), Some(&bytes));
    let changed = first
        .chunks_exact(4)
        .zip(actual.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        changed,
        if differs {
            if v.pattern == 1 {
                32
            } else {
                42
            }
        } else {
            0
        }
    );
    assert_eq!(reference["samples"].as_object().unwrap().len(), 3);
    let mut surfaces = json!({});
    for surface in ["intrinsic", "png", "svg"] {
        let sample = &reference["samples"][surface];
        assert_eq!((n(&sample["width"]), n(&sample["height"])), (9, 7));
        assert_eq!(decode(&sample["rgba_base64"]), actual);
        png_pixels(&decode(&sample["png"]), &actual, false);
        surfaces[surface] = json!({"dimensions_match":true,"changed_pixels":changed,"rgba_sha256":hash(&actual),
            "matches_first":!differs,"matches_hypothesized_later_display":if v.later {json!(false)} else {Value::Null}});
    }
    assert_eq!(
        row["comparison"],
        json!({"admitted":true,"all_match_authored":!differs,"surfaces":surfaces})
    );
    for key in ["png", "svg_png"] {
        png_pixels(&decode(&reference[key]), &actual, false);
    }
}

fn qualify_controls(record: &Value) {
    let base = by_id(record, "cases", BASE);
    let original = decode(&base["gif_base64"]);
    let audit = inspect(&original);
    let mut ids = BTreeSet::new();
    for (mutation, error) in MALFORMED {
        for target in 0..2 {
            let id = format!("{}-{mutation}", if target == 0 { "early" } else { "late" });
            assert!(ids.insert(id.clone()));
            let row = by_id(record, "controls", &id);
            let frame = &audit["frames"][target];
            let (at, min, end) = (
                n(&frame["offset"]),
                n(&frame["minimum_offset"]),
                n(&frame["end_offset"]),
            );
            let mut bytes = original.clone();
            match mutation {
                "bad-palette-index" | "undefined-lzw" => {
                    bytes[min + 2] = (bytes[min + 2] & !56)
                        | (if mutation == "bad-palette-index" {
                            2
                        } else {
                            7
                        } << 3)
                }
                "truncated-gce" => bytes.truncate(at - 3),
                "out-of-bounds" => bytes[at + 1..at + 3].copy_from_slice(&[1, 0]),
                "truncated-subblocks" => bytes.truncate(min + 5),
                "invalid-end" => bytes[at] = 0x7f,
                "minimum-code-size-1" => bytes[min] = 1,
                "early-eoi" | "missing-eoi" => {
                    let codes: &[u8] = if mutation == "early-eoi" {
                        &[4, 5]
                    } else {
                        &[4, 0, 4, 0, 4, 0, 4, 0]
                    };
                    bytes.splice(min..end, [vec![2], subblocks(&pack(codes))].concat());
                }
                "truncated-palette" => bytes.truncate(at + 12),
                _ => unreachable!(),
            }
            binding(row, &bytes);
            assert_eq!(row["base_id"], BASE);
            assert_eq!(row["base_sha256"], hash(&original));
            assert_eq!(row["target_frame"], target);
            assert_eq!(row["mutation"], mutation);
            assert_eq!(row["first_frame_complete"], target == 1);
            let changed = bytes
                .iter()
                .zip(&original)
                .position(|(a, b)| a != b)
                .unwrap_or(bytes.len().min(original.len()));
            assert_eq!(row["first_changed_or_missing_offset"], changed);
            let prefix = if target == 1 {
                n(&audit["frames"][0]["end_offset"])
            } else {
                0
            };
            assert_eq!(&bytes[..prefix], &original[..prefix]);
            assert_eq!(row["preserved_prefix_bytes"], prefix);
            assert_eq!(row["preserved_prefix_sha256"], hash(&bytes[..prefix]));
            assert_eq!(row["audit"]["valid"], false);
            assert_eq!(row["audit"]["error"], error);
            assert_eq!(row["audit"]["completed_images"], target);
            assert_eq!(
                row["audit"]["frames"],
                json!(audit["frames"].as_array().unwrap()[..target])
            );
            assert_eq!(row["collected"], false);
            assert!(row.get("reference").is_none() && row.get("comparison").is_none());
            assert_eq!(
                row["eligibility"],
                "uncollected-malformed-validation-control"
            );
            assert_eq!(row["intended_strict"], "reject-entire-gif");
            assert_eq!(row["intended_recovery"], "image-local-recovery");
            assert_eq!(
                row["expectation_status"],
                "authored bounded JS audit; independent decoder binding still required"
            );
        }
    }
    assert_eq!(record["controls"].as_array().unwrap().len(), 20);
    assert_eq!(
        record["controls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        ids
    );
}

fn evidence() -> Value {
    let bytes = include_bytes!("../../../../tests/oracle/gif-control/reference.json");
    assert_eq!(bytes.len(), 667_351);
    assert_eq!(
        hash(bytes),
        "46e948db20e9bf75abeae48990fb0e4e7eece1e14d51f1aa24bc34436957c387"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/gif-control-reference/v1"
    );
    assert_eq!(record["sources"].as_array().unwrap().len(), 18);
    for (source, path) in record["sources"].as_array().unwrap().iter().zip(SOURCES) {
        assert_eq!(source["path"], path);
        assert_eq!(
            source["sha256"],
            hash(&std::fs::read(crate::common::repository_root().join(path)).unwrap())
        );
    }
    for (bytes, digest) in [
        (
            include_bytes!("png_profile_contract.rs").as_slice(),
            "44a5b9b4fba24704191b32f53008467605e6afa67f3fad02a3937ea53aba64c5",
        ),
        (
            include_bytes!("../common/pdf_inspect.rs").as_slice(),
            "2e46ed33e8decbb629d47dd1a45f43781c102a4ceba5990620b33edc25bab074",
        ),
        (
            include_bytes!("../common/raster.rs").as_slice(),
            "4f6bc5a85213b1268b8536c242687d58c3cdb81423c6643b6cd1c44e9d06d303",
        ),
    ] {
        assert_eq!(hash(bytes), digest);
    }
    let collection = &record["collection"];
    assert_eq!(collection["renderer"], "official v0.18.1");
    assert_eq!(collection["chromium"], "149.0.7827.55");
    assert_eq!(collection["profile_sha256"], record["sources"][2]["sha256"]);
    assert_eq!(
        collection["oci"],
        "sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
    );
    assert_eq!(collection["network"], "none");
    assert_eq!(collection["context"], "fresh per vector per pass");
    assert_eq!(
        collection["passes"],
        json!(["fresh-process-forward", "fresh-process-reverse"])
    );
    assert_eq!(
        record["options"],
        json!({"exportBackground":false,"exportEmbedScene":false,"exportPadding":0,"exportScale":1,
        "exportWithDarkMode":false,"frameRendering":{"enabled":true,"clip":true,"name":true,"outline":true},"maxWidthOrHeight":null,"viewBackgroundColor":"#ffffff"})
    );
    assert_eq!(
        record["matrix"],
        json!({"static_cells":24,"static_vectors":48,"signals":SIGNALS,"palette_scopes":["global","local"],
        "palette_entries":2,"patterns_per_cell":2,"logical_screen":[9,7],"two_frame_cells":8,"two_frame_vectors":16,
        "collected_vectors":64,"spec_valid_vectors":48,"nonconforming_scope_probes":16,"uncollected_malformed_controls":20,"total_vectors":84,"distinct_gif_bindings":84})
    );
    let mut ids = BTreeSet::new();
    let mut cells = BTreeMap::new();
    for later in [false, true] {
        for scope in ["global", "local"] {
            for signal in if later {
                LATER.as_slice()
            } else {
                SIGNALS.as_slice()
            } {
                for pattern in [1, 2] {
                    let v = Vector {
                        scope,
                        signal,
                        pattern,
                        later,
                    };
                    qualify_row(&record, v);
                    assert!(ids.insert(v.id()));
                    *cells.entry(v.cell()).or_insert(0) += 1;
                }
            }
        }
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 64);
    assert_eq!(cells.len(), 32);
    assert!(cells.values().all(|&n| n == 2));
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        ids
    );
    let differences: Vec<_> = rows.iter().filter(|r| r["signal"] == "transparent-plain-text").map(|r|
        json!({"id":r["id"],"signal":r["signal"],"scope_status":r["scope_status"],"comparison":r["comparison"]})).collect();
    assert_eq!(differences.len(), 4);
    assert_eq!(record["differences"], json!(differences)); // 4 vectors x 3 = 12 facts
    let mut groups = Vec::new();
    for scope in ["global", "local"] {
        for pattern in [1, 2] {
            let trio: Vec<_> = LATER[..3]
                .iter()
                .map(|signal| {
                    by_id(
                        &record,
                        "cases",
                        &Vector {
                            scope,
                            signal,
                            pattern,
                            later: true,
                        }
                        .id(),
                    )
                })
                .collect();
            let len = n(&trio[0]["audit"]["frames"][0]["end_offset"]);
            let prefix = decode(&trio[0]["gif_base64"])[..len].to_vec();
            for row in &trio {
                assert_eq!(&decode(&row["gif_base64"])[..len], prefix);
            }
            assert_ne!(
                trio[0]["expected_later_display"],
                trio[1]["expected_later_display"]
            );
            groups.push(json!({"ids":trio.iter().map(|r| &r["id"]).collect::<Vec<_>>(),"identical_first_prefix_bytes":len,
            "identical_first_prefix_sha256":hash(&prefix),"first_rgba_sha256":trio[0]["expected_first"]["rgba_sha256"],
            "later_display_sha256":trio.iter().map(|r| &r["expected_later_display"]["rgba_sha256"]).collect::<Vec<_>>()}));
        }
    }
    assert_eq!(record["timer_groups"], json!(groups));
    qualify_controls(&record);
    let hashes: BTreeSet<_> = rows
        .iter()
        .chain(record["controls"].as_array().unwrap())
        .map(|r| r["gif_sha256"].as_str().unwrap())
        .collect();
    assert_eq!(hashes.len(), 84);
    negative_helpers(&record);
    ordered_reports(&record);
    explicit_late_and_eoi_controls();
    record
}

fn svg_check(svg: &str, source: Option<&[u8]>) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let root = doc.root_element();
    assert!(root.has_tag_name("svg"));
    assert_eq!(root.attribute("width"), Some("9"));
    assert_eq!(root.attribute("height"), Some("7"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 9 7"));
    let images: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), usize::from(source.is_some()));
    if let Some(source) = source {
        let image = images[0];
        let href = image
            .attribute("href")
            .or_else(|| image.attribute(("http://www.w3.org/1999/xlink", "href")))
            .unwrap();
        assert_eq!(
            href,
            format!("data:image/gif;base64,{}", encode(source).as_str().unwrap())
        );
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
fn canonical(samples: &[u8]) {
    assert_eq!(samples.len(), 9 * 7 * 4);
    assert!(samples
        .chunks_exact(4)
        .all(|p| matches!(p[3], 0 | 255) && (p[3] != 0 || p[..3] == [0; 3])));
}
fn png_pixels(bytes: &[u8], expected: &[u8], native: bool) {
    if native {
        super::png_profile_contract::assert_untagged_png(bytes);
        assert_eq!(&bytes[24..26], &[8, 6]);
    }
    let actual = crate::common::raster::decode_png(bytes);
    assert_eq!((actual.width, actual.height), (9, 7));
    assert_eq!(actual.rgba, expected);
    canonical(&actual.rgba);
}
fn pdf_extent(bytes: &[u8]) {
    let text = crate::common::pdf_inspect::pdf_text(bytes);
    let pages: Vec<_> = text.split("/Type /Page\n").skip(1).collect();
    assert_eq!(pages.len(), 1);
    let page = pages[0].split("endobj").next().unwrap();
    for key in ["/MediaBox [", "/CropBox ["] {
        let values: Vec<f64> = page
            .split_once(key)
            .unwrap()
            .1
            .split_once(']')
            .unwrap()
            .0
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(values, [0.0, 0.0, 6.75, 5.25]);
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}
fn artifact(bytes: &[u8], format: OutputFormat, source: &[u8], expected: &[u8]) {
    match format {
        OutputFormat::Svg => svg_check(std::str::from_utf8(bytes).unwrap(), Some(source)),
        OutputFormat::Png => png_pixels(bytes, expected, true),
        OutputFormat::Pdf => {
            pdf_extent(bytes);
            let actual = super::png_profile_contract::pdf_rgba(bytes, 9, 7);
            assert_eq!(actual, expected);
            canonical(&actual);
        }
        _ => unreachable!(),
    }
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
    assert_eq!(
        first.artifact().bytes(),
        again.artifact().bytes(),
        "fresh artifact replay"
    );
    assert_eq!(
        serde_json::to_value(first.report()).unwrap(),
        serde_json::to_value(again.report()).unwrap(),
        "fresh report replay"
    );
    let mut report = serde_json::to_value(first.report()).unwrap();
    for d in report["diagnostics"].as_array_mut().unwrap() {
        assert!(!d["message"].as_str().unwrap().is_empty());
        d.as_object_mut().unwrap().remove("message");
    }
    (first.artifact().bytes().to_vec(), report)
}
fn report(format: OutputFormat, malformed: bool, animated: bool) -> Value {
    let diagnostics = if malformed {
        vec![
            json!({"code":"image.decode-failed","severity":"warning","format":format.as_str(),"effects":["visual"],"element_id":"image",
            "json_pointer":"/files/asset/dataURL","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/gif",
            "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"},
            "recipe_id":if format == OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}}),
        ]
    } else if animated && format != OutputFormat::Svg {
        vec![
            json!({"code":"image.animation-flattened","severity":"info","format":format.as_str(),"effects":["visual"],"element_id":"image",
            "json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/gif","recovery":"flattened","recipe_id":null}}),
        ]
    } else {
        vec![]
    };
    json!({"schema_version":1,"diagnostics":diagnostics})
}
fn recovery(bytes: &[u8], format: OutputFormat, source: &Value) {
    let mut missing = source.clone();
    missing["files"] = json!({});
    assert_eq!(
        bytes,
        render(&missing, format).0,
        "exact missing-asset recovery artifact"
    );
    match format {
        OutputFormat::Svg => svg_check(std::str::from_utf8(bytes).unwrap(), None),
        OutputFormat::Png => {
            let actual = crate::common::raster::decode_png(bytes);
            assert_eq!((actual.width, actual.height), (9, 7));
            // The exact missing-asset comparison above binds the official
            // placeholder recipe, whose PNG metadata differs from decoded GIFs.
        }
        OutputFormat::Pdf => {
            pdf_extent(bytes);
            assert!(!crate::common::pdf_inspect::pdf_text(bytes).contains("/Subtype /Image"));
        }
        _ => unreachable!(),
    }
}
fn rejects(check: impl FnOnce()) {
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)).is_err(),
        "negative helper accepted mutation"
    );
}
fn replace_once(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let at = bytes
        .windows(from.len())
        .position(|w| w == from.as_bytes())
        .expect("mutation target");
    let mut out = bytes.to_vec();
    out.splice(at..at + from.len(), to.bytes());
    out
}
fn negative_helpers(record: &Value) {
    let row = by_id(record, "cases", "static-local-transparent-pattern1");
    let source = decode(&row["gif_base64"]);
    let expected = decode(&row["expected_first"]["rgba_base64"]);
    let other = by_id(record, "cases", "static-local-transparent-pattern2");
    assert_ne!(
        row["frames"][0]["indices_sha256"],
        other["frames"][0]["indices_sha256"]
    );
    let audit = inspect(&source);
    let min = n(&audit["frames"][0]["minimum_offset"]);
    let mut wire = source.clone();
    wire[min + 2] ^= 8;
    assert_ne!(
        inspect(&wire)["frames"][0]["indices_base64"],
        audit["frames"][0]["indices_base64"]
    );
    let mut wire = source.clone();
    wire[min - 6] ^= 1;
    assert_ne!(
        inspect(&wire)["frames"][0]["palette_sha256"],
        audit["frames"][0]["palette_sha256"]
    );
    let mut wire = source.clone();
    wire[22] &= !1;
    assert_ne!(
        inspect(&wire)["frames"][0]["effective_transparent"],
        audit["frames"][0]["effective_transparent"]
    );
    let mut wire = source.clone();
    wire[n(&audit["frames"][0]["offset"]) + 1] = 1;
    assert_eq!(inspect(&wire)["error"], "out-of-bounds");
    assert_eq!(
        literals(&[0x04, 0x41, 0x10], 4, 2).unwrap_err(),
        "missing-eoi"
    );
    assert_eq!(literals(&[0x2c], 1, 2).unwrap_err(), "pixel-count");
    assert_eq!(literals(&[0x44, 0x81], 1, 2).unwrap_err(), "lzw-padding");
    for format in FORMATS {
        let (bytes, actual_report) = render(&row["scene"], format);
        artifact(&bytes, format, &source, &expected);
        assert_eq!(actual_report, report(format, false, false));
        let mut wrong = actual_report.clone();
        wrong["diagnostics"] = json!([{"code":"image.animation-flattened"}]);
        rejects(|| assert_eq!(wrong, report(format, false, false)));
    }
    let (svg, _) = render(&row["scene"], OutputFormat::Svg);
    for (from, to) in [
        ("viewBox=\"0 0 9 7\"", "viewBox=\"0 0 8 7\""),
        ("width=\"9\"", "width=\"8\""),
    ] {
        rejects(|| {
            artifact(
                &replace_once(&svg, from, to),
                OutputFormat::Svg,
                &source,
                &expected,
            )
        });
    }
    rejects(|| {
        artifact(
            &svg,
            OutputFormat::Svg,
            &decode(&other["gif_base64"]),
            &expected,
        )
    });
    let (png, _) = render(&row["scene"], OutputFormat::Png);
    let mut tagged = png.clone();
    let mut chunk = 1_u32.to_be_bytes().to_vec();
    chunk.extend(b"sRGB\0");
    chunk.extend(crc32fast::hash(&chunk[4..]).to_be_bytes());
    tagged.splice(33..33, chunk);
    rejects(|| png_pixels(&tagged, &expected, true));
    for mutation in ["rgb", "palette", "alpha", "zero-rgb", "extent"] {
        let mut samples = expected.clone();
        let color = if mutation == "rgb" {
            png::ColorType::Rgb
        } else {
            png::ColorType::Rgba
        };
        match mutation {
            "rgb" => {
                samples = samples
                    .chunks_exact(4)
                    .flat_map(|p| p[..3].iter().copied())
                    .collect()
            }
            "palette" => samples[4] ^= 1,
            "alpha" => samples[7] = 254,
            "zero-rgb" => samples[0] = 1,
            "extent" => (),
            _ => unreachable!(),
        }
        let mut changed = Vec::new();
        {
            let (w, h) = if mutation == "extent" { (7, 9) } else { (9, 7) };
            let mut encoder = png::Encoder::new(&mut changed, w, h);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&samples)
                .unwrap();
        }
        rejects(|| png_pixels(&changed, &expected, true));
        if ["alpha", "zero-rgb"].contains(&mutation) {
            rejects(|| png_pixels(&changed, &samples, true));
        }
    }
    let (pdf, _) = render(&row["scene"], OutputFormat::Pdf);
    let inflated = crate::common::pdf_inspect::inflate_streams(&pdf);
    for (from, to) in [
        ("/SMask ", "/Unused "),
        ("/Im0 Do", "/Im9 Do"),
        ("/ICCBased ", "/DeviceRGB "),
        ("/OutputIntents ", "/UnusedIntents "),
        ("/DeviceGray", "/DeviceRGB"),
        ("/Width 9\n", "/Width 8\n"),
        ("/MediaBox [0 0 6.75 5.25]", "/MediaBox [0 0 6.75 5.26]"),
        ("/CropBox [0 0 6.75 5.25]", "/CropBox [0 0 6.75 5.26]"),
    ] {
        rejects(|| {
            artifact(
                &replace_once(&inflated, from, to),
                OutputFormat::Pdf,
                &source,
                &expected,
            )
        });
    }
    let text = crate::common::pdf_inspect::pdf_text(&pdf);
    let reference = |key| {
        text.split_once(key)
            .unwrap()
            .1
            .split_once(" 0 R")
            .unwrap()
            .0
    };
    let detached = replace_once(
        &inflated,
        &format!("/Im0 {} 0 R", reference("/Im0 ")),
        &format!("/Im0 {} 0 R", reference("/ICCBased ")),
    );
    rejects(|| artifact(&detached, OutputFormat::Pdf, &source, &expected));
    rejects(|| {
        artifact(
            &pdf,
            OutputFormat::Pdf,
            &source,
            &decode(&other["expected_first"]["rgba_base64"]),
        )
    });
    // Replace the bound alpha plane with a correctly framed mask containing
    // one changed sample. Check that the helper follows that actual binding.
    let mut masked = replace_once(
        &inflated,
        &format!("/SMask {} 0 R", reference("/SMask ")),
        "/SMask 99999 0 R",
    );
    masked.extend(b"\n99999 0 obj\n<< /Type /XObject /Subtype /Image /Width 9 /Height 7 /BitsPerComponent 8 /ColorSpace /DeviceGray /Length 63 >>\nstream\n");
    let mut alpha: Vec<_> = expected.chunks_exact(4).map(|p| p[3]).collect();
    alpha[1] = 254;
    masked.extend(alpha);
    masked.extend(b"\nendstream\nendobj\n");
    let samples = super::png_profile_contract::pdf_rgba(&masked, 9, 7);
    assert_eq!(samples[7], 254);
    assert_eq!(&samples[..7], &expected[..7]);
    assert_eq!(&samples[8..], &expected[8..]);
    rejects(|| artifact(&masked, OutputFormat::Pdf, &source, &expected));
}

fn ordered_reports(record: &Value) {
    for malformed in [false, true] {
        let row = if malformed {
            by_id(record, "controls", "early-undefined-lzw")
        } else {
            by_id(record, "cases", BASE)
        };
        let mut source = scene(&row["gif_base64"]);
        let element = source["elements"][0].clone();
        let ids = ["z-last-lexically", "a-first-lexically", "m-middle"];
        source["elements"] = json!(ids
            .iter()
            .map(|id| {
                let mut e = element.clone();
                e["id"] = json!(id);
                e
            })
            .collect::<Vec<_>>());
        if malformed {
            // Decode failures are asset-local and deduplicated. Distinct file
            // identities make ordering observable rather than assuming one
            // warning per use of a shared malformed asset.
            let file = source["files"]["asset"].clone();
            source["files"] = json!({});
            for (index, id) in ids.iter().enumerate() {
                source["elements"][index]["fileId"] = json!(id);
                source["files"][id] = file.clone();
                source["files"][id]["id"] = json!(id);
            }
        }
        for format in FORMATS {
            let (bytes, actual) = render(&source, format);
            let template = report(format, malformed, true);
            let mut expected = template.clone();
            // serde_json's object map serializes file owners lexically; array
            // element owners retain their deliberately nonlexical order.
            let mut order = ids;
            if malformed {
                order.sort();
            }
            expected["diagnostics"] = json!(order
                .iter()
                .enumerate()
                .flat_map(|(index, id)| {
                    template["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(move |d| {
                            let mut d = d.clone();
                            d["element_id"] = json!(id);
                            if !malformed {
                                d["json_pointer"] = json!(format!("/elements/{index}/fileId"));
                            } else {
                                d["json_pointer"] = json!(format!("/files/{id}/dataURL"));
                                d["details"]["asset_id"] = json!(id);
                            }
                            d
                        })
                })
                .collect::<Vec<_>>());
            assert_eq!(actual, expected, "ordered shared-asset report");
            if !expected["diagnostics"].as_array().unwrap().is_empty() {
                let mut reversed = expected.clone();
                reversed["diagnostics"].as_array_mut().unwrap().reverse();
                rejects(|| assert_eq!(actual, reversed));
            }
            if malformed {
                recovery(&bytes, format, &source);
            }
        }
    }
}

fn explicit_late_and_eoi_controls() {
    // Frozen missing-EOI controls emit only four of 63 pixels. These additional
    // parent-authored controls instead exactly fill a 4x1 descriptor; eight
    // 3-bit clear/literal codes end on a byte boundary. No frozen bytes change.
    // First/later variants hold the same 9x7 viewport and preserve a full first
    // frame for the late pair. Acceptance proves validation, NOT later display.
    let p = palette(1);
    let i = indices(1);
    assert_eq!(pack(&[4, 0, 4, 0, 4, 0, 4, 0]), [0x04, 0x41, 0x10]);
    assert_eq!(literals(&[0x04, 0x41, 0x10, 0x05], 4, 2).unwrap(), [0; 4]);
    let mut surfaces = 0;
    for late in [false, true] {
        let build = |eoi| {
            let mut bytes = b"GIF89a\x09\x00\x07\x00\xf0\x00\x00".to_vec();
            bytes.extend(&p);
            if late {
                bytes.extend(gce(None, 65535, false));
                bytes.extend(image(&i, &p, false, 9, 7, true));
            }
            bytes.extend(gce(None, 0, false));
            bytes.extend(image(&[0; 4], &p, true, 4, 1, eoi));
            bytes.push(0x3b);
            bytes
        };
        let valid = build(true);
        let invalid = build(false);
        let good = inspect(&valid);
        let bad = inspect(&invalid);
        assert_eq!(good["valid"], true);
        assert_eq!(good["completed_images"], if late { 2 } else { 1 });
        assert_eq!(bad["error"], "missing-eoi");
        assert_eq!(bad["completed_images"], usize::from(late));
        let target = &good["frames"][usize::from(late)];
        let min = n(&target["minimum_offset"]);
        assert_eq!(&valid[..min + 1], &invalid[..min + 1]);
        assert_eq!(&valid[min + 1..], &[4, 0x04, 0x41, 0x10, 0x05, 0, 0x3b]);
        assert_eq!(&invalid[min + 1..], &[3, 0x04, 0x41, 0x10, 0, 0x3b]);
        // Diagnostic only: permissive dependency confirms exact output count
        // with EOI checking disabled, including the complete late frame.
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::Indexed);
        options.check_frame_consistency(true);
        options.check_lzw_end_code(false);
        let mut decoder = options.read_info(std::io::Cursor::new(&invalid)).unwrap();
        if late {
            assert_eq!(
                decoder.read_next_frame().unwrap().unwrap().buffer.as_ref(),
                i
            );
        }
        let frame = decoder.read_next_frame().unwrap().unwrap();
        assert_eq!((frame.width, frame.height), (4, 1));
        assert_eq!(frame.buffer.as_ref(), &[0; 4]);
        assert!(decoder.read_next_frame().unwrap().is_none());
        let expected = if late {
            pixels(&i, &p, None)
        } else {
            let mut out = vec![0; 63 * 4];
            out[..16].copy_from_slice(&pixels(&[0; 4], &p, None));
            out
        };
        // All pixels precede the missing EOI, so the reference draws both
        // exactly like the valid file (gif-container missing-eoi-after-full).
        for (bytes, missing_eoi) in [(&valid, false), (&invalid, true)] {
            for format in FORMATS {
                let source = scene(&encode(bytes));
                let (artifact_bytes, actual_report) = render(&source, format);
                assert_eq!(
                    actual_report,
                    report(format, false, late),
                    "isolated EOI late={late} missing_eoi={missing_eoi} {format:?}"
                );
                {
                    artifact(&artifact_bytes, format, bytes, &expected);
                }
                surfaces += 1;
            }
        }
    }
    assert_eq!(surfaces, 12);
    eprintln!("GIF additional exact-pixel EOI controls: 2 valid + 2 malformed, 12/12 surfaces fresh-replayed; no later-composite claim");
}

#[test]
pub(super) fn controls_match_frozen_reference_first_display() {
    let record = evidence();
    let (mut vectors, mut surfaces) = (0, 0);
    let mut cells = BTreeMap::<String, bool>::new();
    let mut failures = Vec::new();
    for row in record["cases"].as_array().unwrap() {
        let source = decode(&row["gif_base64"]);
        let expected = decode(&row["reference"]["samples"]["intrinsic"]["rgba_base64"]);
        let mut passed = true;
        for format in FORMATS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (bytes, actual_report) = render(&row["scene"], format);
                assert_eq!(
                    actual_report,
                    report(format, false, n(&row["image_count"]) > 1)
                );
                artifact(&bytes, format, &source, &expected);
            }));
            if result.is_ok() {
                surfaces += 1;
            } else {
                passed = false;
                failures.push(format!("{}: {format:?}", row["id"]));
            }
        }
        vectors += usize::from(passed);
        *cells
            .entry(row["cell"].as_str().unwrap().to_owned())
            .or_insert(true) &= passed;
    }
    eprintln!("GIF controls: {}/32 cells, {vectors}/64 vectors, {surfaces}/192 surfaces fresh-replayed; includes 6 NON-NORMATIVE duplicate and 2 orphan characterization cells",cells.values().filter(|&&ok|ok).count());
    assert!(
        failures.is_empty(),
        "unqualified first-display surfaces: {failures:?}"
    );
}

#[test]
fn malformed_early_and_late_frames_have_exact_recovery() {
    let record = evidence();
    let (mut cells, mut surfaces) = (0, 0);
    let mut failures = Vec::new();
    // Explicit valid late-frame witnesses run even when only this owner is selected.
    for row in record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["group"] == "two-frame")
    {
        for format in FORMATS {
            let (bytes, actual_report) = render(&row["scene"], format);
            assert_eq!(actual_report, report(format, false, true));
            artifact(
                &bytes,
                format,
                &decode(&row["gif_base64"]),
                &decode(&row["reference"]["samples"]["intrinsic"]["rgba_base64"]),
            );
        }
    }
    // Only controls the pinned reference rejects, or whose first frame it
    // paints partially (the declared divergence), still recover; the others
    // draw like the reference (gif_container_contract).
    for row in record["controls"].as_array().unwrap().iter().filter(|row| {
        super::gif_container_contract::strictly_rejected("control", row["id"].as_str().unwrap())
    }) {
        let source = scene(&row["gif_base64"]);
        let mut passed = true;
        for format in FORMATS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (bytes, actual_report) = render(&source, format);
                assert_eq!(actual_report, report(format, true, false));
                recovery(&bytes, format, &source);
            }));
            if result.is_ok() {
                surfaces += 1;
            } else {
                passed = false;
                failures.push(format!("{}: {format:?}", row["id"]));
            }
        }
        cells += usize::from(passed);
    }
    eprintln!("GIF malformed: {cells}/7 strict cells, {surfaces}/21 surfaces fresh-replayed; 16 valid late witnesses/48 surfaces");
    assert_eq!(cells, 7);
    assert!(
        failures.is_empty(),
        "unqualified malformed surfaces: {failures:?}"
    );
}
