//! WebP container, metadata, color-profile, alpha and animation paths against
//! the pinned reference (#85 WebP facets). The record
//! (`tests/oracle/webp-container/reference.json`, collected by
//! `tools/reference-oracle/run-webp-container.sh`) edits only the containers of
//! frozen external libwebp vectors. Each row is judged in every format: a row
//! the reference rejects recovers exactly as a missing asset; an admitted row
//! keeps its exact report and source octets in SVG, matches the reference
//! canonically in PNG (and in PDF when opaque, otherwise by exact alpha), and
//! equals its base exactly wherever the reference does.
//!
//! The vectors are 9x7, so the canonical small-artifact rule requires exact
//! views. Color-managed rows differ from Skia's arithmetic by a few units; they
//! are held at approved exact-ceiling exceptions (spec section 14.4), and an
//! exception whose view passes is rejected as dead.
use excalidraw_render::OutputFormat;
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, RegionReport, SsimOutcome, VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::webp_sample_contract::{render, report, svg};
use crate::common::lower_hex;

const RECORD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/webp-container/reference.json"
));
const EXCEPTIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/oracle/webp-container/icc-exceptions.json"
));
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

/// Diverging rows, each pinned to its exact current PNG so any movement is
/// seen. See docs/research/webp-umbrella-facets.md.
const KNOWN_DIVERGENCES: [(&str, &str); 1] = [(
    // A still lossless image with alpha, its VP8X alpha flag clear, and a
    // non-sRGB ICC profile: Skia's float transform of the misread
    // premultiplied buffer is not replicated (#197).
    "alpha-lossless-partial-icc-v2-linear-without-alpha-flag",
    "9efc4fbaa661cdd22568d2a6aad16281ffca17d43623304f6666919ff01b4ac8",
)];

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
        "fd9b0a602821ee5b111be5cd7aa8fa7a9c572e6469cfadaace9e0c591858f568"
    );
    let record: Value = serde_json::from_slice(RECORD).unwrap();
    assert_eq!(
        record["schema"],
        "excalidraw-render/webp-container-reference/v1"
    );
    for source in record["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], hash(&bytes), "{}", source["path"]);
    }
    let rows = record["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 195);
    let ids: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), rows.len());
    for row in rows.iter().filter(|r| r["equals_base"] == true) {
        let base = rows.iter().find(|r| r["id"] == row["base_id"]).unwrap();
        assert_eq!(
            row["reference"], base["reference"],
            "{}: the reference does not draw it like its base",
            row["id"]
        );
    }
    record
}

/// Checked-in ICC exceptions, reviewed alongside their regression tests.
fn approved_exceptions() -> Value {
    serde_json::from_slice(EXCEPTIONS).unwrap()
}

fn scene(webp: &Value) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":9,"height":7,
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/webp","created":1,
            "dataURL":format!("data:image/webp;base64,{}", webp.as_str().unwrap())}}})
}

/// Rows built on an animated base, and the #191 controls, are animations.
fn animated(row: &Value) -> bool {
    row["base_id"]
        .as_str()
        .unwrap_or(row["id"].as_str().unwrap())
        .starts_with("base-anim")
        || row["group"] == "controls"
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

/// Judges the rows of `groups` in `formats` against the record.
fn check(groups: &[&str], formats: &[OutputFormat]) -> usize {
    let record = evidence();
    let exceptions = approved_exceptions();
    let rows = record["cases"].as_array().unwrap();
    let mut used = std::collections::BTreeSet::new();
    let mut failures = Vec::new();
    let mut checked = 0;
    for row in rows
        .iter()
        .filter(|r| groups.contains(&r["group"].as_str().unwrap()))
    {
        let id = row["id"].as_str().unwrap();
        let source = scene(&row["webp"]);
        let admitted = row["reference"]["admitted"] == true;
        for &format in formats {
            let (bytes, actual) = render(&source, format);
            if !admitted {
                assert_eq!(actual, report(format, true, false), "{id} {format:?}");
                let mut missing = source.clone();
                missing["files"] = json!({});
                assert_eq!(
                    bytes,
                    render(&missing, format).0,
                    "{id} {format:?} exact recovery"
                );
                continue;
            }
            assert_eq!(
                actual,
                report(format, false, animated(row)),
                "{id} {format:?}"
            );
            let reference = image(&base64(&row["reference"]["png"]));
            let divergence = KNOWN_DIVERGENCES.iter().find(|(k, _)| *k == id);
            match format {
                OutputFormat::Svg => svg(
                    std::str::from_utf8(&bytes).unwrap(),
                    Some(&base64(&row["webp"])),
                ),
                OutputFormat::Png => match divergence {
                    Some((_, pinned)) => {
                        assert_eq!(hash(&bytes), *pinned, "{id}: known divergence moved")
                    }
                    None => judge(
                        id,
                        "png",
                        &reference,
                        &image(&bytes),
                        &exceptions,
                        &mut used,
                        &mut failures,
                    ),
                },
                OutputFormat::Pdf if divergence.is_none() => {
                    let rgba = super::png_profile_contract::pdf_rgba(&bytes, 9, 7);
                    let alpha =
                        |rgba: &[u8]| rgba.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>();
                    assert_eq!(alpha(&rgba), alpha(reference.rgba()), "{id} PDF alpha");
                    if reference.rgba().chunks_exact(4).all(|p| p[3] == 255) {
                        let actual = CanonicalRgba8::new(9, 7, rgba).unwrap();
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
                OutputFormat::Pdf => {}
                _ => unreachable!(),
            }
            if row["equals_base"] == true && format != OutputFormat::Svg {
                let base = rows.iter().find(|r| r["id"] == row["base_id"]).unwrap();
                assert_eq!(
                    bytes,
                    render(&scene(&base["webp"]), format).0,
                    "{id} {format:?} draws like its base"
                );
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
        let judged = groups.contains(&row["group"].as_str().unwrap())
            && formats.iter().any(|f| f.as_str() == format);
        if judged && !used.contains(&(e["row"].as_str().unwrap().to_owned(), format.to_owned())) {
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

/// WebP orientation facet: EXIF orientation 0-9, prefixed, flag-clear and
/// garbage EXIF over lossless, lossy, alpha and animated images are ignored,
/// exactly as the reference ignores them.
#[test]
pub(super) fn webp_orientation_metadata_is_ignored_as_the_reference_ignores_it() {
    assert_eq!(check(&["base", "orientation"], &FORMATS), 59);
}

/// WebP color-profile facet: v2/v4 matrix, LUT and Lab-LUT RGB profiles apply
/// to a still image with the ICC flag set; gray, CMYK, Lab-model and
/// unparseable profiles, a clear flag, a missing chunk and animations are
/// ignored; an sRGB-equivalent profile snaps to sRGB; the first of duplicate
/// profiles and a trailing profile follow the demuxer. The untagged VP8/VP8L
/// color itself is the sample matrix's.
#[test]
pub(super) fn webp_color_profiles_apply_as_the_reference_applies_them() {
    assert_eq!(check(&["base", "color-profile"], &FORMATS), 57);
    super::webp_sample_contract::all_sample_pngs_match_reference();
}

/// WebP malformed facet: container faults the reference rejects recover
/// exactly; faults its demuxer tolerates (trailing bytes, reserved bytes and
/// frame bits, missing or unknown chunks, oversized metadata) render like
/// their base. Includes the reference-collected #191 controls.
#[test]
pub(super) fn webp_containers_and_malformed_payloads_match_the_reference() {
    assert_eq!(check(&["base", "container", "controls"], &FORMATS), 67);
    super::webp_sample_contract::malformed_controls_reject_exactly_as_the_reference_rejects_them();
    super::webp_sample_contract::reserved_and_intrinsic_controls_are_accepted_as_the_reference_accepts_them();
}

/// WebP passthrough facet: direct SVG embeds every admitted payload's exact
/// octets, whatever its metadata, trailing bytes or unknown chunks.
#[test]
pub(super) fn webp_svg_embeds_every_admitted_payload_exactly() {
    let groups = [
        "base",
        "orientation",
        "color-profile",
        "container",
        "animation",
        "alpha",
        "controls",
    ];
    assert_eq!(check(&groups, &[OutputFormat::Svg]), 195);
}

/// WebP alpha facet: the VP8X alpha flag against the bitstream's alpha, PDF's
/// exact alpha, and the sample matrix's alpha, soft mask and transparent RGB
/// zero across opaque, binary and partial alpha.
#[test]
pub(super) fn webp_alpha_matches_the_reference() {
    assert_eq!(check(&["base", "alpha"], &FORMATS), 12);
    super::webp_sample_contract::qualified_13_cells_first_display_contract();
    super::webp_sample_contract::corrected_composition_cells_match_reference();
    super::webp_sample_contract::partial_alpha_cells_match_reference_under_approved_pdf_exceptions(
    );
    super::webp_sample_contract::nested_webp_png_preserves_draw_time_alpha_quantization();
}

/// WebP default-frame and animation facets: every frame validates, the first
/// frame is drawn on a transparent canvas whatever the frame count, reserved
/// frame bits, alpha flag, EXIF or duplicate ANIM, PNG/PDF report one
/// informational flattening and SVG keeps the animation.
#[test]
pub(super) fn webp_default_frame_and_animation_match_the_reference() {
    assert_eq!(check(&["base", "animation"], &FORMATS), 28);
    super::webp_sample_contract::qualified_13_cells_first_display_contract();
    super::webp_sample_contract::corrected_composition_cells_match_reference();
}
