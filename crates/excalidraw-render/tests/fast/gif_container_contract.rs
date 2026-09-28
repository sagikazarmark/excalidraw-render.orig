//! GIF container, metadata, palette, bounds, animation and partial-data paths
//! against the pinned reference (#85 GIF facets). The record
//! (`tests/oracle/gif-container/reference.json`, collected by
//! `tools/reference-oracle/run-gif-container.sh`) edits frozen GIF vectors and
//! carries the 48 frozen GIF controls, each drawn at its natural size. A
//! `match` row must equal the reference exactly in PNG and PDF (GIF alpha is
//! binary) and keep its source octets in SVG; a `partial` row, whose first
//! frame the reference paints partially, and a `rejected` row recover exactly
//! as a missing asset. `partial` rows are the declared divergence.
use excalidraw_render::OutputFormat;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::webp_sample_contract::render;
use crate::common::lower_hex;

const RECORD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/gif-container/reference.json"
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

pub(super) fn evidence() -> Value {
    assert_eq!(
        hash(RECORD),
        "a484c48f5fedeffff93a68c1f75c916a5ed7dd29887336781260fe5f807f66d8"
    );
    let record: Value = serde_json::from_slice(RECORD).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/gif-container-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], hash(&bytes), "{}", source["path"]);
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 97);
    for row in rows {
        let admitted = row["reference"]["admitted"] == true;
        let rejected = row["expectation"].as_str().unwrap().ends_with("rejected");
        assert_eq!(admitted, !rejected, "{}", row["id"]);
        if admitted {
            assert_eq!(row["reference"]["size"], row["size"], "{}", row["id"]);
        }
    }
    record
}

/// Whether a frozen control (`prefix` is `first-frame`, `control` or `lzw`)
/// is still rejected: the reference rejects it, or paints its first frame
/// only partially (the declared divergence).
pub(super) fn strictly_rejected(prefix: &str, id: &str) -> bool {
    let record = evidence();
    let row = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == format!("control-{prefix}-{id}").as_str())
        .unwrap_or_else(|| panic!("no reference row for control {prefix}-{id}"));
    row["expectation"] != "match"
}

fn scene(row: &Value) -> Value {
    let size = &row["size"];
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":size[0],"height":size[1],
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/gif","created":1,
            "dataURL":format!("data:image/gif;base64,{}", row["gif"].as_str().unwrap())}}})
}

fn only_codes(report: &Value) -> Vec<String> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_owned())
        .collect()
}

/// Judges the rows of `groups` in `formats`.
fn check(groups: &[&str], formats: &[OutputFormat]) -> usize {
    let record = evidence();
    let mut checked = 0;
    for row in record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| groups.contains(&r["group"].as_str().unwrap()))
    {
        let id = row["id"].as_str().unwrap();
        let source = scene(row);
        for &format in formats {
            let (bytes, report) = render(&source, format);
            let codes = only_codes(&report);
            if row["expectation"] == "resource-rejected" {
                // Direct SVG derives no pixels and embeds it; PNG/PDF cross the
                // renderer's resource limits and recover.
                if format == OutputFormat::Svg {
                    assert!(codes.is_empty(), "{id} {format:?}: {report}");
                    continue;
                }
                assert_eq!(codes.len(), 1, "{id} {format:?}: {report}");
                assert!(
                    codes[0].starts_with("resource."),
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
            if row["expectation"] != "match" {
                assert_eq!(codes.len(), 1, "{id} {format:?}: {report}");
                assert!(
                    codes[0] == "image.decode-failed" || codes[0].starts_with("resource."),
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
            let flattening = format != OutputFormat::Svg && codes == ["image.animation-flattened"];
            assert!(codes.is_empty() || flattening, "{id} {format:?}: {report}");
            let reference = crate::common::raster::decode_png(&base64(&row["reference"]["png"]));
            let (width, height) = (reference.width as usize, reference.height as usize);
            match format {
                OutputFormat::Svg => {
                    let svg = std::str::from_utf8(&bytes).unwrap();
                    use base64::Engine as _;
                    let wire =
                        base64::engine::general_purpose::STANDARD.encode(base64(&row["gif"]));
                    assert!(
                        svg.contains(&format!("data:image/gif;base64,{wire}\"")),
                        "{id}: exact source octets"
                    );
                }
                OutputFormat::Png => {
                    let actual = crate::common::raster::decode_png(&bytes);
                    assert_eq!(
                        (actual.width, actual.height),
                        (reference.width, reference.height),
                        "{id}"
                    );
                    assert!(
                        actual.rgba == reference.rgba,
                        "{id} PNG differs from the reference"
                    );
                }
                OutputFormat::Pdf => {
                    let actual = super::png_profile_contract::pdf_rgba(&bytes, width, height);
                    assert!(
                        actual == reference.rgba,
                        "{id} PDF differs from the reference"
                    );
                }
                _ => unreachable!(),
            }
        }
        checked += 1;
    }
    checked
}

/// GIF orientation facet: GIF has no orientation, and XMP `tiff:Orientation`
/// (also oversized) is ignored, exactly as the reference ignores it.
#[test]
pub(super) fn gif_orientation_metadata_is_ignored_as_the_reference_ignores_it() {
    assert_eq!(check(&["base", "orientation"], &FORMATS), 7);
}

/// GIF color-profile facet: the palette is the color model; ICC application
/// extensions (transforming, unparseable, trailing, oversized) are never
/// applied, as the reference never applies them. With the palette matrices of
/// the first-frame record.
#[test]
pub(super) fn gif_palettes_are_the_only_color_model_as_in_the_reference() {
    assert_eq!(check(&["base", "color-profile"], &FORMATS), 9);
    super::gif_first_frame_contract::valid_first_frame_matrix_matches_frozen_reference_exactly();
}

/// GIF malformed facet: container faults the reference tolerates render like
/// their base; faults it rejects, and first frames it paints only partially
/// (the declared divergence), recover exactly; includes the 48 frozen
/// controls, reference-collected.
#[test]
pub(super) fn gif_containers_and_malformed_payloads_match_the_reference() {
    assert_eq!(
        check(&["base", "container", "partial", "controls"], &FORMATS),
        5 + 16 + 6 + 48
    );
}

/// GIF passthrough facet: direct SVG embeds the exact octets of every row the
/// reference admits completely.
#[test]
pub(super) fn gif_svg_embeds_every_admitted_payload_exactly() {
    let groups = [
        "base",
        "container",
        "color-profile",
        "orientation",
        "alpha",
        "default-frame",
        "animation",
        "partial",
        "controls",
    ];
    assert_eq!(check(&groups, &[OutputFormat::Svg]), 97);
}

/// GIF alpha facet: transparent indices (also out of palette range), indices
/// outside the palette drawn opaque black, a missing palette, exact PDF alpha
/// and soft mask, with the first-frame record's transparency matrices.
#[test]
pub(super) fn gif_alpha_matches_the_reference() {
    assert_eq!(check(&["base", "alpha"], &FORMATS), 10);
    super::gif_first_frame_contract::valid_first_frame_matrix_matches_frozen_reference_exactly();
}

/// GIF default-frame and animation facets: the canvas grows to the first
/// frame's extent; zero-size first frames and image-less files draw a
/// transparent screen; data after a complete first frame is ignored; later
/// frames only count (overhang, zero size, a malformed later GCE); PNG/PDF
/// flatten with one informational diagnostic. With the first-frame and
/// two-frame control records.
#[test]
pub(super) fn gif_default_frame_and_animation_match_the_reference() {
    assert_eq!(check(&["base", "default-frame", "animation"], &FORMATS), 16);
    super::gif_first_frame_contract::valid_first_frame_matrix_matches_frozen_reference_exactly();
    super::gif_control_contract::controls_match_frozen_reference_first_display();
}
