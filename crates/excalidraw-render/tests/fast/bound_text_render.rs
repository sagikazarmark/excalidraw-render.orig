//! The bound-text family's one end-to-end smoke. Placement, paint order and
//! the reciprocity rules are asserted once on the resolved scene in
//! `render::resolve_tests`; this keeps the arrow label reaching every format.

use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{OutputFormat, SceneDocument};

use crate::common::render::render_artifact;

#[test]
pub(super) fn arrow_bound_text_renders_at_the_arrow_midpoint_in_every_format() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"arrow","type":"arrow","x":0,"y":20,"width":100,"height":0,"points":[[0,0],[100,0]],"boundElements":[{"type":"text","id":"label"}],"roughness":0,"endArrowhead":"arrow","updated":1},
            {"id":"label","type":"text","x":400,"y":400,"width":40,"height":20,"angle":0,"containerId":"arrow","fontSize":16,"fontFamily":5,"text":"Label","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("arrow label scene is admitted");

    let svg = String::from_utf8(render_artifact(&scene, OutputFormat::Svg)).expect("SVG is UTF-8");
    let view_box = svg
        .split("viewBox=\"0 0 ")
        .nth(1)
        .and_then(|tail| tail.split('\"').next())
        .expect("SVG has a viewBox");
    let width: f64 = view_box
        .split_whitespace()
        .next()
        .expect("viewBox has a width")
        .parse()
        .expect("viewBox width is numeric");
    assert!(
        width < 150.0,
        "stale text coordinates expanded the viewBox: {width}"
    );
    assert!(svg.contains("<mask id=\"arrow-label-mask-"));
    assert!(svg.contains("mask=\"url(#arrow-label-mask-"));
    assert!(svg.contains("<text data-source-text=\"Label\""));

    let png = render_artifact(&scene, OutputFormat::Png);
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let pdf = pdf_text(&render_artifact(&scene, OutputFormat::Pdf));
    assert!(pdf.contains("W*\nn\n"));
    assert!(!pdf.contains("/Subtype /Image"));
}
