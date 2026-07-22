use std::io::Cursor;

use excalidraw_render::{
    DiagnosticCode, FrameOptions, OutputFormat, RenderRequest, Renderer, SceneDocument,
};
use png::{BitDepth, ColorType, Decoder, Transformations};

const NESTED_FRAME_SCENE: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"outer","type":"frame","x":0,"y":0,"width":100,"height":60,"name":"Outer","updated":1},
        {"id":"inner","type":"frame","x":30,"y":10,"width":50,"height":40,"frameId":"outer","opacity":50,"updated":1},
        {"id":"child","type":"rectangle","x":60,"y":20,"width":80,"height":20,"frameId":"inner","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"opacity":50,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

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
                assert!(svg.matches("<clipPath id=\"frame-clip-").count() >= 2);
                assert!(svg.matches("clip-path=\"url(#frame-clip-").count() >= 2);
                assert!(!svg.contains("fill=\"#f1f3f5\""));
                assert!(svg.contains("fill-opacity=\"0.25\""));
            }
            OutputFormat::Png => {
                let (width, _, rgba) = decode_png(outcome.artifact().bytes());
                let pixel = |x: u32, y: u32| {
                    let offset =
                        usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4)
                            .expect("pixel offset fits usize");
                    &rgba[offset..offset + 4]
                };
                assert_ne!(pixel(75, 64), [255, 255, 255, 255]);
                assert_eq!(pixel(100, 64), [255, 255, 255, 255]);
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
    let (width, _, rgba) = decode_png(outcome.artifact().bytes());
    let pixel = |x: u32, y: u32| {
        let offset = usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4)
            .expect("pixel offset fits usize");
        &rgba[offset..offset + 4]
    };
    assert_eq!(pixel(11, 11), [255, 255, 255, 255]);
    assert_ne!(pixel(20, 20), [255, 255, 255, 255]);
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
    Renderer::new()
        .render(scene, &request)
        .expect("frame scene renders")
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    assert_eq!(reader.info().color_type, ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, BitDepth::Eight);
    let mut rgba = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
    rgba.truncate(frame.buffer_size());
    (frame.width, frame.height, rgba)
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
