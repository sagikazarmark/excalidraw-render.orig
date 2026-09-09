//! Helvetica-family text (`fontFamily: 2`) is shaped with the bundled Liberation
//! Sans substitute but positioned by Excalidraw's metadata metrics for
//! Helvetica, as `getVerticalOffset` does in the reference renderer. Decided on
//! #105. The layout itself is tested at `text::OfficialFont::layout`
//! (`render/tests.rs`); this render proves the resolve stage hands each element
//! its own source family, and the embeddable label Helvetica's, when the three
//! share one loaded font.

use excalidraw_render::SceneDocument;

use crate::common::svg::{font_family_attribute, LIBERATION_FAMILIES};
use crate::common::svg_baselines::{assert_baselines, render_svg, tspan_baselines};

/// `1.15 / 2 + (1577 - 471) / 2048 / 2`: the first-baseline factor for
/// Helvetica at `lineHeight: 1.15`.
const HELVETICA_FACTOR: f64 = 0.84501953125;
/// `1.15 / 2 + (1854 - 434) / 2048 / 2`: the same factor for Liberation Sans,
/// whose metadata metrics are the bundled face's own.
const LIBERATION_SANS_FACTOR: f64 = 0.9216796875;

#[test]
fn helvetica_and_liberation_sans_text_keep_their_own_metrics_when_they_share_a_font() {
    // Families 2 and 9 share one loaded Liberation Sans font. Whichever loads
    // first must not decide the other's baseline, and neither must the label,
    // which the official export sizes at width / 30 and centers as one line.
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
    assert!(svg.contains(&font_family_attribute(LIBERATION_FAMILIES)));
}
