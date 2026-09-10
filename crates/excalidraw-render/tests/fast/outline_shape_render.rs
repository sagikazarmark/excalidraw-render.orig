use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{
    DiagnosticCode, ErrorCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

use crate::common::raster::decode_png;
use crate::common::svg::path_data;

const SCENE: &[u8] = br##"{
  "type":"excalidraw","version":2,
  "elements":[
    {"id":"rectangle","type":"rectangle","x":10,"y":10,"width":80,"height":50,"seed":1,"updated":1},
    {"id":"rounded-rectangle","type":"rectangle","x":110,"y":10,"width":80,"height":50,"roundness":{"type":3},"seed":2,"updated":1},
    {"id":"ellipse","type":"ellipse","x":210,"y":10,"width":80,"height":50,"seed":3,"updated":1},
    {"id":"diamond","type":"diamond","x":310,"y":10,"width":80,"height":50,"seed":4,"updated":1},
    {"id":"rounded-diamond","type":"diamond","x":410,"y":10,"width":80,"height":50,"roundness":{"type":2},"seed":5,"updated":1},
    {"id":"line","type":"line","x":10,"y":90,"width":160,"height":40,"points":[[0,0],[80,40],[160,10]],"seed":6,"updated":1},
    {"id":"rounded-line","type":"line","x":210,"y":90,"width":160,"height":40,"points":[[0,0],[80,40],[160,10]],"roundness":{"type":2},"seed":7,"updated":1}
  ],
  "appState":{"viewBackgroundColor":"#ffffff"},"files":{}
}"##;

#[test]
fn admitted_outline_shape_families_render_directly_in_every_format() {
    let scene = SceneDocument::parse(SCENE).expect("outline family scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("default render request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("all outline families render");
        // The SVG's one pin is the path-vector digest below, which holds the
        // geometry; PNG and PDF each keep one whole-artifact digest. Both
        // moved with the rounded diamond's corner radii (#160), which now come
        // from the vertex offsets `floor(extent / 2) + 1` as the reference's
        // do: the 80x50 rounded diamond's radii are 10.25 / 6.5, not 10 / 6.25.
        let expected_digest = match format {
            OutputFormat::Svg => None,
            OutputFormat::Png => {
                Some("267d2f7b19997dbc9df1e0b5643a3bfeccfd317b78914428db09ab5a9064f170")
            }
            OutputFormat::Pdf => {
                Some("e05b4e14cfc0cb2b7f2290632a247d52511188b0d367e95e0d982dda04bc72c7")
            }
            _ => unreachable!("the test enumerates every revision-1 output format"),
        };
        if let Some(expected_digest) = expected_digest {
            assert_eq!(
                format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
                expected_digest,
                "exact {format:?} focused-case artifact changed"
            );
        }

        assert!(
            outcome.report().is_empty(),
            "direct outline rendering must not use compatibility fallbacks for {format:?}: {:?}",
            outcome.report().diagnostics()
        );
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                assert_eq!(svg.matches("<path ").count(), 7);
                assert!(!svg.contains("Unavailable:"));
            }
            OutputFormat::Png => {
                assert!(decode_png(outcome.artifact().bytes())
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| *pixel != [255, 255, 255, 255]));
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                assert_eq!(pdf.matches("\nS\nQ\n").count(), 7);
                assert!(!pdf.contains("Unavailable:"));
            }
            _ => unreachable!("the test enumerates every revision-1 output format"),
        }
    }
}

#[test]
fn outline_geometry_accounting_uses_each_generated_operation_count() {
    let scene = SceneDocument::parse(SCENE).unwrap();
    let request = |maximum| {
        RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(
                ResourceLimits::builder()
                    .max_geometry_segments_per_element(32)
                    .max_total_geometry_segments(maximum)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    };

    // The scene's total geometry is the sum of every generated path's
    // operations, read off the unconstrained render rather than pinned.
    let unconstrained = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Svg).build().unwrap(),
        )
        .unwrap();
    let total_operations: u64 =
        path_data(std::str::from_utf8(unconstrained.artifact().bytes()).unwrap())
            .iter()
            .map(|path| path.matches(['M', 'C']).count() as u64)
            .sum();

    Renderer::new()
        .render(&scene, &request(total_operations))
        .expect("the exact total Rough operation count is admitted");
    let error = Renderer::new()
        .render(&scene, &request(total_operations - 1))
        .expect_err("the first operation beyond the aggregate limit is rejected");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("rounded-line"));

    let local_limits = ResourceLimits::builder()
        .max_geometry_segments_per_element(31)
        .build()
        .unwrap();
    let local_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(local_limits)
        .build()
        .unwrap();
    let local = Renderer::new().render(&scene, &local_request).unwrap();
    let diagnostic = local
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.element_id() == Some("rounded-rectangle")
                && diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
        })
        .expect("the first local operation crossing is diagnosed");
    let descriptor = serde_json::to_value(diagnostic).unwrap();
    assert_eq!(descriptor["details"]["configured"], 31);
    assert_eq!(descriptor["details"]["measurement"]["value"], 32);
}

#[test]
fn negative_linear_points_define_bounds_and_rotation_center() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"negative-line","type":"line","x":100,"y":10,
            "width":80,"height":40,"angle":1.5707963267948966,
            "points":[[0,0],[-80,40]],"seed":9,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();

    assert!(
        svg.contains("viewBox=\"0 0 60 100\""),
        "unexpected bounds: {svg}"
    );
    assert!(
        svg.contains("transform=\"translate(70 30) rotate(90 -40 20)\""),
        "unexpected linear transform: {svg}"
    );
}

#[test]
fn huge_finite_pattern_generation_recovers_locally_in_every_format() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"tall-hachure","type":"rectangle","x":0,"y":0,
            "width":100,"height":2000,"backgroundColor":"#e03131",
            "fillStyle":"hachure","strokeWidth":0.025,"seed":71,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let limits = ResourceLimits::builder()
        .max_element_work_units(1_000)
        .build()
        .unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        let diagnostic = outcome
            .report()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED)
            .expect("the local work crossing is diagnosed");
        assert_eq!(diagnostic.element_id(), Some("tall-hachure"));
    }
}

#[test]
fn patterned_coordinate_crossing_recovers_before_generation_in_every_format() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"wide-hachure","type":"rectangle","x":0,"y":0,
            "width":100,"height":50,"backgroundColor":"#e03131",
            "fillStyle":"hachure","seed":73,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let limits = ResourceLimits::builder()
        .max_coordinate_magnitude(99.0)
        .build()
        .unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        let diagnostics = outcome.report().diagnostics();
        assert_eq!(diagnostics.len(), 1, "unexpected {format:?} report");
        assert_eq!(
            diagnostics[0].code(),
            DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED
        );
        assert_eq!(diagnostics[0].json_pointer(), "/elements/0/width");
    }
}

#[test]
fn pathological_curve_subdivision_recovers_locally_in_every_format() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"subdivision","type":"line","x":0,"y":0,
            "width":100,"height":100,"points":[[0,0],[200,200],[-200,200],
            [200,-200],[0,0]],"roundness":{"type":2},
            "backgroundColor":"#e03131","fillStyle":"hachure","seed":72,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let limits = ResourceLimits::builder()
        .max_coordinate_magnitude(400.0)
        .max_element_work_units(1_000)
        .build()
        .unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        let diagnostic = outcome
            .report()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED)
            .expect("the local work crossing is diagnosed");
        assert_eq!(diagnostic.element_id(), Some("subdivision"));
    }
}
