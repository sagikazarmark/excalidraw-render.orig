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
    variant_request, visual_class, PinnedDivergence, RegisteredVariant,
};
use common::reports::assert_report_matches;
use common::repository_root;

/// Diagnosed defects whose fix is pending a maintainer decision. See
/// `PinnedDivergence` for why these are pinned rather than excepted.
///
/// The evidence round A cases (#153, #137 item 2) are registered with a null
/// approval and every variant is held here at its exact direct-PNG
/// measurement until the issue named on each entry is decided, so a fix, a
/// recorded exception, or a regression moves a pin instead of passing quietly.
const KNOWN_DEFECTS: &[PinnedDivergence] = &[
    // #160: the rounded diamond's corner radii come from width/2 and height/2
    // rather than the reference's diamond vertex offsets, so its outline is a
    // quarter pixel off (the path dimension of `parity_registered_case_svg`
    // shows the exact difference). The rest of the measurement is the frame
    // of the legacy `20px Virgil` text's glyph coverage (#104 class) and the
    // roughness-0 stroke edges.
    PinnedDivergence {
        case: "focused.legacy-stroke-sharpness.v1",
        variant: "default",
        issue: "#160",
        changed_pixels: 100,
        ssim_bits: 0x3fef_eb47_cf5f_f759,
    },
    // #162: the fractional-size images' edge rows and columns are area-
    // antialiased where the reference canvas snaps axis-aligned images to
    // pixel-centre coverage, plus the 32->96 resampling footprint at the
    // payload's colour boundaries; zero Pixelmatch-changed pixels in either
    // variant. The asset-bearing scene schema admits no tolerance exception.
    PinnedDivergence {
        case: "focused.rounded-image.v1",
        variant: "default",
        issue: "#162",
        changed_pixels: 0,
        ssim_bits: 0x3fef_eb08_a50a_ed8c,
    },
    PinnedDivergence {
        case: "focused.rounded-image.v1",
        variant: "scale-2",
        issue: "#162",
        changed_pixels: 0,
        ssim_bits: 0x3fef_f417_75e1_62ed,
    },
    // #162 as well: the same resampling footprint on the two cropped images;
    // the rounded clip itself agrees with the official canvas. The reference
    // SVG's misplaced clip for this element is #163, observed by the SVG
    // harness, not here.
    PinnedDivergence {
        case: "focused.rounded-image-crop.v1",
        variant: "default",
        issue: "#162",
        changed_pixels: 0,
        ssim_bits: 0x3fef_f2ed_dc0c_3648,
    },
    // #153: the two frame names' sub-pixel glyph coverage (the #104 class, the
    // largest component in both variants) with the raster image's resampling
    // footprint and the rounded frame corners' edge antialiasing. Not a
    // renderer defect; pinned because the asset-bearing scene schema admits no
    // tolerance exception, so whether the schema gains one or the case is
    // approved another way is the maintainer's decision on #153.
    PinnedDivergence {
        case: "focused.dark-mode-frames-and-images.v1",
        variant: "default",
        issue: "#153",
        changed_pixels: 52,
        ssim_bits: 0x3fef_f289_2ae7_60db,
    },
    PinnedDivergence {
        case: "focused.dark-mode-frames-and-images.v1",
        variant: "dark-mode",
        issue: "#153",
        changed_pixels: 60,
        ssim_bits: 0x3fef_f1b7_27cd_b3d5,
    },
    // #137: fill and stroke rasterization, the class #104 excepted for
    // `focused.ellipse-diamond-fills.v1`. All 64 SVG paths match the official
    // export byte for byte and the document rasterizes identically under one
    // rasterizer (0 px, SSIM 1.0), so the whole difference is the two
    // rasterizers' antialiasing of the fill lines, largest on the strokeWidth-1
    // rows whose fillWeight is half a pixel. Not a renderer defect; pinned
    // until the maintainer records the exception on #137.
    PinnedDivergence {
        case: "focused.rough-fillers-dashed-zigzag-line.v1",
        variant: "default",
        issue: "#137",
        changed_pixels: 487,
        ssim_bits: 0x3fef_910b_4970_cfd8,
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
