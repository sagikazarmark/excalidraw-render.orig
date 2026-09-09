use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

const SIMULATED_PRESSURE_SCENE: &[u8] = br##"{
  "type":"excalidraw","version":2,
  "elements":[{
    "id":"simulated","type":"freedraw","x":10,"y":20,"width":80,"height":30,
    "points":[[0,0],[20,10],[40,5],[60,30],[80,20]],
    "pressures":[],"simulatePressure":true,"strokeColor":"#e03131",
    "strokeWidth":2,"opacity":80,"updated":1
  }],
  "appState":{},"files":{}
}"##;

#[test]
fn simulated_pressure_freedraw_renders_directly_in_every_format() {
    let scene = SceneDocument::parse(SIMULATED_PRESSURE_SCENE).unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let first = Renderer::new().render(&scene, &request).unwrap();

        assert!(first.report().is_empty(), "unexpected {format:?} fallback");
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).unwrap();
                assert!(!svg.contains("Unavailable:"));
                assert!(svg.contains("viewBox=\"0 0 100 50\""));
                assert!(svg.contains("transform=\"translate(10 10) rotate(0 40 15)\""));
                let path = svg
                    .split_once("<path d=\"")
                    .and_then(|(_, tail)| tail.split_once("\" fill="))
                    .unwrap()
                    .0;
                assert_eq!(
                    format!("{:x}", Sha256::digest(path)),
                    "9adff0c560710203718626bc817520baba2819a06898054e0b55d52a7cccd41e",
                    "path vector comes from perfect-freehand 1.2.0 and Excalidraw v0.18.1"
                );
                assert!(svg.contains("fill=\"#e03131\" fill-opacity=\"0.8\""));
            }
            OutputFormat::Png => {
                assert!(first.artifact().bytes().starts_with(b"\x89PNG\r\n\x1a\n"))
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(first.artifact().bytes());
                assert!(
                    pdf.contains("\nf\nQ\n"),
                    "freedraw remains native vector fill"
                );
                assert!(!pdf.contains("/Subtype /Image"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn freedraw_geometry_limit_uses_the_exact_generated_command_count() {
    let scene = SceneDocument::parse(SIMULATED_PRESSURE_SCENE).unwrap();
    let request = |maximum| {
        RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(
                ResourceLimits::builder()
                    .max_geometry_segments_per_element(maximum)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    };

    Renderer::new()
        .render(&scene, &request(55))
        .expect("the exact move, quadratic, line, and close count is admitted");
    let outcome = Renderer::new().render(&scene, &request(54)).unwrap();
    let diagnostic = outcome
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
        })
        .unwrap();
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
    );
    assert_eq!(diagnostic.element_id(), Some("simulated"));
}

#[test]
fn closed_freedraw_paints_rough_background_before_the_pressure_stroke() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"loop","type":"freedraw","width":40,"height":40,"points":[[0,0],[40,0],[40,40],[0,40],[0,0]],"pressures":[],"simulatePressure":true,"strokeColor":"#1971c2","backgroundColor":"#ffec99","fillStyle":"solid","strokeWidth":2,"roughness":1,"seed":23,"updated":1}],"appState":{},"files":{}}"##,
    )
    .unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        assert!(outcome.report().is_empty());
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                assert_eq!(svg.matches("<path d=\"").count(), 2);
                assert!(
                    svg.find("fill=\"#ffec99\"").unwrap() < svg.find("fill=\"#1971c2\"").unwrap()
                );
            }
            OutputFormat::Png => {
                assert!(outcome.artifact().bytes().starts_with(b"\x89PNG\r\n\x1a\n"))
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                assert!(pdf.contains("f*\nQ\n"));
                assert!(pdf.contains("h\nf\nQ\n"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn empty_simulated_pressure_stroke_is_a_direct_noop() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"empty","type":"freedraw","points":[],"pressures":[],"simulatePressure":true,"updated":1}],"appState":{},"files":{}}"##,
    )
    .unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        assert!(outcome.report().is_empty());
        assert!(!outcome
            .artifact()
            .bytes()
            .windows("Unavailable:".len())
            .any(|window| window == b"Unavailable:"));
    }
}

#[test]
fn zero_width_freedraw_recovery_succeeds_in_every_format() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"zero","type":"freedraw","width":40,"height":10,"points":[[0,0],[20,10],[40,5]],"pressures":[],"simulatePressure":true,"strokeWidth":0,"updated":1}],"appState":{},"files":{}}"##,
    )
    .unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        Renderer::new().render(&scene, &request).unwrap();
    }
}

#[test]
fn invalid_real_pressure_samples_recover_without_disabling_the_stroke() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"invalid-pressure","type":"freedraw","width":40,"height":10,"points":[[0,0],[20,10],[40,5]],"pressures":[0.2,2,0.5],"simulatePressure":false,"strokeWidth":2,"updated":1}],"appState":{},"files":{}}"##,
    )
    .unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        assert!(outcome
            .report()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.json_pointer() == "/elements/0/pressures"));
        assert!(!outcome
            .artifact()
            .bytes()
            .windows("Unavailable:".len())
            .any(|window| window == b"Unavailable:"));
    }
}

#[test]
fn rotated_freedraw_bounds_follow_source_points_not_their_bounding_box_corners() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"rotated","type":"freedraw","x":10,"y":20,"width":100,"height":20,"angle":1.5707963267948966,"points":[[0,0],[100,0],[0,20]],"pressures":[],"simulatePressure":true,"strokeWidth":2,"updated":1}],"appState":{},"files":{}}"##,
    )
    .unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    assert!(svg.contains("viewBox=\"0 0 40 120\""), "{svg}");
    assert!(svg.contains("transform=\"translate(-30 50) rotate(90 50 10)\""));

    let limited_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_geometry_segments_per_element(1)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    let limited = Renderer::new().render(&scene, &limited_request).unwrap();
    let limited_svg = std::str::from_utf8(limited.artifact().bytes()).unwrap();
    assert!(limited_svg.contains("viewBox=\"0 0 40 120\""));
}
