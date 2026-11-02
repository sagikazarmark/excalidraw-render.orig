//! SVG parity for every registered scene case: path geometry, paint and text
//! attributes, and the rasterized document, judged on one render of each
//! variant.
//!
//! **Path geometry.** The `d` attribute of every `<path>` the renderer emits is
//! compared against the official export's, in document order. It is a sharper
//! check than raster comparison for geometry and a far cheaper one: no viewer,
//! no rasterization, and no antialiasing noise to threshold against. A single
//! wrong coordinate shows up as an exact mismatch. It exists because two
//! geometry defects were caught this way and one wrong fix was caught only this
//! way: zeroing the roughness of elbow arrows made a scene extent match official
//! to fifteen digits while corrupting three paths, and path comparison rejects
//! that immediately. The root `viewBox` is compared too, which is where
//! scene-bounds defects live: a bounds error emits every path at identical
//! coordinates inside a differently sized document, so path data alone cannot
//! see it. The root `width` and `height` are compared as well: an export scale
//! multiplies only those and leaves the `viewBox` logical, so they are the one
//! place a scaled SVG variant differs from the default and the one place a
//! scaling defect shows. Path data and extent are the invariants, not SVG
//! structure: the renderer legitimately differs from the official export in how
//! it positions text and groups elements.
//!
//! **Paint and text attributes.** The raster flags a covered pixel only above
//! about 26 grey levels, so a stroke drawn `#333333` instead of `#1e1e1e`
//! passes every pixel. The attribute dimension compares, exactly and without a
//! rasterizer, what every painted shape and every text line is drawn with:
//! fill, stroke, stroke width, line cap and dash array, opacity and the
//! composed transform for each `<path>` and `<rect>`, and orientation,
//! baseline, `font-size`, fill and opacity, and where the line has one anchor
//! its position and `text-anchor`, for each line of text. Both documents are
//! resolved through their group structure first, since the two exporters
//! group and position differently. `common::svg_paint` states what is
//! normalized and what is excluded, each with its reason: `font-family`, which
//! the renderer spells as the spec's fallback list (spec 8.4); `clip-path` and
//! `mask`, whose ids differ; stroke details on unstroked shapes; shapes that
//! paint nothing; and the along-the-line position and anchor of lines the
//! renderer positions glyph by glyph. The first differing attribute is named
//! with both values. A divergence awaiting a decision is pinned at its exact
//! message in `KNOWN_ATTRIBUTE_DIVERGENCES`, like the geometry pins.
//!
//! **Document raster.** The direct-PNG harness compares rasters two different
//! rasterizers produced, so a difference there can be the document or the
//! rasterizer. Here the rasterizer is removed from the question: the renderer's
//! SVG and the official export's SVG are both rasterized in-process by the same
//! `resvg`, each with a font database built from its own embedded `@font-face`
//! payloads, and compared under the pure-Rust SVG budgets. What is left is the
//! document itself: path geometry, stroke and fill attributes, text placement
//! and sizing, dark-mode filters, and embedded images. This is document parity,
//! not a canonical viewer gate. The spec's canonical SVG viewers
//! (`canonical-svg.chromium.v1`, `canonical-svg.pure-rust.v1`) are pinned,
//! isolated and declaration-bound; neither currently admits the renderer's or
//! the official export's text-bearing SVG, and running them is a separate piece
//! of work. The workspace `resvg` used here is not the pinned viewer, so its
//! output is never evidence and never approved.
//!
//! Fonts. Both documents embed WOFF2 subsets under CSS families; each payload is
//! decoded to its sfnt and registered under the family the document names, so
//! `Excalifont` in the official export and `Excalifont` in ours resolve to their
//! respective embedded subsets. The official export names some system families
//! without embedding them (`Helvetica`); those are resolved through the
//! renderer's manifested metric-compatible substitute so that the comparison is
//! about placement, not about which face the browser happened to have. A family
//! that is neither embedded nor substituted (`Segoe UI Emoji` in a fallback
//! list) stays unresolved and contributes nothing on either side.
//!
//! The raster dimension holds four kinds of pinned divergence apart from the
//! budget: known defects (renderer defects awaiting a decision), compatibility
//! exception pins (approved direct-PNG exceptions this dimension observes too),
//! reference authority divergences (official SVGs this rasterizer misrenders
//! where the renderer follows the official canvas, proved by the variant's
//! direct PNG passing the canonical budget) and rasterizer limitations
//! (official SVGs this rasterizer cannot lay out as browsers do, proved by the
//! variant's direct PNG being where its record says it is).
//!
//! Every pin is a measurement under the workspace `resvg`, `tiny-skia` and
//! `fontdb` at whatever versions `Cargo.lock` resolves, and is exact in both
//! directions. A rasterizer bump that moves a measurement therefore fails on
//! that pin with the observed values printed, and a bump that moves nothing
//! needs no edit here; the harness does not mirror the lockfile's versions.
//! The canonical judge of the pure-Rust SVG dimension is the pinned viewer in
//! `tools/svg-viewer-rust`, which holds its own lock; running the raster
//! dimension through it as a subprocess stays in reserve for the day a bump
//! does move a pin.
//!
//! Every registered variant is compared on both dimensions, since dark mode and
//! sizing must leave path geometry untouched, and each variant is rendered once:
//! the two dimensions judge the same bytes, and a divergence in each is reported
//! separately so that a change corrupting path data while making a raster agree
//! produces an entry for each rather than hiding the more serious half. The
//! variants are judged over the available cores; the comparator, not the
//! renderer, is what takes the time.

mod common;

use std::collections::BTreeSet;
use std::fs;

use excalidraw_render::{OutputFormat, Renderer};
use excalidraw_render_compare::{
    compare, ComparisonMode, ComparisonReport, RegionReport, SsimOutcome,
};
use serde_json::Value;

use base64::Engine;
use common::lower_hex;
use common::registered_cases::{
    case_count, judge_each, observed, registered_scene_variants, variant_request, visual_class,
    PinnedAttributeDivergence, PinnedDivergence, PinnedGeometryDefect, RasterizerLimitation,
    ReferenceAuthorityDivergence, RegisteredVariant,
};
use common::reports::assert_report_matches;
use common::repository_root;
use common::svg_paint::attribute_divergence;
use common::svg_raster::{embedded_faces, rasterize, referenced_families, EmbeddedFace};

/// Diagnosed defects whose fix is pending a maintainer decision, observed on the
/// rasterized document. See `PinnedDivergence` for why these are pinned rather
/// than excepted.
const KNOWN_DEFECTS: &[PinnedDivergence] = &[];

/// Diagnosed defects whose fix is pending a maintainer decision, observed on
/// the extent or path dimension, where the renderer's own geometry is held at
/// its exact digest instead of a measurement. See `PinnedGeometryDefect`.
const KNOWN_GEOMETRY_DEFECTS: &[PinnedGeometryDefect] = &[];

/// Diagnosed paint or text attribute divergences whose fix is pending a
/// maintainer decision, held at the attribute dimension's exact message.
const KNOWN_ATTRIBUTE_DIVERGENCES: &[PinnedAttributeDivergence] = &[];

/// Compatibility exceptions whose approved tolerance exception the case
/// manifest carries for the direct PNG, observed here on the rasterized SVG
/// document as well. The manifest admits no `format: svg` exception, so each is
/// pinned at its exact measurement and must correspond to that recorded
/// exception; see `PinnedDivergence`.
///
/// The measurements are those of the workspace `resvg`, `tiny-skia` and
/// `fontdb` at whatever versions `Cargo.lock` resolves; see the module docs.
const COMPATIBILITY_EXCEPTION_PINS: &[PinnedDivergence] = &[
    // Spec 8.3's replacement glyph (#124): the official document sets the three
    // unmapped scalars in the embedded Excalifont and Liberation Sans subsets,
    // which have no glyph for them, while the renderer's sets `?` from
    // Liberation Sans, and the differing advances shift the trailing word.
    PinnedDivergence {
        case: "focused.missing-glyph.v1",
        variant: "default",
        issue: "#124",
        changed_pixels: 473,
        ssim_bits: 0x3fea_c711_a5b4_5706,
    },
    // Spec 8.3's Noto Emoji substitution (#121): the official document sets
    // the emoji in the embedded Excalifont and Liberation Sans subsets, which
    // have no glyph for them (its `Segoe UI Emoji` fallback is unresolved
    // here, see the module docs), while the renderer's sets them from the
    // complete monochrome Noto Emoji face it embeds.
    PinnedDivergence {
        case: "focused.emoji-fallback.v1",
        variant: "default",
        issue: "#121",
        changed_pixels: 577,
        ssim_bits: 0x3fea_ff42_54bc_d188,
    },
    // Spec 8.3's replacement glyph on bidirectional text (#152, the #124
    // class): no bundled face maps a Hebrew or Arabic scalar, so the official
    // document sets them in the embedded Excalifont subset and in Helvetica
    // (resolved here to the Liberation Sans substitute), which draw blank
    // advances and `.notdef` boxes, while the renderer's sets `?` from
    // Liberation Sans; the differing advances displace the word on each
    // line's unanchored side, and the trailing U+2029 is a `?` against
    // nothing. The run order agrees: the case keeps its right-to-left
    // elements centred and single-run so this rasterizer's direction-blind
    // anchoring (#168) stays out of the measurement.
    PinnedDivergence {
        case: "focused.bidi-text.v1",
        variant: "default",
        issue: "#152",
        changed_pixels: 2331,
        ssim_bits: 0x3feb_ec2a_82aa_04f8,
    },
];

/// Reference authority divergences the rasterized-document dimension can see
/// (spec 8.2 and 8.4): variants whose official SVG misrenders under this
/// harness's own rasterizer because of a defect in the reference exporter's
/// serialization, while the renderer follows the official canvas in every
/// format. The frame clip and the rounded-image corner clip are written by
/// the reference as a namespaced `clip-path` attribute that browsers ignore
/// and `usvg` honours, so `usvg` clips the official document as the renderer
/// does and this harness is blind to those; it is not blind when the honoured
/// clip is itself misplaced, as the official export misplaces the corner clip
/// of a rounded and cropped image at the crop-shifted group origin. Each entry
/// is held at its exact measurement, names the spec section recording the
/// divergence and the issue that measured it, and requires the same variant's
/// direct PNG to pass the canonical budget against the official canvas
/// raster, which is the proof that the renderer, not the reference SVG, is
/// right. Nothing here awaits a fix and nothing here is excepted.
const REFERENCE_AUTHORITY_DIVERGENCES: &[ReferenceAuthorityDivergence] = &[
    // The rounded and cropped 48x48 image (#163, measured on this rasterizer):
    // the official SVG's corner clip sits at the crop-shifted group origin, so
    // `usvg` shows the image's top-left quadrant with one rounded corner where
    // the renderer clips the whole element as the canvas does. The unrounded
    // crop control beside it agrees in both dimensions, and the variant's
    // direct PNG passes the canonical budget (0 changed pixels, SSIM 0.99979).
    ReferenceAuthorityDivergence {
        pin: PinnedDivergence {
            case: "focused.rounded-image-crop.v1",
            variant: "default",
            issue: "#163",
            changed_pixels: 1572,
            ssim_bits: 0x3fe9_11f7_8d01_2926,
        },
        spec_section: "spec 8.2",
    },
];

/// Limitations of this harness's own rasterizer (#168): official SVGs it
/// cannot lay out as the pinned Chromium and the canvas do, where neither the
/// renderer nor the reference exporter is wrong. `usvg` parses `direction`
/// but never reads it and resolves `text-anchor` as if every text were
/// left-to-right, and it lays out only the first UAX #9 paragraph of a `<text>`.
/// The official exporter writes every right-to-left element as
/// `<text direction="rtl" text-anchor="end">` with `x` at 0, width/2 or width
/// for textAlign left, center or right: in Chromium `end` under `rtl` puts the
/// text's left edge at `x`, so a left-aligned element sits where the canvas
/// draws it, while under `usvg` its right edge does and the element lands off
/// the canvas. Each entry is held at its exact measurement, names the issue
/// and the limitation, and requires the same variant's direct PNG to be where
/// its record says it is against the official canvas raster (spec 8.3 and
/// `tests/conformance/README.md`). Nothing here awaits a renderer fix; a bump
/// of the rasterizer that lifts the limitation moves the entry, which then
/// goes.
const RASTERIZER_LIMITATIONS: &[RasterizerLimitation] = &[
    // The left- and right-aligned right-to-left elements and the two
    // direction-mixed elements (#168, measured on this rasterizer): the
    // official document's left-aligned right-to-left elements (`x="0"
    // text-anchor="end" direction="rtl"`) land off the canvas, its pure
    // Hebrew lines set in the embedded Excalifont subset draw nothing, and
    // the left-to-right element's Hebrew-first second line is laid out at a
    // left-to-right base level, while the renderer's document positions every
    // run's `?` glyphs (#124) explicitly where the canvas puts them. The
    // extent and every path match, and the variant's direct PNG is under its
    // recorded #124-class exception, with the anchored side of every line
    // agreeing with the official canvas.
    RasterizerLimitation {
        pin: PinnedDivergence {
            case: "focused.bidi-alignment.v1",
            variant: "default",
            issue: "#168",
            changed_pixels: 1190,
            ssim_bits: 0x3fea_564b_9c97_ab4f,
        },
        limitation: "usvg ignores direction and anchors right-to-left text as left-to-right",
    },
];

#[test]
fn every_registered_scene_case_matches_its_official_svg_geometry_and_raster() {
    let variants = registered_scene_variants(&repository_root());
    let seen: BTreeSet<_> = variants
        .iter()
        .map(|variant| (variant.case.clone(), variant.variant.clone()))
        .collect();

    let failures: Vec<String> = judge_each(&variants, svg_divergences)
        .into_iter()
        .flatten()
        .collect();

    PinnedDivergence::assert_all_observed(KNOWN_DEFECTS, &seen);
    PinnedGeometryDefect::assert_all_observed(KNOWN_GEOMETRY_DEFECTS, &seen);
    PinnedAttributeDivergence::assert_all_observed(KNOWN_ATTRIBUTE_DIVERGENCES, &seen);
    PinnedDivergence::assert_all_observed(COMPATIBILITY_EXCEPTION_PINS, &seen);
    ReferenceAuthorityDivergence::assert_all_observed(REFERENCE_AUTHORITY_DIVERGENCES, &seen);
    RasterizerLimitation::assert_all_observed(RASTERIZER_LIMITATIONS, &seen);
    assert!(
        failures.is_empty(),
        "{} divergence(s) from the official SVG export across {} cases:\n  {}",
        failures.len(),
        case_count(&variants),
        failures.join("\n  ")
    );
}

/// Renders one variant's SVG once and judges it on every dimension, one entry
/// per dimension that diverges.
fn svg_divergences(variant: &RegisteredVariant) -> Vec<String> {
    let context = variant.context();
    let request = variant_request(OutputFormat::Svg, variant.options(), &context);
    let outcome = Renderer::new()
        .render(&variant.scene, &request)
        .unwrap_or_else(|error| panic!("{context} renders as SVG: {error:?}"));
    let candidate_svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    let svg = variant.format("svg");
    // A report mismatch is never pinned or excepted: the renderer's diagnostics
    // disagreeing with what the case declares is a contract break, not a
    // document difference. The checked report is canonical JSON and the
    // renderer's is spec-ordered, so they compare as values.
    assert_report_matches(
        outcome.report(),
        &variant.dir.join(
            svg["expected_report"]["path"]
                .as_str()
                .expect("expected report path"),
        ),
        &format!("{context}: SVG"),
    );
    let official_svg = fs::read_to_string(
        variant
            .dir
            .join(svg["artifact"]["path"].as_str().expect("official SVG path")),
    )
    .expect("official SVG present");

    let extent = extent_divergence(candidate_svg, &official_svg);
    let paths = path_divergence(candidate_svg, &official_svg);
    // A geometry defect pin holds the renderer's extent and paths in place of
    // the two geometry dimensions; the raster dimension is still judged when
    // the two documents rasterize to one size, and is the pinned extent's
    // consequence when they do not.
    let geometry =
        PinnedGeometryDefect::lookup(KNOWN_GEOMETRY_DEFECTS, &variant.case, &variant.variant);
    let dimensions: Vec<(&str, Option<String>)> = match geometry {
        Some(pin) => vec![(
            "geometry",
            pin.mismatch(candidate_svg, extent.is_some() || paths.is_some()),
        )],
        None => vec![("extent", extent), ("paths", paths)],
    };
    let attributes = attribute_divergence(candidate_svg, &official_svg);
    let attributes = match PinnedAttributeDivergence::lookup(
        KNOWN_ATTRIBUTE_DIVERGENCES,
        &variant.case,
        &variant.variant,
    ) {
        Some(pin) => pin.mismatch(attributes.as_deref()),
        None => attributes,
    };
    dimensions
        .into_iter()
        .chain(std::iter::once(("attributes", attributes)))
        .chain(std::iter::once((
            "raster",
            raster_divergence(variant, candidate_svg, &official_svg, geometry.is_some()),
        )))
        .filter_map(|(dimension, divergence)| {
            divergence.map(|detail| format!("{context} [{dimension}]: {detail}"))
        })
        .collect()
}

/// The root extent divergence, if any. The viewBox is the logical extent;
/// width and height are that extent times the export scale.
fn extent_divergence(candidate_svg: &str, official_svg: &str) -> Option<String> {
    let extent = ["viewBox", "width", "height"]
        .into_iter()
        .filter_map(|attribute| {
            let ours = root_attribute(candidate_svg, attribute);
            let theirs = root_attribute(official_svg, attribute);
            (ours != theirs).then(|| format!("{attribute} {ours:?} against official {theirs:?}"))
        })
        .collect::<Vec<_>>();
    (!extent.is_empty()).then(|| extent.join("; "))
}

/// The first path-data divergence in document order, if any.
fn path_divergence(candidate_svg: &str, official_svg: &str) -> Option<String> {
    let candidate = path_data(candidate_svg);
    let official = path_data(official_svg);
    if candidate.len() != official.len() {
        return Some(format!(
            "emits {} paths, official has {}",
            candidate.len(),
            official.len()
        ));
    }
    candidate
        .iter()
        .zip(&official)
        .position(|(a, b)| a != b)
        .map(|index| {
            format!(
                "path {index} differs\n      ours     {}\n      official {}",
                truncate(&candidate[index]),
                truncate(&official[index])
            )
        })
}

/// The rasterized-document divergence, if any: a size the evidence does not
/// declare, a measurement off its pin, or a comparison outside the budget. A
/// candidate whose size differs under a geometry defect pin
/// (`extent_pinned`) is that pinned extent's consequence and is not compared.
fn raster_divergence(
    variant: &RegisteredVariant,
    candidate_svg: &str,
    official_svg: &str,
    extent_pinned: bool,
) -> Option<String> {
    let context = variant.context();
    let reference = rasterize(official_svg, &format!("{context} official"));
    let candidate = rasterize(candidate_svg, &format!("{context} candidate"));

    // The evidence records the size Chromium rasterized the official SVG at;
    // resvg must land on the same size or the comparison is not of the artifact
    // the case declares.
    let svg = variant.format("svg");
    let raw = &svg["raw_raster"];
    let declared = (
        raw["width"].as_u64().expect("raster width") as u32,
        raw["height"].as_u64().expect("raster height") as u32,
    );
    if (reference.width(), reference.height()) != declared {
        return Some(format!(
            "official SVG rasterizes to {}x{}, evidence declares {}x{}",
            reference.width(),
            reference.height(),
            declared.0,
            declared.1
        ));
    }
    if (candidate.width(), candidate.height()) != declared {
        return (!extent_pinned).then(|| {
            format!(
                "candidate SVG rasterizes to {}x{}, official is {}x{}",
                candidate.width(),
                candidate.height(),
                declared.0,
                declared.1
            )
        });
    }

    let class = visual_class(svg["visual_class"].as_str().expect("visual class"));
    let report = compare(
        &reference,
        &candidate,
        class,
        &[],
        ComparisonMode::PureRustSvg,
    )
    .expect("dimensions agree");
    let defect = PinnedDivergence::lookup(KNOWN_DEFECTS, &variant.case, &variant.variant);
    let exception = PinnedDivergence::lookup(
        COMPATIBILITY_EXCEPTION_PINS,
        &variant.case,
        &variant.variant,
    );
    let authority = ReferenceAuthorityDivergence::lookup(
        REFERENCE_AUTHORITY_DIVERGENCES,
        &variant.case,
        &variant.variant,
    );
    let limitation =
        RasterizerLimitation::lookup(RASTERIZER_LIMITATIONS, &variant.case, &variant.variant);
    assert!(
        [
            defect.is_some(),
            exception.is_some(),
            authority.is_some(),
            limitation.is_some()
        ]
        .into_iter()
        .filter(|pinned| *pinned)
        .count()
            <= 1,
        "{context} is pinned in more than one list; a variant is a known defect, a \
         compatibility exception, a reference authority divergence or a rasterizer \
         limitation, never two of them"
    );
    if exception.is_some() {
        // The pin stands for the manifest's approved judgment on this variant,
        // so that judgment must exist: a pin with no recorded exception would
        // be a suppression.
        assert!(
            has_recorded_exception(&variant.manifest, &variant.variant),
            "{context} carries a compatibility exception pin but its manifest records no \
             direct-PNG tolerance exception for the variant"
        );
    }
    match (defect, exception, authority, limitation) {
        (Some(defect), ..) => defect.mismatch(&report),
        (None, Some(exception), ..) => exception.mismatch(&report),
        (None, None, Some(authority), _) => authority.mismatch(&report, variant),
        (None, None, None, Some(limitation)) => limitation.mismatch(&report, variant),
        (None, None, None, None) => (!report.passed())
            .then(|| format!("outside the pure-Rust SVG budget: {}", observed(&report))),
    }
}

/// Whether the case manifest records a whole direct-PNG tolerance exception
/// for the variant: the same record `parity_registered_case_png` consumes, and
/// the approved judgment a compatibility exception pin here stands for.
fn has_recorded_exception(manifest: &Value, variant_id: &str) -> bool {
    manifest["exceptions"]
        .as_array()
        .expect("manifest exceptions are an array")
        .iter()
        .any(|exception| {
            exception["variant"] == variant_id
                && exception["format"] == "png"
                && exception["region"] == "whole"
        })
}

/// An attribute of the `<svg>` root, located by name within the root tag so a
/// same-named attribute on a nested element cannot stand in for it.
fn root_attribute(svg: &str, name: &str) -> String {
    let start = svg.find("<svg").expect("SVG has a root element");
    let tag = &svg[start..];
    let tag = &tag[..tag.find('>').expect("SVG root tag closes")];
    let marker = format!(" {name}=\"");
    let value = &tag[tag
        .find(&marker)
        .unwrap_or_else(|| panic!("SVG root declares {name}"))
        + marker.len()..];
    value[..value.find('"').expect("attribute value closes")].to_owned()
}

/// Every `<path>` element's `d` attribute, in document order. The attribute is
/// located by name rather than by position: the official export writes `fill`
/// before `d` on freedraw strokes and `d` first elsewhere.
fn path_data(svg: &str) -> Vec<String> {
    let mut data = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find("<path") {
        rest = &rest[start + "<path".len()..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        if let Some(attribute) = tag.find(" d=\"") {
            let value = &tag[attribute + " d=\"".len()..];
            if let Some(close) = value.find('"') {
                data.push(value[..close].to_owned());
            }
        }
        rest = &rest[end..];
    }
    data
}

fn truncate(value: &str) -> String {
    if value.len() <= 90 {
        value.to_owned()
    } else {
        format!("{}...", &value[..90])
    }
}

#[test]
fn a_pinned_divergence_holds_its_variant_to_the_exact_measurement() {
    const PINNED_SSIM_BITS: u64 = 0x3fea_c711_a5b4_5706;
    let pin = PinnedDivergence {
        case: "focused.example.v1",
        variant: "default",
        issue: "#124",
        changed_pixels: 473,
        ssim_bits: PINNED_SSIM_BITS,
    };
    let report = |changed_pixels: u64, ssim: SsimOutcome| ComparisonReport {
        whole: RegionReport {
            width: 1,
            height: 1,
            area: 1,
            changed_pixels,
            changed_mask: Vec::new(),
            ssim,
            changed_pixels_pass: false,
            ssim_pass: false,
        },
        crops: Vec::new(),
    };
    let pinned_score = SsimOutcome::Score(f64::from_bits(PINNED_SSIM_BITS));

    assert_eq!(pin.mismatch(&report(473, pinned_score.clone())), None);
    // The pin is exact in both directions: an improvement is as much a moved
    // measurement as a regression, and says so with the values to transcribe.
    let improved = pin
        .mismatch(&report(472, pinned_score.clone()))
        .expect("fewer changed pixels is a moved measurement");
    assert!(improved.contains("#124"), "{improved}");
    assert!(improved.contains("changed_pixels=472"), "{improved}");
    assert!(pin.mismatch(&report(474, pinned_score)).is_some());
    assert!(pin
        .mismatch(&report(473, SsimOutcome::Score(0.9)))
        .is_some());
    assert!(pin
        .mismatch(&report(473, SsimOutcome::SmallArtifactMismatch))
        .is_some());
}

#[test]
fn a_geometry_defect_pin_holds_the_renderer_geometry_and_requires_the_divergence() {
    use common::registered_cases::svg_geometry;

    let svg = "<svg viewBox=\"0 0 200 120\" width=\"200\" height=\"120\">\
               <path d=\"M0 0 L1 1\"/><path fill=\"#000\" d=\"M2 2 L3 3\"/></svg>";
    assert_eq!(
        svg_geometry(svg),
        "viewBox=0 0 200 120\nwidth=200\nheight=120\nM0 0 L1 1\nM2 2 L3 3\n"
    );
    let digest = lower_hex(&<sha2::Sha256 as sha2::Digest>::digest(svg_geometry(svg)));
    let pinned: &'static str = Box::leak(digest.into_boxed_str());
    let pin = PinnedGeometryDefect {
        case: "focused.example.v1",
        variant: "default",
        issue: "#172",
        geometry_sha256: pinned,
    };
    assert_eq!(pin.mismatch(svg, true), None);
    // The pin cannot outlive the defect: a renderer that agrees with the
    // official export makes the entry stale.
    let stale = pin.mismatch(svg, false).expect("agreement is a stale pin");
    assert!(stale.contains("stale") && stale.contains("#172"), "{stale}");
    // And it holds the geometry exactly: a moved path is a moved pin.
    let moved = svg.replace("L3 3", "L3 4");
    let detail = pin
        .mismatch(&moved, true)
        .expect("a moved coordinate moves the pin");
    assert!(
        detail.contains("geometry moved") && detail.contains("#172"),
        "{detail}"
    );
}

#[test]
fn embedded_faces_read_the_official_export_and_the_renderer_declarations() {
    let payload = base64::engine::general_purpose::STANDARD.encode(b"wOF2-bytes");
    let official = format!(
        "<style class=\"style-fonts\">\n      @font-face {{ font-family: Excalifont; \
         src: url(data:font/woff2;base64,{payload}); }}\n      @font-face {{ font-family: \
         Xiaolai; src: url(data:font/woff2;base64,{payload}); }}\n    </style>"
    );
    let renderer = format!(
        "<style class=\"style-fonts\">@font-face{{font-family:'ExcalidrawRender-9';\
         src:url(data:font/ttf;base64,{payload})}}</style>"
    );
    assert_eq!(
        embedded_faces(&official),
        [
            EmbeddedFace {
                family: "Excalifont".into(),
                media_type: "font/woff2".into(),
                bytes: b"wOF2-bytes".to_vec(),
            },
            EmbeddedFace {
                family: "Xiaolai".into(),
                media_type: "font/woff2".into(),
                bytes: b"wOF2-bytes".to_vec(),
            },
        ]
    );
    assert_eq!(
        embedded_faces(&renderer),
        [EmbeddedFace {
            family: "ExcalidrawRender-9".into(),
            media_type: "font/ttf".into(),
            bytes: b"wOF2-bytes".to_vec(),
        }]
    );
    assert!(embedded_faces("<svg><path d=\"M0 0\"/></svg>").is_empty());
}

#[test]
fn referenced_families_split_lists_and_keep_document_order() {
    let svg = "<text font-family=\"Helvetica, Segoe UI Emoji\">a</text>\
               <text font-family=\"Excalifont\"><tspan font-family=\"Xiaolai\">b</tspan></text>\
               <text font-family=\"Helvetica\">c</text>";
    assert_eq!(
        referenced_families(svg),
        ["Helvetica", "Segoe UI Emoji", "Excalifont", "Xiaolai"]
    );
}

#[test]
fn path_divergence_reports_the_first_differing_path_and_a_count_mismatch() {
    let official = "<svg><path d=\"M0 0 L1 1\"/><path fill=\"#000\" d=\"M2 2 L3 3\"/></svg>";
    assert_eq!(path_divergence(official, official), None);
    let moved = "<svg><path d=\"M0 0 L1 1\"/><path fill=\"#000\" d=\"M2 2 L3 4\"/></svg>";
    let detail = path_divergence(moved, official).expect("a moved coordinate is a divergence");
    assert!(detail.starts_with("path 1 differs"), "{detail}");
    assert!(detail.contains("ours     M2 2 L3 4"), "{detail}");
    assert!(detail.contains("official M2 2 L3 3"), "{detail}");
    let fewer = "<svg><path d=\"M0 0 L1 1\"/></svg>";
    assert_eq!(
        path_divergence(fewer, official).as_deref(),
        Some("emits 1 paths, official has 2")
    );
}

#[test]
fn extent_divergence_compares_only_the_root_element() {
    let official = "<svg viewBox=\"0 0 200 120\" width=\"200\" height=\"120\">\
                    <svg viewBox=\"0 0 1 1\" width=\"1\" height=\"1\"/></svg>";
    assert_eq!(extent_divergence(official, official), None);
    let scaled = "<svg viewBox=\"0 0 200 120\" width=\"400\" height=\"240\"></svg>";
    assert_eq!(
        extent_divergence(scaled, official).as_deref(),
        Some("width \"400\" against official \"200\"; height \"240\" against official \"120\"")
    );
}

/// The official export of a registered case, as the attribute dimension's
/// tests mutate it.
fn official_svg(case: &str) -> String {
    fs::read_to_string(repository_root().join(format!(
        "tests/conformance/cases/focused/{case}/evidence/official.svg"
    )))
    .expect("official SVG present")
}

#[test]
fn attribute_divergence_names_a_changed_stroke_colour_width_and_opacity() {
    let official = official_svg("ellipse-diamond-fills");
    assert_eq!(attribute_divergence(&official, &official), None);

    let stroke = official.replacen("stroke=\"#1e1e1e\"", "stroke=\"#333333\"", 1);
    assert_ne!(stroke, official, "the case strokes in #1e1e1e");
    let detail = attribute_divergence(&stroke, &official).expect("a changed stroke diverges");
    assert!(
        detail.contains("stroke \"#333333\" against official \"#1e1e1e\""),
        "{detail}"
    );

    let width = official.replacen("stroke-width=\"2\"", "stroke-width=\"3\"", 1);
    assert_ne!(width, official, "the case strokes at width 2");
    let detail = attribute_divergence(&width, &official).expect("a changed width diverges");
    assert!(
        detail.contains("stroke-width \"3\" against official \"2\""),
        "{detail}"
    );

    let faded = official.replacen("<path ", "<path opacity=\"0.5\" ", 1);
    let detail = attribute_divergence(&faded, &official).expect("an opacity diverges");
    assert!(
        detail.contains("path 0 opacity \"0.5\" against official \"1\""),
        "{detail}"
    );
}

#[test]
fn attribute_divergence_names_a_changed_text_anchor_and_font_size() {
    let official = official_svg("helvetica-text");
    assert_eq!(attribute_divergence(&official, &official), None);

    let anchor = official.replacen("text-anchor=\"middle\"", "text-anchor=\"start\"", 1);
    assert_ne!(anchor, official, "the case centres a line");
    let detail = attribute_divergence(&anchor, &official).expect("a changed anchor diverges");
    assert!(
        detail.contains("text line 1 text-anchor \"start\" against official \"middle\""),
        "{detail}"
    );

    let size = official.replacen("font-size=\"20px\"", "font-size=\"21px\"", 1);
    let detail = attribute_divergence(&size, &official).expect("a changed size diverges");
    assert!(
        detail.contains("text line 0 font-size \"21\" against official \"20\""),
        "{detail}"
    );
}

#[test]
fn attribute_records_resolve_group_structure_and_spelling() {
    // A fill-only path with and without the reference's `stroke-width="0"`,
    // and a shorthand colour, inherited through differently nested groups.
    let official = "<svg><g transform=\"translate(10 10) rotate(0 90 6.900000000000006)\">\
                    <path fill=\"#bbb\" stroke=\"none\" stroke-width=\"0\" d=\"M0 0\"/></g>\
                    <g transform=\"translate(10 40)\"><text x=\"110\" y=\"16.9\" \
                    font-size=\"28px\" text-anchor=\"middle\" fill=\"#1e1e1e\">a</text></g></svg>";
    let candidate = "<svg><g fill=\"#BBBBBB\"><g transform=\"translate(10 10) rotate(0 90 6.9)\">\
                     <path stroke=\"none\" d=\"M0 0\"/></g></g>\
                     <rect fill=\"transparent\" stroke=\"none\"/>\
                     <g transform=\"translate(10 40)\"><text font-size=\"28\" \
                     text-anchor=\"middle\" fill=\"#1e1e1e\"><tspan x=\"110\" y=\"16.9\">a</tspan>\
                     </text></g></svg>";
    assert_eq!(attribute_divergence(candidate, official), None);

    // A line positioned glyph by glyph keeps its baseline but not its anchor.
    let resolved = candidate.replace(
        "<tspan x=\"110\" y=\"16.9\">a</tspan>",
        "<tspan x=\"100 105\" y=\"16.9 16.9\" text-anchor=\"start\">ab</tspan>",
    );
    assert_eq!(attribute_divergence(&resolved, official), None);
    let lowered = resolved.replace("y=\"16.9 16.9\"", "y=\"17.9 17.9\"");
    let detail = attribute_divergence(&lowered, official).expect("a moved baseline diverges");
    assert!(detail.contains("text line 0 baseline"), "{detail}");
}

#[test]
fn a_pinned_attribute_divergence_is_exact_and_cannot_outlive_its_divergence() {
    let pin = PinnedAttributeDivergence {
        case: "focused.example.v1",
        variant: "default",
        issue: "#999",
        detail: "path 0 stroke \"#333333\" against official \"#1e1e1e\"",
    };
    assert_eq!(pin.mismatch(Some(pin.detail)), None);
    let moved = pin
        .mismatch(Some(
            "path 0 stroke \"#444444\" against official \"#1e1e1e\"",
        ))
        .expect("another divergence moves the pin");
    assert!(
        moved.contains("#999") && moved.contains("#444444"),
        "{moved}"
    );
    let stale = pin.mismatch(None).expect("agreement is a stale pin");
    assert!(stale.contains("stale") && stale.contains("#999"), "{stale}");
}
