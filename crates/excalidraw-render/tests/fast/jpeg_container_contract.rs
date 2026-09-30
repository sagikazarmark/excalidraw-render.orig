//! JPEG containers, markers, entropy faults, default images, orientation,
//! color and gain-map containers against the pinned reference (#85 JPEG
//! facets). The record (`tests/oracle/jpeg-container/reference.json`,
//! collected by `tools/reference-oracle/run-jpeg-container.sh`) edits frozen
//! project-authored JPEGs, each drawn at its natural size. Each row declares
//! its expectation:
//! - `match`: PNG and PDF pass the canonical comparison with the reference,
//!   are opaque, and SVG keeps the source octets.
//! - `partial`: the reference loads the file but paints only what it could
//!   decode; the renderer recovers exactly as a missing asset (the declared
//!   divergence).
//! - `rejected`: both reject, with exact missing-asset recovery.
use excalidraw_render::OutputFormat;
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::webp_sample_contract::render;
use crate::common::lower_hex;

const RECORD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/jpeg-container/reference.json"
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
        "14c17d867fb54269a64ac0255ae22131e7e98454ebbf57da540d5cfd79bddc4c"
    );
    let record: Value = serde_json::from_slice(RECORD).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/jpeg-container-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], hash(&bytes), "{}", source["path"]);
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 65);
    for row in rows {
        let admitted = row["reference"]["admitted"] == true;
        assert_eq!(admitted, row["expectation"] != "rejected", "{}", row["id"]);
        if admitted {
            assert_eq!(row["reference"]["size"], row["size"], "{}", row["id"]);
        }
    }
    record
}

fn scene(row: &Value) -> Value {
    let size = &row["size"];
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":size[0],"height":size[1],
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/jpeg","created":1,
            "dataURL":format!("data:image/jpeg;base64,{}", row["jpeg"].as_str().unwrap())}}})
}

fn codes(report: &Value) -> Vec<String> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_owned())
        .collect()
}

/// Which views of an admitted row a check judges.
#[derive(Clone, Copy, PartialEq)]
enum Judge {
    /// PNG and PDF against the reference, SVG octets.
    Views,
    /// Only that PNG and PDF are opaque.
    Alpha,
}

/// Judges the rows of `groups` in `formats`.
fn check(groups: &[&str], formats: &[OutputFormat], judge: Judge) -> usize {
    let record = evidence();
    let mut failures = Vec::new();
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
            let found = codes(&report);
            if row["expectation"] != "match" {
                assert_eq!(found, ["image.decode-failed"], "{id} {format:?}: {report}");
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
            let (width, height) = (reference.width(), reference.height());
            let actual = match format {
                OutputFormat::Svg => {
                    if judge == Judge::Views {
                        use base64::Engine as _;
                        let wire =
                            base64::engine::general_purpose::STANDARD.encode(base64(&row["jpeg"]));
                        assert!(
                            std::str::from_utf8(&bytes)
                                .unwrap()
                                .contains(&format!("data:image/jpeg;base64,{wire}\"")),
                            "{id}: exact source octets"
                        );
                    }
                    continue;
                }
                OutputFormat::Png => image(&bytes),
                OutputFormat::Pdf => CanonicalRgba8::new(
                    width,
                    height,
                    super::png_profile_contract::pdf_rgba(&bytes, width as usize, height as usize),
                )
                .unwrap(),
                _ => unreachable!(),
            };
            match judge {
                Judge::Alpha => assert!(
                    actual.rgba().chunks_exact(4).all(|p| p[3] == 255),
                    "{id} {format:?}: JPEG is opaque"
                ),
                Judge::Views => {
                    let result = compare(
                        &reference,
                        &actual,
                        VisualClass::GeometryImage,
                        &[],
                        ComparisonMode::Canonical,
                    )
                    .unwrap();
                    if !result.passed() {
                        failures.push(format!("{id} {format:?}: {:?}", result.whole));
                    }
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

const ALL: [&str; 8] = [
    "base",
    "malformed",
    "partial",
    "default-frame",
    "orientation",
    "color-profile",
    "hdr-gain-map",
    "controls",
];

/// JPEG malformed facet: trailing data, junk and fill bytes between
/// segments, unknown or oversized application segments, stray restart and
/// TEM markers, a missing DHT, a progressive file missing only its EOI and
/// corrupt entropy draw as the reference draws them; truncated scans, a
/// baseline file missing its EOI, a missing DQT and a restart interval
/// without its markers, which the reference paints only partially (the
/// declared divergence), and the files it rejects, recover exactly.
#[test]
pub(super) fn jpeg_containers_and_malformed_payloads_match_the_reference() {
    assert_eq!(
        check(&["base", "malformed", "partial"], &FORMATS, Judge::Views),
        32
    );
    super::jpeg_contract::malformed_markers_have_exact_recovery_reports();
    super::jpeg_contract::trailing_data_and_malformed_icc_sequences_draw_the_first_image_untagged();
    super::jpeg_gain_map_contract::corrected_mpf_index_ranges_have_exact_recovery();
    super::jpeg_gain_map_contract::invalid_container_framing_has_exact_recovery();
    super::jpeg_gain_map_contract::malformed_mpf_fields_and_duplicate_indexes_have_exact_recovery();
    super::jpeg_gain_map_contract::secondary_mpf_index_has_exact_recovery();
}

/// JPEG default-frame facet: the first image draws; appended images, EXIF
/// IFD1 and JFXX thumbnails, and gain-map containers other than a valid MPF
/// + hdrgm pair are data after it.
#[test]
pub(super) fn jpeg_default_image_is_the_first_as_in_the_reference() {
    assert_eq!(
        check(
            &["base", "default-frame", "hdr-gain-map"],
            &FORMATS,
            Judge::Views
        ),
        9
    );
    super::jpeg_gain_map_contract::valid_sdr_gain_maps_match_reference_base_projection();
}

/// JPEG orientation facet: every EXIF orientation 1-8 in both byte orders
/// from the first IFD0 value, values out of range, other tag types and
/// counts, IFD1-only and XMP-only orientation ignored, EXIF over XMP, the
/// header variant the reference accepts and a looping IFD, on YCbCr, gray and
/// CMYK images.
#[test]
pub(super) fn jpeg_orientation_matches_the_reference() {
    assert_eq!(check(&["base", "orientation"], &FORMATS, Judge::Views), 22);
    super::jpeg_exif_contract::qualified_exif_precedence_matches_reference();
    super::jpeg_exif_contract::corrected_duplicate_exif_precedence_matches_reference();
    super::jpeg_exif_contract::authored_invalid_tiff_blocks_and_malformed_headers_have_bounded_recovery();
    super::jpeg_contract::all_tiff_orientations_preserve_exact_samples_and_svg_source_octets();
    super::jpeg_contract::absent_and_invalid_tiff_orientation_leave_the_source_layout_intact();
}

/// JPEG color-profile facet: sRGB-equivalent profiles draw untransformed,
/// RGB profiles apply to RGB and gray images, a profile of the wrong model or
/// a malformed one is ignored, and JFIF precedes the Adobe transform and
/// component identifiers.
#[test]
pub(super) fn jpeg_color_profiles_apply_as_the_reference_applies_them() {
    assert_eq!(
        check(&["base", "color-profile"], &FORMATS, Judge::Views),
        11
    );
    super::jpeg_icc_contract::rgb_and_gamma22_gray_icc_profiles_match_both_reference_authorities();
    super::jpeg_icc_contract::malformed_profiles_and_sequences_draw_untagged_as_the_reference();
    super::jpeg_icc_contract::corrected_gray_precision_and_model_mismatches_match_reference();
    super::jpeg_icc_contract::multipart_profiles_preserve_semantics_and_discriminate_ignored_icc();
    super::jpeg_color_contract::qualified_gray_rgb_and_ycbcr_cells_pass_both_reference_authorities(
    );
    super::jpeg_color_contract::cmyk_component_probe_isolates_reference_truncation_from_decoder_rounding();
    super::jpeg_color_contract::ycck_unconverted_components_with_integer_color_conversion_match_reference();
}

/// JPEG HDR gain-map facet: a valid SDR-base MPF + hdrgm container draws its
/// base as the reference does; invalid indexes and framing keep their approved
/// strict recovery (a declared divergence: the reference draws the base);
/// other gain-map containers draw the primary image. HDR-base projection is
/// the known #190 divergence, pinned to its exact current output.
#[test]
pub(super) fn jpeg_gain_maps_match_the_reference_with_the_190_divergence_pinned() {
    assert_eq!(check(&["base", "hdr-gain-map"], &FORMATS, Judge::Views), 5);
    super::jpeg_gain_map_contract::valid_sdr_gain_maps_match_reference_base_projection();
    super::jpeg_gain_map_contract::hdr_gain_maps_pin_known_color_defect_190();
    super::jpeg_gain_map_contract::corrected_mpf_index_ranges_have_exact_recovery();
    super::jpeg_gain_map_contract::invalid_container_framing_has_exact_recovery();
}

/// JPEG passthrough facet: direct SVG embeds the exact source octets of every
/// row the reference draws, and recovers exactly for the rest.
#[test]
pub(super) fn jpeg_svg_embeds_every_admitted_payload_exactly() {
    assert_eq!(check(&ALL, &[OutputFormat::Svg], Judge::Views), 65);
}

/// JPEG alpha facet: every admitted JPEG, whatever its model, markers or
/// orientation, draws fully opaque in PNG and PDF.
#[test]
pub(super) fn jpeg_draws_opaque_in_every_raster_view() {
    assert_eq!(
        check(&ALL, &[OutputFormat::Png, OutputFormat::Pdf], Judge::Alpha),
        65
    );
}
