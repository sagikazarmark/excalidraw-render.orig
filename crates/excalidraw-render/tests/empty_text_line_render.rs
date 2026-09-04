//! A text line with no glyph outline — an empty line between paragraphs, a
//! trailing line break, or whitespace only — paints nothing in PNG and still
//! takes its line step, as it does in SVG and PDF. Diagnosed on #111, where
//! the PNG encoder failed the whole artifact on such a line.

use std::io::Cursor;

use excalidraw_render::{OutputFormat, RenderOutcome, RenderRequest, Renderer, SceneDocument};
use png::{BitDepth, ColorType, Decoder, Transformations};

fn render(scene: &[u8], format: OutputFormat) -> RenderOutcome {
    let scene = SceneDocument::parse(scene).expect("scene is admitted");
    let request = RenderRequest::builder(format)
        .build()
        .expect("request is valid");
    Renderer::new()
        .render(&scene, &request)
        .unwrap_or_else(|error| panic!("{format:?} renders: {error:?}"))
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

/// The rows of the raster that carry any non-background ink.
fn inked_rows(width: u32, height: u32, rgba: &[u8]) -> Vec<u32> {
    (0..height)
        .filter(|row| {
            (0..width).any(|column| {
                let index = 4 * (row * width + column) as usize;
                rgba[index..index + 3] != [0xff, 0xff, 0xff]
            })
        })
        .collect()
}

/// Diagnostic codes in report order; diagnostics carry their format, so the
/// reports of two formats compare by code.
fn codes(outcome: &RenderOutcome) -> Vec<excalidraw_render::DiagnosticCode> {
    outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
}

fn text_scene(text: &str, height: f64) -> Vec<u8> {
    format!(
        r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"text","type":"text","x":0,"y":0,"width":100,"height":{height},"fontSize":20,"fontFamily":5,"lineHeight":1.25,"text":{text},"seed":1,"updated":1}}],"appState":{{}},"files":{{}}}}"##,
        text = serde_json::to_string(text).expect("text serializes"),
    )
    .into_bytes()
}

#[test]
fn an_empty_line_between_paragraphs_paints_nothing_and_keeps_its_line_step() {
    let scene = text_scene("a\n\nb", 75.0);
    let png = render(&scene, OutputFormat::Png);
    let svg = render(&scene, OutputFormat::Svg);
    assert_eq!(codes(&png), codes(&svg));
    assert!(png.report().is_empty());

    let (width, height, rgba) = decode_png(png.artifact().bytes());
    // Default padding 10; lines are 25px apart, so line 2 spans rows 35..60.
    assert_eq!((width, height), (120, 95));
    let rows = inked_rows(width, height, &rgba);
    assert!(
        rows.iter().any(|&row| (10..35).contains(&row)),
        "the first line has ink: {rows:?}"
    );
    assert!(
        !rows.iter().any(|&row| (35..60).contains(&row)),
        "the empty second line has no ink: {rows:?}"
    );
    assert!(
        rows.iter().any(|&row| (60..85).contains(&row)),
        "the third line has ink: {rows:?}"
    );
}

#[test]
fn trailing_and_whitespace_only_lines_render_in_every_format() {
    for text in ["a\n", "\n", " ", "a\n \nb", "\u{a0}"] {
        let scene = text_scene(text, 50.0);
        let png = render(&scene, OutputFormat::Png);
        let svg = render(&scene, OutputFormat::Svg);
        let pdf = render(&scene, OutputFormat::Pdf);
        assert_eq!(codes(&png), codes(&svg), "{text:?}");
        assert_eq!(codes(&png), codes(&pdf), "{text:?}");
        assert!(png.report().is_empty(), "{text:?}: {:?}", png.report());
        assert!(png.artifact().bytes().starts_with(b"\x89PNG"), "{text:?}");

        let repeated = render(&scene, OutputFormat::Png);
        assert_eq!(png.artifact().bytes(), repeated.artifact().bytes());
    }
}

#[test]
fn bound_text_with_an_empty_line_renders_to_png() {
    let scene = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"box","type":"rectangle","x":0,"y":0,"width":200,"height":120,"boundElements":[{"id":"label","type":"text"}],"seed":1,"updated":1},
        {"id":"label","type":"text","x":50,"y":22.5,"width":100,"height":75,"fontSize":20,"fontFamily":5,"lineHeight":1.25,"text":"top\n\nbottom","originalText":"top\n\nbottom","textAlign":"center","verticalAlign":"middle","containerId":"box","seed":2,"updated":2}
    ],"appState":{},"files":{}}"##;
    let png = render(scene, OutputFormat::Png);
    let svg = render(scene, OutputFormat::Svg);
    assert_eq!(codes(&png), codes(&svg));
    assert!(png.report().is_empty());
}

#[test]
fn a_whitespace_only_frame_name_renders_to_png() {
    // The frame name is laid out with the fallback font through the same
    // outline path; a name that shapes to no outline must not fail the export.
    let scene = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"frame","type":"frame","x":0,"y":0,"width":200,"height":120,"name":" ","seed":1,"updated":1},
        {"id":"child","type":"rectangle","x":20,"y":20,"width":60,"height":40,"frameId":"frame","seed":2,"updated":2}
    ],"appState":{},"files":{}}"##;
    let png = render(scene, OutputFormat::Png);
    let svg = render(scene, OutputFormat::Svg);
    assert_eq!(codes(&png), codes(&svg));
    assert!(png.report().is_empty());
    assert!(png.artifact().bytes().starts_with(b"\x89PNG"));
}
