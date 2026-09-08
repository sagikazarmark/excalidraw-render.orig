use std::io::Cursor;

use excalidraw_render::{
    ColorMode, DiagnosticCode, ErrorCode, OutputFormat, RenderErrorCategory, RenderRequest,
    Renderer, SceneDocument, Sizing,
};
use png::{BitDepth, ColorType, Decoder, Transformations};

const COLORED_RECTANGLE: &[u8] = br##"{
    "type":"excalidraw",
    "version":2,
    "elements":[{
        "id":"colored",
        "type":"rectangle",
        "width":20,
        "height":10,
        "strokeColor":"color-mix(in srgb, red 25%, blue)",
        "seed":1,
        "updated":1
    }],
    "appState":{"viewBackgroundColor":"rebeccapurple"},
    "files":{}
}"##;

#[test]
fn chromium_css_colors_are_canonicalized_for_every_projection() {
    let svg = render(COLORED_RECTANGLE, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("fill=\"#663399\""));
    assert!(svg.contains("stroke=\"#4000bf\""));

    let png = render(COLORED_RECTANGLE, OutputFormat::Png);
    let (width, height, rgba) = decode_png(png.artifact().bytes());
    assert_eq!((width, height), (40, 30));
    assert_eq!(&rgba[..4], &[102, 51, 153, 255]);
    assert!(rgba.chunks_exact(4).any(|pixel| pixel == [64, 0, 191, 255]));

    let pdf = render(COLORED_RECTANGLE, OutputFormat::Pdf);
    let content = std::str::from_utf8(stream_data(pdf.artifact().bytes(), b"4 0 obj"))
        .expect("PDF content is ASCII");
    assert!(content.contains(concat!("0.4 0.2 0.6 scn\n", "0 0 40 30 re\nf\n")));
    assert!(content.contains("0.25098039215686274 0 0.7490196078431373 SCN\n"));
}

#[test]
fn canonical_color_alpha_is_preserved_by_every_projection() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"alpha",
            "type":"rectangle",
            "width":20,
            "height":10,
            "strokeColor":"#ff000080",
            "seed":1,
            "updated":1
        }],
        "appState":{"viewBackgroundColor":"rgb(10 20 30 / 50%)"},
        "files":{}
    }"##;
    const ALPHA: &str = "0.5019607843137255";

    let svg = render(source, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&format!("fill=\"#0a141e\" fill-opacity=\"{ALPHA}\"")));
    assert!(svg.contains(&format!(
        "stroke=\"#ff0000\" stroke-width=\"2\" fill=\"none\" stroke-opacity=\"{ALPHA}\""
    )));

    let png = render(source, OutputFormat::Png);
    let (_, _, rgba) = decode_png(png.artifact().bytes());
    assert_eq!(&rgba[..4], &[10, 20, 30, 128]);
    let transparent_png = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Png)
            .include_background(false)
            .build()
            .expect("transparent PNG request is valid"),
    );
    let (_, _, rgba) = decode_png(transparent_png.artifact().bytes());
    assert!(rgba.chunks_exact(4).any(|pixel| pixel == [255, 0, 0, 128]));

    let pdf = render(source, OutputFormat::Pdf);
    let pdf_text = String::from_utf8_lossy(pdf.artifact().bytes());
    assert_eq!(pdf_text.matches("/Type /ExtGState\n").count(), 1);
    assert!(pdf_text.contains(&format!("/CA {ALPHA}\n/ca {ALPHA}\n")));
    let content = std::str::from_utf8(stream_data(pdf.artifact().bytes(), b"4 0 obj"))
        .expect("PDF content is ASCII");
    assert_eq!(content.matches("/GS1 gs").count(), 2);
}

#[test]
fn element_opacity_multiplies_color_alpha_in_every_projection() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"combined-alpha",
            "type":"rectangle",
            "width":20,
            "height":10,
            "strokeColor":"#ff000080",
            "opacity":50,
            "seed":1,
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"##;
    const EFFECTIVE_ALPHA: &str = "0.25098039215686274";

    let svg = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Svg)
            .include_background(false)
            .build()
            .expect("transparent SVG request is valid"),
    );
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&format!("stroke-opacity=\"{EFFECTIVE_ALPHA}\"")));

    let png = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Png)
            .include_background(false)
            .build()
            .expect("transparent PNG request is valid"),
    );
    let (_, _, rgba) = decode_png(png.artifact().bytes());
    assert!(rgba.chunks_exact(4).any(|pixel| pixel == [255, 0, 0, 64]));

    let pdf = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Pdf)
            .include_background(false)
            .build()
            .expect("transparent PDF request is valid"),
    );
    assert!(String::from_utf8_lossy(pdf.artifact().bytes())
        .contains(&format!("/CA {EFFECTIVE_ALPHA}\n/ca {EFFECTIVE_ALPHA}\n")));
}

#[test]
fn dark_mode_lowers_scene_colors_before_every_projection() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"dark",
            "type":"rectangle",
            "width":20,
            "height":10,
            "strokeColor":"#1e1e1e",
            "seed":1,
            "updated":1
        }],
        "appState":{"viewBackgroundColor":"#ffffff"},
        "files":{}
    }"##;

    let dark_svg = render_dark(source, OutputFormat::Svg);
    let svg = std::str::from_utf8(dark_svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("fill=\"#111111\""));
    assert!(svg.contains("stroke=\"#d3d3d3\""));

    let dark_png = render_dark(source, OutputFormat::Png);
    let (_, _, rgba) = decode_png(dark_png.artifact().bytes());
    assert_eq!(&rgba[..4], &[17, 17, 17, 255]);
    assert!(rgba
        .chunks_exact(4)
        .any(|pixel| pixel == [211, 211, 211, 255]));

    let dark_pdf = render_dark(source, OutputFormat::Pdf);
    let content = std::str::from_utf8(stream_data(dark_pdf.artifact().bytes(), b"4 0 obj"))
        .expect("PDF content is ASCII");
    assert!(content.contains("0.06666666666666667 0.06666666666666667 0.06666666666666667 scn"));
    assert!(content.contains("0.8274509803921568 0.8274509803921568 0.8274509803921568 SCN"));

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let light = render(source, format);
        let dark = render_dark(source, format);
        assert_ne!(light.artifact().bytes(), dark.artifact().bytes());
        assert_eq!(light.report(), dark.report());
    }
}

#[test]
fn renderer_owned_fallback_colors_follow_dark_mode() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"future",
            "type":"future",
            "width":80,
            "height":40,
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"##;

    let svg = render_dark(source, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("fill=\"#1a1c1e\""));
    assert!(svg.contains("stroke=\"#6e747b\""));
    assert!(svg.contains("fill=\"#a4aab0\" font-family=\"ExcalidrawRender-9\""));

    let png = render_dark(source, OutputFormat::Png);
    let (_, _, rgba) = decode_png(png.artifact().bytes());
    assert!(rgba.chunks_exact(4).any(|pixel| pixel == [26, 28, 30, 255]));

    let pdf = render_dark(source, OutputFormat::Pdf);
    let content = std::str::from_utf8(stream_data(pdf.artifact().bytes(), b"4 0 obj"))
        .expect("PDF content is ASCII");
    assert!(content.contains("0.10196078431372549 0.10980392156862745 0.11764705882352941 scn"));
    assert!(content.contains("0.43137254901960786 0.4549019607843137 0.4823529411764706 SCN"));
}

#[test]
fn empty_and_fallback_only_scenes_use_the_scene_background() {
    let empty = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[],
        "appState":{"viewBackgroundColor":"rebeccapurple"},
        "files":{}
    }"##;
    let fallback = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{"id":"future","type":"future","width":20,"height":10,"updated":1}],
        "appState":{"viewBackgroundColor":"rebeccapurple"},
        "files":{}
    }"##;

    for source in [empty.as_slice(), fallback.as_slice()] {
        let svg = render(source, OutputFormat::Svg);
        let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
        assert!(svg.contains("fill=\"#663399\""));

        let png = render(source, OutputFormat::Png);
        let (_, _, rgba) = decode_png(png.artifact().bytes());
        assert_eq!(&rgba[..4], &[102, 51, 153, 255]);

        let pdf = render(source, OutputFormat::Pdf);
        let content = std::str::from_utf8(stream_data(pdf.artifact().bytes(), b"4 0 obj"))
            .expect("PDF content is ASCII");
        assert!(content.contains("0.4 0.2 0.6 scn"));
    }
}

#[test]
fn zero_padding_empty_scenes_remain_meaningful_in_every_format() {
    let source = br#"{
        "type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}
    }"#;

    let svg = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Svg)
            .padding(0.0)
            .build()
            .expect("zero-padding SVG request is valid"),
    );
    assert!(std::str::from_utf8(svg.artifact().bytes())
        .expect("SVG is UTF-8")
        .contains("viewBox=\"0 0 1 1\" width=\"1\" height=\"1\""));

    let png = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Png)
            .padding(0.0)
            .build()
            .expect("zero-padding PNG request is valid"),
    );
    let (width, height, _) = decode_png(png.artifact().bytes());
    assert_eq!((width, height), (1, 1));

    let pdf = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Pdf)
            .padding(0.0)
            .build()
            .expect("zero-padding PDF request is valid"),
    );
    assert!(String::from_utf8_lossy(pdf.artifact().bytes()).contains("/MediaBox [0 0 0.75 0.75]"));
}

#[test]
fn bounds_padding_and_sizing_follow_each_format_rule() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"negative",
            "type":"rectangle",
            "x":-20,
            "y":30,
            "width":30,
            "height":10,
            "seed":1,
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"##;

    let scaled_svg = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Svg)
            .padding(5.0)
            .sizing(Sizing::Scale(2.0))
            .build()
            .expect("scaled SVG request is valid"),
    );
    let scaled_svg = std::str::from_utf8(scaled_svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(scaled_svg.contains("viewBox=\"0 0 40 20\" width=\"80\" height=\"40\""));
    assert!(scaled_svg.contains("transform=\"translate(5 5)"));

    let maximum_svg = render_with_request(
        source,
        RenderRequest::builder(OutputFormat::Svg)
            .padding(5.0)
            .sizing(Sizing::MaxDimension(10.0))
            .build()
            .expect("maximum-dimension SVG request is valid"),
    );
    let maximum_svg = std::str::from_utf8(maximum_svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(maximum_svg.contains("viewBox=\"0 0 40 20\" width=\"40\" height=\"20\""));

    for (sizing, expected) in [
        (Sizing::Scale(1.5), (60, 30)),
        (Sizing::MaxDimension(20.0), (20, 10)),
        (Sizing::MaxDimension(100.0), (40, 20)),
    ] {
        let png = render_with_request(
            source,
            RenderRequest::builder(OutputFormat::Png)
                .padding(5.0)
                .sizing(sizing)
                .build()
                .expect("sized PNG request is valid"),
        );
        let (width, height, _) = decode_png(png.artifact().bytes());
        assert_eq!((width, height), expected);
    }

    for (sizing, media_box) in [
        (Sizing::Scale(2.0), "/MediaBox [0 0 60 30]"),
        (Sizing::MaxDimension(20.0), "/MediaBox [0 0 15 7.5]"),
        (Sizing::MaxDimension(100.0), "/MediaBox [0 0 30 15]"),
    ] {
        let pdf = render_with_request(
            source,
            RenderRequest::builder(OutputFormat::Pdf)
                .padding(5.0)
                .sizing(sizing)
                .build()
                .expect("sized PDF request is valid"),
        );
        assert!(String::from_utf8_lossy(pdf.artifact().bytes()).contains(media_box));
    }
}

#[test]
fn malformed_colors_use_field_specific_substitutions_in_source_order() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"invalid-colors",
            "type":"rectangle",
            "width":20,
            "height":10,
            "strokeColor":"rgb(1, 2 3)",
            "backgroundColor":"color(invalid 1 1 1)",
            "seed":1,
            "updated":1
        }],
        "appState":{"viewBackgroundColor":"rgb(future)"},
        "files":{}
    }"##;

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(source, format);
        assert_eq!(
            outcome
                .report()
                .diagnostics()
                .iter()
                .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
                .collect::<Vec<_>>(),
            [
                (DiagnosticCode::COLOR_INVALID, "/elements/0/strokeColor"),
                (DiagnosticCode::COLOR_INVALID, "/elements/0/backgroundColor"),
                (
                    DiagnosticCode::COLOR_INVALID,
                    "/appState/viewBackgroundColor"
                ),
            ]
        );
    }
}

#[test]
fn non_finite_derived_bounds_use_the_stable_scene_error() {
    let source = br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"unbounded",
            "type":"rectangle",
            "x":1e308,
            "width":1e308,
            "height":10,
            "seed":1,
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"##;
    let scene = SceneDocument::parse(source).expect("large finite coordinates are admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default SVG request is valid");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("non-finite derived bounds are fatal");

    assert_eq!(error.category(), RenderErrorCategory::Input);
    assert_eq!(error.code(), ErrorCode::SCENE_UNBOUNDED_GEOMETRY);
    assert_eq!(error.json_pointer(), "");
    assert_eq!(error.report().diagnostics().len(), 1);

    let empty = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#,
    )
    .expect("empty scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .padding(f64::MAX)
        .build()
        .expect("maximum finite padding is a valid request");
    let error = Renderer::new()
        .render(&empty, &request)
        .expect_err("overflowing empty-scene bounds are fatal");
    assert_eq!(error.category(), RenderErrorCategory::Input);
    assert_eq!(error.code(), ErrorCode::SCENE_UNBOUNDED_GEOMETRY);
    assert_eq!(error.json_pointer(), "");
}

#[test]
fn nested_color_mix_is_bounded_before_rendering() {
    let mut color = String::from("red");
    for _ in 0..130 {
        color = format!("color-mix(in srgb, {color}, blue)");
    }
    let source = format!(
        concat!(
            "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],",
            "\"appState\":{{\"viewBackgroundColor\":\"{}\"}},\"files\":{{}}}}"
        ),
        color
    );
    let outcome = render(source.as_bytes(), OutputFormat::Svg);
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("over-nested color has one field diagnostic");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::COLOR_INVALID);
    assert_eq!(diagnostic.json_pointer(), "/appState/viewBackgroundColor");
}

#[test]
fn pinned_chromium_color_level_4_forms_lower_to_canonical_srgb() {
    for (color, expected) in [
        ("RGB(30 30 30)", "#1e1e1e"),
        ("hwb(120 0% 0% / .5)", "#00ff00"),
        ("lab(50% 0 0)", "#777777"),
        ("oklab(50% 0 0)", "#636363"),
        ("color(display-p3 1 0 0)", "#ff0000"),
        ("color(srgb 0.25 0 0.75)", "#4000bf"),
        (
            "color(srgb-linear 0.21586050011389926 0.21586050011389926 0.21586050011389926)",
            "#808080",
        ),
        ("color-mix(IN SRGB, red 25%, blue)", "#4000bf"),
        ("color-mix(in oklab, red, blue)", "#8c53a2"),
        ("color-mix(in hsl, red, blue)", "#ff00ff"),
        ("color-mix(in hsl longer hue, red, blue)", "#00ff00"),
        ("color-mix(in hwb, red, blue)", "#ff00ff"),
        ("color-mix(in display-p3, red, blue)", "#800a91"),
        ("color-mix(in xyz, red, blue)", "#bc00bc"),
        ("currentcolor", "#000000"),
    ] {
        let source = format!(
            concat!(
                "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],",
                "\"appState\":{{\"viewBackgroundColor\":\"{}\"}},\"files\":{{}}}}"
            ),
            color
        );
        let svg = render(source.as_bytes(), OutputFormat::Svg);
        let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
        assert!(
            svg.contains(&format!("fill=\"{expected}\"")),
            "{color} did not lower to {expected}: {svg}"
        );
    }
}

fn render(source: &[u8], format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("whole-scene control request is valid");
    render_with_request(source, request)
}

fn render_with_request(source: &[u8], request: RenderRequest) -> excalidraw_render::RenderOutcome {
    let scene = SceneDocument::parse(source).expect("whole-scene control vector is admitted");
    Renderer::new()
        .render(&scene, &request)
        .expect("whole-scene control vector renders")
}

fn render_dark(source: &[u8], format: OutputFormat) -> excalidraw_render::RenderOutcome {
    render_with_request(
        source,
        RenderRequest::builder(format)
            .color_mode(ColorMode::Dark)
            .build()
            .expect("dark-mode request is valid"),
    )
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

fn stream_data<'a>(pdf: &'a [u8], object: &[u8]) -> &'a [u8] {
    let object_start = find_bytes(pdf, object).expect("expected PDF object exists");
    let marker = b"stream\n";
    let stream_start = object_start
        + find_bytes(&pdf[object_start..], marker).expect("object has a stream")
        + marker.len();
    let stream_end = stream_start
        + find_bytes(&pdf[stream_start..], b"\nendstream").expect("stream has an end marker");
    &pdf[stream_start..stream_end]
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
