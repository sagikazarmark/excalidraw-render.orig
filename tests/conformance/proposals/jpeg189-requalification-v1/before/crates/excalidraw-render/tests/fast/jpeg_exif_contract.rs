//! Bounded EXIF precedence: 82 cells / 164 vectors, independently authored APP1
//! bytes and pinned official-browser evidence. #189 tracks the 36 failing cells;
//! their exact measurements characterize defects, not approved exceptions.
//! Browser selection is first APP1 (even invalid/absent/bad magic), then first
//! valid orientation within its IFD. Runtime selection is last pre-scan APP1,
//! then first tag, even invalid. Late/between-scan APP1 is ignored by both.
//! PDF checks inspect the page-bound native image and page extents, not a viewer.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonError, ComparisonMode, ComparisonReport, SsimOutcome,
    VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

// Each tuple authors (signal, browser orientation, current runtime orientation).
// Selection never comes from the production metadata parser or measured pixels.
const QUALIFIED: &[(&str, u8, u8)] = &[
    ("control-1", 1, 1),
    ("control-3", 3, 3),
    ("control-6", 6, 6),
    ("control-8", 8, 8),
    ("duplicate-tags-6-3", 6, 6),
    ("duplicate-tags-3-6", 3, 3),
    ("valid-tag-then-invalid", 6, 6),
    ("late-only", 1, 1),
    ("early-3-late-6", 3, 3),
    ("early-6-late-3", 6, 6),
];
const BETWEEN_SCANS: &[(&str, u8, u8)] = &[
    ("between-scans-only", 1, 1),
    ("early-3-between-6", 3, 3),
    ("early-6-between-3", 6, 6),
];
const FAILURES: &[(&str, u8, u8)] = &[
    ("duplicate-6-3", 6, 3),
    ("duplicate-3-6", 3, 6),
    ("invalid-then-valid", 1, 6),
    ("absent-tag-then-valid", 1, 6),
    ("bad-magic-then-valid", 1, 6),
    ("valid-then-invalid", 6, 1),
    ("valid-then-absent-tag", 6, 1),
    ("valid-then-bad-magic", 6, 1),
    ("invalid-tag-then-valid", 6, 1),
];

fn orientations(row: &Value) -> (u8, u8) {
    let &(_, browser, runtime) = QUALIFIED
        .iter()
        .chain(BETWEEN_SCANS)
        .chain(FAILURES)
        .find(|(signal, _, _)| row["signal"] == *signal)
        .unwrap();
    (browser, runtime)
}
fn decode(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn pinned_json(bytes: &[u8], expected: &str) -> Value {
    assert_eq!(hash(bytes), expected);
    serde_json::from_slice(bytes).unwrap()
}
fn evidence() -> Value {
    let record = pinned_json(
        include_bytes!("../../../../tests/oracle/jpeg-exif/reference.json"),
        "cd811bca81e24aaeb652c86d3e6dfd7b4aeb65d8050764da34b129530aac6ecf",
    );
    assert_eq!(record["schema"], "excalidraw-render/jpeg-exif-reference/v1");
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(hash(&bytes), source["sha256"].as_str().unwrap());
    }
    // Bind unchanged helpers, including resource binding/ICC/plane decoding.
    for (bytes, expected) in [
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
        assert_eq!(hash(bytes), expected);
    }
    reconstruct_contract(&record);
    record
}
fn inputs() -> Value {
    pinned_json(
        include_bytes!("../../../../tests/oracle/jpeg-color/inputs.json"),
        "acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258",
    )
}
fn untagged_evidence() -> Value {
    let record = pinned_json(
        include_bytes!("../../../../tests/oracle/jpeg-color/reference.json"),
        "9b5ab60a18e83b52c582ccb38728b71633b11b592ffe540bab2c796857694638",
    );
    assert_eq!(
        record["schema"],
        "excalidraw-render/jpeg-color-reference/v1"
    );
    record
}
fn base<'a>(record: &'a Value, row: &Value) -> &'a Value {
    let matches: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["id"] == row["base_id"])
        .collect();
    assert_eq!(matches.len(), 1);
    matches[0]
}
fn dimensions(pattern: &str, orientation: u8) -> (u32, u32) {
    let (w, h) = match pattern {
        "ramp" => (33, 25),
        "edges" => (48, 32),
        _ => panic!("unexpected pattern: {pattern}"),
    };
    match orientation {
        1 | 3 => (w, h),
        6 | 8 => (h, w),
        _ => panic!("unexpected orientation: {orientation}"),
    }
}

// Test-side TIFF writer: SHORT orientation values in IFD0, inline count=1.
fn tiff(values: &[u16], big: bool, magic: u16) -> Vec<u8> {
    let short = |n: u16| {
        if big {
            n.to_be_bytes()
        } else {
            n.to_le_bytes()
        }
    };
    let long = |n: u32| {
        if big {
            n.to_be_bytes()
        } else {
            n.to_le_bytes()
        }
    };
    let mut bytes = b"Exif\0\0".to_vec();
    bytes.extend_from_slice(if big { b"MM" } else { b"II" });
    bytes.extend(short(magic));
    bytes.extend(long(8));
    bytes.extend(short(u16::try_from(values.len()).unwrap()));
    for &value in values {
        bytes.extend(short(0x112));
        bytes.extend(short(3));
        bytes.extend(long(1));
        bytes.extend(short(value));
        bytes.extend(short(0));
    }
    bytes.extend(long(0));
    bytes
}
fn app1(payload: &[u8]) -> Vec<u8> {
    let mut segment = vec![0xff, 0xe1];
    segment.extend(u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    segment.extend_from_slice(payload);
    segment
}
fn authored_payloads(signal: &str, big: bool) -> (Vec<Vec<u8>>, Option<Vec<u8>>) {
    let tag = |n| tiff(&[n], big, 42);
    let early = match signal {
        "control-1" => vec![tag(1)],
        "control-3" => vec![tag(3)],
        "control-6" => vec![tag(6)],
        "control-8" => vec![tag(8)],
        "duplicate-6-3" => vec![tag(6), tag(3)],
        "duplicate-3-6" => vec![tag(3), tag(6)],
        "invalid-then-valid" => vec![tag(0), tag(6)],
        "valid-then-invalid" => vec![tag(6), tag(0)],
        "absent-tag-then-valid" => vec![tiff(&[], big, 42), tag(6)],
        "valid-then-absent-tag" => vec![tag(6), tiff(&[], big, 42)],
        "bad-magic-then-valid" => vec![tiff(&[6], big, 0), tag(6)],
        "valid-then-bad-magic" => vec![tag(6), tiff(&[6], big, 0)],
        "duplicate-tags-6-3" => vec![tiff(&[6, 3], big, 42)],
        "duplicate-tags-3-6" => vec![tiff(&[3, 6], big, 42)],
        "invalid-tag-then-valid" => vec![tiff(&[0, 6], big, 42)],
        "valid-tag-then-invalid" => vec![tiff(&[6, 0], big, 42)],
        "late-only" | "between-scans-only" => vec![],
        "early-3-late-6" | "early-3-between-6" => vec![tag(3)],
        "early-6-late-3" | "early-6-between-3" => vec![tag(6)],
        _ => panic!("unexpected signal: {signal}"),
    };
    let late = match signal {
        "late-only" | "between-scans-only" | "early-3-late-6" | "early-3-between-6" => Some(tag(6)),
        "early-6-late-3" | "early-6-between-3" => Some(tag(3)),
        _ => None,
    };
    (early, late)
}

// Walk the independently pinned JPEG, not the EXIF record's claimed offsets.
// Entropy byte stuffing and restart markers cannot masquerade as scan boundaries.
fn markers(bytes: &[u8]) -> Vec<(u8, usize)> {
    assert_eq!(&bytes[..2], b"\xff\xd8");
    let mut at = 2;
    let mut result = Vec::new();
    loop {
        assert_eq!(bytes[at], 255);
        let kind = bytes[at + 1];
        result.push((kind, at));
        if kind == 0xd9 {
            assert_eq!(at + 2, bytes.len());
            return result;
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        assert!(length >= 2);
        at += 2 + length;
        if kind == 0xda {
            while at + 1 < bytes.len() {
                if bytes[at] == 255 && !matches!(bytes[at + 1], 0 | 0xd0..=0xd7) {
                    break;
                }
                at += 1;
            }
        }
    }
}
fn reconstruct_contract(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    let mut expected = BTreeSet::new();
    let mut cells = BTreeSet::new();
    for mode in ["baseline", "progressive"] {
        for order in ["little", "big"] {
            for &(signal, _, _) in QUALIFIED
                .iter()
                .chain(FAILURES)
                .chain(BETWEEN_SCANS.iter().filter(|_| mode == "progressive"))
            {
                let cell = format!("{mode}-{order}-{signal}");
                cells.insert(cell.clone());
                for pattern in ["ramp", "edges"] {
                    expected.insert(format!("{cell}-{pattern}"));
                }
            }
        }
    }
    assert_eq!((cells.len(), expected.len(), rows.len()), (82, 164, 164));
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(
        rows.iter()
            .map(|r| r["cell"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        cells
    );
    let inputs = inputs();
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let mode = row["mode"].as_str().unwrap();
        let order = row["byte_order"].as_str().unwrap();
        let signal = row["signal"].as_str().unwrap();
        let pattern = row["pattern"].as_str().unwrap();
        assert_eq!(row["cell"], format!("{mode}-{order}-{signal}"));
        assert_eq!(id, format!("{mode}-{order}-{signal}-{pattern}"));
        assert_eq!(row["base_id"], format!("rgb-1x1-{mode}-{pattern}"));
        let base = base(&inputs, row);
        assert_eq!(base["model"], "rgb");
        assert_eq!(base["sampling"], json!([1, 1]));
        assert_eq!(base["mode"], mode);
        assert_eq!(base["pattern"], pattern);
        let (w, h) = dimensions(pattern, 1);
        assert_eq!(
            (
                base["width"].as_u64().unwrap(),
                base["height"].as_u64().unwrap()
            ),
            (u64::from(w), u64::from(h))
        );
        assert_eq!(row["source_width"], w);
        assert_eq!(row["source_height"], h);
        let (w, h) = dimensions(pattern, orientations(row).0);
        assert_eq!(row["width"], w);
        assert_eq!(row["height"], h);
        assert_eq!(row["scene"]["elements"].as_array().unwrap().len(), 1);
        assert_eq!(row["scene"]["elements"][0]["width"], w);
        assert_eq!(row["scene"]["elements"][0]["height"], h);
        assert_eq!(row["scene"]["files"]["asset"]["mimeType"], "image/jpeg");
        assert_eq!(
            row["scene"]["files"]["asset"]["dataURL"],
            format!(
                "data:image/jpeg;base64,{}",
                row["jpeg_base64"].as_str().unwrap()
            )
        );

        let source = decode(&base["jpeg_base64"]);
        assert_eq!(hash(&source), base["jpeg_sha256"].as_str().unwrap());
        let markers = markers(&source);
        assert!(!markers.iter().any(|(kind, _)| matches!(kind, 0xe1 | 0xe2)));
        let frames: Vec<_> = markers
            .iter()
            .filter(|(kind, _)| matches!(kind, 0xc0 | 0xc2))
            .collect();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, if mode == "progressive" { 0xc2 } else { 0xc0 });
        let scans: Vec<_> = markers.iter().filter(|(kind, _)| *kind == 0xda).collect();
        assert_eq!(scans.len() > 1, mode == "progressive");
        let (early, late) = authored_payloads(signal, order == "big");
        assert_eq!(
            row["early_app1_base64"]
                .as_array()
                .unwrap()
                .iter()
                .map(decode)
                .collect::<Vec<_>>(),
            early,
            "{id}"
        );
        assert_eq!(
            row["late_app1_base64"]
                .as_str()
                .map(|_| decode(&row["late_app1_base64"])),
            late,
            "{id}"
        );
        let offset = late.as_ref().map(|_| {
            if signal.contains("between") {
                scans[1].1
            } else {
                markers.last().unwrap().1
            }
        });
        assert_eq!(row["late_source_offset"], json!(offset), "{id}");
        let mut jpeg = source[..2].to_vec();
        for payload in early {
            jpeg.extend(app1(&payload));
        }
        if let Some(offset) = offset {
            jpeg.extend_from_slice(&source[2..offset]);
            jpeg.extend(app1(late.as_ref().unwrap()));
            jpeg.extend_from_slice(&source[offset..]);
        } else {
            jpeg.extend_from_slice(&source[2..]);
        }
        assert_eq!(
            jpeg,
            decode(&row["jpeg_base64"]),
            "{id}: exact APP1 reconstruction"
        );
        assert_eq!(hash(&jpeg), row["jpeg_sha256"].as_str().unwrap());
    }
}

fn raster(bytes: &[u8]) -> CanonicalRgba8 {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    bytes.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => bytes,
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => panic!("reference PNG type"),
    };
    let image = CanonicalRgba8::new(info.width, info.height, pixels).unwrap();
    assert!(image.rgba().chunks_exact(4).all(|p| p[3] == 255));
    image
}
fn extent(image: &CanonicalRgba8) -> (u32, u32) {
    (image.width(), image.height())
}
// Independent pixel permutation: no renderer, image decoder orientation API,
// resampling, or production transform is used to compute expected geometry.
fn orient(source: &CanonicalRgba8, orientation: u8) -> CanonicalRgba8 {
    let (w, h) = extent(source);
    let (out_w, out_h) = if matches!(orientation, 6 | 8) {
        (h, w)
    } else {
        (w, h)
    };
    let mut pixels = vec![0; source.rgba().len()];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match orientation {
                1 => (x, y),
                3 => (w - 1 - x, h - 1 - y),
                6 => (h - 1 - y, x),
                8 => (y, w - 1 - x),
                _ => panic!("unexpected orientation"),
            };
            let from = ((y * w + x) * 4) as usize;
            let to = ((dy * out_w + dx) * 4) as usize;
            pixels[to..to + 4].copy_from_slice(&source.rgba()[from..from + 4]);
        }
    }
    CanonicalRgba8::new(out_w, out_h, pixels).unwrap()
}
fn comparison(
    reference: &CanonicalRgba8,
    actual: &CanonicalRgba8,
) -> Result<ComparisonReport, ComparisonError> {
    compare(
        reference,
        actual,
        VisualClass::GeometryImage,
        &[],
        ComparisonMode::Canonical,
    )
}
fn control<'a>(rows: &'a [Value], row: &Value, orientation: u8) -> &'a Value {
    let matches: Vec<_> = rows
        .iter()
        .filter(|c| {
            c["mode"] == row["mode"]
                && c["byte_order"] == row["byte_order"]
                && c["pattern"] == row["pattern"]
                && c["signal"] == format!("control-{orientation}")
        })
        .collect();
    assert_eq!(matches.len(), 1);
    matches[0]
}
fn reference_contract(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    let untagged = untagged_evidence();
    let inputs = inputs();
    for row in rows {
        let base = base(&untagged, row);
        assert_eq!(base["jpeg_base64"], self::base(&inputs, row)["jpeg_base64"]);
        let orientation = orientations(row).0;
        for key in ["png", "svg_png"] {
            let reference = raster(&decode(&row["reference"][key]));
            let untagged = raster(&decode(&base["reference"][key]));
            assert_eq!(
                extent(&untagged),
                dimensions(row["pattern"].as_str().unwrap(), 1)
            );
            assert_eq!(
                extent(&reference),
                dimensions(row["pattern"].as_str().unwrap(), orientation)
            );
            assert_eq!(
                reference,
                orient(&untagged, orientation),
                "{}: {key}: independent browser permutation",
                row["id"]
            );
            let matches: Vec<_> = [1, 3, 6, 8]
                .into_iter()
                .filter(|&n| reference == raster(&decode(&control(rows, row, n)["reference"][key])))
                .collect();
            assert_eq!(
                matches,
                [orientation],
                "{}: {key}: unique control",
                row["id"]
            );
        }
        svg_check(row["reference"]["svg"].as_str().unwrap(), row);
    }
}
fn svg_check(svg: &str, row: &Value) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let root = doc.root_element();
    let (w, h) = dimensions(row["pattern"].as_str().unwrap(), orientations(row).0);
    assert!(root.has_tag_name("svg"));
    assert_eq!(root.attribute("width").unwrap().parse::<u32>().unwrap(), w);
    assert_eq!(root.attribute("height").unwrap().parse::<u32>().unwrap(), h);
    assert_eq!(
        root.attribute("viewBox"),
        Some(format!("0 0 {w} {h}").as_str())
    );
    let images: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), 1);
    let href = images[0]
        .attribute("href")
        .or_else(|| images[0].attribute(("http://www.w3.org/1999/xlink", "href")));
    assert_eq!(href, row["scene"]["files"]["asset"]["dataURL"].as_str());
}
fn pdf_extent(bytes: &[u8], (w, h): (u32, u32)) {
    let text = crate::common::pdf_inspect::pdf_text(bytes);
    let pages: Vec<_> = text.split("/Type /Page\n").skip(1).collect();
    assert_eq!(pages.len(), 1);
    let page = pages[0].split("endobj").next().unwrap();
    for key in ["/MediaBox [", "/CropBox ["] {
        let bbox = page.split_once(key).unwrap().1.split_once(']').unwrap().0;
        let values: Vec<f64> = bbox
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        assert_eq!(values, [0.0, 0.0, f64::from(w) * 0.75, f64::from(h) * 0.75]);
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}
fn pdf_pixels(bytes: &[u8], (w, h): (u32, u32)) -> CanonicalRgba8 {
    // Actual intrinsics are authored per signal; never force oracle dimensions
    // into the helper on known failures (the page and image extents differ).
    let rgba = super::png_profile_contract::pdf_rgba(bytes, w as usize, h as usize);
    assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
    CanonicalRgba8::new(w, h, rgba).unwrap()
}
fn render(scene: &Value, format: OutputFormat) -> Vec<u8> {
    let scene = SceneDocument::parse(&serde_json::to_vec(scene).unwrap()).unwrap();
    let request = RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .build()
        .unwrap();
    let first = Renderer::new().render(&scene, &request).unwrap();
    let second = Renderer::new().render(&scene, &request).unwrap();
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(first.report()).unwrap(),
        json!({"schema_version":1,"diagnostics":[]})
    );
    first.artifact().bytes().to_vec()
}
fn png_failure_pin(signal: &str, pattern: &str) -> (u64, u64) {
    let pins = match signal {
        "duplicate-6-3" => [(775, 0x3fdd1a3de3bd40df), (1313, 0x3fba0fdbb322801d)],
        "duplicate-3-6" => [(772, 0x3fc3534e318124f5), (1302, 0x3fbc1dbe2fad4fcd)],
        "invalid-then-valid" | "absent-tag-then-valid" | "bad-magic-then-valid" => {
            [(775, 0x3fdd21fb486f170b), (1313, 0x3fba0fdbb3228058)]
        }
        "valid-then-invalid"
        | "valid-then-absent-tag"
        | "valid-then-bad-magic"
        | "invalid-tag-then-valid" => [(774, 0x3fc33e73b52b94ba), (1304, 0x3fbc24ff8a3f2806)],
        _ => panic!("not a pinned failure: {signal}"),
    };
    pins[match pattern {
        "ramp" => 0,
        "edges" => 1,
        _ => unreachable!(),
    }]
}
fn check(row: &Value, rows: &[Value], inputs: &Value) {
    let id = row["id"].as_str().unwrap();
    let pattern = row["pattern"].as_str().unwrap();
    let (browser, runtime) = orientations(row);
    let qualified = browser == runtime;
    let expected_dims = dimensions(pattern, browser);
    let actual_dims = dimensions(pattern, runtime);
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let bytes = render(&row["scene"], format);
        match format {
            OutputFormat::Svg => svg_check(std::str::from_utf8(&bytes).unwrap(), row),
            OutputFormat::Png => {
                super::png_profile_contract::assert_untagged_png(&bytes);
                let actual = crate::common::raster::decode_png(&bytes).into_canonical();
                assert_eq!(extent(&actual), expected_dims, "{id}: PNG layout");
                assert!(actual.rgba().chunks_exact(4).all(|p| p[3] == 255));
                for key in ["png", "svg_png"] {
                    let expected = raster(&decode(&row["reference"][key]));
                    let result = comparison(&expected, &actual).unwrap();
                    assert_eq!(
                        result.passed(),
                        qualified,
                        "{id}: {key}: {:?}",
                        result.whole
                    );
                    if !qualified {
                        let (changed, bits) =
                            png_failure_pin(row["signal"].as_str().unwrap(), pattern);
                        assert_eq!(result.whole.changed_pixels, changed, "{id}: {key}");
                        let SsimOutcome::Score(score) = result.whole.ssim else {
                            panic!("{id}: expected finite SSIM score")
                        };
                        assert_eq!(score.to_bits(), bits, "{id}: {key}");
                        assert!(!result.whole.changed_pixels_pass && !result.whole.ssim_pass);
                    }
                }
            }
            OutputFormat::Pdf => {
                pdf_extent(&bytes, expected_dims);
                let actual = pdf_pixels(&bytes, actual_dims);
                let single = control(rows, row, runtime);
                let single_bytes = render(&single["scene"], OutputFormat::Pdf);
                pdf_extent(&single_bytes, actual_dims);
                assert_eq!(
                    actual,
                    pdf_pixels(&single_bytes, actual_dims),
                    "{id}: exact single-orientation bound control"
                );

                let base = base(inputs, row);
                let mut scene = row["scene"].clone();
                let (w, h) = dimensions(pattern, 1);
                scene["elements"][0]["width"] = json!(w);
                scene["elements"][0]["height"] = json!(h);
                scene["files"]["asset"]["dataURL"] = json!(format!(
                    "data:image/jpeg;base64,{}",
                    base["jpeg_base64"].as_str().unwrap()
                ));
                let untagged = render(&scene, OutputFormat::Pdf);
                pdf_extent(&untagged, (w, h));
                let native = pdf_pixels(&untagged, (w, h));
                assert_eq!(
                    actual,
                    orient(&native, runtime),
                    "{id}: independently permuted untagged PDF samples"
                );
                let intended = orient(&native, browser);
                assert_eq!(extent(&intended), expected_dims);
                assert_eq!(
                    actual == intended,
                    qualified,
                    "{id}: structural orientation judgment"
                );
                for key in ["png", "svg_png"] {
                    // Color budget is checked at native size against the browser
                    // control for the *authored actual* orientation. It does not
                    // approve that orientation for a failing precedence signal.
                    let actual_control = raster(&decode(&single["reference"][key]));
                    assert!(
                        comparison(&actual_control, &actual).unwrap().passed(),
                        "{id}: PDF native color budget"
                    );
                    let expected = raster(&decode(&row["reference"][key]));
                    let judgment = comparison(&expected, &actual);
                    if qualified {
                        assert!(judgment.unwrap().passed(), "{id}: PDF bound image");
                    } else {
                        assert_eq!(
                            actual_dims,
                            (expected_dims.1, expected_dims.0),
                            "{id}: wrong dimensions proved"
                        );
                        assert_eq!(
                            judgment,
                            Err(ComparisonError::DimensionMismatch {
                                reference: expected_dims,
                                candidate: actual_dims
                            }),
                            "{id}: exact PDF failure judgment"
                        );
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn evidence_has_closed_matrix_and_reconstructible_app1_precedence() {
    let record = evidence();
    reference_contract(&record);
}

#[test]
fn orientation_controls_discriminate_all_four_pixel_permutations() {
    let record = evidence();
    reference_contract(&record);
    discrimination_contract(&record);
}

fn discrimination_contract(record: &Value) {
    let rows = record["cases"].as_array().unwrap();
    let controls: Vec<_> = rows.iter().filter(|r| r["signal"] == "control-1").collect();
    assert_eq!(controls.len(), 8);
    for row in controls {
        for key in ["png", "svg_png"] {
            for a in [1, 3, 6, 8] {
                for b in [1, 3, 6, 8] {
                    let left = raster(&decode(&control(rows, row, a)["reference"][key]));
                    let right = raster(&decode(&control(rows, row, b)["reference"][key]));
                    assert_eq!(left == right, a == b);
                    if extent(&left) == extent(&right) {
                        assert_eq!(
                            comparison(&left, &right).unwrap().passed(),
                            a == b,
                            "{}: {key}: {a} vs {b}",
                            row["id"]
                        );
                    } else {
                        assert_eq!(
                            comparison(&left, &right),
                            Err(ComparisonError::DimensionMismatch {
                                reference: extent(&left),
                                candidate: extent(&right)
                            })
                        );
                    }
                }
            }
        }
    }
}

fn check_partition(qualified: bool, cells: usize, vectors: usize) {
    let record = evidence();
    reference_contract(&record);
    discrimination_contract(&record);
    let rows = record["cases"].as_array().unwrap();
    let selected: Vec<_> = rows
        .iter()
        .filter(|r| {
            let (browser, runtime) = orientations(r);
            (browser == runtime) == qualified
        })
        .collect();
    assert_eq!(selected.len(), vectors);
    assert_eq!(
        selected
            .iter()
            .map(|r| r["cell"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        cells
    );
    for pattern in ["ramp", "edges"] {
        assert_eq!(
            selected.iter().filter(|r| r["pattern"] == pattern).count(),
            cells
        );
    }
    let inputs = inputs();
    for row in selected {
        check(row, rows, &inputs);
    }
}
#[test]
fn qualified_exif_precedence_matches_reference() {
    check_partition(true, 46, 92);
}
#[test]
fn known_exif_precedence_failures_have_exact_png_pins_and_pdf_wrong_intrinsics() {
    check_partition(false, 36, 72);
}
