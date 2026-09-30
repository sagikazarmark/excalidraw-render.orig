//! Bounded EXIF precedence: 82 cells / 164 vectors, independently authored APP1
//! bytes and pinned official-browser evidence. All 164 vectors must pass the
//! canonical gates, including the 36 cells corrected by #189.
//! In this matrix the first pre-SOS EXIF APP1 wins (even invalid/absent/bad
//! magic), then the first valid orientation within its IFD. Late/between-scan
//! APP1 is ignored. Authored bounds controls below make no new browser claims.
//! PDF checks inspect the page-bound native image and page extents, not a viewer.
use crate::common::lower_hex;
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonError, ComparisonMode, ComparisonReport, VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

// Each tuple authors (signal, browser orientation).
// Selection never comes from the production metadata parser or measured pixels.
const QUALIFIED: &[(&str, u8)] = &[
    ("control-1", 1),
    ("control-3", 3),
    ("control-6", 6),
    ("control-8", 8),
    ("duplicate-tags-6-3", 6),
    ("duplicate-tags-3-6", 3),
    ("valid-tag-then-invalid", 6),
    ("late-only", 1),
    ("early-3-late-6", 3),
    ("early-6-late-3", 6),
];
const BETWEEN_SCANS: &[(&str, u8)] = &[
    ("between-scans-only", 1),
    ("early-3-between-6", 3),
    ("early-6-between-3", 6),
];
// Historical ownership partition only: these nine signals were corrected by
// #189. Both partitions now have identical canonical acceptance gates.
const CORRECTED: &[(&str, u8)] = &[
    ("duplicate-6-3", 6),
    ("duplicate-3-6", 3),
    ("invalid-then-valid", 1),
    ("absent-tag-then-valid", 1),
    ("bad-magic-then-valid", 1),
    ("valid-then-invalid", 6),
    ("valid-then-absent-tag", 6),
    ("valid-then-bad-magic", 6),
    ("invalid-tag-then-valid", 6),
];

fn browser_orientation(row: &Value) -> u8 {
    let &(_, browser) = QUALIFIED
        .iter()
        .chain(BETWEEN_SCANS)
        .chain(CORRECTED)
        .find(|(signal, _)| row["signal"] == *signal)
        .unwrap();
    browser
}
fn decode(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
fn hash(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}
fn pinned_json(bytes: &[u8], expected: &str) -> Value {
    assert_eq!(hash(bytes), expected);
    serde_json::from_slice(bytes).unwrap()
}
fn evidence() -> Value {
    let record = pinned_json(
        include_bytes!("../../../../tests/oracle/jpeg-exif/reference.json"),
        "fdfcde18c00cba54e0348619ac83f02b06f4546f6c83c6fbc93ea2c01cc29197",
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
        "976955ebad540f3aa0291e6073838d59f1519dad1a8efe4cdab89198fbb57088",
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
            for &(signal, _) in QUALIFIED
                .iter()
                .chain(CORRECTED)
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
        let (w, h) = dimensions(pattern, browser_orientation(row));
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
    let mut bytes = vec![0; reader.output_buffer_size().expect("PNG buffer fits")];
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
        let orientation = browser_orientation(row);
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
    let (w, h) = dimensions(row["pattern"].as_str().unwrap(), browser_orientation(row));
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
    // The helper verifies the bound image's native dimensions and sample count.
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
fn check(row: &Value, rows: &[Value], inputs: &Value) {
    let id = row["id"].as_str().unwrap();
    let pattern = row["pattern"].as_str().unwrap();
    let browser = browser_orientation(row);
    let expected_dims = dimensions(pattern, browser);
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
                    assert!(result.passed(), "{id}: {key}: {:?}", result.whole);
                }
            }
            OutputFormat::Pdf => {
                pdf_extent(&bytes, expected_dims);
                let actual = pdf_pixels(&bytes, expected_dims);
                let single = control(rows, row, browser);
                let single_bytes = render(&single["scene"], OutputFormat::Pdf);
                pdf_extent(&single_bytes, expected_dims);
                assert_eq!(
                    actual,
                    pdf_pixels(&single_bytes, expected_dims),
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
                    orient(&native, browser),
                    "{id}: independently permuted untagged PDF samples"
                );
                for key in ["png", "svg_png"] {
                    let expected = raster(&decode(&row["reference"][key]));
                    let result = comparison(&expected, &actual).unwrap();
                    assert!(
                        result.passed(),
                        "{id}: {key}: PDF bound image: {:?}",
                        result.whole
                    );
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

fn check_partition(corrected: bool, cells: usize, vectors: usize) {
    let record = evidence();
    reference_contract(&record);
    discrimination_contract(&record);
    let rows = record["cases"].as_array().unwrap();
    let selected: Vec<_> = rows
        .iter()
        .filter(|r| CORRECTED.iter().any(|(signal, _)| r["signal"] == *signal) == corrected)
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
pub(super) fn qualified_exif_precedence_matches_reference() {
    check_partition(false, 46, 92);
}
#[test]
pub(super) fn corrected_duplicate_exif_precedence_matches_reference() {
    check_partition(true, 36, 72);
}

fn with_early_segments(source: &[u8], segments: &[Vec<u8>]) -> Vec<u8> {
    assert_eq!(&source[..2], b"\xff\xd8");
    let mut bytes = source[..2].to_vec();
    for segment in segments {
        bytes.extend_from_slice(segment);
    }
    bytes.extend_from_slice(&source[2..]);
    bytes
}

// Authored robustness controls compare native output with a single-orientation
// runtime control. They do not extend the collected browser precedence matrix.
fn check_authored(bytes: &[u8], single: &Value, name: &str) {
    let mut row = single.clone();
    row["scene"]["files"]["asset"]["dataURL"] =
        json!(crate::common::image_scene::data_url("image/jpeg", bytes));
    let dims = dimensions(
        single["pattern"].as_str().unwrap(),
        browser_orientation(single),
    );
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        // render also requires empty diagnostics and stable bytes/reports.
        let actual = render(&row["scene"], format);
        match format {
            OutputFormat::Svg => svg_check(std::str::from_utf8(&actual).unwrap(), &row),
            OutputFormat::Png => {
                super::png_profile_contract::assert_untagged_png(&actual);
                let actual = crate::common::raster::decode_png(&actual).into_canonical();
                let expected = render(&single["scene"], format);
                assert_eq!(extent(&actual), dims, "{name}");
                assert_eq!(actual, raster(&expected), "{name}: PNG samples");
            }
            OutputFormat::Pdf => {
                pdf_extent(&actual, dims);
                let expected = render(&single["scene"], format);
                assert_eq!(
                    pdf_pixels(&actual, dims),
                    pdf_pixels(&expected, dims),
                    "{name}: bound PDF samples"
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn authored_earlier_app1_and_marker_fill_preserve_exif_selection() {
    let record = evidence();
    let rows = record["cases"].as_array().unwrap();
    let inputs = inputs();
    for row in rows.iter().filter(|r| r["signal"] == "control-1") {
        let source = decode(&base(&inputs, row)["jpeg_base64"]);
        let valid = app1(&tiff(&[6], row["byte_order"] == "big", 42));
        for (name, segments, orientation) in [
            (
                "non-exif-app1-before-valid",
                vec![
                    app1(b"http://ns.adobe.com/xap/1.0/\0unrelated"),
                    valid.clone(),
                ],
                6,
            ),
            (
                "truncated-exif-prefix-is-not-an-exif-block",
                vec![app1(b"Exif\0"), valid.clone()],
                6,
            ),
            (
                "complete-exif-prefix-without-tiff-captures-first-block",
                vec![app1(b"Exif\0\0"), valid.clone()],
                1,
            ),
            (
                "marker-fill-before-app1",
                vec![[vec![0xff, 0xff], valid].concat()],
                6,
            ),
        ] {
            check_authored(
                &with_early_segments(&source, &segments),
                control(rows, row, orientation),
                name,
            );
        }
    }
}

fn check_rejected_header(bytes: &[u8], row: &Value, name: &str) {
    let mut scene = row["scene"].clone();
    scene["files"]["asset"]["dataURL"] =
        json!(crate::common::image_scene::data_url("image/jpeg", bytes));
    let parsed = SceneDocument::parse(&serde_json::to_vec(&scene).unwrap()).unwrap();
    scene["files"] = json!({});
    let missing = SceneDocument::parse(&serde_json::to_vec(&scene).unwrap()).unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .include_background(false)
            .build()
            .unwrap();
        let first = Renderer::new().render(&parsed, &request).unwrap();
        let second = Renderer::new().render(&parsed, &request).unwrap();
        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{name}"
        );
        assert_eq!(first.report(), second.report(), "{name}");
        let mut report = serde_json::to_value(first.report()).unwrap();
        for diagnostic in report["diagnostics"].as_array_mut().unwrap() {
            assert!(!diagnostic["message"].as_str().unwrap().is_empty());
            diagnostic.as_object_mut().unwrap().remove("message");
        }
        let placeholder = format == OutputFormat::Png;
        assert_eq!(
            report,
            json!({"schema_version":1,"diagnostics":[{
                "code":"image.decode-failed","severity":"warning","format":format.as_str(),
                "effects":["visual"],"element_id":row["scene"]["elements"][0]["id"],
                "json_pointer":"/files/asset/dataURL",
                "details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,
                "validated_media_type":"image/jpeg","recovery":if placeholder {"placeholder"} else {"omitted"},
                "recipe_id":if placeholder {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}
            }]}),
            "{name}: {format:?}"
        );
        let control = Renderer::new().render(&missing, &request).unwrap();
        assert_eq!(
            first.artifact().bytes(),
            control.artifact().bytes(),
            "{name}: recovery"
        );
    }
}

#[test]
pub(super) fn authored_invalid_tiff_blocks_and_malformed_headers_have_bounded_recovery() {
    let record = evidence();
    let rows = record["cases"].as_array().unwrap();
    let inputs = inputs();
    for row in rows.iter().filter(|r| r["signal"] == "control-1") {
        let source = decode(&base(&inputs, row)["jpeg_base64"]);
        let big = row["byte_order"] == "big";
        let good = tiff(&[6], big, 42);
        let mut payloads = vec![
            ("truncated-byte-order", good[..7].to_vec()),
            ("truncated-ifd-offset", good[..13].to_vec()),
            ("truncated-entry-count", good[..15].to_vec()),
            ("truncated-entry-value", good[..25].to_vec()),
            // Even a present orientation value cannot make a short entry valid.
            ("truncated-entry-padding", good[..27].to_vec()),
        ];
        for (name, range, replacement) in [
            ("ifd-offset-overflow", 10..14, vec![255; 4]),
            ("invalid-tag-type", 18..20, vec![0; 2]),
            ("invalid-tag-count-zero", 20..24, vec![0; 4]),
            ("invalid-tag-count-overflow", 20..24, vec![255; 4]),
        ] {
            let mut bad = good.clone();
            bad[range].copy_from_slice(&replacement);
            payloads.push((name, bad));
        }
        // A huge IFD count after an invalid value must stop at the APP1 boundary,
        // rather than treating the next APP1's valid orientation as another entry.
        let mut short_ifd = tiff(&[0], big, 42);
        short_ifd[14..16].copy_from_slice(&[255; 2]);
        short_ifd.truncate(28);
        payloads.push(("entry-count-exceeds-block", short_ifd));
        for (name, payload) in payloads {
            for later in [false, true] {
                let mut segments = vec![app1(&payload)];
                if later {
                    segments.push(app1(&good));
                }
                check_authored(&with_early_segments(&source, &segments), row, name);
            }
        }
        for (name, header) in [
            ("app1-length-zero", b"\xff\xe1\0\0".as_slice()),
            ("app1-length-one", b"\xff\xe1\0\x01".as_slice()),
            ("app1-past-eof", b"\xff\xe1\xff\xffExif\0\0".as_slice()),
        ] {
            // Keep the valid frame and scans: rejection must come from the
            // malformed APP1 framing, not merely a missing image body.
            let bytes = with_early_segments(&source, &[header.to_vec()]);
            assert!(bytes.len() < usize::from(u16::MAX));
            check_rejected_header(&bytes, row, name);
        }
        for (name, header) in [
            ("truncated-app1-length", b"\xff\xe1\0".as_slice()),
            ("unterminated-marker-fill", b"\xff\xff\xff".as_slice()),
        ] {
            let bytes = [source[..2].to_vec(), header.to_vec()].concat();
            check_rejected_header(&bytes, row, name);
        }
    }
}
