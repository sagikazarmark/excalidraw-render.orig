use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};

#[test]
fn shape_bound_text_inherits_the_container_rotation() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"box","type":"rectangle","x":0,"y":0,"width":100,"height":60,"angle":0.5,"boundElements":[{"type":"text","id":"label"}],"roughness":0,"updated":1},
            {"id":"label","type":"text","x":20,"y":20,"width":60,"height":20,"angle":0,"containerId":"box","fontSize":16,"fontFamily":5,"text":"Bound","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("bound text scene is admitted");

    let svg = render_svg(&scene);
    let text_group = svg
        .split("<text data-source-text=\"Bound\"")
        .next()
        .and_then(|prefix| prefix.rsplit("<g transform=\"").next())
        .expect("bound text transform is emitted");
    assert!(text_group.contains("rotate(28.64788975654116 30 10)"));
}

#[test]
fn shape_bound_text_uses_container_padding_and_alignment() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"box","type":"rectangle","x":0,"y":0,"width":120,"height":80,"boundElements":[{"type":"text","id":"label"}],"roughness":0,"updated":1},
            {"id":"label","type":"text","x":400,"y":400,"width":30,"height":20,"containerId":"box","fontSize":16,"fontFamily":5,"text":"Fit","textAlign":"right","verticalAlign":"bottom","lineHeight":1.25,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("aligned bound text scene is admitted");

    let svg = render_svg(&scene);
    let text_group = svg
        .split("<text data-source-text=\"Fit\"")
        .next()
        .and_then(|prefix| prefix.rsplit("<g transform=\"").next())
        .expect("bound text transform is emitted");
    assert!(text_group.starts_with("translate(95 65)"));
}

#[test]
fn arrow_bound_text_uses_the_resolved_arrow_midpoint() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"arrow","type":"arrow","x":0,"y":20,"width":100,"height":0,"points":[[0,0],[100,0]],"boundElements":[{"type":"text","id":"label"}],"roughness":0,"endArrowhead":"arrow","updated":1},
            {"id":"label","type":"text","x":400,"y":400,"width":40,"height":20,"angle":0,"containerId":"arrow","fontSize":16,"fontFamily":5,"text":"Label","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("arrow label scene is admitted");

    let svg = render_svg(&scene);
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

    let png = render(&scene, OutputFormat::Png);
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let pdf = String::from_utf8_lossy(&render(&scene, OutputFormat::Pdf)).into_owned();
    assert!(pdf.contains("W*\nn\n"));
    assert!(!pdf.contains("/Subtype /Image"));
}

#[test]
fn bound_text_paints_with_its_reciprocal_container() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"label","type":"text","x":10,"y":10,"width":40,"height":20,"containerId":"box","fontSize":16,"fontFamily":5,"text":"Label","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1},
            {"id":"box","type":"rectangle","x":0,"y":0,"width":60,"height":40,"boundElements":[{"type":"text","id":"label"}],"strokeColor":"#e03131","roughness":0,"updated":1},
            {"id":"later","type":"rectangle","x":80,"y":0,"width":20,"height":20,"strokeColor":"#1971c2","roughness":0,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("reordered bound text scene is admitted");

    let svg = render_svg(&scene);
    let container = svg.find("stroke=\"#e03131\"").expect("container paints");
    let label = svg
        .find("data-source-text=\"Label\"")
        .expect("bound label paints");
    let later = svg.find("stroke=\"#1971c2\"").expect("later shape paints");
    assert!(container < label && label < later);
}

#[test]
fn one_way_container_reference_does_not_create_an_arrow_mask_or_label() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"arrow","type":"arrow","x":0,"y":20,"width":100,"height":0,"points":[[0,0],[100,0]],"roughness":0,"updated":1},
            {"id":"label","type":"text","x":30,"y":10,"width":40,"height":20,"containerId":"arrow","fontSize":16,"fontFamily":5,"text":"Hidden","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("one-way bound text scene is admitted");

    let svg = render_svg(&scene);
    assert!(!svg.contains("arrow-label-mask"));
    assert!(!svg.contains("data-source-text=\"Hidden\""));
}

fn render_svg(scene: &SceneDocument) -> String {
    String::from_utf8(render(scene, OutputFormat::Svg)).expect("SVG is UTF-8")
}

fn render(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    let request = RenderRequest::builder(format)
        .build()
        .expect("render request is valid");
    let outcome = Renderer::new()
        .render(scene, &request)
        .expect("bound text renders");
    assert!(outcome.report().is_empty());
    outcome.artifact().bytes().to_vec()
}
