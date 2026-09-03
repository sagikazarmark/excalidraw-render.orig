//! Direct-PNG parity for every registered scene case.
//!
//! The corpus stores official browser evidence for each case, but only the three
//! pre-existing cases had a renderer comparison against it. This drives the
//! comparison from `tests/conformance/cases/registry.json`, so a case is covered
//! the moment it is registered rather than when someone remembers to hand-write a
//! test for it.
//!
//! Direct PNG needs no canonical viewer: the renderer's own raster is compared
//! against the browser's `official-png.rgba` under the canonical Pixelmatch and
//! SSIM budgets. SVG raster parity needs a viewer and is not covered here.
//!
//! A variant may carry an approved tolerance exception in its case manifest
//! (`exceptions`, scoped to `format: png` and `region: whole`). Such a variant is
//! held to the exception's exact recorded ceilings instead of the canonical
//! budget, and must still fail the canonical budget: an exception is a recorded
//! judgment about an observed difference, so once the difference is gone the
//! record is stale and the test says so rather than letting it linger.

// Included by path rather than through `common/mod.rs` so this binary does not
// also compile the PDF harness helpers, and the PDF binaries do not compile this.
#[path = "common/registered_cases.rs"]
mod registered_cases;

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use excalidraw_render::{ColorMode, OutputFormat, Renderer, SceneDocument, Sizing};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonLimits, ComparisonMode, ComparisonReport, RegionReport,
    SsimOutcome, VisualClass,
};
use png::{BitDepth, ColorType, Decoder, Transformations};
use serde_json::Value;

use registered_cases::variant_request;

#[test]
fn every_registered_scene_case_matches_its_official_direct_png() {
    let root = repository_root();
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("tests/conformance/cases/registry.json"))
            .expect("case registry is present"),
    )
    .expect("case registry is valid JSON");

    let mut total = 0usize;
    let mut failures = Vec::new();

    for case in registry["cases"]
        .as_array()
        .expect("registry cases are an array")
        .iter()
        .filter(|case| case["kind"] == "scene")
    {
        let id = case["id"].as_str().expect("case id is a string");
        total += 1;
        let dir = root.join(case["path"].as_str().expect("case path is a string"));
        let manifest: Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("manifest present"))
                .expect("manifest is valid JSON");

        let variants = manifest["variants"]
            .as_array()
            .expect("manifest variants are an array");
        for variant in variants {
            let scene = SceneDocument::parse(
                &fs::read(dir.join("scene.excalidraw")).expect("scene document present"),
            )
            .unwrap_or_else(|error| panic!("{id} scene is admitted: {error:?}"));
            let variant_id = variant["id"].as_str().expect("variant id is a string");
            let request = variant_request(
                OutputFormat::Png,
                &variant["options"],
                &format!("{id} variant {variant_id}"),
            );
            let outcome = Renderer::new()
                .render(&scene, &request)
                .unwrap_or_else(|error| panic!("{id} renders as PNG: {error:?}"));

            let png = &variant["formats"]["png"];
            let expected_report = dir.join(
                png["expected_report"]["path"]
                    .as_str()
                    .expect("expected report path"),
            );
            // A report mismatch is never suppressed: it means the renderer's
            // diagnostics disagree with what the case declares, which is a contract
            // break rather than a pixel difference.
            assert_eq!(
                serde_json::to_vec(outcome.report()).expect("report serializes"),
                fs::read(&expected_report).expect("expected report present"),
                "{id}: PNG report differs from its declared expectation"
            );

            let raw = &png["raw_raster"];
            let width = raw["width"].as_u64().expect("raster width") as u32;
            let height = raw["height"].as_u64().expect("raster height") as u32;
            let reference = CanonicalRgba8::new(
                width,
                height,
                fs::read(dir.join(raw["path"].as_str().expect("raster path")))
                    .expect("official raster present"),
            )
            .expect("official evidence is canonical RGBA8");
            let candidate = decode_png(outcome.artifact().bytes());

            let exception = tolerance_exception(&manifest, variant_id);
            let divergence = if (candidate.width(), candidate.height()) != (width, height) {
                Some(format!(
                    "rendered {}x{}, official evidence is {width}x{height}",
                    candidate.width(),
                    candidate.height()
                ))
            } else {
                let class = visual_class(png["visual_class"].as_str().expect("visual class"));
                let canonical = compare(
                    &reference,
                    &candidate,
                    class,
                    &[],
                    ComparisonMode::Canonical,
                )
                .expect("dimensions agree");
                // The measurement is the same under either budget; only the
                // pass judgment differs, so the exception is judged on the
                // canonical report rather than by comparing the rasters again.
                match exception {
                    None => (!canonical.passed())
                        .then(|| format!("outside the canonical budget: {}", observed(&canonical))),
                    Some(_) if canonical.passed() => Some(format!(
                        "passes the canonical budget, so its tolerance exception is stale and \
                         must be withdrawn: {}",
                        observed(&canonical)
                    )),
                    Some(limits) if !within(limits, &canonical.whole) => Some(format!(
                        "exceeds its approved tolerance exception ceilings: {}",
                        observed(&canonical)
                    )),
                    Some(_) => None,
                }
            };

            if let Some(detail) = divergence {
                failures.push(format!("{id} [{variant_id}]: {detail}"));
            }
        }
    }

    assert!(total > 0, "the registry declared no scene cases");
    assert!(
        failures.is_empty(),
        "{} of {} registered scene cases diverge from their official direct PNG:\n  {}",
        failures.len(),
        total,
        failures.join("\n  ")
    );
}

/// The approved tolerance exception governing one variant's whole direct PNG, if
/// the case manifest records one. Ceilings are the exact observed values the
/// approval bound, so they are consumed verbatim rather than rounded.
fn tolerance_exception(manifest: &Value, variant_id: &str) -> Option<ComparisonLimits> {
    manifest["exceptions"]
        .as_array()
        .expect("manifest exceptions are an array")
        .iter()
        .find(|exception| {
            exception["variant"] == variant_id
                && exception["format"] == "png"
                && exception["region"] == "whole"
        })
        .map(|exception| {
            let max_changed_pixels = exception["max_changed_pixels"]
                .as_u64()
                .expect("exception max_changed_pixels is an integer");
            let minimum_ssim_bits = u64::from_str_radix(
                exception["minimum_ssim_bits"]
                    .as_str()
                    .expect("exception minimum_ssim_bits is a string"),
                16,
            )
            .expect("exception minimum_ssim_bits is binary64 hex");
            ComparisonLimits::tolerance_exception(
                max_changed_pixels,
                f64::from_bits(minimum_ssim_bits),
            )
            .expect("exception SSIM floor is finite")
        })
}

/// Whether a whole-artifact measurement meets an exception's exact ceilings.
fn within(limits: ComparisonLimits, whole: &RegionReport) -> bool {
    limits.changed_pixels_pass(whole.changed_pixels, whole.area)
        && matches!(whole.ssim, SsimOutcome::Score(score) if limits.ssim_pass(score))
}

/// The observed whole-artifact measurement in the form a tolerance exception
/// records it, so a failure prints values that can be transcribed exactly.
fn observed(report: &ComparisonReport) -> String {
    match report.whole.ssim {
        SsimOutcome::Score(score) => format!(
            "changed_pixels={} ssim={score:?} (minimum_ssim_bits={:016x})",
            report.whole.changed_pixels,
            score.to_bits()
        ),
        ref other => format!(
            "changed_pixels={} ssim={other:?}",
            report.whole.changed_pixels
        ),
    }
}

fn visual_class(name: &str) -> VisualClass {
    match name {
        "exact" => VisualClass::Exact,
        "geometry/image" => VisualClass::GeometryImage,
        "typography" => VisualClass::Typography,
        "mixed/representative" => VisualClass::MixedRepresentative,
        other => panic!("unknown visual class {other:?}"),
    }
}

fn decode_png(bytes: &[u8]) -> CanonicalRgba8 {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
    assert_eq!(frame.color_type, ColorType::Rgba);
    assert_eq!(frame.bit_depth, BitDepth::Eight);
    rgba.truncate(frame.buffer_size());
    CanonicalRgba8::new(frame.width, frame.height, rgba).expect("decoded PNG is canonical RGBA8")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}

/// The default reference export, as every registered variant declares it
/// before varying anything.
fn default_options() -> Value {
    serde_json::json!({
        "exportBackground": true,
        "exportEmbedScene": false,
        "exportPadding": 10,
        "exportScale": 1,
        "exportWithDarkMode": false,
        "frameRendering": {"clip": true, "enabled": true, "name": true, "outline": true},
        "maxWidthOrHeight": null,
        "viewBackgroundColor": "#ffffff"
    })
}

#[test]
fn variant_request_builds_the_default_export_from_default_options() {
    let request = variant_request(OutputFormat::Png, &default_options(), "test");
    assert_eq!(request.format(), OutputFormat::Png);
    assert_eq!(request.sizing(), Sizing::Scale(1.0));
    assert_eq!(request.color_mode(), ColorMode::Light);
}

#[test]
fn variant_request_honours_dark_mode_and_each_sizing_mode() {
    let mut dark = default_options();
    dark["exportWithDarkMode"] = serde_json::json!(true);
    assert_eq!(
        variant_request(OutputFormat::Png, &dark, "test").color_mode(),
        ColorMode::Dark
    );

    let mut scaled = default_options();
    scaled["exportScale"] = serde_json::json!(2);
    assert_eq!(
        variant_request(OutputFormat::Svg, &scaled, "test").sizing(),
        Sizing::Scale(2.0)
    );

    let mut bounded = default_options();
    bounded["exportScale"] = Value::Null;
    bounded["maxWidthOrHeight"] = serde_json::json!(160);
    assert_eq!(
        variant_request(OutputFormat::Png, &bounded, "test").sizing(),
        Sizing::MaxDimension(160.0)
    );
}

#[test]
#[should_panic(expected = "varies exportPadding")]
fn variant_request_refuses_an_option_it_does_not_build() {
    let mut padded = default_options();
    padded["exportPadding"] = serde_json::json!(20);
    variant_request(OutputFormat::Png, &padded, "test");
}

#[test]
#[should_panic(expected = "exactly one sizing mode")]
fn variant_request_refuses_two_sizing_modes() {
    let mut both = default_options();
    both["maxWidthOrHeight"] = serde_json::json!(160);
    variant_request(OutputFormat::Png, &both, "test");
}

#[test]
#[should_panic(expected = "varies frameRendering")]
fn variant_request_refuses_a_frame_control_deviation() {
    let mut unclipped = default_options();
    unclipped["frameRendering"]["clip"] = serde_json::json!(false);
    variant_request(OutputFormat::Png, &unclipped, "test");
}

#[test]
#[should_panic(expected = "exportWithDarkMode")]
fn variant_request_refuses_a_non_boolean_dark_mode() {
    let mut odd = default_options();
    odd["exportWithDarkMode"] = serde_json::json!("yes");
    variant_request(OutputFormat::Png, &odd, "test");
}
