//! The approved inputs that more than one tier renders, each rendered the way
//! its approval was recorded: the rectangle-outline focused case and the PDF
//! backend qualification scenes. The render tier inspects these outcomes for
//! structure and determinism; the `harness_*` binaries hand the same outcomes
//! to the pinned viewers. One definition keeps the two tiers looking at the
//! same artifact.

use std::fs;

use excalidraw_render::{
    OutputFormat, PdfOptions, PdfRasterFallback, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};

use super::repository_root;

/// The complete PDF operation matrix of the PDF backend qualification.
pub const PROJECT_OPERATIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/project-operations.excalidraw"
));

/// The nested-SVG native operation matrix of the PDF backend qualification.
pub const NESTED_NATIVE_OPERATIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/nested-native-operations.excalidraw"
));

/// One nested SVG whose blur filter has no PDF-native form, so its closure is
/// the one place the PDF backend may fall back to raster.
pub const ISOLATED_FALLBACK: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGZpbHRlciBpZD0iYmx1ciIgeD0iMCIgeT0iMCIgd2lkdGg9IjEwMCUiIGhlaWdodD0iMTAwJSI+PGZlR2F1c3NpYW5CbHVyIHN0ZERldmlhdGlvbj0iMSIvPjwvZmlsdGVyPjwvZGVmcz48cmVjdCB3aWR0aD0iMjAiIGhlaWdodD0iMTAiIGZpbGw9IiMwMGZmMDAiIGZpbHRlcj0idXJsKCNibHVyKSIvPjwvc3ZnPg==","created":1}}}"##;

/// The approved rectangle-outline scene (`focused.rectangle-outline.v1`).
pub fn rectangle_outline_scene() -> SceneDocument {
    SceneDocument::parse(
        &fs::read(
            repository_root()
                .join("tests/conformance/cases/focused/rectangle-outline/scene.excalidraw"),
        )
        .expect("approved rectangle scene is present"),
    )
    .expect("approved rectangle scene is admitted")
}

/// The approved rectangle as the default SVG export.
pub fn render_rectangle_outline_svg() -> RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default SVG request is valid");
    Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect("approved rectangle renders")
}

/// The approved rectangle as a PDF that may not fall back to raster.
pub fn render_rectangle_outline_pdf() -> RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("default PDF request is valid");
    Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect("approved rectangle renders directly as PDF vectors")
}

/// The complete project operation matrix as a PDF that may not fall back.
pub fn render_project_operations() -> RenderOutcome {
    render_without_fallback(PROJECT_OPERATIONS, "project operation matrix")
        .expect("the complete project operation matrix renders")
}

/// The nested-SVG native operation matrix as a PDF that may not fall back.
pub fn render_nested_native_operations() -> RenderOutcome {
    render_without_fallback(NESTED_NATIVE_OPERATIONS, "nested native operation matrix")
        .expect("every nested native family renders without fallback")
}

/// The isolated fallback scene as a default PDF, which falls back for exactly
/// the nested blur closure.
pub fn render_isolated_fallback() -> RenderOutcome {
    let scene = SceneDocument::parse(ISOLATED_FALLBACK).expect("fallback scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("fallback request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("the isolated fallback renders")
}

/// Renders one qualification scene, named for the failure message, as a PDF
/// that may not fall back to raster.
fn render_without_fallback(
    scene: &[u8],
    name: &str,
) -> Result<RenderOutcome, excalidraw_render::RenderError> {
    let scene = SceneDocument::parse(scene)
        .unwrap_or_else(|error| panic!("the {name} qualification scene is admitted: {error:?}"));
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("qualification request is valid");
    Renderer::new().render(&scene, &request)
}
