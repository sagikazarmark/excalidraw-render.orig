//! BMP and ICO headers, palettes, masks, alpha, run-length encoding, color
//! spaces, icon directories and entries against the pinned reference (#85 BMP
//! and ICO facets). The record (`tests/oracle/bmp-ico-container/reference.json`,
//! collected by `tools/reference-oracle/run-bmp-ico-container.sh`) builds every
//! row from one 4x3 picture. Each row declares its expectation:
//! - `match`: exact in PNG, and in PDF where the reference is opaque, with
//!   exact PDF alpha, and the source octets in SVG. Color-managed views that
//!   differ from Skia's float arithmetic by rounding are held at approved
//!   exact-ceiling exceptions (spec section 14.4), and an exception whose view
//!   passes is rejected as dead.
//! - `partial`: the reference loads the file but paints only the pixels it
//!   could decode; the renderer recovers exactly as a missing asset (the
//!   declared divergence).
//! - `rejected`: both reject, with exact missing-asset recovery.
//! - `divergent`: the reference paints a PNG icon entry that needs a color
//!   transform fully transparent; the renderer draws the entry exactly as the
//!   same PNG standalone.
use excalidraw_render::OutputFormat;
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, RegionReport, SsimOutcome, VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::webp_sample_contract::render;
use crate::common::lower_hex;

const RECORD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/bmp-ico-container/reference.json"
));
const EXCEPTIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/bmp-ico-container/color-exceptions.json"
));
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn hash(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn base64(value: &Value) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}

fn image(png: &[u8]) -> CanonicalRgba8 {
    let image = crate::common::raster::decode_png(png);
    CanonicalRgba8::new(image.width, image.height, image.rgba).unwrap()
}

fn evidence() -> Value {
    assert_eq!(
        hash(RECORD),
        "20ec948b1895b67499735da746460d385b3bce830cb206bd5e96b5146fee5f05"
    );
    let record: Value = serde_json::from_slice(RECORD).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/bmp-ico-container-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], hash(&bytes), "{}", source["path"]);
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 104);
    for row in rows {
        let admitted = row["reference"]["admitted"] == true;
        assert_eq!(admitted, row["expectation"] != "rejected", "{}", row["id"]);
        if admitted {
            assert_eq!(row["reference"]["size"], row["size"], "{}", row["id"]);
        }
    }
    record
}

/// Checked-in color exceptions, reviewed alongside their regression tests.
fn approved_exceptions() -> Value {
    serde_json::from_slice(EXCEPTIONS).unwrap()
}

fn scene(row: &Value, mime: &str, payload: &str) -> Value {
    let size = &row["size"];
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":size[0],"height":size[1],
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":mime,"created":1,
            "dataURL":format!("data:{mime};base64,{payload}")}}})
}

fn codes(report: &Value) -> Vec<String> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_owned())
        .collect()
}

/// The PNG of an icon's only entry.
fn png_entry(icon: &[u8]) -> String {
    use base64::Engine as _;
    assert_eq!(&icon[4..6], &[1, 0], "one entry");
    let size = u32::from_le_bytes(icon[14..18].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(icon[18..22].try_into().unwrap()) as usize;
    base64::engine::general_purpose::STANDARD.encode(&icon[offset..offset + size])
}

/// `(Pixelmatch changed pixels, differing pixels, maximum channel delta)`.
fn measure(whole: &RegionReport, expected: &[u8], actual: &[u8]) -> (usize, usize, u8) {
    let (mut differing, mut maximum) = (0, 0);
    for (a, b) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        differing += usize::from(a != b);
        maximum = maximum.max((0..4).map(|c| a[c].abs_diff(b[c])).max().unwrap());
    }
    (whole.changed_pixels as usize, differing, maximum)
}

/// Compares one view canonically; a failing view must sit within its
/// exception's exact ceilings, and a passing view must have none.
fn judge(
    id: &str,
    format: &str,
    expected: &CanonicalRgba8,
    actual: &CanonicalRgba8,
    exceptions: &Value,
    used: &mut std::collections::BTreeSet<(String, String)>,
    failures: &mut Vec<String>,
) {
    let result = compare(
        expected,
        actual,
        VisualClass::GeometryImage,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    let exception = exceptions["exceptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["row"] == id && e["format"] == format);
    match (result.passed(), exception) {
        (true, None) => {}
        (true, Some(_)) => failures.push(format!("dead exception {id} {format}")),
        (false, None) => {
            let (changed, differing, delta) =
                measure(&result.whole, expected.rgba(), actual.rgba());
            failures.push(format!(
                "{{\"row\":\"{id}\",\"format\":\"{format}\",\"max_pixelmatch_changed_pixels\":{changed},\"max_differing_pixels\":{differing},\"max_rgb_delta\":{delta}}}"
            ));
        }
        (false, Some(e)) => {
            used.insert((id.to_owned(), format.to_owned()));
            let (changed, differing, delta) =
                measure(&result.whole, expected.rgba(), actual.rgba());
            let within = matches!(result.whole.ssim, SsimOutcome::SmallArtifactMismatch)
                && changed as u64 <= e["max_pixelmatch_changed_pixels"].as_u64().unwrap()
                && differing as u64 <= e["max_differing_pixels"].as_u64().unwrap()
                && u64::from(delta) <= e["max_rgb_delta"].as_u64().unwrap();
            if !within {
                failures.push(format!(
                    "{id} {format} exceeds its exception: {changed}/{differing}/{delta}"
                ));
            }
        }
    }
}

/// Judges the `family` (`bmp` or `ico`) rows of `groups` in `formats`.
fn check(family: &str, groups: &[&str], formats: &[OutputFormat]) -> usize {
    let record = evidence();
    let exceptions = approved_exceptions();
    let rows = record["cases"].as_array().unwrap();
    let judged = |row: &Value| {
        row["id"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{family}-"))
            && groups.contains(&row["group"].as_str().unwrap())
    };
    let mut used = std::collections::BTreeSet::new();
    let mut failures = Vec::new();
    let mut checked = 0;
    for row in rows.iter().filter(|r| judged(r)) {
        let id = row["id"].as_str().unwrap();
        let mime = row["mime"].as_str().unwrap();
        let source = scene(row, mime, row["payload"].as_str().unwrap());
        let expectation = row["expectation"].as_str().unwrap();
        for &format in formats {
            let (bytes, report) = render(&source, format);
            let found = codes(&report);
            if matches!(expectation, "partial" | "rejected") {
                assert_eq!(found.len(), 1, "{id} {format:?}: {report}");
                assert!(
                    matches!(
                        found[0].as_str(),
                        "image.decode-failed" | "image.unsupported-format"
                    ),
                    "{id} {format:?}: {report}"
                );
                let mut missing = source.clone();
                missing["files"] = json!({});
                assert_eq!(
                    bytes,
                    render(&missing, format).0,
                    "{id} {format:?} exact recovery"
                );
                continue;
            }
            assert!(found.is_empty(), "{id} {format:?}: {report}");
            let reference = image(&base64(&row["reference"]["png"]));
            let (width, height) = (reference.width() as usize, reference.height() as usize);
            if expectation == "divergent" {
                assert!(
                    reference.rgba().chunks_exact(4).all(|p| p[3] == 0),
                    "{id}: the reference no longer paints the entry transparent"
                );
                if format != OutputFormat::Svg {
                    let standalone = scene(row, "image/png", &png_entry(&base64(&row["payload"])));
                    let (expected, report) = render(&standalone, format);
                    assert!(codes(&report).is_empty(), "{id}: {report}");
                    assert_eq!(
                        bytes, expected,
                        "{id} {format:?} draws as the PNG standalone"
                    );
                    continue;
                }
            }
            match format {
                OutputFormat::Svg => {
                    let svg = std::str::from_utf8(&bytes).unwrap();
                    use base64::Engine as _;
                    let wire =
                        base64::engine::general_purpose::STANDARD.encode(base64(&row["payload"]));
                    assert!(
                        svg.contains(&format!("base64,{wire}\"")),
                        "{id}: exact source octets"
                    );
                }
                OutputFormat::Png => judge(
                    id,
                    "png",
                    &reference,
                    &image(&bytes),
                    &exceptions,
                    &mut used,
                    &mut failures,
                ),
                OutputFormat::Pdf => {
                    let rgba = super::png_profile_contract::pdf_rgba(&bytes, width, height);
                    let alpha =
                        |rgba: &[u8]| rgba.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>();
                    assert_eq!(alpha(&rgba), alpha(reference.rgba()), "{id} PDF alpha");
                    if reference.rgba().chunks_exact(4).all(|p| p[3] == 255) {
                        let actual =
                            CanonicalRgba8::new(width as u32, height as u32, rgba).unwrap();
                        judge(
                            id,
                            "pdf",
                            &reference,
                            &actual,
                            &exceptions,
                            &mut used,
                            &mut failures,
                        );
                    }
                }
                _ => unreachable!(),
            }
        }
        checked += 1;
    }
    // Every exception for a judged row and format must have been used.
    for e in exceptions["exceptions"].as_array().unwrap() {
        let row = rows
            .iter()
            .find(|r| r["id"] == e["row"])
            .expect("exception for an unknown row");
        let format = e["format"].as_str().unwrap();
        if judged(row)
            && formats.iter().any(|f| f.as_str() == format)
            && !used.contains(&(e["row"].as_str().unwrap().to_owned(), format.to_owned()))
        {
            failures.push(format!("unused exception {} {format}", e["row"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    checked
}

/// BMP orientation facet: BMP has no orientation metadata; bottom-up and
/// top-down row order (direct and run-length) draw the same picture.
#[test]
pub(super) fn bmp_row_order_is_its_only_orientation() {
    assert_eq!(check("bmp", &["base", "orientation"], &FORMATS), 9);
}

/// BMP color-profile facet: sRGB, `Win `, `LINK` and unknown color space
/// types draw as sRGB; calibrated RGB is color-managed unless unusable; an
/// embedded profile applies, and one outside the file or unparseable rejects.
#[test]
pub(super) fn bmp_color_spaces_apply_as_the_reference_applies_them() {
    assert_eq!(check("bmp", &["base", "color-profile"], &FORMATS), 20);
}

/// BMP alpha facet: alpha comes only from a V3+ alpha mask or
/// `BI_ALPHABITFIELDS`; an all-zero alpha channel draws opaque; exact PDF
/// alpha and soft mask; RLE holes are transparent.
#[test]
pub(super) fn bmp_alpha_matches_the_reference() {
    assert_eq!(check("bmp", &["base", "alpha", "rle"], &FORMATS), 24);
}

/// BMP default-frame facet: every accepted depth and compression draws its
/// one image.
#[test]
pub(super) fn bmp_default_image_matches_the_reference() {
    assert_eq!(check("bmp", &["base", "rle", "masks"], &FORMATS), 22);
}

/// BMP malformed facet: header, palette, mask and run-length faults the
/// reference tolerates draw as it draws them; faults it rejects, and pixel
/// data it paints only partially (the declared divergence), recover exactly.
#[test]
pub(super) fn bmp_containers_and_malformed_payloads_match_the_reference() {
    assert_eq!(
        check(
            "bmp",
            &["base", "container", "palette", "masks", "rle", "partial"],
            &FORMATS
        ),
        42
    );
}

/// BMP passthrough facet: direct SVG embeds the exact octets of every row the
/// reference draws.
#[test]
pub(super) fn bmp_svg_embeds_every_admitted_payload_exactly() {
    let groups = [
        "base",
        "orientation",
        "container",
        "palette",
        "alpha",
        "masks",
        "rle",
        "partial",
        "color-profile",
    ];
    assert_eq!(check("bmp", &groups, &[OutputFormat::Svg]), 66);
}

/// ICO orientation facet: a PNG entry's EXIF orientation is ignored, as the
/// reference ignores it, and a top-down DIB entry draws upright.
#[test]
pub(super) fn ico_orientation_metadata_is_ignored_as_the_reference_ignores_it() {
    assert_eq!(check("ico", &["base", "orientation"], &FORMATS), 6);
}

/// ICO color-profile facet: DIB entries are sRGB; PNG entries follow the PNG
/// color rules, and sRGB-equivalent ones match the reference exactly. A PNG
/// entry that needs a transform is the declared divergence: the reference
/// paints it transparent, the renderer draws it as the PNG standalone.
#[test]
pub(super) fn ico_color_profiles_apply_as_png_entries_apply_them() {
    assert_eq!(check("ico", &["base", "color-profile"], &FORMATS), 12);
}

/// ICO alpha facet: a 32-bit entry's alpha, the AND mask where the alpha is
/// all zero or the depth has none, exact PDF alpha and soft mask.
#[test]
pub(super) fn ico_alpha_matches_the_reference() {
    assert_eq!(check("ico", &["base", "alpha"], &FORMATS), 9);
}

/// ICO default-frame facet: the largest entry, then the deepest, then the
/// first, for icons and cursors.
#[test]
pub(super) fn ico_default_entry_matches_the_reference() {
    assert_eq!(check("ico", &["base", "default-frame"], &FORMATS), 8);
}

/// ICO malformed facet: directory and entry faults the reference tolerates
/// draw as it draws them; faults it rejects, and entries it cannot decode or
/// paints only partially (the declared divergence), recover exactly.
#[test]
pub(super) fn ico_containers_and_malformed_payloads_match_the_reference() {
    assert_eq!(
        check("ico", &["base", "container", "partial"], &FORMATS),
        15
    );
}

/// ICO passthrough facet: direct SVG embeds the exact octets of every row the
/// reference draws.
#[test]
pub(super) fn ico_svg_embeds_every_admitted_payload_exactly() {
    let groups = [
        "base",
        "orientation",
        "container",
        "alpha",
        "default-frame",
        "partial",
        "color-profile",
    ];
    assert_eq!(check("ico", &groups, &[OutputFormat::Svg]), 38);
}
