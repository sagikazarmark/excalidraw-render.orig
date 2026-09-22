//! Long PDF sketch fills are split across stroke operations so PDFium's
//! rasterizer does not silently drop the geometry drawn last (#84). A
//! cross-hatch of several thousand long lines in one path loses its second
//! direction in the pinned PDFium viewer; operations below the length budget
//! render completely.

use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};

use crate::common::pdf_inspect::pdf_text;

fn cross_hatch_scene(opacity: u32) -> SceneDocument {
    let scene = format!(
        r##"{{"type":"excalidraw","version":2,"source":"excalidraw-render tests","elements":[
{{"id":"hatch","type":"rectangle","x":0,"y":0,"width":8000,"height":8000,"angle":0,
"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","fillStyle":"cross-hatch","strokeWidth":4,
"strokeStyle":"solid","roughness":1,"opacity":{opacity},"groupIds":[],"frameId":null,"index":"a0",
"roundness":null,"seed":84,"version":1,"versionNonce":84,"isDeleted":false,"boundElements":null,
"updated":1,"link":null,"locked":false}}],"appState":{{"viewBackgroundColor":"#ffffff"}},"files":{{}}}}"##
    );
    SceneDocument::parse(scene.as_bytes()).expect("cross-hatch scene is admitted")
}

/// Stroke operations and subpaths in the page content.
fn strokes_and_subpaths(opacity: u32) -> (usize, usize) {
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("default PDF request is valid");
    let outcome = Renderer::new()
        .render(&cross_hatch_scene(opacity), &request)
        .expect("cross-hatch scene renders as PDF");
    let pdf = pdf_text(outcome.artifact().bytes());
    (
        pdf.lines().filter(|line| *line == "S").count(),
        pdf.lines().filter(|line| line.ends_with(" m")).count(),
    )
}

#[test]
fn opaque_long_sketch_fills_split_without_losing_geometry() {
    let (opaque_strokes, opaque_subpaths) = strokes_and_subpaths(100);
    let (translucent_strokes, translucent_subpaths) = strokes_and_subpaths(50);
    // Translucent paint would composite twice where split operations overlap,
    // so it stays one sketch operation plus the outline's.
    assert_eq!(translucent_strokes, 2);
    assert!(
        opaque_strokes > 3,
        "an 8000 px cross-hatch exceeds the length budget several times, got {opaque_strokes} strokes"
    );
    assert_eq!(opaque_subpaths, translucent_subpaths);
}
