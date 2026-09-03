//! Helvetica-family text (`fontFamily: 2`) is shaped with the bundled Liberation
//! Sans substitute but positioned by Excalidraw's metadata metrics for
//! Helvetica, as `getVerticalOffset` does in the reference renderer. Decided on
//! #105.

use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, Renderer, SceneDocument};

/// `1.15 / 2 + (1577 - 471) / 2048 / 2`: the first-baseline factor for
/// Helvetica at `lineHeight: 1.15`.
const HELVETICA_FACTOR: f64 = 0.84501953125;
/// `1.15 / 2 + (1854 - 434) / 2048 / 2`: the same factor for Liberation Sans,
/// whose metadata metrics are the bundled face's own.
const LIBERATION_SANS_FACTOR: f64 = 0.9216796875;

fn two_line_text_scene(font_family: u32) -> SceneDocument {
    SceneDocument::parse(
        format!(
            r##"{{"type":"excalidraw","version":2,"elements":[{{
                "id":"text","type":"text","x":20,"y":30,"width":200,"height":64.4,
                "fontSize":28,"fontFamily":{font_family},"lineHeight":1.15,
                "text":"Two lines\nof Helvetica","textAlign":"center","updated":1
            }}],"appState":{{}},"files":{{}}}}"##
        )
        .as_bytes(),
    )
    .expect("two-line text scene is admitted")
}

fn render_svg(scene: &SceneDocument) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("text scene renders")
}

/// The baseline of every `tspan`, taken from the first `y` value: ordinary text
/// emits one baseline per line and generated labels emit one per glyph, all on
/// the line's baseline.
fn tspan_baselines(svg: &str) -> Vec<f64> {
    svg.split("<tspan ")
        .skip(1)
        .map(|tspan| {
            tspan
                .split(" y=\"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .and_then(|value| value.split(' ').next())
                .expect("tspan carries a baseline")
                .parse::<f64>()
                .expect("baseline is a number")
        })
        .collect()
}

fn assert_baselines(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "line count: {actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "baseline {actual} should be {expected}"
        );
    }
}

#[test]
fn helvetica_scene_text_is_positioned_by_excalidraw_metadata_metrics() {
    let outcome = render_svg(&two_line_text_scene(2));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    let line_step = 28.0 * 1.15;
    let first = 28.0 * HELVETICA_FACTOR;
    assert_baselines(&tspan_baselines(svg), &[first, first + line_step]);
    assert!(svg.contains("font-family=\"ExcalidrawRender-9\""));

    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("Helvetica text reports exactly one substitution");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED
    );
}

#[test]
fn liberation_sans_scene_text_is_positioned_by_its_own_family_metadata_metrics() {
    let outcome = render_svg(&two_line_text_scene(9));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    let line_step = 28.0 * 1.15;
    let first = 28.0 * LIBERATION_SANS_FACTOR;
    assert_baselines(&tspan_baselines(svg), &[first, first + line_step]);
    assert!(outcome.report().is_empty());
}

#[test]
fn embeddable_label_baseline_follows_helvetica_metadata_metrics() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{
            "id":"embed","type":"embeddable","x":10,"y":10,"width":280,"height":180,
            "link":"https://example.com/embedded-page","updated":1
        }],"appState":{},"files":{}}"##,
    )
    .expect("embeddable scene is admitted");
    let outcome = render_svg(&scene);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    // The official export sizes this label at width / 30 and lays out one line.
    let font_size = 280.0 / 30.0;
    let block_top = (180.0 - font_size * 1.15) / 2.0;
    assert_baselines(
        &tspan_baselines(svg),
        &[block_top + font_size * HELVETICA_FACTOR],
    );
}

#[test]
fn helvetica_and_liberation_sans_text_keep_their_own_metrics_when_they_share_a_font() {
    // Families 2 and 9 share one loaded Liberation Sans font. Whichever loads
    // first must not decide the other's baseline, and neither must the label.
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[
            {"id":"liberation","type":"text","x":0,"y":0,"width":200,"height":23,
             "fontSize":20,"fontFamily":9,"lineHeight":1.15,"text":"Liberation first","updated":1},
            {"id":"helvetica","type":"text","x":0,"y":40,"width":200,"height":23,
             "fontSize":20,"fontFamily":2,"lineHeight":1.15,"text":"Helvetica second","updated":2},
            {"id":"embed","type":"embeddable","x":0,"y":80,"width":280,"height":180,
             "link":"https://example.com/embedded-page","updated":3}
        ],"appState":{},"files":{}}"##,
    )
    .expect("mixed-family scene is admitted");
    let outcome = render_svg(&scene);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    let label_font_size = 280.0 / 30.0;
    let label_block_top = (180.0 - label_font_size * 1.15) / 2.0;
    assert_baselines(
        &tspan_baselines(svg),
        &[
            20.0 * LIBERATION_SANS_FACTOR,
            20.0 * HELVETICA_FACTOR,
            label_block_top + label_font_size * HELVETICA_FACTOR,
        ],
    );
}
