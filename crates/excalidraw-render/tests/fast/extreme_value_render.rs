//! Finite but extreme scene values at the default limits: each one renders or
//! fails with a typed resource error, never a panic, an encoder failure, or a
//! memory blow-up. The render fuzzers run with lowered limits, so they do not
//! reach these magnitudes.
use base64::Engine as _;
use excalidraw_render::{
    ErrorCode, OutputFormat, RenderError, RenderOutcome, RenderRequest, Renderer, SceneDocument,
    Sizing,
};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn scene(elements: &str, files: &str) -> SceneDocument {
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{elements}],"appState":{{}},"files":{{{files}}}}}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("scene parses")
}

fn render(scene: &SceneDocument, format: OutputFormat) -> Result<RenderOutcome, RenderError> {
    let request = RenderRequest::builder(format).build().expect("request");
    Renderer::new().render(scene, &request)
}

fn rectangle(extra: &str) -> String {
    format!(
        r#"{{"id":"r","type":"rectangle","x":0,"y":0,"width":10,"height":10,"seed":1,"updated":1{extra}}}"#
    )
}

#[test]
fn a_stroke_far_wider_than_the_canvas_renders() {
    // tiny-skia's scan converter panicked on this stroke's outline, which with
    // `panic = "abort"` ended the process.
    let scene = scene(&rectangle(r#","strokeWidth":9.9e8"#), "");
    for format in FORMATS {
        render(&scene, format).unwrap_or_else(|error| panic!("{format:?}: {error}"));
    }
    for scale in [2.0, 4.0, 10.0] {
        let request = RenderRequest::builder(OutputFormat::Png)
            .sizing(Sizing::Scale(scale))
            .build()
            .expect("request");
        Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("scale {scale}: {error}"));
    }
}

#[test]
fn a_nested_svg_stroke_past_the_raster_range_is_a_png_failure() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="10" y="10" width="50" height="50" fill="none" stroke="red" stroke-width="9.9e8"/></svg>"#;
    let encoded = base64::engine::general_purpose::STANDARD.encode(svg);
    let scene = scene(
        r#"{"id":"img","type":"image","x":0,"y":0,"width":100,"height":100,"seed":1,"updated":1,"fileId":"f","status":"saved"}"#,
        &format!(
            r#""f":{{"id":"f","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}"#
        ),
    );
    let error = render(&scene, OutputFormat::Png).expect_err("PNG refuses the nested SVG");
    assert_eq!(error.code(), ErrorCode::ENCODE_PNG_FAILED, "{error}");
    for format in [OutputFormat::Svg, OutputFormat::Pdf] {
        render(&scene, format).unwrap_or_else(|error| panic!("{format:?}: {error}"));
    }
}

#[test]
fn degenerate_geometry_draws_nothing_instead_of_failing() {
    for extra in [
        // An overflowing rotation in degrees.
        r#","angle":1e308"#,
        // A width that narrows to zero in f32 strokes as a hairline.
        r#","strokeWidth":1e-308"#,
    ] {
        let scene = scene(&rectangle(extra), "");
        for format in FORMATS {
            render(&scene, format).unwrap_or_else(|error| panic!("{extra} {format:?}: {error}"));
        }
    }
    // A dots fill that places no mark in a sliver of an ellipse.
    let sliver = scene(
        r##"{"id":"e","type":"ellipse","x":0,"y":0,"width":90,"height":0.00006,"seed":1535104,"updated":1,"fillStyle":"dots","backgroundColor":"#ffc9c9","strokeWidth":4}"##,
        "",
    );
    for format in FORMATS {
        render(&sliver, format).unwrap_or_else(|error| panic!("{format:?}: {error}"));
    }
}

#[test]
fn an_overflowing_scale_is_a_resource_crossing() {
    let scene = scene(&rectangle(""), "");
    for (format, code) in [
        (
            OutputFormat::Svg,
            ErrorCode::RESOURCE_SVG_DIMENSIONS_EXCEEDED,
        ),
        (
            OutputFormat::Png,
            ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED,
        ),
        (
            OutputFormat::Pdf,
            ErrorCode::RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED,
        ),
    ] {
        let request = RenderRequest::builder(format)
            .sizing(Sizing::Scale(1e308))
            .build()
            .expect("request");
        let error = Renderer::new()
            .render(&scene, &request)
            .expect_err("the scale is rejected");
        assert_eq!(error.code(), code, "{format:?}: {error}");
    }
}

#[test]
fn a_saturated_png_surface_reports_a_lower_bound() {
    // Both sides saturate at u32::MAX; their product is not the attempted area,
    // so the crossing is the checked-overflow lower bound, limit + 1.
    let scene = scene(&rectangle(""), "");
    let request = RenderRequest::builder(OutputFormat::Png)
        .sizing(Sizing::Scale(1e300))
        .build()
        .expect("request");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the scale is rejected");
    assert_eq!(error.code(), ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED);
    let details = serde_json::to_value(error.details()).expect("details serialize");
    assert_eq!(
        details["measurement"],
        serde_json::json!({"kind": "at_least", "value": 33_554_433})
    );
    assert!(
        error.to_string().contains("needs at least 33554433 pixels"),
        "{error}"
    );
}

#[test]
fn a_sizing_that_rounds_the_output_to_nothing_says_so() {
    let scene = scene(&rectangle(""), "");
    for (format, sizing, code, message) in [
        (
            OutputFormat::Png,
            Sizing::Scale(1e-300),
            ErrorCode::ENCODE_PNG_FAILED,
            "PNG width rounds to zero pixels at the requested size",
        ),
        // A maximum dimension under one pixel: 30 x 30 logical becomes 0.5 x 0.5.
        (
            OutputFormat::Png,
            Sizing::MaxDimension(0.5),
            ErrorCode::ENCODE_PNG_FAILED,
            "PNG width rounds to zero pixels at the requested size",
        ),
        (
            OutputFormat::Pdf,
            Sizing::Scale(1e-300),
            ErrorCode::ENCODE_PDF_FAILED,
            "PDF page rounds to zero points at the requested size",
        ),
    ] {
        let request = RenderRequest::builder(format)
            .sizing(sizing)
            .build()
            .expect("the request is valid; the scene decides the size");
        let error = Renderer::new()
            .render(&scene, &request)
            .expect_err("the artifact has no size");
        assert_eq!(error.code(), code, "{sizing:?}: {error}");
        assert_eq!(error.descriptor().message(), message, "{sizing:?}");
    }
}

fn dragons(id: &str, y: u32, count: usize) -> String {
    let text = "🐉".repeat(count);
    format!(
        r#"{{"id":"{id}","type":"text","x":0,"y":{y},"width":100,"height":25,"text":"{text}","originalText":"{text}","fontSize":20,"fontFamily":5,"updated":1}}"#
    )
}

#[test]
fn text_over_the_geometry_limit_fails_without_holding_its_outlines() {
    // 40,000 detailed emoji outlines held about 2.4 GB before the geometry
    // limit was checked; the count, and so the error, is unchanged.
    let scene = scene(&dragons("t", 0, 40_000), "");
    let error = render(&scene, OutputFormat::Svg).expect_err("over the geometry limit");
    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED,
        "{error}"
    );
    assert_eq!(error.element_id(), Some("t"));
    assert!(error.to_string().contains("needs 19280000"), "{error}");
}

#[test]
fn many_texts_under_the_element_limit_stop_at_the_total_limit() {
    // Each element stays under the per-element limit; together they held
    // about 5 GB of outlines before accounting saw the first of them.
    let elements = (0..400)
        .map(|index| dragons(&format!("t{index}"), index * 30, 500))
        .collect::<Vec<_>>()
        .join(",");
    let scene = scene(&elements, "");
    for format in FORMATS {
        let error = render(&scene, format).expect_err("over the total geometry limit");
        assert_eq!(
            error.code(),
            ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED,
            "{format:?}: {error}"
        );
        assert_eq!(error.element_id(), Some("t17"), "{format:?}");
    }
}
