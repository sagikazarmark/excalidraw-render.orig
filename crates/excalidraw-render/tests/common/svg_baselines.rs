//! Baseline assertions over a rendered SVG document, shared by the text tests
//! that pin where ordinary scene text and generated labels sit.

use excalidraw_render::{OutputFormat, RenderOutcome, RenderRequest, Renderer, SceneDocument};

/// Renders one scene document to SVG.
pub fn render_svg(scene: &SceneDocument) -> RenderOutcome {
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
pub fn tspan_baselines(svg: &str) -> Vec<f64> {
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

pub fn assert_baselines(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "line count: {actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "baseline {actual} should be {expected}"
        );
    }
}
