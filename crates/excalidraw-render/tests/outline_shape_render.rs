use std::io::Cursor;

use excalidraw_render::{
    DiagnosticCode, ErrorCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use png::{ColorType, Decoder, Transformations};
use sha2::{Digest, Sha256};

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
        let expected_digest = match format {
            OutputFormat::Svg => "92cb399bdb9891670afc384c84ef686513467c08acf93164bbb0ae838d4d82ca",
            OutputFormat::Png => "5963335877cfde4ec0079ee163a4c29c2e170e44dcbfff7965f4f89bd0019d6b",
            OutputFormat::Pdf => "58ce5d784ad2594d87b9000b1cb84248f3b40e843c51d624536c8e002dbac88e",
            _ => unreachable!("the test enumerates every revision-1 output format"),
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
            expected_digest,
            "exact {format:?} focused-case artifact changed"
        );

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
                let mut decoder = Decoder::new(Cursor::new(outcome.artifact().bytes()));
                decoder.set_transformations(Transformations::IDENTITY);
                let mut reader = decoder.read_info().unwrap();
                assert_eq!(reader.info().color_type, ColorType::Rgba);
                let mut rgba = vec![0; reader.output_buffer_size()];
                let frame = reader.next_frame(&mut rgba).unwrap();
                assert!(rgba[..frame.buffer_size()]
                    .chunks_exact(4)
                    .any(|pixel| pixel != [255, 255, 255, 255]));
            }
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
                assert_eq!(pdf.matches("\nS\nQ\n").count(), 7);
                assert!(!pdf.contains("Unavailable:"));
            }
            _ => unreachable!("the test enumerates every revision-1 output format"),
        }
    }
}

#[test]
fn svg_outline_operations_match_the_pinned_roughjs_vectors() {
    let scene = SceneDocument::parse(SCENE).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let paths: Vec<_> = svg
        .split("<path d=\"")
        .skip(1)
        .map(|tail| tail.split_once("\" stroke=").unwrap().0)
        .collect();

    assert_eq!(paths.len(), 7);
    assert_eq!(
        paths
            .iter()
            .map(|path| path.matches(['M', 'C']).count())
            .collect::<Vec<_>>(),
        [16, 32, 24, 16, 32, 8, 6]
    );
    let digest = Sha256::digest(paths.join("\n"));
    assert_eq!(
        format!("{digest:x}"),
        "6d0e81bef0d09500efd1e8f97e78d2a998331d4a4e7683d0adbf596a695fbe80",
        // The original vectors came from an independent pinned Rough.js 4.6.4 run,
        // but that run was fed midpoint diamond vertices, which is not what the
        // official export uses. The two diamond entries were re-derived after the
        // vertex fix; their geometry is corroborated by official evidence at two
        // sizes of differing parity, 170x110 and 130x95, where every path matches
        // byte for byte. The five non-diamond entries are unchanged from the
        // independent run.
        "diamond vectors re-derived against official evidence; the rest are the \
         independent pinned Rough.js 4.6.4 run"
    );
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

    Renderer::new()
        .render(&scene, &request(134))
        .expect("the exact total Rough operation count is admitted");
    let error = Renderer::new()
        .render(&scene, &request(133))
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
fn architect_ellipse_uses_the_single_core_roughjs_stroke() {
    let source = std::str::from_utf8(SCENE).unwrap().replacen(
        "\"seed\":3,\"updated\"",
        "\"roughness\":0,\"seed\":3,\"updated\"",
        1,
    );
    let scene = SceneDocument::parse(source.as_bytes()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let ellipse = svg
        .split("<path d=\"")
        .nth(3)
        .and_then(|tail| tail.split_once("\" stroke="))
        .unwrap()
        .0;

    assert_eq!(ellipse.matches(['M', 'C']).count(), 42);
    assert_eq!(
        format!("{:x}", Sha256::digest(ellipse)),
        "d74c0380e58abcacfb14e495406d5f38af852f32f37a0ea528ecb0f0454bec05"
    );
}

#[test]
fn cartoonist_outline_consumes_vertex_offsets_in_roughjs_order() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"cartoonist","type":"rectangle","width":8,"height":5,
            "roughness":2,"seed":8,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let path = svg
        .split_once("<path d=\"")
        .and_then(|(_, tail)| tail.split_once("\" stroke="))
        .unwrap()
        .0;

    assert_eq!(
        format!("{:x}", Sha256::digest(path)),
        "fe33b396a5e457111305917fe234efdc6f856b635e7a55ab7fc5e5ed3083c19b"
    );
}

#[test]
fn cartoonist_rounded_outline_preserves_continuous_path_vertices() {
    let source = std::str::from_utf8(SCENE).unwrap().replacen(
        "\"seed\":2,\"updated\"",
        "\"roughness\":2,\"seed\":2,\"updated\"",
        1,
    );
    let scene = SceneDocument::parse(source.as_bytes()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let rounded_rectangle = svg
        .split("<path d=\"")
        .nth(2)
        .and_then(|tail| tail.split_once("\" stroke="))
        .unwrap()
        .0;

    assert_eq!(
        format!("{:x}", Sha256::digest(rounded_rectangle)),
        "e9be3be60300e30a0f37ea1c58c8a7b18ddf966d468a0afb6629b3fc5f81aa7f"
    );
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
fn ellipse_ignores_inapplicable_roundness_without_consuming_randomness() {
    let rounded_source = std::str::from_utf8(SCENE).unwrap().replacen(
        "\"seed\":3,\"updated\"",
        "\"roundness\":{\"type\":2},\"seed\":3,\"updated\"",
        1,
    );
    let plain = SceneDocument::parse(SCENE).unwrap();
    let rounded = SceneDocument::parse(rounded_source.as_bytes()).unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let plain = Renderer::new().render(&plain, &request).unwrap();
        let rounded = Renderer::new().render(&rounded, &request).unwrap();
        assert_eq!(plain.artifact().bytes(), rounded.artifact().bytes());
        assert_eq!(plain.report(), rounded.report());
    }
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
