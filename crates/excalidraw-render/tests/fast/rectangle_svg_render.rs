use std::fs;

use crate::common::approved_scenes::{rectangle_outline_scene, render_rectangle_outline_svg};
use crate::common::reports::assert_report_matches;
use crate::common::repository_root;
use excalidraw_render::{
    ErrorCode, OutputFormat, RenderErrorCategory, RenderOutcome, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};
use tiny_skia::{Pixmap, Transform};

#[test]
fn approved_rectangle_renders_as_deterministic_semantic_svg() {
    let case = repository_root().join("tests/conformance/cases/focused/rectangle-outline");
    let first = render_rectangle_outline_svg();
    let second = render_rectangle_outline_svg();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert_report_matches(
        first.report(),
        &case.join("expected-report-svg.json"),
        "approved rectangle SVG",
    );

    let candidate = parse_svg(first.artifact().bytes());
    let official_bytes = fs::read(case.join("evidence/official.svg")).unwrap();
    let official = parse_svg(&official_bytes);
    assert!(candidate.starts_with(concat!(
        "<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" ",
        "viewBox=\"0 0 200 120\" width=\"200\" height=\"120\">"
    )));
    assert!(
        candidate.contains("<rect x=\"0\" y=\"0\" width=\"200\" height=\"120\" fill=\"#ffffff\"/>")
    );
    assert_eq!(candidate.matches("<path ").count(), 1);
    assert_eq!(
        path_data(candidate),
        path_data(official),
        "candidate Rough operations match the approved independent oracle"
    );
    assert!(candidate.contains(concat!(
        "<g stroke-linecap=\"round\" ",
        "transform=\"translate(10 10) rotate(0 90 50)\">"
    )));
    assert!(candidate.contains("stroke=\"#1e1e1e\" stroke-width=\"2\" fill=\"none\""));
    for absent in ["<text", "<image", "<a ", "<mask"] {
        assert!(!candidate.contains(absent));
    }

    let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
    for forbidden in ["http://", "https://", "file:", "javascript:"] {
        assert!(
            !svg.replace("http://www.w3.org/2000/svg", "")
                .contains(forbidden),
            "SVG contains an external or active reference: {forbidden}"
        );
    }
}

#[test]
fn approved_rectangle_passes_the_fast_pure_rust_svg_visual_gate() {
    let root = repository_root();
    let outcome = render_rectangle_outline_svg();

    let candidate = rasterize_svg(outcome.artifact().bytes());
    let reference = fs::read(root.join(concat!(
        "tests/conformance/svg-viewer-qualification/rectangle-outline/",
        "evidence/pure-rust-reference.rgba"
    )))
    .unwrap();

    assert_eq!(candidate, reference);
}

#[test]
fn rough_operations_are_admitted_atomically_at_the_geometry_boundary() {
    for configured in [16, 17] {
        let limits = ResourceLimits::builder()
            .max_geometry_segments_per_element(configured)
            .max_total_geometry_segments(u64::from(configured))
            .build()
            .expect("geometry limits are valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("resource-limited SVG request is valid");
        Renderer::new()
            .render(&rectangle_outline_scene(), &request)
            .expect("equality and spare Rough-operation capacity pass");
    }

    let limits = ResourceLimits::builder()
        .max_geometry_segments_per_element(15)
        .max_total_geometry_segments(15)
        .build()
        .expect("geometry crossing limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited SVG request is valid");
    let error = Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect_err("the sixteenth Rough operation crosses atomically");

    assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("rectangle-outline"));
    assert_eq!(error.json_pointer(), "/elements/0");
    assert!(error.report().is_empty());
}

#[test]
fn local_rectangle_limits_use_the_exact_available_recovery() {
    let cases = [
        (
            ResourceLimits::builder()
                .max_geometry_segments_per_element(15)
                .build()
                .expect("local geometry limits are valid"),
            "resource.element-complexity-replaced",
            "max_geometry_segments_per_element",
            "placeholder",
        ),
        (
            ResourceLimits::builder()
                .max_element_work_units(639)
                .build()
                .expect("local work limits are valid"),
            "resource.element-work-exceeded",
            "max_element_work_units",
            "placeholder",
        ),
        (
            ResourceLimits::builder()
                .max_coordinate_magnitude(179.0)
                .build()
                .expect("local coordinate limits are valid"),
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            "omitted",
        ),
    ];

    for (limits, expected_code, expected_field, expected_recovery) in cases {
        let outcome = render_approved_svg_with_limits(limits);
        let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
        assert_eq!(
            svg.contains("fill=\"#f1f3f5\""),
            expected_recovery == "placeholder"
        );
        let diagnostic = outcome
            .report()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == expected_code)
            .expect("the local resource diagnostic is retained");
        assert_eq!(diagnostic.code().as_str(), expected_code);
        let descriptor = serde_json::to_value(diagnostic).expect("diagnostic serializes");
        assert_eq!(descriptor["details"]["field"], expected_field);
        assert_eq!(descriptor["details"]["recovery"], expected_recovery);
    }
}

#[test]
fn svg_dimensions_are_checked_at_equality_and_first_crossing() {
    render_approved_svg_with_limits(
        ResourceLimits::builder()
            .max_svg_dimension(200.0)
            .build()
            .expect("SVG dimension equality limit is valid"),
    );

    let limits = ResourceLimits::builder()
        .max_svg_dimension(199.0)
        .build()
        .expect("SVG dimension crossing limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited SVG request is valid");
    let error = Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect_err("logical width crosses the SVG dimension limit");
    assert_eq!(error.code(), ErrorCode::RESOURCE_SVG_DIMENSIONS_EXCEEDED);
    assert!(error.report().is_empty());
}

#[test]
fn rough_path_reduction_uses_ecmascript_half_ties() {
    let source = fs::read_to_string(
        repository_root()
            .join("tests/conformance/cases/focused/rectangle-outline/scene.excalidraw"),
    )
    .unwrap()
    .replacen("\"width\": 180", "\"width\": 179.2695111226207", 1);
    let scene = SceneDocument::parse(source.as_bytes()).expect("tie vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("tie vector renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(
        path_data(svg).starts_with("M0 0 C60.13"),
        "ECMAScript toFixed rounds the exact 60.125 tie upward"
    );
}

fn render_approved_svg_with_limits(limits: ResourceLimits) -> RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited SVG request is valid");
    Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect("approved rectangle reaches a meaningful local recovery")
}

fn parse_svg(bytes: &[u8]) -> &str {
    let svg = std::str::from_utf8(bytes).expect("SVG is UTF-8");
    usvg::Tree::from_str(svg, &usvg::Options::default()).expect("SVG is well-formed and semantic");
    svg
}

fn path_data(svg: &str) -> &str {
    svg.split_once("<path d=\"")
        .and_then(|(_, remainder)| remainder.split_once("\" stroke="))
        .map(|(path, _)| path)
        .expect("SVG contains one semantic path")
}

fn rasterize_svg(bytes: &[u8]) -> Vec<u8> {
    let svg = std::str::from_utf8(bytes).expect("SVG is UTF-8");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .expect("candidate SVG is accepted by the local fast viewer");
    let mut pixmap = Pixmap::new(200, 120).expect("focused raster dimensions are valid");
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    pixmap.data().to_vec()
}
