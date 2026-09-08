//! A text element's `text` breaks lines on `\r\n` and `\r` as well as `\n`,
//! as the reference's `text.replace(/\r\n?/g, "\n").split("\n")` does, while
//! the stored bytes stay as they are. Decided on #110.

use excalidraw_render::{OutputFormat, RenderOutcome, SceneDocument};

use crate::common::render::render;
use crate::common::svg_baselines::{assert_baselines, render_svg, tspan_baselines};

/// Excalifont's metadata metrics: `20 * (lineHeight / 2 + (886 - 374) / 1000 / 2)`.
fn excalifont_first_baseline(font_size: f64, line_height: f64) -> f64 {
    font_size * (line_height / 2.0 + (886.0 - 374.0) / 1000.0 / 2.0)
}

/// One Excalifont text element at 20 px; `line_height` `None` leaves
/// `lineHeight` out so the restored line height derives from `height`.
fn text_scene(text: &str, height: f64, line_height: Option<f64>) -> SceneDocument {
    let line_height =
        line_height.map_or(String::new(), |value| format!(r#""lineHeight":{value},"#));
    let scene = format!(
        r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"text","type":"text","x":0,"y":0,"width":100,"height":{height},"fontSize":20,"fontFamily":5,{line_height}"text":{text},"originalText":{text},"seed":1,"updated":1}}],"appState":{{}},"files":{{}}}}"##,
        text = serde_json::to_string(text).expect("text serializes"),
    );
    SceneDocument::parse(scene.as_bytes()).expect("text scene is admitted")
}

fn svg_text(outcome: &RenderOutcome) -> &str {
    std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8")
}

/// The `<text>` element's `data-source-text` attribute, still escaped.
fn data_source_text(svg: &str) -> &str {
    svg.split("data-source-text=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("ordinary text carries data-source-text")
}

/// Everything from the first `<tspan` to `</text>`: the laid-out lines.
fn tspans(svg: &str) -> &str {
    let start = svg.find("<tspan").expect("text has a line");
    let end = svg.find("</text>").expect("text closes");
    &svg[start..end]
}

#[test]
fn crlf_and_cr_break_lines_like_lf_and_keep_their_source_bytes() {
    let lf = render_svg(&text_scene("a\nb", 50.0, Some(1.25)));
    let crlf = render_svg(&text_scene("a\r\nb", 50.0, Some(1.25)));
    let cr = render_svg(&text_scene("a\rb", 50.0, Some(1.25)));

    // Two lines 25 px apart in every case.
    let first = excalifont_first_baseline(20.0, 1.25);
    for outcome in [&lf, &crlf, &cr] {
        assert_baselines(&tspan_baselines(svg_text(outcome)), &[first, first + 25.0]);
        assert!(outcome.report().is_empty(), "{:?}", outcome.report());
    }

    // The laid-out lines are the same markup; only the source attribute
    // carries the element's own line endings.
    assert_eq!(tspans(svg_text(&crlf)), tspans(svg_text(&lf)));
    assert_eq!(tspans(svg_text(&cr)), tspans(svg_text(&lf)));
    assert_eq!(data_source_text(svg_text(&lf)), "a&#10;b");
    assert_eq!(data_source_text(svg_text(&crlf)), "a&#13;&#10;b");
    assert_eq!(data_source_text(svg_text(&cr)), "a&#13;b");
}

#[test]
fn a_restored_line_height_agrees_with_the_lines_laid_out() {
    // No lineHeight: 60 / 2 lines / 20 restores 1.5 for CRLF text, so the two
    // lines sit 30 px apart, as they would with LF endings.
    let crlf = render_svg(&text_scene("a\r\nb", 60.0, None));
    let cr = render_svg(&text_scene("a\rb", 60.0, None));
    let first = excalifont_first_baseline(20.0, 1.5);
    assert_baselines(&tspan_baselines(svg_text(&crlf)), &[first, first + 30.0]);
    assert_baselines(&tspan_baselines(svg_text(&cr)), &[first, first + 30.0]);
    assert!(crlf.report().is_empty(), "{:?}", crlf.report());
}

#[test]
fn line_endings_do_not_change_the_raster_or_the_diagnostics() {
    let lf = text_scene("a\nb", 50.0, Some(1.25));
    let png = render(&lf, OutputFormat::Png);
    assert!(png.artifact().bytes().starts_with(b"\x89PNG"));
    for text in ["a\r\nb", "a\rb"] {
        let scene = text_scene(text, 50.0, Some(1.25));
        let other = render(&scene, OutputFormat::Png);
        assert_eq!(
            other.artifact().bytes(),
            png.artifact().bytes(),
            "{text:?} rasterizes as LF text does"
        );
        for format in [OutputFormat::Png, OutputFormat::Svg, OutputFormat::Pdf] {
            let outcome = render(&scene, format);
            assert!(
                outcome.report().is_empty(),
                "{text:?} {format:?}: {:?}",
                outcome.report()
            );
        }
    }
}
