//! A build without the `png` or `pdf` feature keeps the whole request API, but
//! refuses to build a request for the format it leaves out (ADR-0023). This
//! module compiles only in such a build.

use excalidraw_render::{
    DiagnosticDetails, ErrorCode, ErrorFamily, FrameRenderRequest, OutputFormat, PdfOptions,
    RenderRequest, Renderer, RequestError, SceneDocument, Sizing,
};

const SCENE: &[u8] = br#"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"frame","type":"frame","x":0,"y":0,"width":40,"height":30,"updated":1},
        {"id":"box","type":"rectangle","x":5,"y":5,"width":20,"height":10,"frameId":"frame","roughness":0,"updated":1}
    ],
    "appState":{},"files":{}
}"#;

/// `error` is the disabled-format failure for `format`, naming its feature.
fn assert_format_unavailable(error: &RequestError, format: OutputFormat, feature: &str) {
    assert_eq!(error.code(), ErrorCode::REQUEST_FORMAT_UNAVAILABLE);
    assert_eq!(error.code().as_str(), "request.format-unavailable");
    assert_eq!(
        error.to_string(),
        format!(
            "{} output is not enabled in this build; enable the `{feature}` feature of \
             excalidraw-render",
            format.as_str().to_ascii_uppercase()
        )
    );
    assert_eq!(error.field(), "format");
    assert_eq!(error.asset_id(), None);
    assert_eq!(error.option(), None);
    assert!(matches!(
        error.details(),
        Some(DiagnosticDetails::RequestValue {
            field: "format",
            asset_id: None,
            option: None,
            ..
        })
    ));
    assert_eq!(error.descriptor().family(), ErrorFamily::Request);
    assert_eq!(error.descriptor().details(), error.details());
}

/// Every way to ask for `format` fails at request construction, before the
/// request's other values are validated, for scenes and frames alike.
fn assert_requests_fail_to_build(format: OutputFormat, feature: &str) {
    let builders = [
        RenderRequest::builder(format),
        RenderRequest::builder(format).padding(-1.0),
        RenderRequest::builder(format).sizing(Sizing::Scale(0.0)),
        RenderRequest::builder(format).pdf_options(PdfOptions::default()),
    ];
    for builder in builders {
        let error = builder
            .build()
            .expect_err("a disabled format does not build");
        assert_format_unavailable(&error, format, feature);
    }
    let error = FrameRenderRequest::builder(format)
        .sizing(Sizing::MaxDimension(f64::NAN))
        .build()
        .expect_err("a disabled format does not build a frame request");
    assert_format_unavailable(&error, format, feature);
}

#[cfg(not(feature = "png"))]
#[test]
fn png_requests_fail_to_build_without_the_png_feature() {
    assert_requests_fail_to_build(OutputFormat::Png, "png");
    let error = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect_err("PNG is disabled");
    assert_eq!(
        serde_json::to_string(&error.descriptor()).expect("serialize request descriptor"),
        concat!(
            "{\"family\":\"request\",",
            "\"category\":null,",
            "\"code\":\"request.format-unavailable\",",
            "\"message\":\"PNG output is not enabled in this build; ",
            "enable the `png` feature of excalidraw-render\",",
            "\"format\":null,",
            "\"element_id\":null,",
            "\"json_pointer\":null,",
            "\"details\":{\"kind\":\"request_value\",",
            "\"field\":\"format\",",
            "\"asset_id\":null,",
            "\"option\":null}}"
        )
    );
}

#[cfg(not(feature = "pdf"))]
#[test]
fn pdf_requests_fail_to_build_without_the_pdf_feature() {
    assert_requests_fail_to_build(OutputFormat::Pdf, "pdf");
}

#[test]
fn svg_scenes_and_frame_batches_render_without_the_other_formats() {
    let scene = SceneDocument::parse(SCENE).expect("scene is admitted");
    let renderer = Renderer::new();
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG is in every build");
    let outcome = renderer.render(&scene, &request).expect("SVG renders");
    assert!(outcome.artifact().bytes().starts_with(b"<svg"));

    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG frame requests are in every build");
    let mut cursor = renderer
        .render_frames(&scene, scene.frames(), &request)
        .expect("the batch starts");
    let entry = cursor.next().expect("one frame").into_result();
    assert!(entry
        .expect("the frame renders")
        .artifact()
        .bytes()
        .starts_with(b"<svg"));
    assert!(cursor.next().is_none());
}
