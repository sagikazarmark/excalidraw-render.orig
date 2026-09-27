//! Nested SVG sanitization and references against the pinned reference (#85
//! nested-SVG facets). The record (`tests/oracle/nested-svg/reference.json`,
//! collected by `tools/reference-oracle/run-nested-svg.sh`) draws each 24x24
//! SVG as an image at its natural size. Each row declares its expectation:
//! - `match`: the PNG passes the canonical comparison with the reference. The
//!   sanitizer may report what it removed, but nothing fails, and SVG and PDF
//!   report exactly what PNG reports.
//! - `rejected`: both reject; every format recovers exactly as a missing asset.
//! - `divergent`: the reference draws animation state or foreign content the
//!   renderer removes (spec section 8.2). PNG and PDF equal the row's static
//!   twin exactly, and an external image with no twin keeps its pinned
//!   placeholder.
use excalidraw_render::OutputFormat;
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::webp_sample_contract::render;

const RECORD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/nested-svg/reference.json"
));
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
/// Diagnostics the sanitizer may report for a row the reference draws.
const REMOVALS: [&str; 2] = [
    "image.unsafe-svg-content-removed",
    "image.forbidden-resource",
];
/// The external image, pinned to the renderer's exact forbidden-resource
/// placeholder PNG; the reference draws its own broken-image placeholder.
const EXTERNAL_IMAGE_PNG: &str = "368ec0b0b05d7c238069191bb1ba2edd12eaa6758da6daaf8ce1e2295aa883fd";

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn image(png: &[u8]) -> CanonicalRgba8 {
    let image = crate::common::raster::decode_png(png);
    CanonicalRgba8::new(image.width, image.height, image.rgba).unwrap()
}

fn evidence() -> Value {
    assert_eq!(
        hash(RECORD),
        "1fcf9c728b1ba653b5fa47aa2b1f652bcb872a2e58e7c4de1063b5788e8a8e54"
    );
    let record: Value = serde_json::from_slice(RECORD).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/nested-svg-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], hash(&bytes), "{}", source["path"]);
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 54);
    for row in rows {
        let admitted = row["reference"]["admitted"] == true;
        assert_eq!(admitted, row["expectation"] != "rejected", "{}", row["id"]);
        if let Some(twin) = row["twin"].as_str() {
            let twin = rows.iter().find(|r| r["id"] == twin).unwrap();
            assert_ne!(
                row["reference"], twin["reference"],
                "{}: the reference now draws it like its twin",
                row["id"]
            );
        }
    }
    record
}

fn scene(svg: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":24,"height":24,
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/svg+xml","created":1,
            "dataURL":format!("data:image/svg+xml;base64,{svg}")}}})
}

/// The report's diagnostics without their format, and without the PDF
/// raster fallback an isolated effect such as `feImage` adds (Section 8.6).
fn diagnostics(report: &Value) -> Vec<Value> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] != "pdf.raster-fallback")
        .map(|d| {
            let mut d = d.clone();
            d.as_object_mut().unwrap().remove("format");
            d
        })
        .collect()
}

/// Judges the rows of `groups` in every format.
fn check(groups: &[&str]) -> usize {
    use base64::Engine as _;
    let record = evidence();
    let rows = record["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for row in rows
        .iter()
        .filter(|r| groups.contains(&r["group"].as_str().unwrap()))
    {
        let id = row["id"].as_str().unwrap();
        let source = scene(row["svg"].as_str().unwrap());
        let renders: Vec<_> = FORMATS.iter().map(|&f| render(&source, f)).collect();
        match row["expectation"].as_str().unwrap() {
            "rejected" => {
                let mut missing = source.clone();
                missing["files"] = json!({});
                for (&format, (bytes, report)) in FORMATS.iter().zip(&renders) {
                    let codes: Vec<_> = report["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|d| d["code"].as_str().unwrap())
                        .collect();
                    assert_eq!(codes, ["image.decode-failed"], "{id} {format:?}: {report}");
                    assert_eq!(
                        *bytes,
                        render(&missing, format).0,
                        "{id} {format:?} exact recovery"
                    );
                }
            }
            expectation => {
                let png = &renders[1];
                for d in png.1["diagnostics"].as_array().unwrap() {
                    assert!(
                        REMOVALS.contains(&d["code"].as_str().unwrap()),
                        "{id}: {}",
                        png.1
                    );
                }
                for (&format, (_, report)) in FORMATS.iter().zip(&renders) {
                    assert_eq!(
                        diagnostics(report),
                        diagnostics(&png.1),
                        "{id} {format:?} reports what PNG reports"
                    );
                }
                if expectation == "divergent" {
                    match row["twin"].as_str() {
                        Some(twin) => {
                            let twin = rows.iter().find(|r| r["id"] == twin).unwrap();
                            let twin = scene(twin["svg"].as_str().unwrap());
                            for (index, format) in [(1, OutputFormat::Png), (2, OutputFormat::Pdf)]
                            {
                                assert_eq!(
                                    renders[index].0,
                                    render(&twin, format).0,
                                    "{id} {format:?} draws its static twin"
                                );
                            }
                        }
                        None => {
                            assert_eq!(hash(&png.0), EXTERNAL_IMAGE_PNG, "{id}: placeholder moved")
                        }
                    }
                    checked += 1;
                    continue;
                }
                let reference = image(
                    &base64::engine::general_purpose::STANDARD
                        .decode(row["reference"]["png"].as_str().unwrap())
                        .unwrap(),
                );
                let result = compare(
                    &reference,
                    &image(&png.0),
                    VisualClass::GeometryImage,
                    &[],
                    ComparisonMode::Canonical,
                )
                .unwrap();
                if !result.passed() {
                    failures.push(format!("{id}: {:?}", result.whole));
                }
            }
        }
        checked += 1;
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    checked
}

/// Nested-SVG sanitization facet: scripts, event handlers, `javascript:`
/// links, stylesheet processing instructions, external entities, `@import`,
/// `@font-face` and external `url()`, `use` and `xml:base` references never
/// run or load, and what remains draws as the reference draws it: the other
/// stylesheet rules survive, an external paint falls back as a missing one,
/// and internal entities expand. An undefined entity or an expansion bomb
/// rejects, as in the reference. Animation state and `foreignObject`, which
/// the reference draws, are removed (the declared divergence).
#[test]
pub(super) fn nested_svg_sanitization_matches_the_reference_where_it_stays_static() {
    assert_eq!(check(&["base", "sanitization"]), 35);
}

/// Nested-SVG references facet: same-document fragments, missing fragments
/// and their fallbacks, `use`, pattern, mask, clip and gradient cycles cut at
/// the closing edge, and base64 and percent-encoded data images, recursively
/// sanitized, drawn as the reference draws them; with the depth and node
/// limits.
#[test]
pub(super) fn nested_svg_references_match_the_reference() {
    assert_eq!(check(&["base", "references"]), 22);
    super::resource_diagnostic_contract::nested_depth_contract_covers_owners_paths_frames_and_silence();
    super::resource_diagnostic_contract::nested_node_limits_cover_root_and_child_reports_and_silence();
}
