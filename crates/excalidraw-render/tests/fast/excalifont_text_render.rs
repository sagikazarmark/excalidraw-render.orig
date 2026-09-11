use std::fs;

use crate::common::pdf_inspect::{declares_base_font, pdf_actual_text, pdf_text, stream_data};
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request};
use crate::common::reports::assert_report_matches;
use crate::common::repository_root;
use crate::common::svg::{font_family_attribute, EXCALIFONT_FAMILIES};
use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderOutcome, RenderRequest, Renderer, ResourceLimits,
    SceneDocument,
};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The text of the focused scene: two lines, eighteen scalars.
const FOCUSED_TEXT: &str = "One two three\nFour";

#[test]
fn excalifont_text_renders_semantically_in_every_format() {
    let scene = focused_scene();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let first = render(&scene, format);
        assert!(first.report().diagnostics().is_empty());

        // One whole-artifact digest per format is the artifact byte stability
        // canary for shaped text and its embedded font subset; every other
        // exact measurement of the same bytes would only repeat it.
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("@font-face{font-family:'Excalifont'"));
                assert!(svg.contains("data:font/woff2;base64,"));
                assert!(
                    svg.contains("font.excalifont.ofl-1.1")
                        || svg.contains("Excalifont and Virgil provenance is retained together")
                );
                assert!(svg.contains(&format!(
                    "{} font-size=\"20\" text-anchor=\"middle\"",
                    font_family_attribute(EXCALIFONT_FAMILIES)
                )));
                assert_eq!(svg.matches("<text data-source-text=").count(), 1);
                assert_eq!(svg.matches("<tspan ").count(), 2);
                let first_text = svg
                    .split("<tspan x=\"")
                    .nth(1)
                    .expect("text line is emitted");
                let line_x = first_text
                    .split('"')
                    .next()
                    .expect("text has a line anchor");
                let line_y = first_text
                    .split(" y=\"")
                    .nth(1)
                    .and_then(|value| value.split('"').next())
                    .expect("text has a line baseline");
                assert_eq!(line_x, "45");
                assert_eq!(line_y.split_whitespace().count(), 1);
                // Lines are adjacent tspans with nothing between them: under
                // xml:space="preserve" any separator is a rendered character,
                // and a trailing space would shift a middle- or end-anchored
                // line. The line break lives in data-source-text.
                assert!(svg.contains(">One two three</tspan><tspan"));
                assert!(svg.contains(">Four</tspan></text>"));
                assert_eq!(svg_source_text(svg), FOCUSED_TEXT);
                assert!(!svg.contains("Unavailable: text"));
                // Moved with #150/#151: `font-family` carries the consumer
                // fallback list after the embedded family; then again with
                // #151 item 1, when `<metadata>` began carrying the Excalifont
                // attribution and the OFL body as two notices instead of one
                // OFL text under the family's license identity. Nothing
                // outside `<metadata>` moved the second time. Moved a third
                // time with #172: the text element is rotated 0.125 rad and
                // its box corners now rotate in the reference's
                // `pointRotateRads` order, which lands the layout origin one
                // ulp away (`translate(14.137185427188001 ...)`); the extent,
                // the PNG and the PDF are unchanged.
                // ADR-0003's identity refresh changes the embedded subset's
                // name table and its profile-derived identity/notice.
                assert_eq!(
                    artifact_sha256(&first),
                    "25ce18f298281f6a899de079bd4e065d3f0e62d0e863fb431f8e55cc3350a90c"
                );
            }
            OutputFormat::Png => {
                let png = decode_png(first.artifact().bytes());
                assert_eq!((png.width, png.height), (118, 102));
                assert!(png.rgba.as_chunks::<4>().0.iter().any(|pixel| {
                    pixel[2] > pixel[0] && pixel[2] > pixel[1] && pixel[3] == 255
                }));
                assert_eq!(
                    artifact_sha256(&first),
                    "cb90c943bc71d5eec983bf2c933e1c3c16dd4ca7b5fc14ba79f0230404209534"
                );
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(first.artifact().bytes());
                assert!(pdf.contains("/Subtype /Type0"));
                assert!(declares_base_font(&pdf, "Excalifont"));
                assert!(pdf.contains("/ToUnicode "));
                assert!(pdf.contains("/FontFile2 "));
                assert!(pdf.contains("/CIDToGIDMap "));
                assert!(!pdf.contains("/CIDToGIDMap /Identity"));
                assert!(pdf.contains("<0001> <004F>"));
                assert!(pdf.contains(
                    "/ActualText <FEFF004F006E0065002000740077006F002000740068007200650065000A>"
                ));
                assert!(pdf.contains("/ActualText <FEFF0046006F00750072>"));
                assert_eq!(pdf_actual_text(&pdf), FOCUSED_TEXT);
                assert!(!pdf.contains("Unavailable: text"));
                assert!(pdf.contains("Excalifont and Virgil provenance is retained together"));
                assert!(pdf.contains("license.ofl-1.1"));
                // Moved with #151 item 1: the XMP manifest carries the
                // Excalifont attribution and the OFL body as two notices;
                // every stream outside the metadata is unchanged.
                assert_eq!(
                    artifact_sha256(&first),
                    "ed5dd7b2faf6fcd7c56c6afd5f8adbfb01d4326079414de7b16ef9d53cc2c6bb"
                );
            }
            _ => panic!("the test covers the revision-1 format set"),
        }
    }
}

fn artifact_sha256(outcome: &RenderOutcome) -> String {
    format!("{:x}", Sha256::digest(outcome.artifact().bytes()))
}

#[test]
fn approved_excalifont_text_passes_the_direct_png_typography_budget() {
    let case = repository_root().join("tests/conformance/cases/focused/excalifont-text");
    let scene_bytes = fs::read(case.join("scene.excalidraw")).expect("approved scene is present");
    let scene = SceneDocument::parse(&scene_bytes).expect("approved scene is admitted");
    let outcome = render(&scene, OutputFormat::Png);

    assert_report_matches(
        outcome.report(),
        &case.join("expected-report.json"),
        "approved Excalifont text PNG",
    );
    let candidate = decode_png(outcome.artifact().bytes()).into_canonical();
    let reference = CanonicalRgba8::new(
        110,
        92,
        fs::read(case.join("evidence/official-png.rgba"))
            .expect("approved direct-canvas evidence is present"),
    )
    .expect("approved evidence is canonical RGBA8");
    let report = compare(
        &reference,
        &candidate,
        VisualClass::Typography,
        &[],
        ComparisonMode::Canonical,
    )
    .expect("candidate and reference dimensions agree");
    assert!(
        report.passed(),
        "typography budget failed: changed_pixels={}, ssim={:?}",
        report.whole.changed_pixels,
        report.whole.ssim
    );
}

#[test]
fn excalifont_text_limits_recover_at_exact_scalar_and_shaping_work_boundaries() {
    let scene = focused_scene();
    let unconstrained = render(&scene, OutputFormat::Svg);
    let scalars = u32::try_from(FOCUSED_TEXT.chars().count()).expect("scalar count fits u32");

    let accepted_limits = ResourceLimits::builder()
        .max_text_scalars_per_element(scalars)
        .max_total_text_scalars(u64::from(scalars))
        .build()
        .expect("text scalar equality is valid");
    let accepted = render_svg_with_limits(&scene, accepted_limits);
    assert!(accepted.report().is_empty());
    assert_eq!(
        accepted.artifact().bytes(),
        unconstrained.artifact().bytes()
    );

    let crossing_limits = ResourceLimits::builder()
        .max_text_scalars_per_element(scalars - 1)
        .build()
        .expect("text scalar crossing profile is valid");
    let crossing = render_svg_with_limits(&scene, crossing_limits);
    let diagnostic = crossing
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
        })
        .expect("the local text crossing diagnostic is retained");
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
    );
    assert_eq!(diagnostic.element_id(), Some("text"));
    assert_eq!(diagnostic.json_pointer(), "/elements/0");
    assert_eq!(
        serde_json::to_value(diagnostic).expect("diagnostic serializes"),
        serde_json::json!({
            "code":"resource.element-complexity-replaced",
            "severity":"warning",
            "format":"svg",
            "effects":["visual"],
            "element_id":"text",
            "json_pointer":"/elements/0",
            "message":"text was omitted because it exceeds its scalar limit",
            "details":{
                "kind":"resource_limit",
                "field":"max_text_scalars_per_element",
                "configured":scalars - 1,
                "measurement":{"kind":"attempted","value":scalars},
                "value_kind":"count",
                "recovery":"placeholder"
            }
        })
    );
    let crossing_svg = std::str::from_utf8(crossing.artifact().bytes()).expect("SVG is UTF-8");
    assert!(crossing_svg.contains("fill=\"#f1f3f5\""));
    assert!(!crossing_svg.contains("font-family=\"Excalifont\""));
    assert!(!crossing_svg.contains("@font-face{font-family:'Excalifont'"));

    let aggregate_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_text_scalars_per_element(scalars - 1)
                .max_total_text_scalars(u64::from(scalars - 1))
                .build()
                .expect("aggregate text crossing limits are valid"),
        )
        .build()
        .expect("aggregate text crossing request is valid");
    let aggregate = Renderer::new()
        .render(&scene, &aggregate_request)
        .expect_err("aggregate text crossing is fatal before shaping");
    assert_eq!(
        aggregate.code(),
        excalidraw_render::ErrorCode::RESOURCE_TOTAL_TEXT_EXCEEDED
    );
    assert!(aggregate.report().is_empty());

    let shaping_work = element_work_boundary(&scene, OutputFormat::Svg);
    let work_equality = render_svg_with_limits(
        &scene,
        ResourceLimits::builder()
            .max_element_work_units(shaping_work)
            .build()
            .expect("work equality profile is valid"),
    );
    assert!(work_equality.report().is_empty());
    assert_eq!(
        work_equality.artifact().bytes(),
        unconstrained.artifact().bytes()
    );
    let work_crossing = render_svg_with_limits(
        &scene,
        ResourceLimits::builder()
            .max_element_work_units(shaping_work - 1)
            .build()
            .expect("work crossing profile is valid"),
    );
    let descriptor =
        work_crossing_descriptor(&work_crossing).expect("the shaping-work diagnostic is retained");
    assert_eq!(descriptor["details"]["configured"], shaping_work - 1);
    assert_eq!(descriptor["details"]["measurement"]["value"], shaping_work);
    assert_eq!(descriptor["details"]["value_kind"], "work_units");
}

#[test]
fn basic_latin_svg_leaves_line_shaping_and_alignment_to_the_viewer() {
    for (alignment, anchor_x, text_anchor) in [
        ("left", "0", "start"),
        ("center", "50", "middle"),
        ("right", "100", "end"),
    ] {
        let source = format!(
            "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"text\",\"type\":\"text\",\"width\":100,\"height\":30,\"fontSize\":20,\"fontFamily\":5,\"text\":\"office\",\"textAlign\":\"{alignment}\",\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}"
        );
        let scene = SceneDocument::parse(source.as_bytes()).expect("text scene is admitted");
        let outcome = render_svg_with_limits(&scene, ResourceLimits::default());
        assert!(outcome.report().is_empty());
        let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
        let first_text = svg.split("<tspan x=\"").nth(1).expect("text is emitted");
        let line_x = first_text.split('"').next().expect("line x terminates");
        let line_y = first_text
            .split(" y=\"")
            .nth(1)
            .and_then(|value| value.split('"').next())
            .expect("line y terminates");

        assert_eq!(line_x, anchor_x);
        assert_eq!(line_y.split_whitespace().count(), 1);
        assert!(svg.contains(&format!("text-anchor=\"{text_anchor}\"")));
        assert!(svg.contains(">office</tspan>"));
        assert_eq!(svg_source_text(svg), "office");
    }
}

#[test]
fn png_text_coverage_uses_the_transformed_glyph_rectangle() {
    let scene = focused_scene();
    let unconstrained = render(&scene, OutputFormat::Png);
    // PNG charges the glyph coverage raster on top of the shaping work every
    // format shares, so its boundary sits above the SVG one.
    let coverage_work = element_work_boundary(&scene, OutputFormat::Png);
    assert!(coverage_work > element_work_boundary(&scene, OutputFormat::Svg));

    let equality = render_with_work_limit(&scene, OutputFormat::Png, coverage_work);
    assert!(equality.report().is_empty());
    assert_eq!(
        equality.artifact().bytes(),
        unconstrained.artifact().bytes()
    );

    let crossing = render_with_work_limit(&scene, OutputFormat::Png, coverage_work - 1);
    let [diagnostic] = crossing.report().diagnostics() else {
        panic!("PNG glyph coverage crossing has one diagnostic");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
    );
    let descriptor = serde_json::to_value(diagnostic).expect("coverage diagnostic serializes");
    assert_eq!(descriptor["details"]["configured"], coverage_work - 1);
    let attempted = descriptor["details"]["measurement"]["value"]
        .as_u64()
        .expect("coverage attempt is an integer");
    assert_eq!(attempted, coverage_work);
    assert!(
        attempted < 200_000,
        "coverage must not charge the 118x102 full surface"
    );
}

#[test]
fn pdf_maps_the_same_glyph_id_to_distinct_unicode_character_codes() {
    let scene = text_scene(" \u{a0}", 40.0, 30.0);
    let outcome = render(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(outcome.artifact().bytes());
    assert!(pdf.contains("<0001> <0020>"));
    assert!(pdf.contains("<0002> <00A0>"));
    assert!(pdf.contains("<0001> Tj"));
    assert!(pdf.contains("<0002> Tj"));
    assert!(!pdf.contains("/CIDToGIDMap /Identity"));
    let cid_to_gid = pdf_stream_referenced_after(outcome.artifact().bytes(), "/CIDToGIDMap ");
    assert_eq!(&cid_to_gid[2..4], &cid_to_gid[4..6]);
    assert_ne!(&cid_to_gid[2..4], &[0, 0]);
}

/// The focused Excalifont text: two centered lines, rotated, translucent.
fn focused_scene() -> SceneDocument {
    let source = format!(
        r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"text","type":"text","x":20,"y":30,"width":90,"height":72,"angle":0.125,"strokeColor":"#1e66f5","opacity":70,"fontSize":20,"fontFamily":5,"text":{},"textAlign":"center","verticalAlign":"middle","lineHeight":1.2,"frameId":null,"updated":1}}],"appState":{{}},"files":{{}}}}"##,
        serde_json::to_string(FOCUSED_TEXT).expect("focused text serializes")
    );
    SceneDocument::parse(source.as_bytes()).expect("focused text scene is admitted")
}

fn text_scene(text: &str, width: f64, height: f64) -> SceneDocument {
    let source = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"text\",\"type\":\"text\",\"width\":{width},\"height\":{height},\"fontSize\":20,\"fontFamily\":5,\"text\":{},\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}",
        serde_json::to_string(text).expect("test text serializes")
    );
    SceneDocument::parse(source.as_bytes()).expect("text scene is admitted")
}

fn render_svg_with_limits(scene: &SceneDocument, limits: ResourceLimits) -> RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");
    render_with_request(scene, &request)
}

fn render_with_work_limit(
    scene: &SceneDocument,
    format: OutputFormat,
    max_element_work_units: u64,
) -> RenderOutcome {
    let request = RenderRequest::builder(format)
        .resource_limits(
            ResourceLimits::builder()
                .max_element_work_units(max_element_work_units)
                .build()
                .expect("element work limit is valid"),
        )
        .build()
        .expect("work-limited request is valid");
    render_with_request(scene, &request)
}

/// More probes than the pipeline has stages that meter element work (shared
/// resolution, projection and the PNG raster), so a probe that keeps crossing
/// is a metering change rather than a slow convergence.
const WORK_PROBE_LIMIT: usize = 8;

/// The text element's semantic work for `format`: the smallest
/// `max_element_work_units` at which it renders without a work crossing. Each
/// stage that meters element work refuses the whole attempt and reports it, so
/// a probe that starts at one unit and adopts each refused attempt as its next
/// limit settles on the boundary in as many renders as there are stages; the
/// tests then hold the ±1 boundary against that value instead of a literal.
fn element_work_boundary(scene: &SceneDocument, format: OutputFormat) -> u64 {
    let mut limit = 1;
    for _ in 0..WORK_PROBE_LIMIT {
        let outcome = render_with_work_limit(scene, format, limit);
        let Some(descriptor) = work_crossing_descriptor(&outcome) else {
            return limit;
        };
        let attempted = descriptor["details"]["measurement"]["value"]
            .as_u64()
            .expect("work attempt is an integer");
        assert!(
            attempted > limit,
            "a work crossing at {limit} reports a larger attempt, not {attempted}"
        );
        limit = attempted;
    }
    panic!("{format:?} element work did not settle within {WORK_PROBE_LIMIT} probes");
}

/// The descriptor of the text element's work crossing, if the render raised
/// one; several stages may refuse, in which case the largest attempt is the one
/// the next probe must clear.
fn work_crossing_descriptor(outcome: &RenderOutcome) -> Option<Value> {
    outcome
        .report()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
                && diagnostic.element_id() == Some("text")
        })
        .map(|diagnostic| serde_json::to_value(diagnostic).expect("work diagnostic serializes"))
        .max_by_key(|descriptor| descriptor["details"]["measurement"]["value"].as_u64())
}

/// The stream of the object that the reference following `key` names.
fn pdf_stream_referenced_after(pdf: &[u8], key: &str) -> Vec<u8> {
    let text = pdf_text(pdf);
    let object = text
        .split(key)
        .nth(1)
        .and_then(|tail| tail.split_whitespace().next())
        .expect("PDF reference has an object number");
    stream_data(pdf, format!("{object} 0 obj\n").as_bytes())
}

fn svg_source_text(svg: &str) -> String {
    let mut source = String::new();
    for tail in svg.split("data-source-text=\"").skip(1) {
        let value = tail
            .split('"')
            .next()
            .expect("source annotation terminates");
        source.push_str(
            &value
                .replace("&#10;", "\n")
                .replace("&#13;", "\r")
                .replace("&#9;", "\t")
                .replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&"),
        );
    }
    source
}
