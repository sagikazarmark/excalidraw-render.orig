use excalidraw_render::{
    ColorMode, DiagnosticCode, FrameOptions, OutputFormat, RenderRequest, Renderer, SceneDocument,
};

use crate::common::raster::decode_png;
use crate::common::render::render_with_request;

const NESTED_FRAME_SCENE: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"outer","type":"frame","x":0,"y":0,"width":100,"height":60,"name":"Outer","updated":1},
        {"id":"inner","type":"frame","x":30,"y":10,"width":50,"height":40,"frameId":"outer","opacity":50,"updated":1},
        {"id":"child","type":"rectangle","x":60,"y":20,"width":80,"height":20,"frameId":"inner","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"opacity":50,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

/// The #115 scene: one named 580x580 frame clipping three filled rectangles,
/// the last of which straddles the frame's right edge.
const NAMED_FRAME_SCENE: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"clip-frame","type":"frame","name":"Clip","x":0,"y":0,"width":580,"height":580,"roughness":0,"updated":1},
        {"id":"cell-0","type":"rectangle","x":35,"y":35,"width":30,"height":30,"frameId":"clip-frame","strokeColor":"#1971c2","backgroundColor":"#a5d8ff","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"cell-1","type":"rectangle","x":75,"y":35,"width":30,"height":30,"frameId":"clip-frame","strokeColor":"#1971c2","backgroundColor":"#a5d8ff","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"cell-2","type":"rectangle","x":555,"y":35,"width":30,"height":30,"frameId":"clip-frame","strokeColor":"#1971c2","backgroundColor":"#a5d8ff","fillStyle":"solid","roughness":0,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

/// The first baseline Excalidraw's Helvetica metadata metrics (`2048`, `1577`,
/// `-471`) give a 14 px line in the 17.5 px line box `FRAME_STYLE.nameFontSize`
/// 14 and `nameLineHeight` 1.25 make: `17.5 / 2 + 14 * (1577 - 471) / 2048 / 2`.
/// The box sits `nameOffsetY` 3 px above the frame, so its top is 20.5 px above
/// the frame's top edge.
const FRAME_NAME_BASELINE: f64 = 12.530_273_437_5;

#[test]
fn frame_name_box_extends_the_canvas_by_the_reference_metrics() {
    let scene = SceneDocument::parse(NAMED_FRAME_SCENE).expect("named frame is admitted");

    let svg = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(
        svg.contains("viewBox=\"0 0 600 620.5\""),
        "the name box is 20.5 px tall above the frame: {}",
        root_tag(svg)
    );

    let png = render(&scene, OutputFormat::Png, FrameOptions::default());
    let decoded = decode_png(png.artifact().bytes());
    assert_eq!(
        (decoded.width, decoded.height),
        (600, 620),
        "PNG truncates the 620.5 extent"
    );
}

#[test]
fn frame_name_is_left_aligned_helvetica_text_above_the_frame() {
    let scene = SceneDocument::parse(NAMED_FRAME_SCENE).expect("named frame is admitted");
    let svg = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");

    let (group, text) = frame_name(svg, "clip-frame");
    // The name is an unrotated text element at the frame's x, 20.5 px above
    // its y; after padding that is (10, 10) with the frame group at (10, 30.5).
    assert!(
        group.contains("transform=\"translate(10 10) rotate(0 "),
        "name group: {group}"
    );
    assert!(svg.contains("translate(10 30.5) rotate(0 290 290)"));
    assert!(text.contains("fill=\"#999999\""), "name color: {text}");
    assert!(text.contains("font-size=\"14\""), "name size: {text}");
    assert!(text.contains(">Clip</text>"), "name content: {text}");
    let (first_x, first_y) = first_glyph_position(&text);
    assert!(
        first_x.abs() < 1e-9,
        "left aligned at the frame's x, not centered: {first_x}"
    );
    assert!(
        (first_y - FRAME_NAME_BASELINE).abs() < 1e-9,
        "baseline from Helvetica's metadata metrics: {first_y}"
    );
}

#[test]
fn frame_name_uses_the_dark_theme_name_color_in_dark_mode() {
    let scene = SceneDocument::parse(NAMED_FRAME_SCENE).expect("named frame is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .color_mode(ColorMode::Dark)
        .build()
        .expect("dark SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("named frame renders in dark mode");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    let (_, text) = frame_name(svg, "clip-frame");
    // `FRAME_STYLE.nameColorDarkTheme` #7a7a7a through the dark-mode filter.
    assert!(text.contains("fill=\"#848484\""), "dark name color: {text}");
}

#[test]
fn frame_name_is_painted_grey_above_the_frame_in_png() {
    let scene = SceneDocument::parse(NAMED_FRAME_SCENE).expect("named frame is admitted");
    let png = render(&scene, OutputFormat::Png, FrameOptions::default());
    let decoded = decode_png(png.artifact().bytes());

    // The name box spans y 10..27.5 in the canvas; "Clip" at 14 px is under
    // 30 px wide, so every inked pixel is inside x 10..40.
    let mut inked = 0;
    for y in 10..28 {
        for x in 10..40 {
            let [red, green, blue, alpha] = decoded.pixel(x, y);
            assert_eq!(alpha, 255);
            if [red, green, blue] == [255, 255, 255] {
                continue;
            }
            inked += 1;
            // #999999 blended with the white background stays grey and no
            // darker than #999999; #1b1b1f would break both.
            assert_eq!(red, green, "name ink is grey at ({x}, {y})");
            assert_eq!(green, blue, "name ink is grey at ({x}, {y})");
            assert!(red >= 0x99, "name ink is #999999 or lighter at ({x}, {y})");
        }
    }
    assert!(inked > 20, "the name is painted: {inked} inked pixels");
    for x in 10..40 {
        assert_eq!(
            decoded.pixel(x, 5),
            [255, 255, 255, 255],
            "nothing above the name"
        );
    }
}

#[test]
fn frame_name_is_truncated_to_the_frame_width_with_three_dots() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"narrow","type":"frame","name":"A rather long frame name","x":0,"y":0,"width":60,"height":40,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("narrow frame is admitted");
    let svg = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");

    let (_, text) = frame_name(svg, "narrow");
    let content = text
        .split_once('>')
        .and_then(|(_, rest)| rest.split_once("</text>"))
        .map(|(content, _)| content)
        .expect("name has content");
    let kept = content
        .strip_suffix("...")
        .expect("truncated with ASCII dots, not an ellipsis glyph");
    assert!(
        !kept.is_empty() && "A rather long frame name".starts_with(kept) && kept.len() < 12,
        "a prefix of the name that fits 60 px with the dots: {content:?}"
    );
    // A truncated name is as wide as its frame, so it adds nothing sideways.
    assert!(svg.contains("viewBox=\"0 0 80 80.5\""), "{}", root_tag(svg));
}

#[test]
fn frame_name_is_neither_rotated_nor_clipped_with_its_frame() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"outer","type":"frame","x":0,"y":0,"width":200,"height":200,"updated":1},
            {"id":"tilted","type":"frame","name":"Tilted","x":50,"y":50,"width":100,"height":60,"angle":0.5,"frameId":"outer","updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("tilted frame is admitted");
    let svg = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");

    let (group, _) = frame_name(svg, "tilted");
    assert!(
        group.contains(" rotate(0 "),
        "the name keeps angle 0: {group}"
    );
    let name_start = svg.find(&group).expect("name group is in the document");
    let clipped_start = svg
        .find("<g clip-path=\"url(#frame-clip-0)\">")
        .expect("the nested frame is clipped by the outer frame");
    assert!(
        name_start < clipped_start,
        "the name precedes the clipped frame rather than sitting inside its clip"
    );
}

#[test]
fn frame_clipping_defines_one_clip_path_per_frame_in_defs() {
    let scene = SceneDocument::parse(NAMED_FRAME_SCENE).expect("named frame is admitted");
    let svg = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");

    let defs = svg
        .split_once("<defs>")
        .and_then(|(_, rest)| rest.split_once("</defs>"))
        .map(|(defs, _)| defs)
        .expect("the document has one defs block");
    assert_eq!(
        svg.matches("<clipPath").count(),
        1,
        "one clipPath per frame"
    );
    assert!(
        defs.contains(concat!(
            "<clipPath id=\"frame-clip-0\">",
            "<rect transform=\"translate(10 30.5) rotate(0 290 290)\" ",
            "width=\"580\" height=\"580\" rx=\"8\" ry=\"8\"/></clipPath>"
        )),
        "the clip is the rounded frame rectangle in defs: {defs}"
    );
    assert_eq!(
        svg.matches("<g clip-path=\"url(#frame-clip-0)\">").count(),
        3,
        "every child references the frame's clip"
    );
    assert!(
        svg.contains(concat!(
            "<rect data-frame-outline=\"clip-frame\" width=\"580\" height=\"580\" ",
            "rx=\"8\" ry=\"8\" fill=\"none\" stroke=\"#bbbbbb\" stroke-width=\"2\"/>"
        )),
        "the outline is a rounded rect, not a path"
    );
    assert_eq!(
        svg.matches("<path ").count(),
        6,
        "only the children's fill and stroke paths remain"
    );
}

#[test]
fn a_nested_frames_name_extends_the_scene_bounds_like_any_root_element() {
    // The inner frame is a clipped child of the outer one, so its own box
    // never widens the scene; its name belongs to no frame and does, as the
    // reference's synthesized text element has no frameId.
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"outer","type":"frame","x":0,"y":0,"width":200,"height":200,"updated":1},
            {"id":"inner","type":"frame","name":"Inner","x":100,"y":10,"width":150,"height":60,"frameId":"outer","updated":1}
        ],
        "appState":{"frameRendering":{"enabled":true,"clip":true,"name":true,"outline":true}},
        "files":{}
    }"##)
    .expect("nested frame scene is admitted");
    let svg = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");

    // Outer frame 0..200 with its own name above it: width 220 and, with the
    // inner name's box top at y -10.5 inside the outer name's 20.5 px reach,
    // height 200 + 20.5 + 20.
    assert!(
        svg.contains("viewBox=\"0 0 220 240.5\""),
        "{}",
        root_tag(svg)
    );

    let unnamed_outer = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"outer","type":"frame","x":0,"y":0,"width":200,"height":200,"updated":1},
            {"id":"inner","type":"frame","name":"Inner","x":100,"y":100,"width":150,"height":60,"frameId":"outer","updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("nested frame scene is admitted");
    let svg = render(&unnamed_outer, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    // The inner name is "Inner" at 14 px, so it never reaches x 200; the
    // inner frame's own overflow to x 250 is clipped away and adds nothing.
    assert!(
        svg.contains("viewBox=\"0 0 220 240.5\""),
        "{}",
        root_tag(svg)
    );
}

/// The name `<g>` and its `<text>` for one frame.
fn frame_name(svg: &str, frame_id: &str) -> (String, String) {
    let marker = format!("<text data-frame-name=\"{frame_id}\"");
    let text_start = svg.find(&marker).expect("frame name is emitted");
    let text_end = svg[text_start..]
        .find("</text>")
        .map(|end| text_start + end + "</text>".len())
        .expect("frame name closes");
    let group_start = svg[..text_start]
        .rfind("<g ")
        .expect("frame name has a group");
    (
        svg[group_start..text_start].to_owned(),
        svg[text_start..text_end].to_owned(),
    )
}

fn first_glyph_position(text: &str) -> (f64, f64) {
    let first = |attribute: &str| {
        text.split_once(&format!(" {attribute}=\""))
            .and_then(|(_, rest)| rest.split_once('"'))
            .and_then(|(values, _)| values.split(' ').next())
            .expect("name glyph position")
            .parse::<f64>()
            .expect("glyph position is numeric")
    };
    (first("x"), first("y"))
}

fn root_tag(svg: &str) -> &str {
    svg.split_once('>').map_or(svg, |(root, _)| root)
}

#[test]
fn nested_frames_clip_direct_geometry_in_every_format() {
    let scene = SceneDocument::parse(NESTED_FRAME_SCENE).expect("nested frame scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format, FrameOptions::default());
        assert!(
            outcome.report().diagnostics().iter().all(|diagnostic| {
                diagnostic.code() != DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
            }),
            "framed geometry must remain directly renderable in {format:?}"
        );

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
                // One clip per frame: the outer frame clips the inner frame,
                // the inner frame clips the child.
                assert_eq!(svg.matches("<clipPath id=\"frame-clip-0\">").count(), 1);
                assert_eq!(svg.matches("<clipPath id=\"frame-clip-1\">").count(), 1);
                assert_eq!(
                    svg.matches("<g clip-path=\"url(#frame-clip-0)\">").count(),
                    1
                );
                assert_eq!(
                    svg.matches("<g clip-path=\"url(#frame-clip-1)\">").count(),
                    1
                );
                assert!(!svg.contains("fill=\"#f1f3f5\""));
                assert!(svg.contains("fill-opacity=\"0.25\""));
            }
            OutputFormat::Png => {
                let decoded = decode_png(outcome.artifact().bytes());
                assert_ne!(decoded.pixel(75, 64), [255, 255, 255, 255]);
                assert_eq!(decoded.pixel(100, 64), [255, 255, 255, 255]);
            }
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
                assert!(pdf.matches("W\nn\n").count() >= 2);
                assert!(!pdf.contains("/Subtype /Image"));
            }
            _ => unreachable!("the test enumerates every supported output format"),
        }
    }
}

#[test]
fn frame_controls_are_independent() {
    let scene = SceneDocument::parse(NESTED_FRAME_SCENE).expect("nested frame scene is admitted");

    let unclipped = render(
        &scene,
        OutputFormat::Svg,
        FrameOptions::new(true, false, true, true),
    );
    let unclipped = std::str::from_utf8(unclipped.artifact().bytes()).expect("SVG is UTF-8");
    assert!(!unclipped.contains("frame-clip-"));
    assert!(unclipped.contains("data-frame-outline=\"outer\""));
    assert!(unclipped.contains("data-frame-name=\"outer\""));

    let clipped = render(
        &scene,
        OutputFormat::Svg,
        FrameOptions::new(true, true, true, true),
    );
    let clipped = std::str::from_utf8(clipped.artifact().bytes()).expect("SVG is UTF-8");
    assert!(view_box_width(unclipped) > view_box_width(clipped));

    let no_decorations = render(
        &scene,
        OutputFormat::Svg,
        FrameOptions::new(true, true, false, false),
    );
    let no_decorations =
        std::str::from_utf8(no_decorations.artifact().bytes()).expect("SVG is UTF-8");
    assert!(no_decorations.contains("frame-clip-"));
    assert!(!no_decorations.contains("data-frame-outline"));
    assert!(!no_decorations.contains("data-frame-name"));

    let disabled = render(
        &scene,
        OutputFormat::Svg,
        FrameOptions::new(false, true, true, true),
    );
    let disabled = std::str::from_utf8(disabled.artifact().bytes()).expect("SVG is UTF-8");
    assert!(!disabled.contains("frame-clip-"));
    assert!(!disabled.contains("data-frame-outline"));
    assert!(!disabled.contains("data-frame-name"));
    assert!(disabled.contains("fill-opacity=\"0.25\""));
}

#[test]
fn frame_clipping_uses_the_rounded_frame_boundary() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"frame","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
            {"id":"child","type":"rectangle","x":0,"y":0,"width":100,"height":60,"frameId":"frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("rounded frame scene is admitted");
    let outcome = render(
        &scene,
        OutputFormat::Png,
        FrameOptions::new(true, true, false, false),
    );
    let decoded = decode_png(outcome.artifact().bytes());
    assert_eq!(decoded.pixel(11, 11), [255, 255, 255, 255]);
    assert_ne!(decoded.pixel(20, 20), [255, 255, 255, 255]);
}

#[test]
fn group_membership_preserves_direct_paint_order_in_every_format() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"back","type":"rectangle","x":0,"y":0,"width":40,"height":40,"groupIds":["group"],"strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1},
            {"id":"front","type":"ellipse","x":20,"y":20,"width":40,"height":40,"groupIds":["group"],"strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"opacity":50,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("grouped scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format, FrameOptions::default());
        assert!(outcome.report().diagnostics().iter().all(|diagnostic| {
            diagnostic.code() != DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
        }));
        if format == OutputFormat::Pdf {
            assert!(
                !String::from_utf8_lossy(outcome.artifact().bytes()).contains("/Subtype /Image")
            );
        }
    }
}

fn render(
    scene: &SceneDocument,
    format: OutputFormat,
    frame_options: FrameOptions,
) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .frame_options(frame_options)
        .build()
        .expect("frame render request is valid");
    render_with_request(scene, &request)
}

fn view_box_width(svg: &str) -> f64 {
    let marker = "viewBox=\"0 0 ";
    let value = svg
        .split_once(marker)
        .expect("SVG has a viewBox")
        .1
        .split_once(' ')
        .expect("viewBox has a width")
        .0;
    value.parse().expect("viewBox width is numeric")
}
