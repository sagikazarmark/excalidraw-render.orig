//! Bounded MPF/XMP qualification: 52 cells / 104 vectors, frozen official
//! browser evidence, independently inspected TIFF fields, XML and JPEG ranges.
//! Owners: 12 SDR cells, 16 strict framing-recovery cells and 16 strict MPF
//! index-recovery cells. #190 retains 8 unqualified HDR color gaps.
//! Defect pins require canonical failure, never an exception.
//! PDF measurements cover page bounds and the actual bound image/ICC/alpha,
//! against browser-rasterized SVG, not a whole-PDF viewer claim.
//! Optional MPF tags and relocated IFDs are additional native structural
//! controls; they add no browser evidence or qualified matrix cells.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const SDR: &[&str] = &["identity", "sdr-ramp", "sdr-edges"];
const HDR: &[&str] = &["hdr-ramp", "hdr-edges"];
const RECOVERY: &[&str] = &["omit-mpf", "omit-xmp", "truncated-secondary", "third-image"];
const INDEX_RECOVERY: &[&str] = &["bad-offset", "bad-size", "bad-count", "missing-secondary"];
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const HDRGM: &str = "http://ns.adobe.com/hdr-gain-map/1.0/";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const CONTAINER: &str = "http://ns.google.com/photos/1.0/container/";
const ITEM: &str = "http://ns.google.com/photos/1.0/container/item/";

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
fn inputs() -> Value {
    pinned_json(
        include_bytes!("../../../../tests/oracle/jpeg-color/inputs.json"),
        "acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258",
    )
}
fn base_evidence() -> Value {
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
fn by_id<'a>(record: &'a Value, id: &Value) -> &'a Value {
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"] == *id)
        .collect();
    assert_eq!(rows.len(), 1);
    rows[0]
}
fn evidence() -> Value {
    let record = pinned_json(
        include_bytes!("../../../../tests/oracle/jpeg-gain-map/reference.json"),
        "af3c97e389ef230c57bbe9a4b3406dc1ef1d6a2ad37b0e8ad2e1f4fbe6568ce4",
    );
    assert_eq!(
        record["schema"],
        "excalidraw-render/jpeg-gain-map-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(hash(&bytes), source["sha256"].as_str().unwrap());
    }
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
    // Crucially, every owner selector executes the full matrix qualification.
    structural_matrix(&record);
    record
}
fn require(ok: bool, error: &'static str) -> Result<(), &'static str> {
    if ok {
        Ok(())
    } else {
        Err(error)
    }
}
fn segment(bytes: &[u8], at: usize, marker: u8) -> Result<&[u8], &'static str> {
    require(
        bytes.get(at..at + 2) == Some(&[255, marker]),
        "segment marker",
    )?;
    let length = bytes.get(at + 2..at + 4).ok_or("segment length")?;
    let length = usize::from(u16::from_be_bytes(length.try_into().unwrap()));
    require(length >= 2, "segment length")?;
    bytes.get(at..at + 2 + length).ok_or("segment range")
}

// Walk JPEG framing independently of the renderer/fixture author. Entropy byte
// stuffing and restart markers are skipped; progressive scans resume marker
// parsing. This finds actual EOI boundaries, not an MPF-declared boundary.
fn jpeg_end(bytes: &[u8]) -> Result<usize, &'static str> {
    require(bytes.get(..2) == Some(b"\xff\xd8"), "SOI")?;
    let mut at = 2;
    let mut entropy = false;
    loop {
        if entropy {
            while bytes.get(at).is_some_and(|b| *b != 255) {
                at += 1;
            }
        }
        require(bytes.get(at) == Some(&255), "missing EOI")?;
        while bytes.get(at) == Some(&255) {
            at += 1;
        }
        let marker = *bytes.get(at).ok_or("missing EOI")?;
        at += 1;
        if entropy && (marker == 0 || (0xd0..=0xd7).contains(&marker)) {
            continue;
        }
        if marker == 0xd9 {
            return Ok(at);
        }
        require(
            marker != 0 && marker != 0xd8 && !(0xd0..=0xd7).contains(&marker),
            "JPEG marker",
        )?;
        let length = bytes.get(at..at + 2).ok_or("JPEG length")?;
        let length = usize::from(u16::from_be_bytes(length.try_into().unwrap()));
        require(length >= 2 && at + length <= bytes.len(), "JPEG range")?;
        at += length;
        entropy = marker == 0xda;
    }
}

#[derive(Debug)]
struct Index {
    count: u32,
    primary: usize,
    secondary: usize,
    offset: usize,
}
fn index(segment: &[u8], big: bool) -> Result<Index, &'static str> {
    require(
        segment.len() == 90 && segment.get(..8) == Some(b"\xff\xe2\x00\x58MPF\0"),
        "MPF framing",
    )?;
    let tiff = &segment[8..];
    require(&tiff[..2] == if big { b"MM" } else { b"II" }, "TIFF endian")?;
    let short = |at| {
        let b = tiff[at..at + 2].try_into().unwrap();
        if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        }
    };
    let long = |at| {
        let b = tiff[at..at + 4].try_into().unwrap();
        if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        }
    };
    require(short(2) == 42 && long(4) == 8 && short(8) == 3, "TIFF IFD")?;
    for (at, tag, kind, count) in [(10, 0xb000, 7, 4), (22, 0xb001, 4, 1), (34, 0xb002, 7, 32)] {
        require(
            short(at) == tag && short(at + 2) == kind && long(at + 4) == count,
            "MPF tag",
        )?;
    }
    require(&tiff[18..22] == b"0100", "MPF version")?;
    require(long(42) == 50 && long(46) == 0, "MPEntry range")?;
    // All attribute bits and dependency indices are checked, not just sizes.
    require(
        long(50) == 0x030000 && long(58) == 0 && long(62) == 0 && long(66) == 0 && long(78) == 0,
        "MPEntry attributes",
    )?;
    Ok(Index {
        count: long(30),
        primary: long(54) as usize,
        secondary: long(70) as usize,
        offset: long(74) as usize,
    })
}
fn xmp(
    segment: &[u8],
    primary: bool,
    secondary_size: usize,
    hdr: bool,
    max: u8,
) -> Result<(), &'static str> {
    require(segment.get(4..4 + XMP.len()) == Some(XMP), "XMP signature")?;
    let text = std::str::from_utf8(&segment[4 + XMP.len()..]).map_err(|_| "XMP UTF8")?;
    let doc = roxmltree::Document::parse(text).map_err(|_| "XMP XML")?;
    require(
        doc.root_element()
            .has_tag_name(("adobe:ns:meta/", "xmpmeta")),
        "XMP root",
    )?;
    let descriptions: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((RDF, "Description")))
        .collect();
    require(descriptions.len() == 1, "XMP description")?;
    let desc = descriptions[0];
    require(
        desc.parent().is_some_and(|n| n.has_tag_name((RDF, "RDF"))),
        "XMP RDF",
    )?;
    require(desc.attribute((RDF, "about")) == Some(""), "XMP about")?;
    require(
        desc.attribute((HDRGM, "Version")) == Some("1.0"),
        "XMP version",
    )?;
    if primary {
        require(desc.attributes().len() == 2, "primary XMP attributes")?;
        let directories: Vec<_> = desc.children().filter(|n| n.is_element()).collect();
        require(
            directories.len() == 1 && directories[0].has_tag_name((CONTAINER, "Directory")),
            "XMP directory",
        )?;
        let seq: Vec<_> = directories[0]
            .children()
            .filter(|n| n.is_element())
            .collect();
        require(
            seq.len() == 1 && seq[0].has_tag_name((RDF, "Seq")),
            "XMP sequence",
        )?;
        let entries: Vec<_> = seq[0].children().filter(|n| n.is_element()).collect();
        require(entries.len() == 2, "XMP item count")?;
        for (entry, semantic) in entries.iter().zip(["Primary", "GainMap"]) {
            require(
                entry.has_tag_name((RDF, "li"))
                    && entry.attribute((RDF, "parseType")) == Some("Resource"),
                "XMP resource",
            )?;
            let items: Vec<_> = entry.children().filter(|n| n.is_element()).collect();
            require(
                items.len() == 1 && items[0].has_tag_name((CONTAINER, "Item")),
                "XMP item",
            )?;
            require(
                items[0].attribute((ITEM, "Semantic")) == Some(semantic)
                    && items[0].attribute((ITEM, "Mime")) == Some("image/jpeg"),
                "XMP semantics",
            )?;
            if semantic == "GainMap" {
                require(
                    items[0].attributes().len() == 3
                        && items[0].attribute((ITEM, "Length"))
                            == Some(secondary_size.to_string().as_str()),
                    "XMP secondary length",
                )?;
            } else {
                require(items[0].attributes().len() == 2, "XMP primary attributes")?;
            }
        }
    } else {
        require(
            desc.attributes().len() == 10 && !desc.children().any(|n| n.is_element()),
            "gain XMP attributes",
        )?;
        for (key, value) in [
            ("GainMapMin", "0"),
            ("GainMapMax", if max == 0 { "0" } else { "2" }),
            ("Gamma", "1"),
            ("OffsetSDR", "0"),
            ("OffsetHDR", "0"),
            ("HDRCapacityMin", "0"),
            ("HDRCapacityMax", "2"),
            ("BaseRenditionIsHDR", if hdr { "True" } else { "False" }),
        ] {
            require(
                desc.attribute((HDRGM, key)) == Some(value),
                "gain XMP value",
            )?;
        }
    }
    Ok(())
}

fn container(bytes: &[u8], big: bool, hdr: bool, max: u8) -> Result<(), &'static str> {
    let primary_end = jpeg_end(bytes)?;
    let mpf = segment(bytes, 2, 0xe2).map_err(|_| "omit-mpf")?;
    let index = index(mpf, big)?;
    require(index.count == 2, "bad-count")?;
    require(index.primary == primary_end, "primary size")?;
    // TIFF starts at file byte 10, not at SOI or the APP2 marker.
    require(
        index.offset.checked_add(10) == Some(primary_end),
        "bad-offset",
    )?;
    let primary_xmp = segment(bytes, 2 + mpf.len(), 0xe1).map_err(|_| "omit-xmp")?;
    xmp(primary_xmp, true, index.secondary, hdr, max).map_err(|_| "bad-size")?;
    require(bytes.len() > primary_end, "missing-secondary")?;
    let secondary = &bytes[primary_end..];
    let secondary_end = jpeg_end(secondary).map_err(|_| "truncated-secondary")?;
    require(index.secondary == secondary_end, "bad-size")?;
    require(
        primary_end.checked_add(secondary_end) == Some(bytes.len()),
        "third-image",
    )?;
    xmp(segment(secondary, 2, 0xe1)?, false, secondary_end, hdr, max)?;
    Ok(())
}

fn structural_matrix(record: &Value) {
    let inputs = inputs();
    let bases = base_evidence();
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 104);
    let mut expected = BTreeSet::new();
    for mode in ["baseline", "progressive"] {
        for order in ["big", "little"] {
            for signal in SDR.iter().chain(HDR).chain(RECOVERY).chain(INDEX_RECOVERY) {
                for pattern in ["ramp", "edges"] {
                    expected.insert(format!("{mode}-{order}-{signal}-{pattern}"));
                }
            }
        }
    }
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(
        rows.iter()
            .map(|r| r["cell"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        52
    );
    for row in rows {
        structural_row(row, &inputs);
        let base = by_id(&bases, &row["base_id"]);
        let mut expected_scene = base["scene"].clone();
        expected_scene["files"]["asset"]["dataURL"] =
            row["scene"]["files"]["asset"]["dataURL"].clone();
        assert_eq!(row["scene"], expected_scene);
        assert_eq!(
            base["jpeg_base64"],
            by_id(&inputs, &row["base_id"])["jpeg_base64"]
        );
        assert_eq!(row["reference"]["admitted"], true);
        svg_check(row["reference"]["svg"].as_str().unwrap(), row, true);
        for key in ["png", "svg_png"] {
            let actual = raster(&decode(&row["reference"][key]));
            opaque_extent(&actual, row);
            let plain = raster(&decode(&base["reference"][key]));
            if HDR.contains(&row["signal"].as_str().unwrap()) {
                assert_ne!(
                    actual, plain,
                    "{}: {key}: nonidentity gain map must discriminate ignored metadata",
                    row["id"]
                );
                assert!(!comparison(&actual, &plain).passed());
            } else {
                assert_eq!(
                    actual, plain,
                    "{}: {key}: browser base projection",
                    row["id"]
                );
            }
        }
    }
}
fn structural_row(row: &Value, inputs: &Value) {
    let signal = row["signal"].as_str().unwrap();
    let mode = row["mode"].as_str().unwrap();
    let order = row["byte_order"].as_str().unwrap();
    let pattern = row["pattern"].as_str().unwrap();
    assert_eq!(row["cell"], format!("{mode}-{order}-{signal}"));
    assert_eq!(row["id"], format!("{mode}-{order}-{signal}-{pattern}"));
    assert_eq!(row["base_id"], format!("rgb-1x1-{mode}-{pattern}"));
    let map_pattern = if signal.ends_with("edges") {
        "edges"
    } else {
        "ramp"
    };
    assert_eq!(row["map_id"], format!("gray-1x1-{mode}-{map_pattern}"));
    let base = by_id(inputs, &row["base_id"]);
    let map = by_id(inputs, &row["map_id"]);
    assert_eq!(base["model"], "rgb");
    assert_eq!(map["model"], "gray");
    let (width, height) = if pattern == "ramp" {
        (33, 25)
    } else {
        (48, 32)
    };
    assert_eq!(row["width"], width);
    assert_eq!(row["height"], height);
    for key in ["mode", "pattern", "width", "height"] {
        assert_eq!(row[key], base[key]);
    }
    for key in ["width", "height"] {
        assert_eq!(row["reference"][key], row[key]);
    }
    let hdr = HDR.contains(&signal);
    let max = if signal == "identity" { 0 } else { 2 };
    assert_eq!(row["base_hdr"], hdr);
    assert_eq!(row["gain_max_log2"], max);
    assert_eq!(row["container_valid"], SDR.contains(&signal) || hdr);
    let source = decode(&base["jpeg_base64"]);
    let map = decode(&map["jpeg_base64"]);
    assert_eq!(jpeg_end(&source), Ok(source.len()));
    assert_eq!(jpeg_end(&map), Ok(map.len()));
    let primary_xmp = decode(&row["primary_xmp_base64"]);
    let secondary_xmp = decode(&row["secondary_xmp_base64"]);
    let mpf = decode(&row["mpf_base64"]);
    assert_eq!(segment(&secondary_xmp, 0, 0xe1).unwrap(), secondary_xmp);
    let secondary = [&map[..2], &secondary_xmp, &map[2..]].concat();
    assert_eq!(row["secondary_size"], secondary.len());
    xmp(&secondary_xmp, false, secondary.len(), hdr, max).unwrap();
    if signal == "omit-xmp" {
        assert!(primary_xmp.is_empty());
    } else {
        assert_eq!(segment(&primary_xmp, 0, 0xe1).unwrap(), primary_xmp);
        xmp(&primary_xmp, true, secondary.len(), hdr, max).unwrap();
    }
    let primary = [&source[..2], &mpf, &primary_xmp, &source[2..]].concat();
    assert_eq!(row["primary_size"], primary.len());
    assert_eq!(jpeg_end(&primary), Ok(primary.len()));
    assert_eq!(jpeg_end(&secondary), Ok(secondary.len()));
    if signal == "omit-mpf" {
        assert!(mpf.is_empty());
    } else {
        let index = index(&mpf, order == "big").unwrap();
        assert_eq!(index.count, if signal == "bad-count" { 3 } else { 2 });
        assert_eq!(index.primary, primary.len());
        assert_eq!(
            index.secondary,
            secondary.len() + if signal == "bad-size" { 123 } else { 0 }
        );
        assert_eq!(
            index.offset,
            if signal == "bad-offset" {
                0xfffffff0
            } else {
                primary.len() - 10
            }
        );
    }
    let mut expected = primary;
    match signal {
        "missing-secondary" => {}
        "truncated-secondary" => expected.extend_from_slice(&secondary[..secondary.len() - 2]),
        _ => expected.extend_from_slice(&secondary),
    }
    if signal == "third-image" {
        expected.extend_from_slice(&map);
    }
    let jpeg = decode(&row["jpeg_base64"]);
    assert_eq!(
        jpeg, expected,
        "{}: exact base/map/metadata reconstruction",
        row["id"]
    );
    assert_eq!(hash(&jpeg), row["jpeg_sha256"].as_str().unwrap());
    assert_eq!(
        row["scene"]["files"]["asset"]["dataURL"],
        format!(
            "data:image/jpeg;base64,{}",
            row["jpeg_base64"].as_str().unwrap()
        )
    );
    assert_eq!(
        container(&jpeg, order == "big", hdr, max),
        if SDR.contains(&signal) || hdr {
            Ok(())
        } else {
            Err(signal)
        },
        "{}",
        row["id"]
    );
}

fn raster(bytes: &[u8]) -> CanonicalRgba8 {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    pixels.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => panic!("reference PNG layout"),
    };
    CanonicalRgba8::new(info.width, info.height, rgba).unwrap()
}
fn opaque_extent(pixels: &CanonicalRgba8, row: &Value) {
    assert_eq!(u64::from(pixels.width()), row["width"].as_u64().unwrap());
    assert_eq!(u64::from(pixels.height()), row["height"].as_u64().unwrap());
    assert!(pixels.rgba().chunks_exact(4).all(|p| p[3] == 255));
}
fn comparison(
    reference: &CanonicalRgba8,
    actual: &CanonicalRgba8,
) -> excalidraw_render_compare::ComparisonReport {
    compare(
        reference,
        actual,
        VisualClass::GeometryImage,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap()
}
fn svg_check(svg: &str, row: &Value, image: bool) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let root = doc.root_element();
    assert!(root.has_tag_name("svg"));
    let w = row["width"].as_u64().unwrap();
    let h = row["height"].as_u64().unwrap();
    assert_eq!(root.attribute("width").unwrap().parse::<u64>().unwrap(), w);
    assert_eq!(root.attribute("height").unwrap().parse::<u64>().unwrap(), h);
    assert_eq!(
        root.attribute("viewBox"),
        Some(format!("0 0 {w} {h}").as_str())
    );
    let images: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), usize::from(image));
    if image {
        assert_eq!(
            images[0]
                .attribute("href")
                .or_else(|| images[0].attribute(("http://www.w3.org/1999/xlink", "href"))),
            row["scene"]["files"]["asset"]["dataURL"].as_str()
        );
    }
}
fn pdf_extent(bytes: &[u8], row: &Value) {
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
        assert_eq!(
            values,
            [
                0.0,
                0.0,
                row["width"].as_f64().unwrap() * 0.75,
                row["height"].as_f64().unwrap() * 0.75
            ]
        );
    }
    assert!(!page.contains("/Rotate") && !page.contains("/UserUnit"));
}
fn pixels(bytes: &[u8], format: OutputFormat, row: &Value) -> CanonicalRgba8 {
    let actual = if format == OutputFormat::Png {
        super::png_profile_contract::assert_untagged_png(bytes);
        crate::common::raster::decode_png(bytes).into_canonical()
    } else {
        assert_eq!(format, OutputFormat::Pdf);
        pdf_extent(bytes, row);
        let w = row["width"].as_u64().unwrap() as u32;
        let h = row["height"].as_u64().unwrap() as u32;
        CanonicalRgba8::new(
            w,
            h,
            super::png_profile_contract::pdf_rgba(bytes, w as usize, h as usize),
        )
        .unwrap()
    };
    opaque_extent(&actual, row);
    actual
}
fn recovery_artifact(bytes: &[u8], format: OutputFormat, row: &Value) {
    match format {
        OutputFormat::Svg => svg_check(std::str::from_utf8(bytes).unwrap(), row, false),
        OutputFormat::Pdf => {
            pdf_extent(bytes, row);
            assert!(!crate::common::pdf_inspect::pdf_text(bytes).contains("/Subtype /Image"));
        }
        OutputFormat::Png => {
            let actual = crate::common::raster::decode_png(bytes);
            assert_eq!(u64::from(actual.width), row["width"].as_u64().unwrap());
            assert_eq!(u64::from(actual.height), row["height"].as_u64().unwrap());
            let mut at = 8;
            while at < bytes.len() {
                let length = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
                let kind = &bytes[at + 4..at + 8];
                assert!(matches!(kind, b"IHDR" | b"IDAT" | b"IEND" | b"iTXt"));
                if kind == b"iTXt" {
                    assert!(bytes[at + 8..at + 8 + length]
                        .starts_with(b"excalidraw-render:artifact-manifest\0\0\0\0\0"));
                }
                at += length + 12;
            }
            assert_eq!(at, bytes.len());
        }
        _ => panic!("unexpected format"),
    }
}
fn render(scene: &Value, format: OutputFormat) -> (Vec<u8>, Value) {
    let scene = SceneDocument::parse(&serde_json::to_vec(scene).unwrap()).unwrap();
    let request = RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .build()
        .unwrap();
    let output = Renderer::new().render(&scene, &request).unwrap();
    let repeat = Renderer::new().render(&scene, &request).unwrap();
    assert_eq!(output.artifact().bytes(), repeat.artifact().bytes());
    assert_eq!(output.report(), repeat.report());
    (
        output.artifact().bytes().to_vec(),
        serde_json::to_value(output.report()).unwrap(),
    )
}
fn selected<'a>(record: &'a Value, signals: &[&str], cells: usize) -> Vec<&'a Value> {
    let rows: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| signals.contains(&r["signal"].as_str().unwrap()))
        .collect();
    assert_eq!(rows.len(), cells * 2);
    assert_eq!(
        rows.iter()
            .map(|r| r["cell"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        cells
    );
    rows
}
fn admitted(row: &Value, base: &Value, defect: bool) {
    for format in FORMATS {
        let (bytes, report) = render(&row["scene"], format);
        assert_eq!(report, json!({"schema_version":1,"diagnostics":[]}));
        if format == OutputFormat::Svg {
            svg_check(std::str::from_utf8(&bytes).unwrap(), row, true);
            continue;
        }
        let (plain, plain_report) = render(&base["scene"], format);
        assert_eq!(plain_report, json!({"schema_version":1,"diagnostics":[]}));
        assert_eq!(
            bytes, plain,
            "{}: {format:?}: native base projection",
            row["id"]
        );
        let actual = pixels(&bytes, format, row);
        let key = if format == OutputFormat::Png {
            "png"
        } else {
            "svg_png"
        };
        let reference = raster(&decode(&row["reference"][key]));
        let result = comparison(&reference, &actual);
        if defect {
            assert!(
                !result.passed(),
                "#190 must fail the unmodified canonical budget"
            );
            let SsimOutcome::Score(score) = result.whole.ssim else {
                panic!("expected SSIM score")
            };
            // Measurements, not acceptance tolerances: all four pins remain
            // failures under the unmodified canonical geometry/image policy.
            // Both output formats and all mode/endian combinations hit them.
            let (changed, ssim): (u64, f64) = match (
                row["signal"].as_str().unwrap(),
                row["pattern"].as_str().unwrap(),
            ) {
                ("hdr-ramp", "ramp") => (553, 0.860099630813284),
                ("hdr-edges", "ramp") => (648, 0.8479795899866851),
                ("hdr-ramp", "edges") => (1214, 0.9116739114307459),
                ("hdr-edges", "edges") => (1262, 0.8737353963660468),
                _ => panic!("outside #190 color measurement scope"),
            };
            assert_eq!(
                result.whole.changed_pixels, changed,
                "{}: {format:?}",
                row["id"]
            );
            assert_eq!(score.to_bits(), ssim.to_bits(), "{}: {format:?}", row["id"]);
        } else {
            assert!(
                result.passed(),
                "{}: {format:?}: {:?}",
                row["id"],
                result.whole
            );
        }
    }
}

#[test]
fn valid_sdr_gain_maps_match_reference_base_projection() {
    let record = evidence();
    assert_structural_mutations(&record);
    assert_nonidentity_map_discrimination(&record);
    assert_output_mutations(&record);
    let bases = base_evidence();
    for row in selected(&record, SDR, 12) {
        admitted(row, by_id(&bases, &row["base_id"]), false);
    }
}
#[test]
fn hdr_gain_maps_pin_known_color_defect_190() {
    let record = evidence();
    let bases = base_evidence();
    for row in selected(&record, HDR, 8) {
        admitted(row, by_id(&bases, &row["base_id"]), true);
    }
}
#[test]
fn corrected_mpf_index_ranges_have_exact_recovery() {
    let record = evidence();
    assert_optional_mpf_controls(&record);
    assert_malformed_mpf_fields(&record);
    assert_representative_primary_flag(&record);
    assert_secondary_mpf_index_recovery(&record);
    assert_secondary_mpf_attributes(&record);
    for row in selected(&record, INDEX_RECOVERY, 16) {
        exact_recovery(row);
    }
}
#[test]
fn invalid_container_framing_has_exact_recovery() {
    let record = evidence();
    assert_structural_mutations(&record);
    assert_nonidentity_map_discrimination(&record);
    assert_output_mutations(&record);
    for row in selected(&record, RECOVERY, 16) {
        exact_recovery(row);
    }
}

fn exact_recovery(row: &Value) {
    let mut missing = row["scene"].clone();
    missing["files"] = json!({});
    for format in FORMATS {
        let (bytes, mut report) = render(&row["scene"], format);
        for diagnostic in report["diagnostics"].as_array_mut().unwrap() {
            assert!(!diagnostic["message"].as_str().unwrap().is_empty());
            diagnostic.as_object_mut().unwrap().remove("message");
        }
        assert_eq!(
            report,
            json!({"schema_version":1,"diagnostics":[{
                "code":"image.decode-failed","severity":"warning","format":format.as_str(),"effects":["visual"],
                "element_id":"image","json_pointer":"/files/asset/dataURL","details":{"kind":"asset","asset_id":"asset",
                "claimed_media_type":null,"validated_media_type":"image/jpeg",
                "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"},
                "recipe_id":if format == OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}}
            }]}),
            "{}: {format:?}",
            row["id"]
        );
        assert_eq!(
            bytes,
            render(&missing, format).0,
            "{}: {format:?}",
            row["id"]
        );
        recovery_artifact(&bytes, format, row);
    }
}

// Mutate frozen, independently qualified containers, never a production writer
// or a signature-only MPF stand-in. Addresses are relative to TIFF at byte 10.
fn tiff_long(bytes: &mut [u8], big: bool, at: usize, value: u32) {
    let value = if big {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    };
    bytes[10 + at..14 + at].copy_from_slice(&value);
}
fn tiff_short(bytes: &mut [u8], big: bool, at: usize, value: u16) {
    let value = if big {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    };
    bytes[10 + at..12 + at].copy_from_slice(&value);
}

fn with_mpf_bytes(row: &Value, name: &str, bytes: &[u8]) -> Value {
    let mut changed = row.clone();
    changed["id"] = json!(format!("{}-{name}", row["id"].as_str().unwrap()));
    changed["scene"]["files"]["asset"]["dataURL"] = json!(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ));
    changed
}

// Construct legal alternatives from the frozen SDR index, without calling any
// production MPF parser/writer. The unchanged JPEG/XMP payloads remain the
// authority for image extents; TIFF placement is independently inspected below.
fn optional_mpf_bytes(
    original: &[u8],
    big: bool,
    uid: bool,
    frames: bool,
    before: bool,
) -> Vec<u8> {
    let count = 3 + usize::from(uid) + usize::from(frames);
    let ifd = if before { 40 } else { 8 };
    let table_end = ifd + 2 + count * 12 + 4;
    let entries = if before { 8 } else { table_end };
    let uid_at = table_end.max(entries + 32);
    let tiff_len = uid_at + if uid { 66 } else { 0 };
    let mut bytes = original[..18].to_vec();
    bytes.resize(10 + tiff_len, 0);
    bytes[4..6].copy_from_slice(&u16::try_from(tiff_len + 6).unwrap().to_be_bytes());
    tiff_long(&mut bytes, big, 4, ifd as u32);
    tiff_short(&mut bytes, big, ifd, count as u16);
    bytes[10 + ifd + 2..10 + ifd + 38].copy_from_slice(&original[20..56]);
    tiff_long(&mut bytes, big, ifd + 34, entries as u32);
    if uid {
        let tag = ifd + 38;
        tiff_short(&mut bytes, big, tag, 0xb003);
        tiff_short(&mut bytes, big, tag + 2, 7);
        tiff_long(&mut bytes, big, tag + 4, 66);
        tiff_long(&mut bytes, big, tag + 8, uid_at as u32);
        bytes[10 + uid_at..10 + uid_at + 33].copy_from_slice(b"0123456789abcdef0123456789abcdef\0");
        bytes[10 + uid_at + 33..10 + uid_at + 66]
            .copy_from_slice(b"fedcba9876543210fedcba9876543210\0");
    }
    if frames {
        let tag = ifd + 38 + usize::from(uid) * 12;
        tiff_short(&mut bytes, big, tag, 0xb004);
        tiff_short(&mut bytes, big, tag + 2, 4);
        tiff_long(&mut bytes, big, tag + 4, 1);
        // TotalFrames describes a larger collection, not NumberOfImages.
        tiff_long(&mut bytes, big, tag + 8, if before { u32::MAX } else { 0 });
    }
    bytes[10 + entries..10 + entries + 32].copy_from_slice(&original[60..92]);
    let primary = jpeg_end(original).unwrap() + tiff_len - 82;
    tiff_long(&mut bytes, big, entries + 4, primary as u32);
    tiff_long(&mut bytes, big, entries + 24, primary as u32 - 10);
    bytes.extend_from_slice(&original[92..]);
    bytes
}

// Deliberately separate from index(), whose exact three-tag layout qualifies
// only the frozen browser fixtures. Inspect actual TIFF fields and disjoint
// ranges, not a production parse result or just a successful native render.
fn inspect_optional_mpf(
    bytes: &[u8],
    original: &[u8],
    big: bool,
    uid: bool,
    frames: bool,
    before: bool,
) {
    let mpf = segment(bytes, 2, 0xe2).unwrap();
    assert_eq!(&mpf[4..8], b"MPF\0");
    let tiff = &mpf[8..];
    assert_eq!(&tiff[..2], if big { b"MM" } else { b"II" });
    let short = |at| {
        let value = tiff[at..at + 2].try_into().unwrap();
        if big {
            u16::from_be_bytes(value)
        } else {
            u16::from_le_bytes(value)
        }
    };
    let long = |at| {
        let value = tiff[at..at + 4].try_into().unwrap();
        if big {
            u32::from_be_bytes(value)
        } else {
            u32::from_le_bytes(value)
        }
    };
    assert_eq!(short(2), 42);
    let ifd = long(4) as usize;
    let count = usize::from(short(ifd));
    let tags: Vec<_> = (0..count).map(|i| ifd + 2 + i * 12).collect();
    let mut expected = vec![(0xb000, 7, 4), (0xb001, 4, 1), (0xb002, 7, 32)];
    if uid {
        expected.push((0xb003, 7, 66));
    }
    if frames {
        expected.push((0xb004, 4, 1));
    }
    assert_eq!(
        tags.iter()
            .map(|&at| (short(at), short(at + 2), long(at + 4)))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(&tiff[tags[0] + 8..tags[0] + 12], b"0100");
    assert_eq!(long(tags[1] + 8), 2);
    let entries = long(tags[2] + 8) as usize;
    let table_end = ifd + 2 + count * 12 + 4;
    assert_eq!(long(table_end - 4), 0);
    assert_eq!(entries < ifd, before);
    let mut ranges = vec![0..8, ifd..table_end, entries..entries + 32];
    if uid {
        let start = long(tags[3] + 8) as usize;
        assert_eq!(
            &tiff[start..start + 66],
            b"0123456789abcdef0123456789abcdef\0fedcba9876543210fedcba9876543210\0"
        );
        ranges.push(start..start + 66);
    }
    if frames {
        assert_eq!(
            long(tags.last().unwrap() + 8),
            if before { u32::MAX } else { 0 }
        );
    }
    ranges.sort_by_key(|range| range.start);
    for pair in ranges.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    assert_eq!(ranges.last().unwrap().end, tiff.len());
    let primary = jpeg_end(bytes).unwrap();
    let secondary = jpeg_end(&bytes[primary..]).unwrap();
    assert_eq!(primary + secondary, bytes.len());
    assert_eq!(
        (long(entries), long(entries + 8), long(entries + 12)),
        (0x030000, 0, 0)
    );
    assert_eq!((long(entries + 16), long(entries + 28)), (0, 0));
    assert_eq!(long(entries + 4) as usize, primary);
    assert_eq!(long(entries + 20) as usize, secondary);
    assert_eq!(long(entries + 24) as usize + 10, primary);
    assert_eq!(&bytes[2 + mpf.len()..], &original[92..]);
    xmp(
        segment(bytes, 2 + mpf.len(), 0xe1).unwrap(),
        true,
        secondary,
        false,
        0,
    )
    .unwrap();
    xmp(
        segment(&bytes[primary..], 2, 0xe1).unwrap(),
        false,
        secondary,
        false,
        0,
    )
    .unwrap();
}

#[test]
fn optional_mpf_layouts_preserve_native_projection() {
    assert_optional_mpf_controls(&evidence());
}

fn assert_metadata_projection(row: &Value, name: &str, bytes: &[u8]) {
    let changed = with_mpf_bytes(row, name, bytes);
    for format in FORMATS {
        let (plain, plain_report) = render(&row["scene"], format);
        let (artifact, report) = render(&changed["scene"], format);
        assert_eq!(plain_report, json!({"schema_version":1,"diagnostics":[]}));
        assert_eq!(report, plain_report, "{name}: {format:?}");
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(&artifact).unwrap();
            svg_check(svg, &changed, true);
            let normalized = svg.replace(
                changed["scene"]["files"]["asset"]["dataURL"]
                    .as_str()
                    .unwrap(),
                row["scene"]["files"]["asset"]["dataURL"].as_str().unwrap(),
            );
            assert_eq!(normalized.as_bytes(), plain, "{name}");
        } else {
            assert_eq!(artifact, plain, "{name}: {format:?}");
            pixels(&artifact, format, row);
        }
    }
}

// Insert an APP2 immediately after the secondary SOI. Repair both the primary
// MPEntry size and XMP length, so a conflicting secondary index is the only
// rejection cause. These frozen sizes retain the same decimal digit count.
fn with_secondary_mpf(original: &[u8], big: bool, app2: &[u8]) -> Vec<u8> {
    assert_eq!(segment(app2, 0, 0xe2).unwrap(), app2);
    let primary = jpeg_end(original).unwrap();
    let secondary = jpeg_end(&original[primary..]).unwrap();
    assert_eq!(primary + secondary, original.len());
    let old_length = format!("Item:Length=\"{secondary}\"");
    let new_length = format!("Item:Length=\"{}\"", secondary + app2.len());
    assert_eq!(old_length.len(), new_length.len());
    let primary_xmp = segment(original, 92, 0xe1).unwrap();
    let length_at = 92
        + primary_xmp
            .windows(old_length.len())
            .position(|w| w == old_length.as_bytes())
            .unwrap();
    let mut bytes = original.to_vec();
    bytes[length_at..length_at + new_length.len()].copy_from_slice(new_length.as_bytes());
    tiff_long(&mut bytes, big, 70, (secondary + app2.len()) as u32);
    bytes.splice(primary + 2..primary + 2, app2.iter().copied());
    assert_eq!(jpeg_end(&bytes), Ok(primary));
    assert_eq!(jpeg_end(&bytes[primary..]), Ok(secondary + app2.len()));
    assert_eq!(bytes.len(), original.len() + app2.len());
    let entry = index(segment(&bytes, 2, 0xe2).unwrap(), big).unwrap();
    assert_eq!(entry.count, 2);
    assert_eq!(entry.primary, primary);
    assert_eq!(entry.offset + 10, primary);
    assert_eq!(entry.secondary, secondary + app2.len());
    assert_eq!(&bytes[primary + 2 + app2.len()..], &original[primary + 2..]);
    xmp(
        segment(&bytes, 92, 0xe1).unwrap(),
        true,
        secondary + app2.len(),
        false,
        0,
    )
    .unwrap();
    bytes
}

#[test]
fn secondary_mpf_index_has_exact_recovery() {
    assert_secondary_mpf_index_recovery(&evidence());
}

fn assert_secondary_mpf_index_recovery(record: &Value) {
    let mut rows = Vec::new();
    for mode in ["baseline", "progressive"] {
        for order in ["little", "big"] {
            let row = by_id(record, &json!(format!("{mode}-{order}-identity-ramp")));
            let original = decode(&row["jpeg_base64"]);
            let big = order == "big";
            assert_eq!(container(&original, big, false, 0), Ok(()));
            let copy = segment(&original, 2, 0xe2).unwrap();
            assert_eq!(copy.len(), 90);
            let bytes = with_secondary_mpf(&original, big, copy);
            let primary = jpeg_end(&bytes).unwrap();
            assert_eq!(segment(&bytes, primary + 2, 0xe2).unwrap(), copy);
            // It is a complete MP Index IFD, not an MP Attribute IFD. Its
            // copied image ranges also conflict with its new TIFF origin.
            let copied = index(copy, big).unwrap();
            assert_eq!(copied.count, 2);
            assert_ne!(primary + 10 + copied.offset, primary);
            rows.push(with_mpf_bytes(row, "secondary-mp-index", &bytes));
        }
    }
    assert_eq!(rows.len(), 4);
    for row in rows {
        exact_recovery(&row);
    }
}

#[test]
fn representative_primary_flag_preserves_native_projection() {
    assert_representative_primary_flag(&evidence());
}

fn assert_representative_primary_flag(record: &Value) {
    for mode in ["baseline", "progressive"] {
        for order in ["little", "big"] {
            let row = by_id(record, &json!(format!("{mode}-{order}-identity-ramp")));
            let original = decode(&row["jpeg_base64"]);
            let big = order == "big";
            assert_eq!(container(&original, big, false, 0), Ok(()));
            let mut bytes = original.clone();
            tiff_long(&mut bytes, big, 50, 0x20030000);
            let flag_byte = if big { 60 } else { 63 };
            assert_eq!(bytes[flag_byte], original[flag_byte] | 0x20);
            assert_eq!(&bytes[..flag_byte], &original[..flag_byte]);
            assert_eq!(&bytes[flag_byte + 1..], &original[flag_byte + 1..]);
            assert_metadata_projection(row, "representative-primary", &bytes);
        }
    }
}

#[test]
fn secondary_mpf_attribute_ifd_preserves_native_projection() {
    assert_secondary_mpf_attributes(&evidence());
}

fn assert_secondary_mpf_attributes(record: &Value) {
    let mut rejected = 0;
    for mode in ["baseline", "progressive"] {
        for order in ["little", "big"] {
            let row = by_id(record, &json!(format!("{mode}-{order}-identity-ramp")));
            let original = decode(&row["jpeg_base64"]);
            let big = order == "big";
            assert_eq!(container(&original, big, false, 0), Ok(()));
            // Independently authored APP2: 38-byte TIFF, sorted version and
            // MPIndividualNum tags, inline values, no MP Index or next IFD.
            let app2: &[u8] = if big {
                b"\xff\xe2\x00\x2cMPF\0MM\x00\x2a\x00\x00\x00\x08\x00\x02\xb0\x00\x00\x07\x00\x00\x00\x040100\xb1\x01\x00\x04\x00\x00\x00\x01\x00\x00\x00\x02\x00\x00\x00\x00"
            } else {
                b"\xff\xe2\x00\x2cMPF\0II\x2a\x00\x08\x00\x00\x00\x02\x00\x00\xb0\x07\x00\x04\x00\x00\x000100\x01\xb1\x04\x00\x01\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00"
            };
            assert_eq!(app2.len(), 46);
            let bytes = with_secondary_mpf(&original, big, app2);
            assert_eq!(
                segment(&bytes, jpeg_end(&bytes).unwrap() + 2, 0xe2).unwrap(),
                app2
            );
            assert_metadata_projection(row, "secondary-mp-attribute", &bytes);

            // A second individually valid MP Attribute APP2 is still a
            // duplicate. Reuse the same insertion path to repair MPEntry and
            // XMP lengths again, rather than failing on stale image ranges.
            let duplicate = with_secondary_mpf(&bytes, big, app2);
            let primary = jpeg_end(&duplicate).unwrap();
            assert_eq!(duplicate.len(), original.len() + 2 * app2.len());
            for at in [primary + 2, primary + 2 + app2.len()] {
                assert_eq!(segment(&duplicate, at, 0xe2).unwrap(), app2);
            }
            exact_recovery(&with_mpf_bytes(
                row,
                "duplicate-secondary-attribute",
                &duplicate,
            ));
            rejected += 1;

            // All offsets are TIFF-relative in the secondary image. Mutating
            // the TIFF leaves APP2/JPEG framing and repaired XMP lengths valid.
            let mut mutations = Vec::new();
            for (name, at, value) in [
                ("attribute-bad-magic", 2, 0),
                ("attribute-huge-ifd-count", 8, u16::MAX),
                ("attribute-individual-not-long", 24, 3),
            ] {
                let mut changed = bytes.clone();
                tiff_short(&mut changed[primary..], big, at, value);
                mutations.push((name, changed));
            }
            for (name, at, value) in [
                ("attribute-huge-ifd-offset", 4, u32::MAX),
                ("attribute-wrong-version-count", 14, 3),
                ("attribute-wrong-individual-count", 26, 2),
                ("attribute-wrong-individual-number", 30, 1),
            ] {
                let mut changed = bytes.clone();
                tiff_long(&mut changed[primary..], big, at, value);
                mutations.push((name, changed));
            }
            assert_eq!(mutations.len(), 7);
            for (name, changed) in mutations {
                assert_ne!(changed, bytes);
                assert_eq!(&changed[..primary + 10], &bytes[..primary + 10]);
                assert_eq!(
                    &changed[primary + 2 + app2.len()..],
                    &bytes[primary + 2 + app2.len()..]
                );
                assert_eq!(jpeg_end(&changed), Ok(primary));
                assert_eq!(jpeg_end(&changed[primary..]), Ok(changed.len() - primary));
                exact_recovery(&with_mpf_bytes(row, name, &changed));
                rejected += 1;
            }
        }
    }
    assert_eq!(rejected, 32);
}

fn assert_optional_mpf_controls(record: &Value) {
    let mut valid = 0;
    let mut invalid = 0;
    for mode in ["baseline", "progressive"] {
        for order in ["little", "big"] {
            let row = by_id(record, &json!(format!("{mode}-{order}-identity-ramp")));
            let original = decode(&row["jpeg_base64"]);
            let big = order == "big";
            let originals = FORMATS.map(|format| render(&row["scene"], format));
            for (uid, frames) in [(false, false), (true, false), (false, true), (true, true)] {
                for before in [false, true] {
                    let bytes = optional_mpf_bytes(&original, big, uid, frames, before);
                    inspect_optional_mpf(&bytes, &original, big, uid, frames, before);
                    let name = format!("uid-{uid}-frames-{frames}-entries-before-{before}");
                    let changed = with_mpf_bytes(row, &name, &bytes);
                    for (format, (plain, plain_report)) in FORMATS.into_iter().zip(&originals) {
                        let (artifact, report) = render(&changed["scene"], format);
                        assert_eq!(plain_report, &json!({"schema_version":1,"diagnostics":[]}));
                        assert_eq!(&report, plain_report, "{name}: {format:?}");
                        if format == OutputFormat::Svg {
                            let svg = std::str::from_utf8(&artifact).unwrap();
                            svg_check(svg, &changed, true);
                            let normalized = svg.replace(
                                changed["scene"]["files"]["asset"]["dataURL"]
                                    .as_str()
                                    .unwrap(),
                                row["scene"]["files"]["asset"]["dataURL"].as_str().unwrap(),
                            );
                            assert_eq!(normalized.as_bytes(), plain, "{name}");
                        } else {
                            assert_eq!(&artifact, plain, "{name}: {format:?}");
                            pixels(&artifact, format, row);
                        }
                    }
                    valid += 1;

                    let ifd = if before { 40 } else { 8 };
                    let count = 3 + usize::from(uid) + usize::from(frames);
                    let table_end = ifd + 2 + count * 12 + 4;
                    let entries = if before { 8 } else { table_end };
                    let mut mutations = Vec::new();
                    let mut reordered = bytes.clone();
                    let first = 10 + ifd + 2;
                    // Swap whole required tags; all values and ranges survive.
                    reordered[first..first + 24].rotate_left(12);
                    mutations.push(("required-tags-out-of-order", reordered));
                    let mut next = bytes.clone();
                    tiff_long(&mut next, big, table_end - 4, ifd as u32);
                    mutations.push(("nonzero-next-ifd", next));
                    if uid {
                        for (reason, offset) in [
                            ("uid-overlaps-header", 0),
                            ("uid-overlaps-table", ifd as u32),
                            ("uid-overlaps-entries", entries as u32),
                            (
                                "uid-out-of-bounds",
                                (segment(&bytes, 2, 0xe2).unwrap().len() - 8 - 65) as u32,
                            ),
                        ] {
                            let mut bad_uid = bytes.clone();
                            tiff_long(&mut bad_uid, big, ifd + 46, offset);
                            mutations.push((reason, bad_uid));
                        }
                    }
                    for (reason, bad) in mutations {
                        assert_ne!(bad, bytes);
                        assert_eq!(jpeg_end(&bad), jpeg_end(&bytes));
                        let payload = 2 + segment(&bytes, 2, 0xe2).unwrap().len();
                        assert_eq!(&bad[payload..], &bytes[payload..]);
                        exact_recovery(&with_mpf_bytes(row, &format!("{name}-{reason}"), &bad));
                        invalid += 1;
                    }
                }
            }
        }
    }
    assert_eq!((valid, invalid), (32, 128));
}

#[test]
fn malformed_mpf_fields_and_duplicate_indexes_have_exact_recovery() {
    let record = evidence();
    assert_optional_mpf_controls(&record);
    assert_malformed_mpf_fields(&record);
}

fn assert_malformed_mpf_fields(record: &Value) {
    let bases = base_evidence();
    let mut recovery_rows = Vec::new();
    for mode in ["baseline", "progressive"] {
        for order in ["little", "big"] {
            let row = by_id(record, &json!(format!("{mode}-{order}-identity-ramp")));
            admitted(row, by_id(&bases, &row["base_id"]), false);
            let bytes = decode(&row["jpeg_base64"]);
            let big = order == "big";
            assert_eq!(container(&bytes, big, false, 0), Ok(()));
            let primary = jpeg_end(&bytes).unwrap();
            let secondary = jpeg_end(&bytes[primary..]).unwrap();
            assert_eq!(primary + secondary, bytes.len());
            let mut mutations = Vec::new();
            for (name, at, value) in [
                ("huge-ifd-offset", 4, u32::MAX),
                ("huge-version-count", 14, u32::MAX),
                ("huge-number-tag-count", 26, u32::MAX),
                ("huge-image-count", 30, u32::MAX),
                ("huge-entry-tag-count", 38, u32::MAX),
                ("huge-entry-offset", 42, u32::MAX),
                ("entries-overlap-header", 42, 0),
                ("entries-overlap-ifd", 42, 8),
                ("entries-overlap-table-end", 42, 46),
                ("primary-not-jpeg", 50, 0x01030000),
                ("secondary-not-jpeg", 66, 0x01000000),
                ("primary-unsupported-type", 50, 0x030001),
                ("secondary-unsupported-type", 66, 0x030000),
                ("primary-offset-nonzero", 58, 1),
                ("primary-size-short", 54, primary as u32 - 1),
                ("secondary-overlaps-primary", 74, primary as u32 - 11),
                ("secondary-offset-past-end", 74, bytes.len() as u32),
                ("secondary-size-overflow", 70, u32::MAX),
            ] {
                let mut changed = bytes.clone();
                tiff_long(&mut changed, big, at, value);
                mutations.push((name, changed));
            }
            for (name, at, value) in [
                ("huge-ifd-count", 8, u16::MAX),
                ("number-not-long", 24, 3),
                ("entries-not-undefined", 36, 4),
                ("primary-dependency-one-out-of-range", 62, 3),
                ("primary-dependency-two-out-of-range", 64, u16::MAX),
                ("secondary-dependency-one-out-of-range", 78, 3),
                ("secondary-dependency-two-out-of-range", 80, u16::MAX),
            ] {
                let mut changed = bytes.clone();
                tiff_short(&mut changed, big, at, value);
                mutations.push((name, changed));
            }
            let mut version = bytes.clone();
            version[28..32].copy_from_slice(b"0101");
            mutations.push(("unsupported-version", version));

            // Add a fourth IFD tag while retaining all three required tags.
            // Repair sizes and entry locations so duplicate detection, rather
            // than missing tags or stale image ranges, is what must reject it.
            for (name, tag) in [
                ("duplicate-version-tag", 20),
                ("duplicate-number-tag", 32),
                ("duplicate-entry-tag", 44),
            ] {
                let mut changed = bytes.clone();
                changed.splice(56..56, bytes[tag..tag + 12].iter().copied());
                changed[4..6].copy_from_slice(&100u16.to_be_bytes());
                tiff_short(&mut changed, big, 8, 4);
                tiff_long(&mut changed, big, 42, 62);
                if tag == 44 {
                    tiff_long(&mut changed, big, 54, 62);
                }
                tiff_long(&mut changed, big, 66, primary as u32 + 12);
                tiff_long(&mut changed, big, 86, primary as u32 + 2);
                assert_eq!(segment(&changed, 2, 0xe2).unwrap().len(), 102);
                assert_eq!(jpeg_end(&changed), Ok(primary + 12));
                mutations.push((name, changed));
            }

            // Both indexes describe the new actual ranges relative to their
            // own TIFF bases. Also reject an invalid first index followed by a
            // valid one (and the reverse): a later APP2 must not hide an error.
            let mut duplicate = bytes.clone();
            duplicate.splice(92..92, bytes[2..92].iter().copied());
            tiff_long(&mut duplicate, big, 54, primary as u32 + 90);
            tiff_long(&mut duplicate, big, 74, primary as u32 + 80);
            tiff_long(&mut duplicate[90..], big, 54, primary as u32 + 90);
            assert_eq!(jpeg_end(&duplicate), Ok(primary + 90));
            for (at, base) in [(2, 10), (92, 100)] {
                let entry = index(segment(&duplicate, at, 0xe2).unwrap(), big).unwrap();
                assert_eq!(entry.primary, primary + 90);
                assert_eq!(base + entry.offset, primary + 90);
                assert_eq!(entry.secondary, secondary);
            }
            let mut bad_first = duplicate.clone();
            tiff_short(&mut bad_first, big, 2, 0);
            let mut bad_last = duplicate.clone();
            tiff_short(&mut bad_last[90..], big, 2, 0);
            mutations.extend([
                ("duplicate-app2", duplicate),
                ("bad-first-mpf-valid-second", bad_first),
                ("valid-first-mpf-bad-second", bad_last),
            ]);
            assert_eq!(mutations.len(), 32);

            for (name, changed) in mutations {
                assert_ne!(changed, bytes, "{name}");
                let end = jpeg_end(&changed).unwrap();
                // JPEG payload and XMP remain intact: only MPF has changed.
                assert_eq!(&changed[end..], &bytes[primary..], "{name}");
                let original_xmp = 2 + segment(&bytes, 2, 0xe2).unwrap().len();
                let changed_xmp = end - (primary - original_xmp);
                assert_eq!(
                    &changed[changed_xmp..end],
                    &bytes[original_xmp..primary],
                    "{name}"
                );
                assert!(container(&changed, big, false, 0).is_err(), "{name}");
                let mut mutated = row.clone();
                mutated["id"] = json!(format!("{mode}-{order}-{name}"));
                mutated["scene"]["files"]["asset"]["dataURL"] = json!(format!(
                    "data:image/jpeg;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&changed)
                ));
                recovery_rows.push(mutated);
            }
        }
    }
    // Validate every mutation's independent framing before observing renderer
    // recovery, including while the renderer regression is still RED.
    assert_eq!(recovery_rows.len(), 128);
    for row in recovery_rows {
        exact_recovery(&row);
    }
}

#[test]
fn structural_gates_reject_range_metadata_and_scope_mutations() {
    assert_structural_mutations(&evidence());
}

fn assert_structural_mutations(record: &Value) {
    assert_optional_mpf_controls(record);
    for order in ["little", "big"] {
        let row = by_id(record, &json!(format!("baseline-{order}-hdr-ramp-ramp")));
        let bytes = decode(&row["jpeg_base64"]);
        let big = order == "big";
        assert_eq!(container(&bytes, big, true, 2), Ok(()));
        assert!(container(&bytes, !big, true, 2).is_err());
        assert!(container(&bytes, big, false, 2).is_err());
        assert!(container(&bytes, big, true, 0).is_err());
        // Absolute offsets below are independently addressed through TIFF's
        // byte-10 base. Exercise magic, IFD, tag types/counts, entry location,
        // image attributes, sizes, offsets, and both dependency indices.
        for at in [
            12, 14, 18, 20, 22, 24, 28, 32, 34, 36, 40, 44, 46, 48, 52, 56, 60, 64, 68, 72, 74, 76,
            80, 84, 88, 90,
        ] {
            let mut changed = bytes.clone();
            changed[at] ^= 1;
            assert!(
                container(&changed, big, true, 2).is_err(),
                "{order}: byte {at}"
            );
        }
        let secondary = row["primary_size"].as_u64().unwrap() as usize;
        for cut in [0, 1, 2, 10, 91, secondary, bytes.len() - 2, bytes.len() - 1] {
            assert!(
                container(&bytes[..cut], big, true, 2).is_err(),
                "{order}: cut {cut}"
            );
        }
        for marker in [
            b"hdrgm:Version=\"1.0\"".as_slice(),
            b"hdrgm:GainMapMin=\"0\"",
            b"hdrgm:Gamma=\"1\"",
            b"hdrgm:OffsetSDR=\"0\"",
            b"hdrgm:OffsetHDR=\"0\"",
            b"hdrgm:HDRCapacityMin=\"0\"",
            b"hdrgm:HDRCapacityMax=\"2\"",
            b"Item:Length=\"",
            b"Item:Semantic=\"GainMap\"",
            b"http://ns.adobe.com/hdr-gain-map/1.0/",
        ] {
            let mut changed = bytes.clone();
            let at = changed
                .windows(marker.len())
                .rposition(|w| w == marker)
                .unwrap();
            changed[at + marker.len() - 2] ^= 1;
            assert!(
                container(&changed, big, true, 2).is_err(),
                "{order}: {marker:?}"
            );
        }
        let mut third = bytes.clone();
        third.extend_from_slice(&bytes[secondary..]);
        assert_eq!(container(&third, big, true, 2), Err("third-image"));
    }
    // A record outside this bounded matrix cannot acquire qualification by
    // preserving browser pixels or claiming container_valid=true.
    for (key, value) in [
        ("signal", json!("hdr-unknown")),
        ("map_id", json!("gray-1x1-baseline-edges")),
        ("container_valid", json!(false)),
        ("width", json!(34)),
    ] {
        let mut row = by_id(record, &json!("baseline-little-hdr-ramp-ramp")).clone();
        row[key] = value;
        assert!(
            std::panic::catch_unwind(|| structural_row(&row, &inputs())).is_err(),
            "{key}"
        );
    }
}

#[test]
fn hdr_reference_discriminates_map_content_and_base_projection() {
    assert_nonidentity_map_discrimination(&evidence());
}

fn assert_nonidentity_map_discrimination(record: &Value) {
    let bases = base_evidence();
    for row in selected(record, &["hdr-ramp"], 4) {
        let edge = record["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|other| {
                other["mode"] == row["mode"]
                    && other["byte_order"] == row["byte_order"]
                    && other["pattern"] == row["pattern"]
                    && other["signal"] == "hdr-edges"
            })
            .unwrap();
        let base = by_id(&bases, &row["base_id"]);
        for key in ["png", "svg_png"] {
            let ramp = raster(&decode(&row["reference"][key]));
            let edges = raster(&decode(&edge["reference"][key]));
            let plain = raster(&decode(&base["reference"][key]));
            assert_ne!(
                ramp, edges,
                "map content must affect the browser's HDR projection"
            );
            assert!(!comparison(&ramp, &plain).passed());
            assert!(!comparison(&edges, &plain).passed());
        }
    }
}

#[test]
fn output_gates_reject_extent_source_alpha_and_binding_mutations() {
    let record = evidence();
    assert_optional_mpf_controls(&record);
    assert_output_mutations(&record);
}

fn assert_output_mutations(record: &Value) {
    let row = by_id(record, &json!("baseline-little-identity-ramp"));
    let (svg, _) = render(&row["scene"], OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).unwrap();
    for changed in [
        svg.replacen("width=\"33\"", "width=\"34\"", 1),
        svg.replacen("viewBox=\"0 0 33 25\"", "viewBox=\"1 0 33 25\"", 1),
        svg.replace(
            row["scene"]["files"]["asset"]["dataURL"].as_str().unwrap(),
            "data:image/jpeg;base64,/9j/2Q==",
        ),
    ] {
        assert_ne!(changed, svg);
        assert!(std::panic::catch_unwind(|| svg_check(&changed, row, true)).is_err());
    }
    let (pdf, _) = render(&row["scene"], OutputFormat::Pdf);
    for marker in [
        b"/MediaBox [0 0 ".as_slice(),
        b"/CropBox [0 0 ",
        b"/Im0 ",
        b"/ICCBased ",
    ] {
        let mut changed = pdf.clone();
        let at = changed
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap();
        changed[at + marker.len()] = b'9';
        assert_ne!(changed, pdf);
        assert!(std::panic::catch_unwind(|| pixels(&changed, OutputFormat::Pdf, row)).is_err());
    }
    let (png, _) = render(&row["scene"], OutputFormat::Png);
    let original = crate::common::raster::decode_png(&png);
    for (width, color, alpha) in [
        (34, png::ColorType::Rgba, 255),
        (33, png::ColorType::Rgb, 255),
        (33, png::ColorType::Rgba, 254),
    ] {
        let channels = if color == png::ColorType::Rgb { 3 } else { 4 };
        let mut samples: Vec<u8> = original
            .rgba
            .chunks_exact(4)
            .flat_map(|p| {
                if channels == 3 {
                    p[..3].to_vec()
                } else {
                    vec![p[0], p[1], p[2], alpha]
                }
            })
            .collect();
        samples.resize((width * original.height) as usize * channels, 255);
        let mut changed = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut changed, width, original.height);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&samples)
                .unwrap();
        }
        assert!(std::panic::catch_unwind(|| pixels(&changed, OutputFormat::Png, row)).is_err());
    }
    let recovery = by_id(record, &json!("baseline-little-omit-mpf-ramp"));
    for format in FORMATS {
        let (bytes, _) = render(&recovery["scene"], format);
        let mut wrong_extent = recovery.clone();
        wrong_extent["width"] = json!(34);
        assert!(
            std::panic::catch_unwind(|| recovery_artifact(&bytes, format, &wrong_extent)).is_err()
        );
    }
}
