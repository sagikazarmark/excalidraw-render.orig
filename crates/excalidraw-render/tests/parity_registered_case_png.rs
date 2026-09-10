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
//! SSIM budgets. SVG parity is `parity_registered_case_svg`. Each variant is
//! rendered and judged once, the variants spread over the available cores.
//!
//! A variant may carry an approved tolerance exception in its case manifest
//! (`exceptions`, scoped to `format: png` and `region: whole`). Such a variant is
//! held to the exception's exact recorded ceilings instead of the canonical
//! budget, and must still fail the canonical budget: an exception is a recorded
//! judgment about an observed difference, so once the difference is gone the
//! record is stale and the test says so rather than letting it linger.
//!
//! A variant whose divergence is a diagnosed defect awaiting a decision gets no
//! exception; it is listed in `KNOWN_DEFECTS` with its measurement pinned, so
//! the entry fails the moment the divergence moves.

mod common;

use std::collections::BTreeSet;
use std::fs;

use excalidraw_render::{ColorMode, OutputFormat, Renderer, Sizing};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonLimits, ComparisonMode};
use serde_json::Value;

use common::raster::decode_png;
use common::registered_cases::{
    case_count, divergence, exception_limits, judge_each, registered_scene_variants,
    variant_request, visual_class, PinnedDivergence, PinnedExtentDefect, RegisteredVariant,
};
use common::reports::assert_report_matches;
use common::repository_root;

/// Diagnosed defects whose fix is pending a maintainer decision. See
/// `PinnedDivergence` for why these are pinned rather than excepted.
const KNOWN_DEFECTS: &[PinnedDivergence] = &[
    // #170: a freedraw loop's Rough.js background fill starts its seeded
    // stream without the discarded curve outline's draws, and the scene
    // surface substitutes roughness 1 and seed 1 for the element's own, so
    // every fill path differs from the official export's.
    PinnedDivergence {
        case: "focused.freedraw-loop-fills.v1",
        variant: "default",
        issue: "#170",
        changed_pixels: 632,
        ssim_bits: 0x3fed_00ce_bd65_22d1,
    },
];

/// Diagnosed scene-extent defects whose fix is pending a maintainer decision:
/// variants whose raster is not the official evidence's size, held at their
/// rendered dimensions. See `PinnedExtentDefect`.
const KNOWN_EXTENT_DEFECTS: &[PinnedExtentDefect] = &[
    // #172: the scene bounds of a rotated ellipse, diamond and line rotate
    // the element box's corners where the reference takes the ellipse's exact
    // extent, the diamond's vertices and the rotated curve points, so the
    // raster is 27 px wider and 4 px taller than the official 409x146.
    PinnedExtentDefect {
        case: "focused.rotated-shape-bounds.v1",
        variant: "default",
        issue: "#172",
        rendered: (436, 150),
    },
];

#[test]
fn every_registered_scene_case_matches_its_official_direct_png() {
    let variants = registered_scene_variants(&repository_root());
    let seen: BTreeSet<_> = variants
        .iter()
        .map(|variant| (variant.case.clone(), variant.variant.clone()))
        .collect();

    let failures: Vec<String> = judge_each(&variants, direct_png_divergence)
        .into_iter()
        .flatten()
        .collect();

    PinnedDivergence::assert_all_observed(KNOWN_DEFECTS, &seen);
    PinnedExtentDefect::assert_all_observed(KNOWN_EXTENT_DEFECTS, &seen);
    assert!(
        failures.is_empty(),
        "{} of {} registered scene cases diverge from their official direct PNG:\n  {}",
        failures.len(),
        case_count(&variants),
        failures.join("\n  ")
    );
}

/// Renders one variant's direct PNG and judges it against the official
/// evidence: `None` when it is where its record says it should be.
fn direct_png_divergence(variant: &RegisteredVariant) -> Option<String> {
    let context = variant.context();
    let request = variant_request(OutputFormat::Png, variant.options(), &context);
    let outcome = Renderer::new()
        .render(&variant.scene, &request)
        .unwrap_or_else(|error| panic!("{context} renders as PNG: {error:?}"));

    let png = variant.format("png");
    let expected_report = variant.dir.join(
        png["expected_report"]["path"]
            .as_str()
            .expect("expected report path"),
    );
    // A report mismatch is never suppressed: it means the renderer's
    // diagnostics disagree with what the case declares, which is a contract
    // break rather than a pixel difference. The checked report is canonical
    // JSON and the renderer's is spec-ordered, so they compare as values.
    assert_report_matches(
        outcome.report(),
        &expected_report,
        &format!("{context}: PNG"),
    );

    let raw = &png["raw_raster"];
    let width = raw["width"].as_u64().expect("raster width") as u32;
    let height = raw["height"].as_u64().expect("raster height") as u32;
    let reference = CanonicalRgba8::new(
        width,
        height,
        fs::read(variant.dir.join(raw["path"].as_str().expect("raster path")))
            .expect("official raster present"),
    )
    .expect("official evidence is canonical RGBA8");
    let candidate = decode_png(outcome.artifact().bytes()).into_canonical();

    let exception = tolerance_exception(&variant.manifest, &variant.variant);
    let defect = PinnedDivergence::lookup(KNOWN_DEFECTS, &variant.case, &variant.variant);
    assert!(
        exception.is_none() || defect.is_none(),
        "{context} is both excepted and a known defect; a defect has no non-defect cause to \
         except"
    );
    let extent = PinnedExtentDefect::lookup(KNOWN_EXTENT_DEFECTS, &variant.case, &variant.variant);
    let rendered = (candidate.width(), candidate.height());
    let divergence = if let Some(extent) = extent {
        assert!(
            exception.is_none() && defect.is_none(),
            "{context} is pinned as an extent defect and excepted or pinned again; a raster of \
             another size has no comparison to except or pin"
        );
        extent.mismatch(rendered, (width, height))
    } else if rendered != (width, height) {
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
        match defect {
            Some(defect) => defect.mismatch(&canonical),
            None => divergence(&canonical, exception),
        }
    };
    divergence.map(|detail| format!("{context}: {detail}"))
}

/// The approved tolerance exception governing one variant's whole direct PNG, if
/// the case manifest records one.
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
        .map(exception_limits)
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
    assert!(request.include_background());
    assert_eq!(request.padding(), 10.0);
    assert!(!request.metadata_options().embedded_scene());
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
fn variant_request_honours_the_background_padding_and_scene_embedding_options() {
    // Each reference export option maps onto the request control it stands
    // for, so a variant's evidence and its render are produced under the same
    // settings: `exportBackground` is the scene background, `exportPadding`
    // the logical padding, and `exportEmbedScene` the embedded canonicalized
    // original scene document with nothing else beside it.
    let mut transparent = default_options();
    transparent["exportBackground"] = serde_json::json!(false);
    assert!(!variant_request(OutputFormat::Png, &transparent, "test").include_background());

    let mut tight = default_options();
    tight["exportPadding"] = serde_json::json!(0);
    assert_eq!(
        variant_request(OutputFormat::Svg, &tight, "test").padding(),
        0.0
    );

    let mut embedded = default_options();
    embedded["exportEmbedScene"] = serde_json::json!(true);
    let metadata = variant_request(OutputFormat::Svg, &embedded, "test")
        .metadata_options()
        .clone();
    assert!(metadata.embedded_scene());
    assert!(!metadata.provenance());
    assert!(!metadata.diagnostic_summary());
}

#[test]
fn an_extent_defect_pin_holds_the_rendered_size_and_requires_the_mismatch() {
    let pin = PinnedExtentDefect {
        case: "focused.example.v1",
        variant: "default",
        issue: "#172",
        rendered: (436, 150),
    };
    assert_eq!(pin.mismatch((436, 150), (409, 146)), None);
    let stale = pin
        .mismatch((409, 146), (409, 146))
        .expect("agreement with the evidence is a stale pin");
    assert!(stale.contains("stale") && stale.contains("#172"), "{stale}");
    let moved = pin
        .mismatch((436, 151), (409, 146))
        .expect("another size is a moved pin");
    assert!(
        moved.contains("extent moved") && moved.contains("436x151"),
        "{moved}"
    );
}

#[test]
#[should_panic(expected = "varies viewBackgroundColor")]
fn variant_request_refuses_an_option_it_does_not_build() {
    let mut tinted = default_options();
    tinted["viewBackgroundColor"] = serde_json::json!("#f8f9fa");
    variant_request(OutputFormat::Png, &tinted, "test");
}

#[test]
#[should_panic(expected = "exportPadding is not a number")]
fn variant_request_refuses_a_non_numeric_padding() {
    let mut odd = default_options();
    odd["exportPadding"] = serde_json::json!("none");
    variant_request(OutputFormat::Png, &odd, "test");
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
