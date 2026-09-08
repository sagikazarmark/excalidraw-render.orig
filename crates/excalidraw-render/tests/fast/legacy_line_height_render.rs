//! A text element with no usable `lineHeight` takes the restored line height:
//! detected from its height when it has one, otherwise its source family's
//! metadata line height. Decided on #108.

use excalidraw_render::{DiagnosticCode, SceneDocument};

use crate::common::svg_baselines::{assert_baselines, render_svg, tspan_baselines};

/// The vertical family metadata metrics that position a first baseline.
struct Metrics {
    units_per_em: f64,
    ascender: f64,
    descender: f64,
}

impl Metrics {
    /// `font_size * (line_height / 2 + (ascender + descender) / units_per_em / 2)`.
    fn first_baseline(&self, font_size: f64, line_height: f64) -> f64 {
        font_size * (line_height / 2.0 + (self.ascender + self.descender) / self.units_per_em / 2.0)
    }
}

const HELVETICA: Metrics = Metrics {
    units_per_em: 2048.0,
    ascender: 1577.0,
    descender: -471.0,
};
const CASCADIA: Metrics = Metrics {
    units_per_em: 2048.0,
    ascender: 1900.0,
    descender: -480.0,
};
const NUNITO: Metrics = Metrics {
    units_per_em: 1000.0,
    ascender: 1011.0,
    descender: -353.0,
};
const EXCALIFONT: Metrics = Metrics {
    units_per_em: 1000.0,
    ascender: 886.0,
    descender: -374.0,
};

fn parse(scene: &[u8]) -> SceneDocument {
    SceneDocument::parse(scene).expect("text scene is admitted")
}

#[test]
fn text_with_a_height_and_no_line_height_detects_it_from_the_height() {
    // The shape the older fixtures in this suite use: family 2 and 3 elements
    // at height 30 and fontSize 20 detect 1.5, not the family's 1.15 or 1.2.
    let outcome = render_svg(&parse(
        br##"{"type":"excalidraw","version":2,"elements":[
            {"id":"helvetica","type":"text","x":0,"y":0,"width":100,"height":30,
             "fontSize":20,"fontFamily":2,"text":"First","updated":1},
            {"id":"cascadia","type":"text","x":120,"y":0,"width":100,"height":30,
             "fontSize":20,"fontFamily":3,"text":"Code","updated":2},
            {"id":"two-lines","type":"text","x":240,"y":0,"width":100,"height":48,
             "fontSize":12,"fontFamily":2,"text":"Two\nlines","updated":3}
        ],"appState":{},"files":{}}"##,
    ));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    // 48 / 2 lines / 12 = 2.0: the line step is the height split over the lines.
    let two_line_first = HELVETICA.first_baseline(12.0, 2.0);
    assert_baselines(
        &tspan_baselines(svg),
        &[
            HELVETICA.first_baseline(20.0, 1.5),
            CASCADIA.first_baseline(20.0, 1.5),
            two_line_first,
            two_line_first + 24.0,
        ],
    );

    // A missing lineHeight is silent; only the Helvetica substitution reports.
    let codes = outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| (diagnostic.code(), diagnostic.element_id()))
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        [
            (
                DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED,
                Some("helvetica")
            ),
            (
                DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED,
                Some("two-lines")
            ),
        ]
    );
}

#[test]
fn text_without_a_height_or_line_height_takes_its_family_metadata_line_height() {
    let outcome = render_svg(&parse(
        br##"{"type":"excalidraw","version":2,"elements":[
            {"id":"nunito","type":"text","x":0,"y":0,"width":100,
             "fontSize":20,"fontFamily":6,"text":"Nunito","updated":1},
            {"id":"excalifont","type":"text","x":120,"y":0,"width":100,"height":0,
             "fontSize":20,"fontFamily":5,"text":"Control","updated":2},
            {"id":"helvetica","type":"text","x":240,"y":0,"width":100,
             "fontSize":20,"fontFamily":2,"text":"Helvetica","updated":3}
        ],"appState":{},"files":{}}"##,
    ));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert_baselines(
        &tspan_baselines(svg),
        &[
            NUNITO.first_baseline(20.0, 1.35),
            EXCALIFONT.first_baseline(20.0, 1.25),
            HELVETICA.first_baseline(20.0, 1.15),
        ],
    );
}
