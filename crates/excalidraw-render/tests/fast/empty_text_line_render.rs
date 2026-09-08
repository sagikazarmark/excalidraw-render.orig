//! A text line with no glyph outline — an empty line between paragraphs, a
//! trailing line break, or whitespace only — paints nothing in PNG and still
//! takes its line step, as it does in SVG and PDF. Diagnosed on #111, where
//! the PNG encoder failed the whole artifact on such a line.

use excalidraw_render::{OutputFormat, RenderOutcome};

use crate::common::raster::{decode_png, DecodedPng};
use crate::common::render::render_source;

/// The rows of the raster that carry any non-background ink.
fn inked_rows(png: &DecodedPng) -> Vec<u32> {
    (0..png.height)
        .filter(|row| {
            (0..png.width)
                .any(|column| png.pixel(column as usize, *row as usize)[..3] != [0xff, 0xff, 0xff])
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
    let png = render_source(&scene, OutputFormat::Png);
    let svg = render_source(&scene, OutputFormat::Svg);
    assert_eq!(codes(&png), codes(&svg));
    assert!(png.report().is_empty());

    let decoded = decode_png(png.artifact().bytes());
    // Default padding 10; lines are 25px apart, so line 2 spans rows 35..60.
    assert_eq!((decoded.width, decoded.height), (120, 95));
    let rows = inked_rows(&decoded);
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
        let png = render_source(&scene, OutputFormat::Png);
        let svg = render_source(&scene, OutputFormat::Svg);
        let pdf = render_source(&scene, OutputFormat::Pdf);
        assert_eq!(codes(&png), codes(&svg), "{text:?}");
        assert_eq!(codes(&png), codes(&pdf), "{text:?}");
        assert!(png.report().is_empty(), "{text:?}: {:?}", png.report());
        assert!(png.artifact().bytes().starts_with(b"\x89PNG"), "{text:?}");

        let repeated = render_source(&scene, OutputFormat::Png);
        assert_eq!(png.artifact().bytes(), repeated.artifact().bytes());
    }
}

#[test]
fn bound_text_with_an_empty_line_renders_to_png() {
    let scene = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"box","type":"rectangle","x":0,"y":0,"width":200,"height":120,"boundElements":[{"id":"label","type":"text"}],"seed":1,"updated":1},
        {"id":"label","type":"text","x":50,"y":22.5,"width":100,"height":75,"fontSize":20,"fontFamily":5,"lineHeight":1.25,"text":"top\n\nbottom","originalText":"top\n\nbottom","textAlign":"center","verticalAlign":"middle","containerId":"box","seed":2,"updated":2}
    ],"appState":{},"files":{}}"##;
    let png = render_source(scene, OutputFormat::Png);
    let svg = render_source(scene, OutputFormat::Svg);
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
    let png = render_source(scene, OutputFormat::Png);
    let svg = render_source(scene, OutputFormat::Svg);
    assert_eq!(codes(&png), codes(&svg));
    assert!(png.report().is_empty());
    assert!(png.artifact().bytes().starts_with(b"\x89PNG"));
}
