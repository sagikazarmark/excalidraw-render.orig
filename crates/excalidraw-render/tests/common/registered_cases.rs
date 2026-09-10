//! Shared plumbing for the parity harnesses: which variants the case registry
//! declares and how a harness judges each of them once, how a registered export
//! variant's options become a render request, how a recorded tolerance
//! exception's ceilings judge a comparison, and how a divergence a harness must
//! hold in place, a known defect, a compatibility exception or a reference
//! authority divergence, is pinned at its exact measurement.

use std::collections::BTreeSet;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use excalidraw_render::{ColorMode, OutputFormat, RenderRequest, Renderer, SceneDocument, Sizing};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonLimits, ComparisonMode, ComparisonReport, RegionReport,
    SsimOutcome, VisualClass,
};
use serde_json::{json, Value};

use super::raster::decode_png;

/// One export variant of a registered scene case, with everything a harness
/// needs to render and judge it: the identifiers, the case directory, the case
/// manifest, the variant's own record, and the admitted scene document, parsed
/// once per case and shared by its variants.
pub struct RegisteredVariant {
    pub case: String,
    pub variant: String,
    pub dir: PathBuf,
    pub manifest: Arc<Value>,
    pub record: Value,
    pub scene: Arc<SceneDocument>,
}

impl RegisteredVariant {
    /// `case [variant]`, the prefix of every message about this variant.
    pub fn context(&self) -> String {
        format!("{} [{}]", self.case, self.variant)
    }

    /// The export options the variant varies from the default export.
    pub fn options(&self) -> &Value {
        &self.record["options"]
    }

    /// The variant's declared evidence for one format (`svg`, `png`).
    pub fn format(&self, format: &str) -> &Value {
        &self.record["formats"][format]
    }
}

/// Every variant of every scene case `tests/conformance/cases/registry.json`
/// declares, in registry order, so a case is covered the moment it is
/// registered. Panics when the registry declares no scene case: a harness that
/// judged nothing must not pass.
pub fn registered_scene_variants(root: &Path) -> Vec<RegisteredVariant> {
    registered_variants_of_kinds(root, &["scene"])
}

/// Every variant of every registered case whose `kind` is one of `kinds`, in
/// registry order. The `scene` and `typography` kinds share the variant shape
/// (a scene document, a manifest of export variants and their evidence); the
/// `frame` kind is exported through the frame API and has no variants here.
/// Panics when no case matches: a harness that judged nothing must not pass.
pub fn registered_variants_of_kinds(root: &Path, kinds: &[&str]) -> Vec<RegisteredVariant> {
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("tests/conformance/cases/registry.json"))
            .expect("case registry is present"),
    )
    .expect("case registry is valid JSON");
    let mut variants = Vec::new();
    for case in registry["cases"]
        .as_array()
        .expect("registry cases are an array")
        .iter()
        .filter(|case| {
            case["kind"]
                .as_str()
                .is_some_and(|kind| kinds.contains(&kind))
        })
    {
        let id = case["id"].as_str().expect("case id is a string");
        let dir = root.join(case["path"].as_str().expect("case path is a string"));
        let manifest: Arc<Value> = Arc::new(
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("manifest present"))
                .expect("manifest is valid JSON"),
        );
        let scene = Arc::new(
            SceneDocument::parse(
                &fs::read(dir.join("scene.excalidraw")).expect("scene document present"),
            )
            .unwrap_or_else(|error| panic!("{id} scene is admitted: {error:?}")),
        );
        for record in manifest["variants"]
            .as_array()
            .expect("manifest variants are an array")
        {
            variants.push(RegisteredVariant {
                case: id.to_owned(),
                variant: record["id"]
                    .as_str()
                    .expect("variant id is a string")
                    .to_owned(),
                dir: dir.clone(),
                manifest: Arc::clone(&manifest),
                record: record.clone(),
                scene: Arc::clone(&scene),
            });
        }
    }
    assert!(
        !variants.is_empty(),
        "the registry declared no case of kind {kinds:?}"
    );
    variants
}

/// The number of distinct cases among the variants.
pub fn case_count(variants: &[RegisteredVariant]) -> usize {
    variants
        .iter()
        .map(|variant| variant.case.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

/// Judges every variant exactly once, spread over the available cores, and
/// returns the judgments in registry order, so a failure report reads the same
/// however the work was split. Rendering and comparison are pure functions of
/// the variant, so the split changes nothing but the wall time; a panic in one
/// judgment is re-raised on the calling thread with its original message.
pub fn judge_each<T: Send>(
    variants: &[RegisteredVariant],
    judge: impl Fn(&RegisteredVariant) -> T + Sync,
) -> Vec<T> {
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .clamp(1, variants.len().max(1));
    let next = AtomicUsize::new(0);
    let mut judgments: Vec<Option<T>> = (0..variants.len()).map(|_| None).collect();
    thread::scope(|scope| {
        let workers: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut judged = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(variant) = variants.get(index) else {
                            break judged;
                        };
                        judged.push((index, judge(variant)));
                    }
                })
            })
            .collect();
        for worker in workers {
            let judged = worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            for (index, judgment) in judged {
                judgments[index] = Some(judgment);
            }
        }
    });
    judgments
        .into_iter()
        .map(|judgment| judgment.expect("every variant was judged exactly once"))
        .collect()
}

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
/// Three lists use it, and what they pin differs. A known defect is a
/// diagnosed renderer defect whose fix awaits a decision; it has no non-defect
/// cause to record, so it never enters a case manifest. A compatibility
/// exception pin is a specified departure from reference behavior that the
/// case manifest already excepts on the direct PNG, observed by a harness
/// whose scope that exception cannot cover; the pin stands for that approved
/// judgment and must correspond to a recorded exception for the same variant.
/// A reference authority divergence ([`ReferenceAuthorityDivergence`]) is a
/// defect of the reference exporter's serialization that the harness's own
/// rasterizer can see, where the renderer follows the official canvas.
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

/// One variant whose official SVG misrenders under the harness's own
/// rasterizer while the renderer follows the official canvas, as the spec
/// records for frame clips and the rounded-image corner clip (spec 8.2 and
/// 8.4): a reference authority divergence, neither a renderer defect nor a
/// compatibility exception. The rasterized-document dimension of the SVG
/// harness holds the exact measurement, so the divergence can neither grow
/// nor silently disappear, and names the spec section that records the
/// divergence and the issue that measured it. The proof that the renderer and
/// not the reference SVG is right is the same variant's direct PNG, which must
/// pass the canonical budget against the official canvas raster; a pin whose
/// variant fails that comparison is not a reference authority divergence and
/// the harness says so.
pub struct ReferenceAuthorityDivergence {
    pub pin: PinnedDivergence,
    /// The spec section recording the divergence the pin holds.
    pub spec_section: &'static str,
}

impl ReferenceAuthorityDivergence {
    /// The entry pinning one variant, if any.
    pub fn lookup<'a>(pins: &'a [Self], case: &str, variant: &str) -> Option<&'a Self> {
        pins.iter()
            .find(|entry| entry.pin.case == case && entry.pin.variant == variant)
    }

    /// Every entry must name a variant the registry still declares.
    pub fn assert_all_observed(pins: &[Self], observed: &BTreeSet<(String, String)>) {
        let pinned: Vec<&PinnedDivergence> = pins.iter().map(|entry| &entry.pin).collect();
        let orphans: Vec<_> = pinned
            .iter()
            .filter(|pin| !observed.contains(&(pin.case.to_owned(), pin.variant.to_owned())))
            .map(|pin| format!("{} [{}]", pin.case, pin.variant))
            .collect();
        assert!(
            orphans.is_empty(),
            "reference authority divergences name variants the registry no longer declares: \
             {orphans:?}"
        );
    }

    /// `None` when the rasterized-document report carries the pinned
    /// measurement and the variant's direct PNG passes the canonical budget;
    /// otherwise the failure to report.
    pub fn mismatch(
        &self,
        report: &ComparisonReport,
        variant: &RegisteredVariant,
    ) -> Option<String> {
        let direct_png = direct_png_report(variant);
        if !direct_png.passed() {
            return Some(format!(
                "is pinned as a reference authority divergence ({}, {}) but its direct PNG does \
                 not pass the canonical budget against the official canvas raster, so the \
                 renderer is not following the canvas there: {}",
                self.spec_section,
                self.pin.issue,
                observed(&direct_png)
            ));
        }
        self.pin.mismatch(report).map(|detail| {
            format!(
                "reference authority divergence ({}) {detail}",
                self.spec_section
            )
        })
    }
}

/// The canonical comparison of one variant's direct PNG against the official
/// canvas raster: the measurement `parity_registered_case_png` judges, taken
/// here as the proof a reference authority divergence pin requires.
pub fn direct_png_report(variant: &RegisteredVariant) -> ComparisonReport {
    let context = variant.context();
    let request = variant_request(OutputFormat::Png, variant.options(), &context);
    let outcome = Renderer::new()
        .render(&variant.scene, &request)
        .unwrap_or_else(|error| panic!("{context} renders as PNG: {error:?}"));
    let png = variant.format("png");
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
    assert_eq!(
        (candidate.width(), candidate.height()),
        (width, height),
        "{context}: the direct PNG has the official raster's size"
    );
    compare(
        &reference,
        &candidate,
        visual_class(png["visual_class"].as_str().expect("visual class")),
        &[],
        ComparisonMode::Canonical,
    )
    .expect("dimensions agree")
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
