//! Shared plumbing for the parity harnesses: how a registered export variant's
//! options become a render request, how a recorded tolerance exception's
//! ceilings judge a comparison, and how a divergence a harness must hold in
//! place, a known defect or a compatibility exception, is pinned at its exact
//! measurement.

// Each harness binary includes this module by path and uses the part it needs.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use excalidraw_render::{ColorMode, OutputFormat, RenderRequest, Sizing};
use excalidraw_render_compare::{
    ComparisonLimits, ComparisonReport, RegionReport, SsimOutcome, VisualClass,
};
use serde_json::{json, Value};

/// Builds the render request a registered export variant describes.
///
/// Only the options a variant is allowed to vary are honoured: dark mode and the
/// two sizing modes. Any other deviation from the default export panics, so an
/// unrecognised option fails rather than silently rendering the default and
/// comparing it against evidence generated under different settings. `context`
/// names the case and variant in that failure.
pub fn variant_request(format: OutputFormat, options: &Value, context: &str) -> RenderRequest {
    for (key, default) in [
        ("exportBackground", json!(true)),
        ("exportEmbedScene", json!(false)),
        ("exportPadding", json!(10)),
        (
            "frameRendering",
            json!({"clip": true, "enabled": true, "name": true, "outline": true}),
        ),
        ("viewBackgroundColor", json!("#ffffff")),
    ] {
        assert_eq!(
            options[key], default,
            "{context} varies {key}, which the parity harnesses do not build"
        );
    }
    let mut builder = RenderRequest::builder(format);
    match options["exportWithDarkMode"].as_bool() {
        Some(true) => builder = builder.color_mode(ColorMode::Dark),
        Some(false) => {}
        None => panic!("{context} exportWithDarkMode is not a boolean"),
    }
    // The reference wrapper selects exactly one sizing mode; a variant that
    // selects neither or both is malformed rather than a default export.
    builder = match (
        options["exportScale"].as_f64(),
        options["maxWidthOrHeight"].as_f64(),
    ) {
        (Some(scale), None) => builder.sizing(Sizing::Scale(scale)),
        (None, Some(maximum)) => builder.sizing(Sizing::MaxDimension(maximum)),
        (scale, maximum) => panic!(
            "{context} selects exportScale={scale:?} and maxWidthOrHeight={maximum:?}; \
             exactly one sizing mode is expected"
        ),
    };
    builder
        .build()
        .unwrap_or_else(|error| panic!("{context} request is valid: {error:?}"))
}

/// One variant's exact whole-artifact measurement, held in place by the
/// harness that observes it and tracked by the issue that carries the decision.
/// A pin is not a tolerance: the harness fails when the divergence moves in
/// either direction, so an entry can neither outlive its cause nor quietly
/// absorb a second one, and every entry must name a variant the registry still
/// declares, so an orphan cannot pin nothing and never fail.
///
/// Two lists use it, and what they pin differs. A known defect is a diagnosed
/// renderer defect whose fix awaits a decision; it has no non-defect cause to
/// record, so it never enters a case manifest. A compatibility exception pin
/// is a specified departure from reference behavior that the case manifest
/// already excepts on the direct PNG, observed by a harness whose scope that
/// exception cannot cover; the pin stands for that approved judgment and must
/// correspond to a recorded exception for the same variant.
pub struct PinnedDivergence {
    pub case: &'static str,
    pub variant: &'static str,
    pub issue: &'static str,
    pub changed_pixels: u64,
    pub ssim_bits: u64,
}

impl PinnedDivergence {
    /// The entry pinning one variant, if any.
    pub fn lookup<'a>(pins: &'a [Self], case: &str, variant: &str) -> Option<&'a Self> {
        pins.iter()
            .find(|pin| pin.case == case && pin.variant == variant)
    }

    /// Every entry must name a variant the registry still declares.
    pub fn assert_all_observed(pins: &[Self], observed: &BTreeSet<(String, String)>) {
        let orphans: Vec<_> = pins
            .iter()
            .filter(|pin| !observed.contains(&(pin.case.to_owned(), pin.variant.to_owned())))
            .map(|pin| format!("{} [{}]", pin.case, pin.variant))
            .collect();
        assert!(
            orphans.is_empty(),
            "pinned divergences name variants the registry no longer declares: {orphans:?}"
        );
    }

    /// `None` when the report carries the pinned measurement; otherwise the
    /// failure to report, with the observed values ready to transcribe.
    pub fn mismatch(&self, report: &ComparisonReport) -> Option<String> {
        let moved = report.whole.changed_pixels != self.changed_pixels
            || ssim_bits(report) != Some(self.ssim_bits);
        moved.then(|| {
            format!(
                "measurement moved from the pinned changed_pixels={} ssim_bits={:016x} \
                 recorded for {}; revisit the entry: {}",
                self.changed_pixels,
                self.ssim_bits,
                self.issue,
                observed(report)
            )
        })
    }
}

/// The whole-artifact SSIM score as binary64 bits, if it is a score.
pub fn ssim_bits(report: &ComparisonReport) -> Option<u64> {
    match report.whole.ssim {
        SsimOutcome::Score(score) => Some(score.to_bits()),
        _ => None,
    }
}

/// The observed whole-artifact measurement with the SSIM score's exact bits, in
/// the form a tolerance exception or a pinned divergence records it, so a
/// failure prints values that can be transcribed verbatim.
pub fn observed(report: &ComparisonReport) -> String {
    match report.whole.ssim {
        SsimOutcome::Score(score) => format!(
            "changed_pixels={} ssim={score:?} (ssim_bits={:016x})",
            report.whole.changed_pixels,
            score.to_bits()
        ),
        ref other => format!(
            "changed_pixels={} ssim={other:?}",
            report.whole.changed_pixels
        ),
    }
}

/// The exact no-margin ceilings a tolerance exception record carries as
/// `max_changed_pixels` and binary64 `minimum_ssim_bits`. They are the observed
/// values the approval bound, so they are consumed verbatim rather than rounded.
pub fn exception_limits(record: &Value) -> ComparisonLimits {
    let max_changed_pixels = record["max_changed_pixels"]
        .as_u64()
        .expect("exception max_changed_pixels is an integer");
    let minimum_ssim_bits = u64::from_str_radix(
        record["minimum_ssim_bits"]
            .as_str()
            .expect("exception minimum_ssim_bits is a string"),
        16,
    )
    .expect("exception minimum_ssim_bits is binary64 hex");
    ComparisonLimits::tolerance_exception(max_changed_pixels, f64::from_bits(minimum_ssim_bits))
        .expect("exception SSIM floor is finite")
}

/// Whether a whole-artifact measurement meets an exception's exact ceilings.
pub fn within(limits: ComparisonLimits, whole: &RegionReport) -> bool {
    limits.changed_pixels_pass(whole.changed_pixels, whole.area)
        && matches!(whole.ssim, SsimOutcome::Score(score) if limits.ssim_pass(score))
}

/// Judges one whole-artifact comparison under the canonical budget, or under
/// the tolerance exception that governs it: `None` when the artifact is where
/// its record says it should be, otherwise the divergence to report with the
/// observed measurement ready to transcribe.
///
/// The measurement is the same under either budget; only the pass judgment
/// differs, so the exception is judged on the canonical report rather than by
/// comparing the rasters again. An excepted artifact must still fail the
/// canonical budget: an exception is a recorded judgment about an observed
/// difference, so once the difference is gone the record is stale and must be
/// withdrawn rather than left to linger.
pub fn divergence(
    canonical: &ComparisonReport,
    exception: Option<ComparisonLimits>,
) -> Option<String> {
    match exception {
        None => (!canonical.passed())
            .then(|| format!("outside the canonical budget: {}", observed(canonical))),
        Some(_) if canonical.passed() => Some(format!(
            "passes the canonical budget, so its tolerance exception is stale and must be \
             withdrawn: {}",
            observed(canonical)
        )),
        Some(limits) if !within(limits, &canonical.whole) => Some(format!(
            "exceeds its approved tolerance exception ceilings: {}",
            observed(canonical)
        )),
        Some(_) => None,
    }
}

/// The comparator's class for a manifest `visual_class` string.
pub fn visual_class(name: &str) -> VisualClass {
    match name {
        "exact" => VisualClass::Exact,
        "geometry/image" => VisualClass::GeometryImage,
        "typography" => VisualClass::Typography,
        "mixed/representative" => VisualClass::MixedRepresentative,
        other => panic!("unknown visual class {other:?}"),
    }
}

/// The workspace root, two levels above this crate.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}
