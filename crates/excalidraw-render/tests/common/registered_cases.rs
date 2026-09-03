//! Shared plumbing for the registry-driven parity harnesses: how a registered
//! export variant's options become a render request, and how a diagnosed but
//! unfixed defect is pinned.

// Each harness binary includes this module by path and uses the part it needs.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use excalidraw_render::{ColorMode, OutputFormat, RenderRequest, Sizing};
use excalidraw_render_compare::{ComparisonReport, SsimOutcome, VisualClass};
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

/// A diagnosed renderer defect whose fix is pending a decision, tracked by an
/// issue. It is not an approved tolerance exception and never enters a case
/// manifest: a defect has no non-defect cause to record. It lives in the harness
/// that observes it, with its measurement pinned exactly, so the divergence
/// moving in either direction fails the harness and the entry can neither
/// outlive the defect nor quietly absorb a second one.
pub struct KnownDefect {
    pub case: &'static str,
    pub variant: &'static str,
    pub issue: &'static str,
    pub changed_pixels: u64,
    pub ssim_bits: u64,
}

impl KnownDefect {
    /// The entry pinning one variant, if any.
    pub fn lookup<'a>(defects: &'a [Self], case: &str, variant: &str) -> Option<&'a Self> {
        defects
            .iter()
            .find(|defect| defect.case == case && defect.variant == variant)
    }

    /// Every entry must name a variant the registry still declares; an entry
    /// that outlives its variant would otherwise pin nothing and never fail.
    pub fn assert_all_observed(defects: &[Self], observed: &BTreeSet<(String, String)>) {
        let orphans: Vec<_> = defects
            .iter()
            .filter(|defect| {
                !observed.contains(&(defect.case.to_owned(), defect.variant.to_owned()))
            })
            .map(|defect| format!("{} [{}]", defect.case, defect.variant))
            .collect();
        assert!(
            orphans.is_empty(),
            "KNOWN_DEFECTS names variants the registry no longer declares: {orphans:?}"
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
/// the form a tolerance exception or a pinned defect records it, so a failure
/// prints values that can be transcribed verbatim.
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
