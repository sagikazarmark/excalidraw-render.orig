//! Rendering a scene the way most tests want it: the default request for one
//! format, a panic naming the format when the render is refused. A test with a
//! non-default request builds it and hands it to [`render_with_request`].

use excalidraw_render::{OutputFormat, RenderOutcome, RenderRequest, Renderer, SceneDocument};

/// Renders `scene` with the default request for `format`.
pub fn render(scene: &SceneDocument, format: OutputFormat) -> RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("default request is valid");
    render_with_request(scene, &request)
}

/// Parses `source` and renders it with the default request for `format`.
pub fn render_source(source: &[u8], format: OutputFormat) -> RenderOutcome {
    let scene = SceneDocument::parse(source).expect("scene is admitted");
    render(&scene, format)
}

/// Renders `scene` with the default request for `format` minus the background
/// fill, so unpainted pixels stay transparent.
pub fn render_without_background(scene: &SceneDocument, format: OutputFormat) -> RenderOutcome {
    let request = RenderRequest::builder(format)
        .include_background(false)
        .build()
        .expect("transparent request is valid");
    render_with_request(scene, &request)
}

/// Renders `scene` with `request`, refusing to continue when the render fails.
pub fn render_with_request(scene: &SceneDocument, request: &RenderRequest) -> RenderOutcome {
    Renderer::new()
        .render(scene, request)
        .unwrap_or_else(|error| panic!("{:?} render succeeds: {error:?}", request.format()))
}

/// The artifact bytes of a default render of `scene` that raised no diagnostic.
pub fn render_artifact(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    artifact_without_diagnostics(&render(scene, format))
}

/// The artifact bytes of a background-free render of `scene` that raised no
/// diagnostic.
pub fn render_artifact_without_background(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    artifact_without_diagnostics(&render_without_background(scene, format))
}

/// The artifact bytes of an outcome that raised no diagnostic.
pub fn artifact_without_diagnostics(outcome: &RenderOutcome) -> Vec<u8> {
    assert!(
        outcome.report().is_empty(),
        "unexpected diagnostics: {:?}",
        outcome.report().diagnostics()
    );
    outcome.artifact().bytes().to_vec()
}
