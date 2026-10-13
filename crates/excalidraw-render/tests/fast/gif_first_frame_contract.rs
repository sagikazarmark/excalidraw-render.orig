//! Bounded GIF89a first-frame qualification, not a GIF-family umbrella owner.
//! 120 cells / 240 browser vectors; 12 malformed and four uncollected resource
//! controls. Expectations come from coordinates/palettes and an independent,
//! bounded wire reader, never from the production GIF codec. PDF assertions
//! inspect the actual bound image/ICC/SMask, not a whole-page viewer raster.
use crate::common::lower_hex;
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const GLOBAL: [[u8; 3]; 4] = [[239, 31, 73], [19, 211, 97], [43, 67, 229], [251, 193, 17]];
const LOCAL: [[u8; 3]; 4] = [
    [13, 173, 241],
    [227, 53, 181],
    [113, 239, 29],
    [71, 11, 157],
];
const BASE: &str = "global-full-opaque-sequential-animated-d0-asymmetric";
const MALFORMED: [&str; 12] = [
    "bad-palette-index",
    "later-stream-invalid-code",
    "missing-trailer",
    "invalid-gce-size",
    "reserved-gce-bits",
    "reserved-image-bits",
    "reserved-disposal-4",
    "first-stream-invalid-code",
    "truncated-image-data",
    "missing-color-table",
    "zero-logical-width",
    "frame-outside-screen",
];
const RESOURCES: [&str; 4] = [
    "input-byte-budget",
    "logical-pixel-budget",
    "huge-logical-screen",
    "huge-image-descriptor",
];
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
    "tools/reference-oracle/run-gif-first-frame.sh",
    "tools/reference-oracle/scripts/canonical-json.mjs",
    "tools/reference-oracle/scripts/gif-first-frame-evidence.mjs",
    "tools/reference-oracle/scripts/gif-first-frame-fixtures.mjs",
    "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs",
    "tools/reference-oracle/scripts/profile.mjs",
    "tools/reference-oracle/scripts/smoke.mjs",
    "tools/reference-oracle/src/browser-entry.js",
    "tools/reference-oracle/vite.config.mjs",
];

fn hash(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}
fn decode(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
fn by_id<'a>(record: &'a Value, key: &str, id: &str) -> &'a Value {
    record[key]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap()
}
fn require(ok: bool, reason: &'static str) -> Result<(), &'static str> {
    if ok {
        Ok(())
    } else {
        Err(reason)
    }
}

#[derive(Clone, Copy)]
struct Vector<'a> {
    palette: &'a str,
    subrect: bool,
    transparent: bool,
    interlaced: bool,
    animation: &'a str,
    pattern: &'a str,
}
impl Vector<'_> {
    fn cell(self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.palette,
            if self.subrect { "subrect" } else { "full" },
            if self.transparent {
                "transparent"
            } else {
                "opaque"
            },
            if self.interlaced {
                "interlaced"
            } else {
                "sequential"
            },
            self.animation
        )
    }
    fn rect(self, later: bool) -> [usize; 4] {
        if self.subrect && !later {
            [2, 1, 5, 5]
        } else {
            [0, 0, 9, 7]
        }
    }
    fn disposal(self) -> u8 {
        match self.animation {
            "static" | "animated-d0" => 0,
            "animated-d1" => 1,
            "animated-d2" => 2,
            "animated-d3" => 3,
            _ => panic!("animation axis"),
        }
    }
    fn rows(self, later: bool) -> Vec<Vec<u8>> {
        let [_, _, w, h] = self.rect(later);
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        let n = match self.pattern {
                            "alternating" => x + 2 * y + usize::from(x == 1 && y == 2),
                            "asymmetric" => 3 * x + y + x / 3 + if y == 1 { 2 } else { 0 },
                            _ => panic!("pattern axis"),
                        };
                        ((n + usize::from(later)) % 4) as u8
                    })
                    .collect()
            })
            .collect()
    }
    fn rgba(self, later: bool) -> Vec<u8> {
        let palette = if self.palette == "global" {
            GLOBAL
        } else {
            LOCAL
        };
        let [left, top, w, h] = self.rect(later);
        let rows = self.rows(later);
        // Visit logical-screen coordinates, independently of the wire row walk.
        (0..63)
            .flat_map(|p| {
                let (x, y) = (p % 9, p / 9);
                if x < left || x >= left + w || y < top || y >= top + h {
                    return [0; 4];
                }
                let index = rows[y - top][x - left];
                if !later && self.transparent && index == 0 {
                    return [0; 4];
                }
                let [r, g, b] = palette[usize::from(index)];
                [r, g, b, 255]
            })
            .collect()
    }
    fn bytes(self) -> (Vec<u8>, Value) {
        let global = self.palette != "local";
        let local = self.palette != "global";
        let mut bytes = b"GIF89a\x09\x00\x07\x00".to_vec();
        bytes.extend([if global { 0x91 } else { 0x10 }, 3, 0]);
        if global {
            bytes.extend(GLOBAL.into_iter().flatten());
        }
        let mut offsets = json!({"first_gce":bytes.len()});
        for later in [false, true] {
            if later && self.animation == "static" {
                break;
            }
            bytes.extend([
                0x21,
                0xf9,
                4,
                if later {
                    4
                } else {
                    (self.disposal() << 2) | u8::from(self.transparent)
                },
            ]);
            bytes.extend(if later { 100_u16 } else { 65535 }.to_le_bytes());
            bytes.extend([0, 0]);
            if !later {
                offsets["first_descriptor"] = json!(bytes.len());
            }
            bytes.push(0x2c);
            for coordinate in self.rect(later) {
                bytes.extend((coordinate as u16).to_le_bytes());
            }
            bytes.push(if local { 0x81 } else { 0 } | if self.interlaced { 0x40 } else { 0 });
            if local {
                bytes.extend(LOCAL.into_iter().flatten());
            }
            offsets[if later {
                "later_stream"
            } else {
                "first_stream"
            }] = json!(bytes.len());
            let rows = self.rows(later);
            // A bit vector rather than the author's overlapping byte shifts.
            let mut bits = Vec::new();
            for y in wire_rows(rows.len(), self.interlaced) {
                for &index in &rows[y] {
                    for code in [4, index] {
                        for bit in 0..3 {
                            bits.push((code >> bit) & 1);
                        }
                    }
                }
            }
            bits.extend([1, 0, 1]); // EOI = 5
            let packed: Vec<u8> = bits
                .chunks(8)
                .map(|chunk| {
                    chunk
                        .iter()
                        .enumerate()
                        .fold(0, |byte, (bit, value)| byte | (value << bit))
                })
                .collect();
            bytes.push(2);
            for block in packed.chunks(17) {
                bytes.push(block.len() as u8);
                bytes.extend(block);
            }
            bytes.push(0);
        }
        if self.animation == "static" {
            offsets["later_stream"] = Value::Null;
        }
        bytes.push(0x3b);
        (bytes, offsets)
    }
}
fn wire_rows(height: usize, interlaced: bool) -> Vec<usize> {
    let mut rows: Vec<_> = (0..height).collect();
    if interlaced {
        // Sort display rows by their GIF pass, avoiding the author's pass loop.
        rows.sort_by_key(|y| {
            (
                if y % 8 == 0 {
                    0
                } else if y % 8 == 4 {
                    1
                } else if y % 2 == 0 {
                    2
                } else {
                    3
                },
                *y,
            )
        });
    }
    rows
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        let result = self.bytes.get(self.at..self.at + n).ok_or("truncated")?;
        self.at += n;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    fn short(&mut self) -> Result<usize, &'static str> {
        Ok(usize::from(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap(),
        )))
    }
    fn palette(&mut self, packed: u8) -> Result<Vec<[u8; 3]>, &'static str> {
        if packed & 128 == 0 {
            return Ok(Vec::new());
        }
        Ok(self
            .take(3 * (2 << (packed & 7)))?
            .as_chunks::<3>()
            .0
            .to_vec())
    }
}

// Small independent GIF LZW decoder: bounded dictionary, output and input. It
// supports dictionary codes too, so an invalid code cannot masquerade as a
// literal. No gif/image/resvg decoder supplies expected samples.
fn lzw(bytes: &[u8], count: usize) -> Result<Vec<u8>, &'static str> {
    require(count <= 63 && bytes.len() <= 128, "LZW bound")?;
    let mut dictionary: Vec<Vec<u8>> = (0..4).map(|n| vec![n]).collect();
    dictionary.extend([vec![], vec![]]);
    let mut previous: Option<Vec<u8>> = None;
    let mut width = 3;
    let mut bit = 0;
    let mut output = Vec::new();
    loop {
        require(bit + width <= bytes.len() * 8, "missing EOI")?;
        let code = (0..width).fold(0_usize, |code, i| {
            code | (usize::from((bytes[(bit + i) / 8] >> ((bit + i) % 8)) & 1) << i)
        });
        bit += width;
        if code == 4 {
            dictionary.truncate(6);
            width = 3;
            previous = None;
            continue;
        }
        if code == 5 {
            require(output.len() == count, "pixel count")?;
            require(
                (bit..bytes.len() * 8).all(|i| bytes[i / 8] & (1 << (i % 8)) == 0),
                "padding",
            )?;
            return Ok(output);
        }
        let entry = if let Some(entry) = dictionary.get(code) {
            entry.clone()
        } else if code == dictionary.len() {
            let mut entry = previous.clone().ok_or("undefined code")?;
            entry.push(entry[0]);
            entry
        } else {
            return Err("undefined code");
        };
        require(
            !entry.is_empty() && output.len() + entry.len() <= count,
            "pixel overflow",
        )?;
        output.extend(&entry);
        if let Some(mut prefix) = previous {
            prefix.push(entry[0]);
            require(dictionary.len() < 4096, "dictionary bound")?;
            dictionary.push(prefix);
            if dictionary.len() == 1 << width && width < 12 {
                width += 1;
            }
        }
        previous = Some(entry);
    }
}

fn inspect(bytes: &[u8]) -> Result<Vec<Vec<u8>>, &'static str> {
    let mut r = Reader { bytes, at: 0 };
    require(r.take(6)? == b"GIF89a", "signature")?;
    let (width, height) = (r.short()?, r.short()?);
    require(width > 0 && height > 0, "zero screen")?;
    require(width * height <= 63, "screen bound")?;
    let packed = r.byte()?;
    let background = usize::from(r.byte()?);
    require(r.byte()? == 0, "aspect")?;
    let global = r.palette(packed)?;
    require(
        global.is_empty() || background < global.len(),
        "background index",
    )?;
    let mut frames = Vec::new();
    while r.bytes.get(r.at) != Some(&0x3b) {
        require(frames.len() < 2, "frame bound")?;
        require(r.take(3)? == [0x21, 0xf9, 4], "GCE framing")?;
        let control = r.byte()?;
        require(control & 0xe0 == 0 && (control >> 2) & 7 <= 3, "GCE bits")?;
        r.short()?;
        let transparent = r.byte()?;
        require(r.byte()? == 0 && r.byte()? == 0x2c, "image marker")?;
        let (left, top, w, h) = (r.short()?, r.short()?, r.short()?, r.short()?);
        require(
            w > 0 && h > 0 && left + w <= width && top + h <= height,
            "frame bounds",
        )?;
        let packed = r.byte()?;
        require(packed & 0x18 == 0, "image reserved")?;
        let local = r.palette(packed)?;
        let palette = if local.is_empty() { &global } else { &local };
        require(!palette.is_empty(), "missing palette")?;
        require(
            control & 1 == 0 || usize::from(transparent) < palette.len(),
            "transparent index",
        )?;
        require(r.byte()? == 2, "minimum code size")?;
        let mut data = Vec::new();
        loop {
            let n = usize::from(r.byte()?);
            if n == 0 {
                break;
            }
            require(data.len() + n <= 128, "subblock bound")?;
            data.extend(r.take(n)?);
        }
        let indices = lzw(&data, w * h)?;
        let mut rgba = vec![0; width * height * 4];
        for (wire_y, y) in wire_rows(h, packed & 0x40 != 0).into_iter().enumerate() {
            for x in 0..w {
                let index = indices[wire_y * w + x];
                let rgb = palette.get(usize::from(index)).ok_or("palette index")?;
                if control & 1 != 0 && index == transparent {
                    continue;
                }
                let at = ((top + y) * width + left + x) * 4;
                rgba[at..at + 3].copy_from_slice(rgb);
                rgba[at + 3] = 255;
            }
        }
        frames.push(rgba);
    }
    require(
        !frames.is_empty() && r.byte()? == 0x3b && r.at == bytes.len(),
        "trailer",
    )?;
    Ok(frames)
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

fn evidence() -> Value {
    let bytes = include_bytes!("../../../../tests/oracle/gif-first-frame/reference.json");
    assert_eq!(
        hash(bytes),
        "732e65695419a9f30d105b2d54cd3da1a392b9f7d2e51c89905abd116e825ae8"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/gif-first-frame-reference/v1"
    );
    let sources = record["sources"].as_array().unwrap();
    assert_eq!(sources.len(), SOURCES.len());
    for (source, path) in sources.iter().zip(SOURCES) {
        assert_eq!(source["path"], path);
        if crate::common::is_oracle_producer(path) {
            continue;
        }
        assert_eq!(
            hash(&std::fs::read(crate::common::repository_root().join(path)).unwrap()),
            source["sha256"]
        );
    }
    for (bytes, digest) in [
        (
            include_bytes!("png_profile_contract.rs").as_slice(),
            "994389eed1224886cfb0df9e8382d90bcac8b6350d72618409a01a1d108a3661",
        ),
        (
            include_bytes!("../common/pdf_inspect.rs").as_slice(),
            "28e13e1b91eacaf9743e41a1f43aecc1ecb7cab0020d9ac21c0368377500cda2",
        ),
        (
            include_bytes!("../common/raster.rs").as_slice(),
            "46a6ed5daacc715f1f3c28c2b806b95a04e13206b4bc7f6d13c0ea50c99b8e10",
        ),
    ] {
        assert_eq!(hash(bytes), digest);
    }
    assert_eq!(record["collection"]["renderer"], "official v0.18.1");
    assert_eq!(record["collection"]["chromium"], "149.0.7827.55");
    assert_eq!(record["collection"]["profile_sha256"], sources[2]["sha256"]);
    assert_eq!(
        record["collection"]["oci"],
        "sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
    );
    assert_eq!(record["collection"]["network"], "none");
    assert_eq!(record["collection"]["context"], "fresh per vector per pass");
    assert_eq!(
        record["collection"]["capture"]["bound_checked_in"],
        json!(["browser monotonic clock", "host monotonic clock"])
    );
    assert_eq!(
        record["options"],
        json!({"exportBackground":false,"exportEmbedScene":false,
        "exportPadding":0,"exportScale":1,"exportWithDarkMode":false,
        "frameRendering":{"enabled":true,"clip":true,"name":true,"outline":true},
        "maxWidthOrHeight":null,"viewBackgroundColor":"#ffffff"})
    );
    assert_eq!(
        record["matrix"],
        json!({"cells":120,"vectors":240,"patterns_per_cell":2,
        "logical_screen":[9,7],"palettes":["global","local","both-local-override"],
        "rectangles":["full 9x7 at 0,0","5x5 at 2,1"],"transparency":["opaque","index 0 transparent"],
        "interlace":[false,true],"animation":["static disposal 0","animated disposal 0","animated disposal 1",
        "animated disposal 2","animated disposal 3"],"controls":16,"total_vectors":256})
    );
    assert_eq!(
        record["collection"]["passes"],
        json!(["fresh-process-forward", "fresh-process-reverse"])
    );
    assert_eq!(record["collection"]["capture"]["initial_delay_cs"], 65535);
    assert_eq!(record["collection"]["capture"]["maximum_capture_ms"], 30000);
    assert_eq!(record["differences"], json!([]));
    assert_eq!(record["cases"].as_array().unwrap().len(), 240);
    let mut ids = BTreeSet::new();
    let mut cells = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    for palette in ["global", "local", "both-local-override"] {
        for subrect in [false, true] {
            for transparent in [false, true] {
                for interlaced in [false, true] {
                    for animation in [
                        "static",
                        "animated-d0",
                        "animated-d1",
                        "animated-d2",
                        "animated-d3",
                    ] {
                        for pattern in ["alternating", "asymmetric"] {
                            let v = Vector {
                                palette,
                                subrect,
                                transparent,
                                interlaced,
                                animation,
                                pattern,
                            };
                            let id = format!("{}-{pattern}", v.cell());
                            let row = by_id(&record, "cases", &id);
                            qualify_row(row, v);
                            assert!(ids.insert(id));
                            cells.insert(v.cell());
                            assert!(hashes.insert(row["gif_sha256"].as_str().unwrap()));
                        }
                    }
                }
            }
        }
    }
    assert_eq!((ids.len(), cells.len(), hashes.len()), (240, 120, 240));
    assert_eq!(
        record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        ids
    );
    qualify_controls(&record);
    // Every selector executes these, so an owner never depends on a separate
    // unselected negative-oracle or discrimination test having run first.
    negative_oracles(&record);
    record
}

fn qualify_row(row: &Value, v: Vector<'_>) {
    assert_eq!(row["cell"], v.cell());
    assert_eq!(row["palette_mode"], v.palette);
    assert_eq!(row["placement"], if v.subrect { "subrect" } else { "full" });
    assert_eq!(row["transparent"], v.transparent);
    assert_eq!(
        row["transparent_index"],
        if v.transparent { json!(0) } else { Value::Null }
    );
    assert_eq!(row["interlaced"], v.interlaced);
    assert_eq!(row["animation"], v.animation);
    assert_eq!(row["pattern"], v.pattern);
    assert_eq!(row["first_disposal"], v.disposal());
    assert_eq!(row["initial_delay_cs"], 65535);
    assert_eq!(row["background_index"], 3);
    assert_eq!(
        row["background_rgb"],
        if v.palette == "local" {
            Value::Null
        } else {
            json!(GLOBAL[3])
        }
    );
    assert_eq!(
        row["global_palette"],
        if v.palette == "local" {
            Value::Null
        } else {
            json!(GLOBAL)
        }
    );
    assert_eq!(
        row["local_palette"],
        if v.palette == "global" {
            Value::Null
        } else {
            json!(LOCAL)
        }
    );
    assert_eq!(row["container_valid"], true);
    assert_eq!(row["eligibility"], "valid-first-frame-reference");
    assert_eq!(
        (row["width"].as_u64(), row["height"].as_u64()),
        (Some(9), Some(7))
    );
    let animated = v.animation != "static";
    assert_eq!(row["frame_count"], if animated { 2 } else { 1 });
    let (bytes, offsets) = v.bytes();
    assert_eq!(
        bytes,
        decode(&row["gif_base64"]),
        "{}: wire reconstruction",
        row["id"]
    );
    assert_eq!(row["offsets"], offsets);
    assert_eq!(row["gif_bytes"], bytes.len());
    assert_eq!(row["gif_sha256"], hash(&bytes));
    assert_eq!(row["scene"], scene(&row["gif_base64"]));
    let frames = inspect(&bytes).unwrap();
    for later in [false, true] {
        let key = if later {
            "expected_later"
        } else {
            "expected_first"
        };
        let rect_key = if later { "later_rect" } else { "first_rect" };
        let rows_key = if later {
            "later_indices_rows"
        } else {
            "first_indices_rows"
        };
        if later && !animated {
            for key in [key, rect_key, rows_key] {
                assert_eq!(row[key], Value::Null);
            }
            continue;
        }
        let [left, top, width, height] = v.rect(later);
        assert_eq!(
            row[rect_key],
            json!({"left":left,"top":top,"width":width,"height":height})
        );
        assert_eq!(row[rows_key], json!(v.rows(later)));
        let expected = v.rgba(later);
        assert_eq!(frames[usize::from(later)], expected);
        assert_eq!(decode(&row[key]["rgba_base64"]), expected);
        assert_eq!(row[key]["rgba_sha256"], hash(&expected));
    }
    assert_eq!(
        row["first_wire_row_order"],
        json!(wire_rows(v.rect(false)[3], v.interlaced))
    );
    let first = v.rgba(false);
    if animated {
        assert_ne!(first, v.rgba(true), "first/later discriminator");
    }
    if v.subrect {
        assert_eq!(&first[..4], &[0; 4]);
        let mut wrong = first.clone();
        wrong[..4].copy_from_slice(&[251, 193, 17, 255]);
        assert_ne!(first, wrong, "background-index discriminator");
    }
    if v.palette == "both-local-override" {
        assert_ne!(
            first,
            Vector {
                palette: "global",
                ..v
            }
            .rgba(false),
            "local override discriminator"
        );
    }
    if v.interlaced {
        let mut wrong = bytes.clone();
        wrong[offsets["first_descriptor"].as_u64().unwrap() as usize + 9] &= !0x40;
        assert_ne!(
            inspect(&wrong).unwrap()[0],
            first,
            "interlace discriminator"
        );
    }
    // Explicit coordinate/color anchors independently check the pattern math.
    let [left, top, _, _] = v.rect(false);
    let anchor = ((top + 1) * 9 + left + 2) * 4;
    let color = if v.palette == "global" {
        [239, 31, 73, 255]
    } else {
        [13, 173, 241, 255]
    };
    // Both patterns have index 0 at (2,1) only for alternating; asymmetric is 1.
    let color = if v.pattern == "alternating" {
        if v.transparent {
            [0; 4]
        } else {
            color
        }
    } else if v.palette == "global" {
        [19, 211, 97, 255]
    } else {
        [227, 53, 181, 255]
    };
    assert_eq!(&first[anchor..anchor + 4], &color);
    let reference = &row["reference"];
    assert_eq!(reference["admitted"], true);
    assert_eq!(reference["capture_under_30_seconds"], true);
    assert_eq!(
        (reference["width"].as_u64(), reference["height"].as_u64()),
        (Some(9), Some(7))
    );
    svg_check(reference["svg"].as_str().unwrap(), Some(&bytes));
    assert_eq!(row["comparison"]["admitted"], true);
    assert_eq!(row["comparison"]["all_match_authored"], true);
    for surface in ["intrinsic", "png", "svg"] {
        let sample = &reference["samples"][surface];
        assert_eq!(
            (sample["width"].as_u64(), sample["height"].as_u64()),
            (Some(9), Some(7))
        );
        assert_eq!(decode(&sample["rgba_base64"]), first);
        png_pixels(&decode(&sample["png"]), &first, false);
        assert_eq!(
            row["comparison"]["surfaces"][surface],
            json!({"dimensions_match":true,"changed_pixels":0,
            "rgba_sha256":hash(&first),"matches_first":true,"matches_later":if animated {json!(false)} else {Value::Null}})
        );
    }
    for key in ["png", "svg_png"] {
        png_pixels(&decode(&reference[key]), &first, false);
    }
}

fn qualify_controls(record: &Value) {
    let base = by_id(record, "cases", BASE);
    let original = decode(&base["gif_base64"]);
    let offset = |key: &str| base["offsets"][key].as_u64().unwrap() as usize;
    let controls = record["controls"].as_array().unwrap();
    assert_eq!(controls.len(), 16);
    assert_eq!(
        controls
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        MALFORMED.into_iter().chain(RESOURCES).collect()
    );
    for row in controls {
        let id = row["id"].as_str().unwrap();
        assert_eq!(row["cell"], id);
        assert_eq!(row["base_id"], BASE);
        assert_eq!(row["collected"], false);
        assert_eq!(row["eligibility"], "uncollected-parent-control");
        assert_eq!(
            row["kind"],
            if MALFORMED.contains(&id) {
                "malformed"
            } else {
                "resource"
            }
        );
        let mut bytes = original.clone();
        match id {
            "bad-palette-index" => {
                bytes[10] = 0x90;
                bytes.drain(19..25);
            }
            "later-stream-invalid-code" => bytes[offset("later_stream") + 2] = 0x3c,
            "missing-trailer" => {
                bytes.pop();
            }
            "invalid-gce-size" => bytes[offset("first_gce") + 2] = 3,
            "reserved-gce-bits" => bytes[offset("first_gce") + 3] |= 0x80,
            "reserved-image-bits" => bytes[offset("first_descriptor") + 9] |= 8,
            "reserved-disposal-4" => bytes[offset("first_gce") + 3] = 16,
            "first-stream-invalid-code" => bytes[offset("first_stream") + 2] = 0x3c,
            "truncated-image-data" => bytes.truncate(offset("first_stream") + 6),
            "missing-color-table" => {
                bytes[10] &= 0x7f;
                bytes.drain(13..25);
            }
            "zero-logical-width" => bytes[6..8].fill(0),
            "frame-outside-screen" => bytes
                [offset("first_descriptor") + 1..offset("first_descriptor") + 3]
                .copy_from_slice(&8_u16.to_le_bytes()),
            "input-byte-budget" | "logical-pixel-budget" => {}
            "huge-logical-screen" => bytes[6..10].fill(255),
            "huge-image-descriptor" => {
                bytes[offset("first_descriptor") + 5..offset("first_descriptor") + 9].fill(255)
            }
            _ => unreachable!(),
        }
        assert_eq!(
            bytes,
            decode(&row["gif_base64"]),
            "{id}: independently reconstructed mutation"
        );
        assert_eq!(row["gif_bytes"], bytes.len());
        assert_eq!(row["gif_sha256"], hash(&bytes));
        if MALFORMED.contains(&id) || id.starts_with("huge-") {
            assert!(inspect(&bytes).is_err(), "{id}");
        } else {
            assert_eq!(inspect(&bytes).unwrap(), inspect(&original).unwrap());
        }
        let resource = match id {
            "input-byte-budget" => {
                json!({"measure":"input_bytes","required":original.len(),"limit":original.len()-1})
            }
            "logical-pixel-budget" => {
                json!({"measure":"logical_screen_pixels","required":63,"limit":62})
            }
            "huge-logical-screen" => {
                json!({"measure":"logical_screen_pixels","declared":4294836225_u64,"limit":63})
            }
            "huge-image-descriptor" => {
                json!({"measure":"image_descriptor_pixels","declared":4294836225_u64,"limit":63})
            }
            _ => Value::Null,
        };
        assert_eq!(row["resource_control"], resource);
    }
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
        let href = images[0]
            .attribute("href")
            .or_else(|| images[0].attribute(("http://www.w3.org/1999/xlink", "href")))
            .unwrap();
        let data = href.strip_prefix("data:image/gif;base64,").unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap(),
            source
        );
        assert_eq!(
            data,
            base64::engine::general_purpose::STANDARD.encode(source)
        );
    }
}
fn png_pixels(bytes: &[u8], expected: &[u8], untagged: bool) {
    if untagged {
        super::png_profile_contract::assert_untagged_png(bytes);
    }
    let actual = crate::common::raster::decode_png(bytes);
    assert_eq!((actual.width, actual.height), (9, 7));
    assert_eq!(actual.rgba, expected);
    assert!(actual
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| matches!(p[3], 0 | 255) && (p[3] != 0 || p[..3] == [0; 3])));
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
            .map(|n| n.parse().unwrap())
            .collect();
        assert_eq!(values, [0.0, 0.0, 6.75, 5.25]);
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}
fn artifact(bytes: &[u8], format: OutputFormat, row: &Value) {
    let expected = decode(&row["expected_first"]["rgba_base64"]);
    match format {
        OutputFormat::Svg => svg_check(
            std::str::from_utf8(bytes).unwrap(),
            Some(&decode(&row["gif_base64"])),
        ),
        OutputFormat::Png => png_pixels(bytes, &expected, true),
        OutputFormat::Pdf => {
            pdf_extent(bytes);
            assert_eq!(super::png_profile_contract::pdf_rgba(bytes, 9, 7), expected);
        }
        _ => unreachable!(),
    }
}
fn request(format: OutputFormat, limits: ResourceLimits) -> RenderRequest {
    RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .resource_limits(limits)
        .build()
        .unwrap()
}
fn render(source: &Value, request: &RenderRequest) -> (Vec<u8>, Value) {
    let document = SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap();
    let first = Renderer::new().render(&document, request).unwrap();
    let again = Renderer::new().render(&document, request).unwrap();
    assert_eq!(first.artifact().bytes(), again.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(again.report()).unwrap()
    );
    let mut report = serde_json::to_value(first.report()).unwrap();
    // Messages are human-readable, not compatibility keys; everything else,
    // including order and absence of unrelated diagnostics, is exact.
    for diagnostic in report["diagnostics"].as_array_mut().unwrap() {
        assert!(!diagnostic["message"].as_str().unwrap().is_empty());
        diagnostic.as_object_mut().unwrap().remove("message");
    }
    (first.artifact().bytes().to_vec(), report)
}
fn animation_report(format: OutputFormat, animated: bool, ids: &[&str]) -> Value {
    let diagnostics: Vec<_> = if !animated || format == OutputFormat::Svg {
        vec![]
    } else {
        ids.iter().enumerate().map(|(index, id)| json!({"code":"image.animation-flattened","severity":"info",
            "format":format.as_str(),"effects":["visual"],"element_id":id,"json_pointer":format!("/elements/{index}/fileId"),
            "details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/gif",
            "recovery":"flattened","recipe_id":null}})).collect()
    };
    json!({"schema_version":1,"diagnostics":diagnostics})
}
fn recovery_artifact(bytes: &[u8], format: OutputFormat, source: &Value) {
    let mut missing = source.clone();
    missing["files"] = json!({});
    assert_eq!(
        bytes,
        render(&missing, &request(format, ResourceLimits::default())).0
    );
    match format {
        OutputFormat::Svg => svg_check(std::str::from_utf8(bytes).unwrap(), None),
        OutputFormat::Pdf => {
            pdf_extent(bytes);
            assert!(!crate::common::pdf_inspect::pdf_text(bytes).contains("/Subtype /Image"));
        }
        OutputFormat::Png => {
            let image = crate::common::raster::decode_png(bytes);
            assert_eq!((image.width, image.height), (9, 7));
        }
        _ => unreachable!(),
    }
}
fn decode_report(format: OutputFormat) -> Value {
    json!({"schema_version":1,"diagnostics":[{"code":"image.decode-failed","severity":"warning","format":format.as_str(),
        "effects":["visual"],"element_id":"image","json_pointer":"/files/asset/dataURL","details":{"kind":"asset",
        "asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/gif",
        "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"},
        "recipe_id":if format == OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}}]})
}

fn negative_oracles(record: &Value) {
    let row = by_id(
        record,
        "cases",
        "both-local-override-subrect-transparent-interlaced-animated-d3-asymmetric",
    );
    let expected = decode(&row["expected_first"]["rgba_base64"]);
    let rejects = |check: &dyn Fn()| {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)).is_err())
    };
    let mut wrong = expected.clone();
    wrong[0] = 1;
    let png = decode(&row["reference"]["png"]);
    rejects(&|| png_pixels(&png, &wrong, false));
    let svg = row["reference"]["svg"].as_str().unwrap();
    let source = decode(&row["gif_base64"]);
    rejects(&|| {
        svg_check(
            &svg.replace("viewBox=\"0 0 9 7\"", "viewBox=\"0 0 8 7\""),
            Some(&source),
        )
    });
    rejects(&|| {
        svg_check(
            svg,
            Some(&decode(&by_id(record, "cases", BASE)["gif_base64"])),
        )
    });
    let (native_png, _) = render(
        &row["scene"],
        &request(OutputFormat::Png, ResourceLimits::default()),
    );
    let mut tagged = native_png.clone();
    let mut chunk = 1_u32.to_be_bytes().to_vec();
    chunk.extend(b"sRGB\0");
    chunk.extend(crc32fast::hash(&chunk[4..]).to_be_bytes());
    tagged.splice(33..33, chunk);
    rejects(&|| png_pixels(&tagged, &expected, true));
    // Well-formed PNG alternatives isolate layout and canonical-zero checks
    // from corrupt-container rejection. No GIF expectation uses this encoder.
    for (color, samples) in [
        (png::ColorType::Rgba, wrong.clone()),
        (
            png::ColorType::Rgb,
            expected
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| p[..3].iter().copied())
                .collect(),
        ),
    ] {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 9, 7);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&samples)
                .unwrap();
        }
        rejects(&|| png_pixels(&bytes, &expected, true));
    }
    let (pdf, _) = render(
        &row["scene"],
        &request(OutputFormat::Pdf, ResourceLimits::default()),
    );
    artifact(&pdf, OutputFormat::Pdf, row);
    let inflated = crate::common::pdf_inspect::inflate_streams(&pdf);
    // Mutations stay tiny and change only dictionaries, never adversarial stream
    // lengths. The bound helper follows page resources and actual references.
    for (from, to) in [
        ("/SMask ", "/Unused "),
        ("/x0 Do\n", "/x9 Do\n"),
        ("/ICCBased ", "/DeviceRGB "),
        (
            "/Type /Catalog\n",
            "/Type /Catalog\n/OutputIntents [1 0 R]\n",
        ),
        ("/DeviceGray", "/DeviceRGB"),
        ("/Width 9\n", "/Width 8\n"),
        ("/MediaBox [0 0 6.75 5.25]", "/MediaBox [0 0 6.75 5.26]"),
        ("/CropBox [0 0 6.75 5.25]", "/CropBox [0 0 6.75 5.26]"),
    ] {
        let at = inflated
            .windows(from.len())
            .position(|w| w == from.as_bytes())
            .unwrap();
        let mut changed = inflated.clone();
        changed.splice(at..at + from.len(), to.bytes());
        rejects(&|| artifact(&changed, OutputFormat::Pdf, row));
    }
    rejects(&|| assert_eq!(super::png_profile_contract::pdf_rgba(&pdf, 9, 7), wrong));
    // Rebind the actually drawn image to the valid mask object, leaving the
    // correct RGB stream present but detached. Presence-only inspection fails.
    let text = crate::common::pdf_inspect::pdf_text(&pdf);
    let image_ref = text
        .split_once("/XObject << /x0 ")
        .unwrap()
        .1
        .split_once(" 0 R")
        .unwrap()
        .0;
    let mask_ref = text
        .split_once("/SMask ")
        .unwrap()
        .1
        .split_once(" 0 R")
        .unwrap()
        .0;
    let from = format!("/XObject << /x0 {image_ref} 0 R");
    let to = format!("/XObject << /x0 {mask_ref} 0 R");
    let at = inflated
        .windows(from.len())
        .position(|w| w == from.as_bytes())
        .unwrap();
    let mut detached = inflated.clone();
    detached.splice(at..at + from.len(), to.bytes());
    rejects(&|| artifact(&detached, OutputFormat::Pdf, row));
    let original = decode(&by_id(record, "cases", BASE)["gif_base64"]);
    for at in [0, 10, 25, 27, original.len() - 1] {
        let mut changed = original.clone();
        changed[at] ^= 0xff;
        assert!(inspect(&changed).is_err(), "wire oracle mutation at {at}");
    }
    assert_eq!(lzw(&[0x3c], 1), Err("undefined code"));
}

#[test]
pub(super) fn valid_first_frame_matrix_matches_frozen_reference_exactly() {
    let record = evidence();
    for row in record["cases"].as_array().unwrap() {
        for format in FORMATS {
            let (bytes, report) =
                render(&row["scene"], &request(format, ResourceLimits::default()));
            assert_eq!(
                report,
                animation_report(format, row["frame_count"] == 2, &["image"]),
                "{}: {format:?}",
                row["id"]
            );
            artifact(&bytes, format, row);
        }
    }
    repeated_occurrences(&record);
}

fn repeated_occurrences(record: &Value) {
    for id in [BASE, "global-full-opaque-sequential-static-asymmetric"] {
        let row = by_id(record, "cases", id);
        let mut source = row["scene"].clone();
        source["elements"][0]["id"] = json!("z-first");
        let mut second = source["elements"][0].clone();
        second["id"] = json!("a-second");
        let mut deleted = second.clone();
        deleted["id"] = json!("deleted");
        deleted["isDeleted"] = json!(true);
        source["elements"]
            .as_array_mut()
            .unwrap()
            .extend([second, deleted]);
        for format in FORMATS {
            let (bytes, report) = render(&source, &request(format, ResourceLimits::default()));
            assert_eq!(
                report,
                animation_report(format, row["frame_count"] == 2, &["z-first", "a-second"])
            );
            artifact(&bytes, format, row);
        }
    }
}

#[test]
fn authored_malformed_controls_have_exact_decode_recovery() {
    // Only controls the pinned reference rejects, or whose first frame it
    // paints partially (the declared divergence), still recover; the others
    // draw like the reference (gif_container_contract).
    let record = evidence();
    let mut strict = 0;
    for id in MALFORMED {
        if !super::gif_container_contract::strictly_rejected("first-frame", id) {
            continue;
        }
        strict += 1;
        let row = by_id(&record, "controls", id);
        let source = scene(&row["gif_base64"]);
        for format in FORMATS {
            let (bytes, report) = render(&source, &request(format, ResourceLimits::default()));
            assert_eq!(report, decode_report(format), "{id}: {format:?}");
            recovery_artifact(&bytes, format, &source);
        }
    }
    assert_eq!(strict, 3);
}

#[derive(Clone, Copy)]
enum Limit {
    Bytes,
    Pixels,
    Dimension,
    Work,
}
impl Limit {
    fn limits(self, n: u64) -> ResourceLimits {
        let builder = ResourceLimits::builder();
        match self {
            Self::Bytes => builder.max_asset_bytes(n),
            Self::Pixels => builder.max_decoded_pixels_per_asset(n),
            Self::Dimension => builder.max_raster_dimension(n as u32),
            Self::Work => builder.max_asset_work_units(n),
        }
        .build()
        .unwrap()
    }
    fn report(self, format: OutputFormat, configured: u64, attempted: u64) -> Value {
        let (code, field, kind) = match self {
            Self::Bytes => ("resource.asset-bytes-exceeded", "max_asset_bytes", "bytes"),
            Self::Pixels => (
                "resource.image-pixels-exceeded",
                "max_decoded_pixels_per_asset",
                "pixels",
            ),
            Self::Dimension => (
                "resource.image-dimensions-exceeded",
                "max_raster_dimension",
                "dimension",
            ),
            Self::Work => (
                "resource.asset-work-exceeded",
                "max_asset_work_units",
                "work_units",
            ),
        };
        json!({"schema_version":1,"diagnostics":[{"code":code,"severity":"warning","format":format.as_str(),
            "effects":["visual"],"element_id":if matches!(self, Self::Pixels | Self::Dimension) {Value::Null} else {json!("image")},
            "json_pointer":"/files/asset/dataURL","details":{"kind":"resource_limit","field":field,"configured":configured,
            "value_kind":kind,"measurement":{"kind":"attempted","value":attempted},
            "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"}}}]})
    }
}

#[test]
fn byte_pixel_dimension_and_prepaid_work_boundaries_are_exact() {
    let record = evidence();
    // Both placements have a 63-pixel later rectangle. Charging the first
    // subrectangle (25), or only counting frames, cannot satisfy this boundary.
    for id in [
        BASE,
        "global-subrect-opaque-sequential-animated-d0-asymmetric",
        "global-full-opaque-sequential-static-asymmetric",
    ] {
        let row = by_id(&record, "cases", id);
        let n = row["gif_bytes"].as_u64().unwrap();
        let animated = row["frame_count"] == 2;
        let validation = n + if animated { 4 * 63 } else { 0 };
        // Spec §13.2: validation plus the selected intrinsic payload passes;
        // the established raster prepayment is 5 payloads + 4/source pixel.
        let decode = 5 * n + 4 * 63;
        for format in FORMATS {
            for (limit, needed) in [
                (Limit::Bytes, n),
                (Limit::Pixels, 63),
                (Limit::Dimension, 9),
                (
                    Limit::Work,
                    if format == OutputFormat::Svg {
                        validation
                    } else {
                        decode
                    },
                ),
            ] {
                for configured in [needed - 1, needed, needed + 1] {
                    let rejected = configured < needed
                        && !(format == OutputFormat::Svg
                            && matches!(limit, Limit::Pixels | Limit::Dimension));
                    let (bytes, report) =
                        render(&row["scene"], &request(format, limit.limits(configured)));
                    assert_eq!(
                        report,
                        if rejected {
                            limit.report(format, configured, needed)
                        } else {
                            animation_report(format, animated, &["image"])
                        },
                        "{id}: {format:?}: {configured}/{needed}"
                    );
                    if rejected {
                        recovery_artifact(&bytes, format, &row["scene"]);
                    } else {
                        artifact(&bytes, format, row);
                    }
                }
            }
            // Validation precedes prepaid decoding in all formats; at equality
            // PNG/PDF advance to the decode transaction, rather than succeeding.
            for configured in [validation - 1, validation] {
                let (bytes, report) = render(
                    &row["scene"],
                    &request(format, Limit::Work.limits(configured)),
                );
                if configured == validation && format == OutputFormat::Svg {
                    assert_eq!(report, animation_report(format, animated, &["image"]));
                    artifact(&bytes, format, row);
                } else {
                    assert_eq!(
                        report,
                        Limit::Work.report(
                            format,
                            configured,
                            if configured < validation {
                                validation
                            } else {
                                decode
                            }
                        )
                    );
                    recovery_artifact(&bytes, format, &row["scene"]);
                }
            }
        }
    }
    // Explicitly execute the two authored finite resource controls as well.
    for (id, limit, configured, attempted) in [
        (
            "input-byte-budget",
            Limit::Bytes,
            by_id(&record, "cases", BASE)["gif_bytes"].as_u64().unwrap() - 1,
            by_id(&record, "cases", BASE)["gif_bytes"].as_u64().unwrap(),
        ),
        ("logical-pixel-budget", Limit::Pixels, 62, 63),
    ] {
        let row = by_id(&record, "controls", id);
        let source = scene(&row["gif_base64"]);
        for format in FORMATS {
            let (bytes, report) = render(&source, &request(format, limit.limits(configured)));
            if format == OutputFormat::Svg && matches!(limit, Limit::Pixels) {
                assert_eq!(report, animation_report(format, true, &["image"]));
                artifact(&bytes, format, by_id(&record, "cases", BASE));
            } else {
                assert_eq!(report, limit.report(format, configured, attempted));
                recovery_artifact(&bytes, format, &source);
            }
        }
    }
}

#[test]
fn huge_header_controls_are_preempted_by_bounded_validation_work() {
    let record = evidence();
    // These are safety-preemption controls, NOT claims that a logical pixel
    // cap applies to direct SVG or that the oversized descriptor is decoded.
    // The work rejection happens before any untrusted-sized decoder allocation.
    for id in ["huge-logical-screen", "huge-image-descriptor"] {
        let row = by_id(&record, "controls", id);
        let validation = row["gif_bytes"].as_u64().unwrap() + 4 * 63;
        let source = scene(&row["gif_base64"]);
        for format in FORMATS {
            let limits = ResourceLimits::builder()
                .max_asset_work_units(validation - 1)
                .max_decoded_pixels_per_asset(63)
                .max_raster_dimension(9)
                .build()
                .unwrap();
            let (bytes, report) = render(&source, &request(format, limits));
            assert_eq!(
                report,
                Limit::Work.report(format, validation - 1, validation),
                "{id}: {format:?}"
            );
            recovery_artifact(&bytes, format, &source);
        }
    }
}
