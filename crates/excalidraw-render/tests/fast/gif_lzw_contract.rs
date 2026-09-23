//! Closed GIF LZW contract: 36 two-pattern matrix cells, eight extension cells,
//! and 12 uncollected strict-policy controls. No GIF codec supplies expectations.
//! Each exact owner executes the entire evidence and negative-helper audit.
//! The frozen missing-EOI control also underfills its 49,601-pixel descriptor.
//! A separate parent-authored 4x1 pair below isolates EOI with exact pixel counts;
//! it adds no browser evidence or frozen matrix cells.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const BASE: &str = "p2-89a-growth-to-full-b255-seed1";
const EXTENSIONS: [&str; 8] = [
    "comment-before",
    "comment-after",
    "comment-before-split",
    "comment-after-split",
    "unknown-app-before",
    "netscape-infinite",
    "netscape-finite",
    "plain-text-before",
];
const MALFORMED: [&str; 12] = [
    "minimum-code-size-1",
    "minimum-code-size-9",
    "undefined-initial-code",
    "undefined-after-literal",
    "missing-eoi",
    "truncated-subblock",
    "missing-subblock-terminator",
    "early-eoi",
    "excess-pixels",
    "missing-trailer",
    "gif87a-comment",
    "gif87a-application",
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
    "tools/reference-oracle/run-gif-lzw.sh",
    "tools/reference-oracle/scripts/canonical-json.mjs",
    "tools/reference-oracle/scripts/gif-lzw-evidence.mjs",
    "tools/reference-oracle/scripts/gif-lzw-fixtures.mjs",
    "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs",
    "tools/reference-oracle/scripts/profile.mjs",
    "tools/reference-oracle/scripts/smoke.mjs",
    "tools/reference-oracle/src/browser-entry.js",
    "tools/reference-oracle/vite.config.mjs",
];
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn decode(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
fn n(value: &Value) -> usize {
    value.as_u64().unwrap() as usize
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

#[derive(Debug)]
struct Stream {
    indices: Vec<u8>,
    stats: Value,
}

// A prefix/suffix walk, not the author's array-of-strings decoder. Only the
// output grows with the bounded screen; dictionary and reverse stack are fixed.
// Insert AFTER reading a phrase: the encoder is one insertion ahead of us.
fn lzw(data: &[u8], minimum: usize, count: usize) -> Result<Stream, &'static str> {
    require((2..=8).contains(&minimum), "minimum code size")?;
    require(count <= 49_601 && data.len() <= 100_000, "LZW bound")?;
    let clear = 1_usize << minimum;
    let end = clear + 1;
    let mut prefix = [0_u16; 4096];
    let mut suffix = [0_u8; 4096];
    let mut stack = [0_u8; 4096];
    let (mut next, mut width, mut bit) = (end + 1, minimum + 1, 0);
    let mut previous: Option<usize> = None;
    let mut indices = Vec::with_capacity(count);
    let mut widths = Vec::new();
    let mut events = Vec::new();
    let (mut codes, mut clears, mut references, mut kwkwk, mut max_next, mut full) =
        (0, 0, 0, 0, 0, 0);
    loop {
        require(bit + width <= data.len() * 8, "missing EOI")?;
        let start = bit;
        let code = (0..width).fold(0, |v, j| {
            v | (usize::from((data[(bit + j) / 8] >> ((bit + j) % 8)) & 1) << j)
        });
        bit += width;
        codes += 1;
        if !widths.contains(&width) {
            widths.push(width);
        }
        require(codes != 1 || code == clear, "initial clear")?;
        if code == clear {
            clears += 1;
            events.push(
                json!({"kind":"clear","bit":start,"width":width,"output_pixels":indices.len()}),
            );
            next = end + 1;
            width = minimum + 1;
            previous = None;
            continue;
        }
        if code == end {
            require(indices.len() == count, "pixel count")?;
            require(data.len() * 8 - bit < 8, "bytes after EOI")?;
            require(
                (bit..data.len() * 8).all(|i| data[i / 8] & (1 << (i % 8)) == 0),
                "padding",
            )?;
            return Ok(Stream {
                indices,
                stats: json!({"widths":widths,"code_count":codes,
                "clear_count":clears,"dictionary_references":references,"kwkwk_count":kwkwk,
                "max_next_code":max_next,"full_dictionary_codes":full,"events":events}),
            });
        }
        require(
            code < next || (code == next && previous.is_some() && next < 4096),
            "undefined code",
        )?;
        let special = code == next;
        let mut cursor = if special { previous.unwrap() } else { code };
        let mut depth = 0;
        while cursor >= clear {
            require(
                cursor > end && cursor < next && depth < stack.len() - 1,
                "prefix chain",
            )?;
            stack[depth] = suffix[cursor];
            depth += 1;
            let parent = usize::from(prefix[cursor]);
            require(parent < cursor, "prefix cycle")?;
            cursor = parent;
        }
        let first = cursor as u8;
        stack[depth] = first;
        depth += 1;
        require(
            indices.len() + depth + usize::from(special) <= count,
            "pixel overflow",
        )?;
        indices.extend(stack[..depth].iter().rev());
        if special {
            indices.push(first);
            kwkwk += 1;
        }
        if code > end {
            references += 1;
        }
        if next == 4096 {
            full += 1;
        }
        if let Some(old) = previous {
            if next < 4096 {
                prefix[next] = old as u16;
                suffix[next] = first;
                next += 1;
                if next == 1 << width && width < 12 {
                    width += 1;
                    events.push(json!({"kind":"width","bit":bit,"width":width,"next_code":next,"output_pixels":indices.len()}));
                }
            }
        }
        max_next = max_next.max(next);
        previous = Some(code);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let part = self
            .bytes
            .get(self.at..self.at + count)
            .ok_or("truncated")?;
        self.at += count;
        Ok(part)
    }
    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    fn short(&mut self) -> Result<usize, &'static str> {
        Ok(usize::from(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap(),
        )))
    }
    fn blocks(&mut self) -> Result<(Vec<u8>, Vec<usize>), &'static str> {
        let mut data = Vec::new();
        let mut sizes = Vec::new();
        loop {
            let size = usize::from(self.byte()?);
            if size == 0 {
                return Ok((data, sizes));
            }
            require(data.len() + size <= 100_000, "subblock bound")?;
            data.extend(self.take(size)?);
            sizes.push(size);
        }
    }
}
#[derive(Debug)]
struct Wire {
    width: usize,
    height: usize,
    palette: Vec<[u8; 3]>,
    minimum: usize,
    packed: Vec<u8>,
    sizes: Vec<usize>,
    image_at: usize,
    extensions: Vec<(bool, Vec<u8>)>,
    stream: Stream,
}
fn inspect(bytes: &[u8]) -> Result<Wire, &'static str> {
    require(bytes.len() <= 200_000, "input bound")?;
    let mut r = Reader { bytes, at: 0 };
    let version = r.take(6)?;
    require(version == b"GIF87a" || version == b"GIF89a", "version")?;
    let (width, height) = (r.short()?, r.short()?);
    require(
        width > 0 && height > 0 && width * height <= 49_601,
        "screen bound",
    )?;
    let flags = r.byte()?;
    require(flags & 0xf8 == 0xf0, "global palette flags")?;
    require(r.take(2)? == [0, 0], "background/aspect")?;
    let palette: Vec<[u8; 3]> = r
        .take(3 * (2 << (flags & 7)))?
        .chunks_exact(3)
        .map(|p| p.try_into().unwrap())
        .collect();
    let mut extensions = Vec::new();
    let mut image = None;
    loop {
        let at = r.at;
        match r.byte()? {
            0x3b => {
                require(r.at == bytes.len(), "trailing bytes")?;
                break;
            }
            0x21 => {
                require(version == b"GIF89a", "87a extension policy")?;
                require(extensions.is_empty(), "extension count")?;
                match r.byte()? {
                    0xfe => {}
                    0xff => {
                        require(r.byte()? == 11, "application size")?;
                        r.take(11)?;
                    }
                    1 => {
                        require(r.byte()? == 12, "text size")?;
                        r.take(12)?;
                    }
                    _ => return Err("extension label"),
                }
                r.blocks()?;
                extensions.push((image.is_some(), bytes[at..r.at].to_vec()));
            }
            0x2c => {
                require(image.is_none(), "image count")?;
                let (left, top, w, h) = (r.short()?, r.short()?, r.short()?, r.short()?);
                require(
                    w > 0 && h > 0 && left + w <= width && top + h <= height,
                    "image bounds",
                )?;
                require(r.byte()? == 0, "image flags")?;
                let minimum = usize::from(r.byte()?);
                let (packed, sizes) = r.blocks()?;
                let stream = lzw(&packed, minimum, w * h)?;
                require(
                    stream
                        .indices
                        .iter()
                        .all(|&i| usize::from(i) < palette.len()),
                    "palette index",
                )?;
                // Check the stream first: the excess-pixels control must fail
                // on decoded output, not merely our full-screen corpus shape.
                require((left, top, w, h) == (0, 0, width, height), "full screen")?;
                image = Some((at, minimum, packed, sizes, stream));
            }
            _ => return Err("block marker"),
        }
    }
    let (image_at, minimum, packed, sizes, stream) = image.ok_or("missing image")?;
    Ok(Wire {
        width,
        height,
        palette,
        minimum,
        packed,
        sizes,
        image_at,
        extensions,
        stream,
    })
}
fn blocks(data: &[u8], size: usize) -> Vec<u8> {
    let mut result = Vec::new();
    for part in data.chunks(size) {
        result.push(part.len() as u8);
        result.extend(part);
    }
    result.push(0);
    result
}
fn extension(kind: &str) -> Vec<u8> {
    let mut out = vec![0x21];
    if kind.starts_with("comment") {
        out.push(0xfe);
        out.extend(blocks(
            "independent GIF comment / ".repeat(12).as_bytes(),
            if kind.ends_with("split") { 1 } else { 255 },
        ));
    } else if kind == "unknown-app-before" {
        out.extend([0xff, 11]);
        out.extend(b"ORACLEFX010");
        out.extend(blocks(&[0, 0x21, 0x2c, 0x3b, 255, 7], 1));
    } else if kind.starts_with("netscape") {
        out.extend([0xff, 11]);
        out.extend(b"NETSCAPE2.0");
        out.extend([3, 1, if kind == "netscape-finite" { 2 } else { 0 }, 0, 0]);
    } else {
        assert_eq!(kind, "plain-text-before");
        out.extend([1, 12, 0, 0, 0, 0, 8, 0, 8, 0, 8, 8, 1, 0, 1, b'A', 0]);
    }
    out
}

#[derive(Clone, Copy)]
struct Vector<'a> {
    entries: usize,
    version: &'a str,
    program: &'a str,
    block: usize,
    pattern: usize,
    extension: Option<&'a str>,
}
impl Vector<'_> {
    fn cell(self) -> String {
        self.extension.map_or_else(
            || {
                format!(
                    "p{}-{}-{}-b{}",
                    self.entries, self.version, self.program, self.block
                )
            },
            |e| format!("extension-{e}"),
        )
    }
    fn id(self) -> String {
        if self.extension.is_some() {
            self.cell()
        } else {
            format!("{}-seed{}", self.cell(), self.pattern)
        }
    }
    fn dimensions(self) -> (usize, usize) {
        if self.entries == 2 {
            (257, 193)
        } else {
            (129, 97)
        }
    }
    fn seed(self) -> u32 {
        [0x1357_9bdf, 0x2468_ace1][self.pattern - 1]
    }
    fn palette(self) -> Vec<[u8; 3]> {
        let p = self.pattern - 1;
        (0..self.entries)
            .map(|i| {
                [
                    (i * 73 + 19 + p * 31) as u8,
                    (i * 151 + 47 + p * 17) as u8,
                    (i * 199 + 101 + p * 43) as u8,
                ]
            })
            .collect()
    }
    fn indices(self) -> Vec<u8> {
        let (w, h) = self.dimensions();
        let mut state = self.seed();
        if self.program == "kwkwk" {
            let mut output = Vec::with_capacity(w * h);
            for run in 0..w * h {
                let length = 3 + (run * 17 + (self.seed() as usize & 31)) % 251;
                let symbol = ((run * 13 + (self.seed() >> 16) as usize) % self.entries) as u8;
                output.extend(std::iter::repeat_n(
                    symbol,
                    length.min(w * h - output.len()),
                ));
                if output.len() == w * h {
                    break;
                }
            }
            output
        } else {
            (0..w * h)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    ((state >> 16) as usize % self.entries) as u8
                })
                .collect()
        }
    }
    fn wire(self, packed: &[u8]) -> Vec<u8> {
        let (w, h) = self.dimensions();
        let mut out = format!("GIF{}", self.version).into_bytes();
        out.extend((w as u16).to_le_bytes());
        out.extend((h as u16).to_le_bytes());
        out.extend([0xf0 | (self.entries.ilog2() as u8 - 1), 0, 0]);
        out.extend(self.palette().into_iter().flatten());
        let ext = self.extension.map(extension).unwrap_or_default();
        let after = self.extension.is_some_and(|e| e.contains("after"));
        if !after {
            out.extend(&ext);
        }
        out.extend([0x2c, 0, 0, 0, 0]);
        out.extend((w as u16).to_le_bytes());
        out.extend((h as u16).to_le_bytes());
        out.extend([0, self.entries.ilog2().max(2) as u8]);
        out.extend(blocks(packed, self.block));
        if after {
            out.extend(&ext);
        }
        out.push(0x3b);
        out
    }
}
fn expand(indices: &[u8], palette: &[[u8; 3]]) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|&i| {
            let [r, g, b] = palette[usize::from(i)];
            [r, g, b, 255]
        })
        .collect()
}
fn scene(encoded: &Value, width: usize, height: usize) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,
        "width":width,"height":height,"angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],
        "seed":1,"version":1,"versionNonce":1,"updated":1,"isDeleted":false,"groupIds":[],"frameId":null,
        "boundElements":null,"link":null,"locked":false,"strokeColor":"#000000","backgroundColor":"transparent",
        "fillStyle":"solid","strokeWidth":1,"strokeStyle":"solid","roughness":0,"index":"a0","crop":null,"roundness":null}],
        "appState":{},"files":{"asset":{"id":"asset","mimeType":"image/gif","created":1,
        "dataURL":format!("data:image/gif;base64,{}",encoded.as_str().unwrap())}}})
}

fn qualify_row(row: &Value, v: Vector<'_>) {
    let (w, h) = v.dimensions();
    assert_eq!(row["id"], v.id());
    assert_eq!(row["cell"], v.cell());
    assert_eq!(
        row["group"],
        if v.extension.is_some() {
            "extension"
        } else {
            "matrix"
        }
    );
    for (key, expected) in [
        ("width", w),
        ("height", h),
        ("palette_entries", v.entries),
        ("subblock_size", v.block),
        ("pattern", v.pattern),
        ("seed", v.seed() as usize),
        ("minimum_code_size", v.entries.ilog2().max(2) as usize),
        ("image_count", 1),
        ("gce_count", 0),
    ] {
        assert_eq!(row[key], expected, "{}: {key}", v.id());
    }
    assert_eq!(row["version"], v.version);
    assert_eq!(row["program"], v.program);
    assert_eq!(row["extension"], json!(v.extension));
    assert_eq!(
        row["reset_at_pixel"],
        if v.program == "midstream-reset" {
            json!(w * h / 2)
        } else {
            Value::Null
        }
    );
    for key in ["interlaced", "transparent"] {
        assert_eq!(row[key], false);
    }
    assert_eq!(row["container_valid"], true);
    assert_eq!(row["eligibility"], "valid-static-reference");
    let bytes = decode(&row["gif_base64"]);
    assert_eq!(row["gif_sha256"], hash(&bytes));
    assert_eq!(row["gif_bytes"], bytes.len());
    let wire = inspect(&bytes).unwrap();
    assert_eq!((wire.width, wire.height), (w, h));
    assert_eq!(wire.palette, v.palette());
    assert_eq!(row["palette"], json!(wire.palette));
    assert_eq!(wire.minimum, v.entries.ilog2().max(2) as usize);
    assert_eq!(
        row["offsets"],
        json!({"image":wire.image_at,"minimum":wire.image_at+10})
    );
    assert_eq!(
        wire.extensions,
        v.extension
            .map(|e| vec![(e.contains("after"), extension(e))])
            .unwrap_or_default()
    );
    assert_eq!(
        wire.sizes,
        wire.packed
            .chunks(v.block)
            .map(<[u8]>::len)
            .collect::<Vec<_>>()
    );
    assert!(
        v.wire(&wire.packed) == bytes,
        "{}: exact wire/subblock reconstruction",
        v.id()
    );
    let indices = v.indices();
    assert!(
        wire.stream.indices == indices,
        "{}: independent LZW vs authored indices",
        v.id()
    );
    assert!(decode(&row["authored_indices_base64"]) == indices);
    assert_eq!(row["authored_indices_sha256"], hash(&indices));
    let mut stats = wire.stream.stats;
    assert!(n(&stats["dictionary_references"]) > 0);
    assert_eq!(
        stats["clear_count"],
        if v.program == "midstream-reset" { 2 } else { 1 }
    );
    if v.program == "growth-to-full" {
        assert_eq!(stats["max_next_code"], 4096);
        assert!(n(&stats["full_dictionary_codes"]) > 0);
    }
    if v.program == "kwkwk" {
        assert!(n(&stats["kwkwk_count"]) > 0);
    } else {
        assert_eq!(
            stats["widths"],
            json!((wire.minimum + 1..=12).collect::<Vec<_>>())
        );
    }
    if v.program == "midstream-reset" {
        let clear = stats["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "clear")
            .nth(1)
            .unwrap();
        assert_eq!(clear["output_pixels"], w * h / 2);
    }
    stats["packed_bytes"] = json!(wire.packed.len());
    stats["packed_sha256"] = json!(hash(&wire.packed));
    stats["subblocks"] = json!(wire.sizes.len());
    stats["audit"] =
        json!("separate JS table decoder equals authored indices; EOI and zero padding checked");
    assert_eq!(
        row["stream"],
        stats,
        "{}: independently measured trace",
        v.id()
    );
    let expected = expand(&indices, &v.palette());
    assert!(decode(&row["expected_first"]["rgba_base64"]) == expected);
    assert_eq!(row["expected_first"]["rgba_sha256"], hash(&expected));
    assert_eq!(row["scene"], scene(&row["gif_base64"], w, h));
    let reference = &row["reference"];
    assert_eq!(reference["admitted"], true);
    assert_eq!((n(&reference["width"]), n(&reference["height"])), (w, h));
    svg_check(reference["svg"].as_str().unwrap(), Some(&bytes), w, h);
    assert_eq!(row["comparison"]["admitted"], true);
    assert_eq!(row["comparison"]["all_match_authored"], true);
    assert_eq!(reference["samples"].as_object().unwrap().len(), 3);
    assert_eq!(row["comparison"]["surfaces"].as_object().unwrap().len(), 3);
    for surface in ["intrinsic", "png", "svg"] {
        let sample = &reference["samples"][surface];
        assert_eq!((n(&sample["width"]), n(&sample["height"])), (w, h));
        assert!(
            decode(&sample["rgba_base64"]) == expected,
            "{}: browser {surface}",
            v.id()
        );
        png_pixels(&decode(&sample["png"]), &expected, w, h, false);
        assert_eq!(
            row["comparison"]["surfaces"][surface],
            json!({"dimensions_match":true,"changed_pixels":0,"rgba_sha256":hash(&expected),"matches_first":true})
        );
    }
    for key in ["png", "svg_png"] {
        png_pixels(&decode(&reference[key]), &expected, w, h, false);
    }
}

fn qualify_controls(record: &Value) {
    let base = by_id(record, "cases", BASE);
    let original = decode(&base["gif_base64"]);
    let at = inspect(&original).unwrap().image_at;
    let minimum = at + 10;
    let controls = record["controls"].as_array().unwrap();
    assert_eq!(controls.len(), 12);
    assert_eq!(
        controls
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        MALFORMED.into_iter().collect()
    );
    for row in controls {
        let id = row["id"].as_str().unwrap();
        assert_eq!(row["cell"], id);
        assert_eq!(row["base_id"], BASE);
        assert_eq!(row["kind"], "malformed");
        assert_eq!(row["collected"], false);
        assert_eq!(row["eligibility"], "uncollected-parent-control");
        assert_eq!(row["intended_strict"], "reject-malformed");
        assert_eq!(row["intended_recovery"], "image-local-recovery");
        assert_eq!(row["expectation_status"], "authored strict-policy intent; requires independent parent checker; not browser evidence");
        assert!(!row["reason"].as_str().unwrap().is_empty());
        assert!(row.get("reference").is_none() && row.get("comparison").is_none());
        let mut bytes = original.clone();
        let replace =
            |stream: &[u8]| [original[..minimum].to_vec(), stream.to_vec(), vec![0x3b]].concat();
        let error = match id {
            "minimum-code-size-1" => {
                bytes[minimum] = 1;
                "minimum code size"
            }
            "minimum-code-size-9" => {
                bytes[minimum] = 9;
                "minimum code size"
            }
            "undefined-initial-code" => {
                bytes = replace(&[2, 2, 0x7c, 1, 0]);
                "undefined code"
            }
            "undefined-after-literal" => {
                bytes = replace(&[2, 2, 0xc4, 0x0b, 0]);
                "undefined code"
            }
            "missing-eoi" => {
                bytes = replace(&[2, 3, 0x04, 0x41, 0x10, 0]);
                "missing EOI"
            }
            "truncated-subblock" => {
                bytes.truncate(minimum + 12);
                "truncated"
            }
            "missing-subblock-terminator" => {
                bytes.remove(bytes.len() - 2);
                "truncated"
            }
            "early-eoi" => {
                bytes = replace(&[2, 1, 0x2c, 0]);
                "pixel count"
            }
            "excess-pixels" => {
                bytes[at + 5..at + 9].copy_from_slice(&[1, 0, 1, 0]);
                "pixel overflow"
            }
            "missing-trailer" => {
                bytes.pop();
                "truncated"
            }
            "gif87a-comment" | "gif87a-application" => {
                bytes[3..6].copy_from_slice(b"87a");
                bytes.splice(
                    at..at,
                    extension(if id == "gif87a-comment" {
                        "comment-before"
                    } else {
                        "netscape-infinite"
                    }),
                );
                "87a extension policy"
            }
            _ => unreachable!(),
        };
        assert!(
            bytes == decode(&row["gif_base64"]),
            "{id}: reconstructed control"
        );
        assert_eq!(row["gif_sha256"], hash(&bytes));
        assert_eq!(row["gif_bytes"], bytes.len());
        assert_eq!(
            inspect(&bytes).unwrap_err(),
            error,
            "{id}: independent strict rejection"
        );
    }
}

fn evidence() -> Value {
    // One parse per owner; no global cache or per-vector test/thread fan-out.
    let bytes = include_bytes!("../../../../tests/oracle/gif-lzw/reference.json");
    assert_eq!(bytes.len(), 55_363_520);
    assert_eq!(
        hash(bytes),
        "3c05784f1152a10abcc731638ed46a185c16871ecb050da56f5caba7de30f707"
    );
    let record: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(record["schema"], "excalidraw-render/gif-lzw-reference/v1");
    let sources = record["sources"].as_array().unwrap();
    assert_eq!(sources.len(), SOURCES.len());
    for (source, path) in sources.iter().zip(SOURCES) {
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
    assert_eq!(collection["profile_sha256"], sources[2]["sha256"]);
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
        json!({"exportBackground":false,"exportEmbedScene":false,"exportPadding":0,
        "exportScale":1,"exportWithDarkMode":false,"frameRendering":{"enabled":true,"clip":true,"name":true,"outline":true},
        "maxWidthOrHeight":null,"viewBackgroundColor":"#ffffff"})
    );
    assert_eq!(
        record["matrix"],
        json!({"cells":36,"vectors":72,"patterns_per_cell":2,"palette_entries":[2,16,256],
        "versions":["87a","89a"],"programs":["growth-to-full","midstream-reset","kwkwk"],"subblock_sizes":[1,255],
        "logical_screen_by_palette":{"2":[257,193],"16":[129,97],"256":[129,97]},"extension_cells":8,
        "extension_vectors":8,"collected_vectors":80,"uncollected_controls":12,"total_vectors":92,"total_cells":56})
    );
    assert_eq!(record["differences"], json!([]));
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 80);
    let mut ids = BTreeSet::new();
    let mut cells = BTreeMap::new();
    let mut hashes = BTreeSet::new();
    let mut qualify = |v: Vector<'_>| {
        let row = by_id(&record, "cases", &v.id());
        qualify_row(row, v);
        assert!(ids.insert(v.id()));
        *cells.entry(v.cell()).or_insert(0) += 1;
        assert!(hashes.insert(row["gif_sha256"].as_str().unwrap()));
    };
    for entries in [2, 16, 256] {
        for version in ["87a", "89a"] {
            for program in ["growth-to-full", "midstream-reset", "kwkwk"] {
                for block in [1, 255] {
                    for pattern in [1, 2] {
                        qualify(Vector {
                            entries,
                            version,
                            program,
                            block,
                            pattern,
                            extension: None,
                        });
                    }
                }
            }
        }
    }
    for extension in EXTENSIONS {
        qualify(Vector {
            entries: 16,
            version: "89a",
            program: "growth-to-full",
            block: 255,
            pattern: 1,
            extension: Some(extension),
        });
    }
    assert_eq!((ids.len(), cells.len(), hashes.len()), (80, 44, 80));
    assert_eq!(cells.values().filter(|&&count| count == 2).count(), 36);
    assert_eq!(cells.values().filter(|&&count| count == 1).count(), 8);
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        ids
    );
    let mut summary = json!({"widths":(3..=12).collect::<Vec<_>>()});
    for key in [
        "code_count",
        "dictionary_references",
        "clear_count",
        "kwkwk_count",
        "max_next_code",
        "full_dictionary_codes",
    ] {
        let values: Vec<_> = rows.iter().map(|r| n(&r["stream"][key])).collect();
        summary[format!("{key}_range")] =
            json!([values.iter().min().unwrap(), values.iter().max().unwrap()]);
    }
    assert_eq!(record["stream_summary"], summary);
    qualify_controls(&record);
    negative_helpers(&record);
    record
}

fn svg_check(svg: &str, source: Option<&[u8]>, w: usize, h: usize) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let root = doc.root_element();
    assert!(root.has_tag_name("svg"));
    assert_eq!(root.attribute("width"), Some(w.to_string().as_str()));
    assert_eq!(root.attribute("height"), Some(h.to_string().as_str()));
    assert_eq!(
        root.attribute("viewBox"),
        Some(format!("0 0 {w} {h}").as_str())
    );
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
        assert!(
            href == format!(
                "data:image/gif;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(source)
            ),
            "original GIF URI"
        );
        let extent = if image
            .parent_element()
            .is_some_and(|p| p.has_tag_name("symbol"))
        {
            assert_eq!(image.attribute("width"), Some("100%"));
            assert_eq!(image.attribute("height"), Some("100%"));
            let symbol = image.parent_element().unwrap();
            let uses: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name("use"))
                .collect();
            assert_eq!(uses.len(), 1);
            assert_eq!(
                uses[0].attribute("href"),
                Some(format!("#{}", symbol.attribute("id").unwrap()).as_str())
            );
            uses[0]
        } else {
            image
        };
        assert_eq!(extent.attribute("width"), Some(w.to_string().as_str()));
        assert_eq!(extent.attribute("height"), Some(h.to_string().as_str()));
    }
}
fn png_pixels(bytes: &[u8], expected: &[u8], w: usize, h: usize, native: bool) {
    if native {
        super::png_profile_contract::assert_untagged_png(bytes);
        assert_eq!(&bytes[24..26], &[8, 6], "native RGBA8 layout");
    }
    let actual = crate::common::raster::decode_png(bytes);
    assert_eq!((actual.width as usize, actual.height as usize), (w, h));
    assert!(actual.rgba == expected, "exact PNG samples");
    assert!(
        actual.rgba.chunks_exact(4).all(|p| p[3] == 255),
        "opaque samples"
    );
}
fn pdf_extent(bytes: &[u8], w: usize, h: usize) {
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
        assert_eq!(values, [0.0, 0.0, w as f64 * 0.75, h as f64 * 0.75]);
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}
fn artifact(bytes: &[u8], format: OutputFormat, row: &Value) {
    // Row-bound source samples, audited by evidence(), not an inventory policy.
    let expected = decode(&row["expected_first"]["rgba_base64"]);
    let (w, h) = (n(&row["width"]), n(&row["height"]));
    match format {
        OutputFormat::Svg => svg_check(
            std::str::from_utf8(bytes).unwrap(),
            Some(&decode(&row["gif_base64"])),
            w,
            h,
        ),
        OutputFormat::Png => png_pixels(bytes, &expected, w, h, true),
        OutputFormat::Pdf => {
            pdf_extent(bytes, w, h);
            let actual = super::png_profile_contract::pdf_rgba(bytes, w, h);
            assert!(
                actual == expected,
                "exact page-bound PDF image/alpha samples"
            );
            assert!(actual.chunks_exact(4).all(|p| p[3] == 255));
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
    assert!(
        first.artifact().bytes() == again.artifact().bytes(),
        "fresh artifact replay"
    );
    assert_eq!(
        serde_json::to_value(first.report()).unwrap(),
        serde_json::to_value(again.report()).unwrap()
    );
    let mut report = serde_json::to_value(first.report()).unwrap();
    for diagnostic in report["diagnostics"].as_array_mut().unwrap() {
        assert!(!diagnostic["message"].as_str().unwrap().is_empty());
        diagnostic.as_object_mut().unwrap().remove("message");
    }
    (first.artifact().bytes().to_vec(), report)
}
fn decode_report(format: OutputFormat) -> Value {
    json!({"schema_version":1,"diagnostics":[{"code":"image.decode-failed","severity":"warning","format":format.as_str(),
        "effects":["visual"],"element_id":"image","json_pointer":"/files/asset/dataURL","details":{"kind":"asset",
        "asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/gif",
        "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"},
        "recipe_id":if format == OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}}]})
}
fn recovery_artifact(bytes: &[u8], format: OutputFormat, source: &Value) {
    recovery_artifact_at_extent(bytes, format, source, 129, 97);
}
fn recovery_artifact_at_extent(
    bytes: &[u8],
    format: OutputFormat,
    source: &Value,
    width: usize,
    height: usize,
) {
    let mut missing = source.clone();
    missing["files"] = json!({});
    assert!(
        bytes == render(&missing, format).0,
        "exact missing-asset artifact"
    );
    match format {
        OutputFormat::Svg => svg_check(std::str::from_utf8(bytes).unwrap(), None, width, height),
        OutputFormat::Png => {
            let actual = crate::common::raster::decode_png(bytes);
            assert_eq!(
                (actual.width as usize, actual.height as usize),
                (width, height)
            );
        }
        OutputFormat::Pdf => {
            pdf_extent(bytes, width, height);
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
    let mut changed = bytes.to_vec();
    changed.splice(at..at + from.len(), to.bytes());
    changed
}
fn negative_helpers(record: &Value) {
    let row = by_id(record, "cases", "p16-89a-growth-to-full-b255-seed1");
    let bytes = decode(&row["gif_base64"]);
    let wire = inspect(&bytes).unwrap();
    let expected = decode(&row["expected_first"]["rgba_base64"]);
    let other = by_id(record, "cases", "p16-89a-growth-to-full-b255-seed2");
    assert_ne!(
        row["authored_indices_sha256"],
        other["authored_indices_sha256"]
    );
    assert_ne!(
        row["expected_first"]["rgba_sha256"],
        other["expected_first"]["rgba_sha256"]
    );
    // Packed-code changes reach the independent reader, including its width
    // transition (not just container framing). Different output also rejects.
    for bit in [wire.minimum + 1, n(&wire.stream.stats["events"][1]["bit"])] {
        let mut changed = wire.packed.clone();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(lzw(&changed, wire.minimum, 129 * 97)
            .map_or(true, |s| s.indices != wire.stream.indices));
    }
    assert_eq!(lzw(&[0x7c, 1], 2, 1).unwrap_err(), "undefined code");
    assert_eq!(lzw(&[0x04, 0x41, 0x10], 2, 4).unwrap_err(), "missing EOI");
    assert_eq!(lzw(&[0x2c], 2, 1).unwrap_err(), "pixel count");
    assert_eq!(lzw(&[0x44, 0x01], 2, 0).unwrap_err(), "pixel overflow");
    assert_eq!(lzw(&[0x44, 0x81], 2, 1).unwrap_err(), "padding");
    let mut after = wire.packed.clone();
    after.push(0);
    assert_eq!(
        lzw(&after, wire.minimum, 129 * 97).unwrap_err(),
        "bytes after EOI"
    );
    let mut palette = bytes.clone();
    palette[13..16].copy_from_slice(&[0, 0, 0]);
    let wrong = inspect(&palette).unwrap();
    assert_ne!(
        expand(&wrong.stream.indices, &wrong.palette),
        expected,
        "wire palette discrimination"
    );
    // Shrink GCT but retain the original packed stream: isolate bad indices.
    let mut short = bytes.clone();
    short[10] = 0xf0;
    short.drain(19..61);
    assert_eq!(inspect(&short).unwrap_err(), "palette index");
    let svg = row["reference"]["svg"].as_str().unwrap();
    rejects(|| {
        svg_check(
            &svg.replace("viewBox=\"0 0 129 97\"", "viewBox=\"0 0 128 97\""),
            Some(&bytes),
            129,
            97,
        )
    });
    rejects(|| svg_check(svg, Some(&palette), 129, 97));
    let (native_svg, _) = render(&row["scene"], OutputFormat::Svg);
    let changed = replace_once(&native_svg, "width=\"129\"", "width=\"128\"");
    rejects(|| artifact(&changed, OutputFormat::Svg, row));
    let (png, _) = render(&row["scene"], OutputFormat::Png);
    artifact(&png, OutputFormat::Png, row);
    let mut tagged = png.clone();
    let mut chunk = 1_u32.to_be_bytes().to_vec();
    chunk.extend(b"sRGB\0");
    chunk.extend(crc32fast::hash(&chunk[4..]).to_be_bytes());
    tagged.splice(33..33, chunk);
    rejects(|| artifact(&tagged, OutputFormat::Png, row));
    for mutation in ["rgb", "palette", "alpha", "extent"] {
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
            "palette" => samples[0] ^= 1,
            "alpha" => samples[3] = 254,
            "extent" => {}
            _ => unreachable!(),
        }
        let (w, h) = if mutation == "extent" {
            (97, 129)
        } else {
            (129, 97)
        };
        let mut changed = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut changed, w, h);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&samples)
                .unwrap();
        }
        rejects(|| artifact(&changed, OutputFormat::Png, row));
        if mutation == "alpha" {
            rejects(|| png_pixels(&changed, &samples, 129, 97, true));
        }
    }
    let (pdf, _) = render(&row["scene"], OutputFormat::Pdf);
    artifact(&pdf, OutputFormat::Pdf, row);
    let inflated = crate::common::pdf_inspect::inflate_streams(&pdf);
    for (from, to) in [
        ("/Im0 Do", "/Im9 Do"),
        ("/ICCBased ", "/DeviceRGB "),
        ("/OutputIntents ", "/UnusedIntents "),
        ("/Width 129\n", "/Width 128\n"),
        ("/MediaBox [0 0 96.75 72.75]", "/MediaBox [0 0 96.75 72.76]"),
        ("/CropBox [0 0 96.75 72.75]", "/CropBox [0 0 96.75 72.76]"),
    ] {
        let changed = replace_once(&inflated, from, to);
        rejects(|| artifact(&changed, OutputFormat::Pdf, row));
    }
    // Keep the correct RGB stream present but detach it from the page binding.
    let text = crate::common::pdf_inspect::pdf_text(&pdf);
    let image_ref = text
        .split_once("/Im0 ")
        .unwrap()
        .1
        .split_once(" 0 R")
        .unwrap()
        .0;
    let profile_ref = text
        .split_once("/ICCBased ")
        .unwrap()
        .1
        .split_once(" 0 R")
        .unwrap()
        .0;
    let detached = replace_once(
        &inflated,
        &format!("/Im0 {image_ref} 0 R"),
        &format!("/Im0 {profile_ref} 0 R"),
    );
    rejects(|| artifact(&detached, OutputFormat::Pdf, row));
    // A well-framed, correctly sized gray mask with one nonopaque sample.
    // Test the actual alpha plane rather than merely removing an optional mask.
    let mut masked = replace_once(
        &inflated,
        "/Subtype /Image\n",
        "/Subtype /Image\n/SMask 99999 0 R\n",
    );
    masked.extend(format!("\n99999 0 obj\n<< /Type /XObject /Subtype /Image /Width 129 /Height 97 /BitsPerComponent 8 /ColorSpace /DeviceGray /Length {} >>\nstream\n",129*97).bytes());
    let mut alpha = vec![255; 129 * 97];
    alpha[0] = 254;
    masked.extend(alpha);
    masked.extend(b"\nendstream\nendobj\n");
    let masked_samples = super::png_profile_contract::pdf_rgba(&masked, 129, 97);
    assert_eq!(
        masked_samples[3], 254,
        "mutation reaches the bound alpha plane"
    );
    assert_eq!(&masked_samples[..3], &expected[..3]);
    assert_eq!(&masked_samples[4..], &expected[4..]);
    rejects(|| artifact(&masked, OutputFormat::Pdf, row));
    rejects(|| artifact(&pdf, OutputFormat::Pdf, other));
    isolated_eoi_pair();
}

fn isolated_eoi_pair() {
    // Four literal-zero pixels, each preceded by clear: [4,0,4,0,4,0,4,0].
    // Eight three-bit codes occupy exactly 24 bits; there is no ambiguous
    // partial code/padding and no dictionary growth. Only the positive appends
    // EOI=5 (three bits plus five zero padding bits). Both containers terminate.
    let without_eoi = [0x04, 0x41, 0x10];
    let with_eoi = [0x04, 0x41, 0x10, 0x05];
    let container = |packed: &[u8]| {
        let mut gif = b"GIF89a\x04\x00\x01\x00\xf0\x00\x00".to_vec();
        gif.extend([37, 37, 37, 211, 211, 211]); // independently authored GCT
        gif.extend([0x2c, 0, 0, 0, 0, 4, 0, 1, 0, 0, 2]);
        gif.extend(blocks(packed, 255));
        gif.push(0x3b);
        gif
    };
    let valid = container(&with_eoi);
    let invalid = container(&without_eoi);
    assert_eq!(&valid[..30], &invalid[..30]);
    assert_eq!(&valid[30..], &[4, 0x04, 0x41, 0x10, 0x05, 0, 0x3b]);
    assert_eq!(&invalid[30..], &[3, 0x04, 0x41, 0x10, 0, 0x3b]);
    let wire = inspect(&valid).unwrap();
    assert_eq!((wire.width, wire.height), (4, 1));
    assert_eq!(wire.packed, with_eoi);
    assert_eq!(wire.stream.indices, [0; 4]);
    assert_eq!(wire.stream.stats["code_count"], 9);
    assert_eq!(wire.stream.stats["clear_count"], 4);
    assert_eq!(wire.stream.stats["widths"], json!([3]));
    assert_eq!(inspect(&invalid).unwrap_err(), "missing EOI");
    let expected = [37, 37, 37, 255].repeat(4);
    assert_eq!(expand(&wire.stream.indices, &wire.palette), expected);

    // Dependency diagnostic only: disabling EOI validation admits all four
    // authored indices and a properly terminated, single-image container.
    // Neither this codec nor the production renderer supplies expected samples.
    let mut permissive = gif::DecodeOptions::new();
    permissive.set_color_output(gif::ColorOutput::Indexed);
    permissive.check_frame_consistency(true);
    permissive.check_lzw_end_code(false);
    let mut decoder = permissive
        .read_info(std::io::Cursor::new(&invalid))
        .unwrap();
    let frame = decoder.read_next_frame().unwrap().unwrap();
    assert_eq!((frame.width, frame.height), (4, 1));
    assert_eq!(frame.buffer.as_ref(), &[0; 4]);
    assert!(decoder.read_next_frame().unwrap().is_none());

    let encode = |bytes: &[u8]| json!(base64::engine::general_purpose::STANDARD.encode(bytes));
    let row = json!({"width":4,"height":1,"gif_base64":encode(&valid),
        "expected_first":{"rgba_base64":encode(&expected)}});
    let mut missing_row = row.clone();
    missing_row["gif_base64"] = encode(&invalid);
    let positive = scene(&row["gif_base64"], 4, 1);
    let negative = scene(&missing_row["gif_base64"], 4, 1);
    let mut failed = Vec::new();
    for (label, source, row) in [
        ("valid EOI", &positive, &row),
        ("missing EOI", &negative, &missing_row),
    ] {
        for format in FORMATS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (bytes, report) = render(source, format);
                // All four pixels precede the missing EOI, so the reference
                // draws the complete frame (gif-container missing-eoi-after-full).
                assert_eq!(report, json!({"schema_version":1,"diagnostics":[]}));
                artifact(&bytes, format, row);
            }));
            if result.is_err() {
                failed.push(format!("{label}: {format:?}"));
            }
        }
    }
    eprintln!(
        "GIF isolated EOI pair: {}/6 surfaces passed (each fresh-replayed)",
        6 - failed.len()
    );
    assert!(failed.is_empty(), "isolated EOI pair failures: {failed:?}");
}

#[test]
fn valid_lzw_matrix_and_extensions_match_frozen_reference_exactly() {
    let record = evidence();
    let mut failed = Vec::new();
    let mut passed_vectors = 0;
    let mut passed_surfaces = 0;
    let mut cells = BTreeMap::<String, bool>::new();
    for row in record["cases"].as_array().unwrap() {
        let mut passed = true;
        for format in FORMATS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (bytes, report) = render(&row["scene"], format);
                assert_eq!(report, json!({"schema_version":1,"diagnostics":[]}));
                artifact(&bytes, format, row);
            }));
            if result.is_ok() {
                passed_surfaces += 1;
            } else {
                passed = false;
                failed.push(format!("{}: {format:?}", row["id"]));
            }
        }
        if passed {
            passed_vectors += 1;
        }
        *cells
            .entry(row["cell"].as_str().unwrap().to_owned())
            .or_insert(true) &= passed;
    }
    let matrix = cells
        .iter()
        .filter(|(c, ok)| !c.starts_with("extension-") && **ok)
        .count();
    let extensions = cells
        .iter()
        .filter(|(c, ok)| c.starts_with("extension-") && **ok)
        .count();
    eprintln!("GIF LZW qualified: {matrix}/36 matrix cells, {extensions}/8 extension cells; {passed_vectors}/80 vectors; {passed_surfaces}/240 surfaces (each fresh-replayed)");
    assert!(failed.is_empty(), "unqualified valid surfaces: {failed:?}");
}

#[test]
fn malformed_lzw_controls_have_exact_decode_failed_recovery() {
    let record = evidence();
    let mut failed = Vec::new();
    let mut cells = 0;
    let mut surfaces = 0;
    // Only controls the pinned reference rejects, or whose first frame it
    // paints partially (the declared divergence), still recover; the others
    // draw like the reference (gif_container_contract).
    for id in MALFORMED
        .into_iter()
        .filter(|id| super::gif_container_contract::strictly_rejected("lzw", id))
    {
        let row = by_id(&record, "controls", id);
        // Controls use a fixed 12,513-pixel display viewport, independently of
        // the binary GIF's 49,601-pixel intrinsic screen and malformed descriptor.
        let source = scene(&row["gif_base64"], 129, 97);
        let mut passed = true;
        for format in FORMATS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (bytes, report) = render(&source, format);
                assert_eq!(report, decode_report(format), "{id}: {format:?}");
                recovery_artifact(&bytes, format, &source);
            }));
            if result.is_ok() {
                surfaces += 1;
            } else {
                passed = false;
                failed.push(format!("{id}: {format:?}"));
            }
        }
        if passed {
            cells += 1;
        }
    }
    eprintln!("GIF LZW malformed qualified: {cells}/7 strict cells; {surfaces}/21 surfaces (each fresh-replayed)");
    assert_eq!(cells, 7);
    assert!(
        failed.is_empty(),
        "unqualified malformed surfaces: {failed:?}"
    );
}
