use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

use crate::common::pdf_inspect::pdf_text;
use crate::common::raster::decode_png;

const SCENE: &[u8] = br##"{
  "type":"excalidraw","version":2,
  "elements":[
    {"id":"solid","type":"rectangle","x":10,"y":10,"width":80,"height":50,"backgroundColor":"#e03131","fillStyle":"solid","seed":11,"updated":1},
    {"id":"hachure","type":"rectangle","x":110,"y":10,"width":80,"height":50,"backgroundColor":"#2f9e44","fillStyle":"hachure","seed":12,"updated":1},
    {"id":"cross-hatch","type":"rectangle","x":210,"y":10,"width":80,"height":50,"backgroundColor":"#1971c2","fillStyle":"cross-hatch","strokeStyle":"dashed","seed":13,"updated":1},
    {"id":"zigzag","type":"rectangle","x":310,"y":10,"width":80,"height":50,"backgroundColor":"#f08c00","fillStyle":"zigzag","strokeStyle":"dotted","seed":14,"updated":1}
  ],
  "appState":{"viewBackgroundColor":"#ffffff"},"files":{}
}"##;

#[test]
pub(super) fn rough_fills_and_stroke_variants_render_directly_in_every_format() {
    let scene = SceneDocument::parse(SCENE).expect("style scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let first = Renderer::new().render(&scene, &request).unwrap();

        assert!(
            first.report().is_empty(),
            "unexpected {format:?} diagnostics"
        );

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).unwrap();
                assert_eq!(svg.matches("<path ").count(), 8);
                assert!(svg.contains("stroke-dasharray=\"8 10\""));
                assert!(svg.contains("stroke-dasharray=\"1.5 8\""));
                let paths = svg
                    .split("<path d=\"")
                    .skip(1)
                    .map(|tail| tail.split_once('"').unwrap().0)
                    .collect::<Vec<_>>();
                assert_eq!(
                    format!("{:x}", Sha256::digest(paths.join("\n"))),
                    "2bfc1930d3b3801246550bfb278b4a0e8dc43809b04adbc9dcf1cf9474ec255c"
                );
            }
            OutputFormat::Png => {
                let decoded = decode_png(first.artifact().bytes());
                let pixels = decoded.rgba.as_chunks::<4>().0.iter();
                assert!(pixels.clone().any(|pixel| pixel[0] > pixel[1]));
                assert!(pixels.clone().any(|pixel| pixel[1] > pixel[0]));
                assert!(pixels.clone().any(|pixel| pixel[2] > pixel[0]));
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(first.artifact().bytes());
                assert!(pdf.matches("\nf\n").count() >= 1);
                assert!(pdf.matches("\nS\n").count() >= 7);
                assert!(pdf.contains("[8 10] 0 d"));
                assert!(pdf.contains("[1.5 8] 0 d"));
                assert!(!pdf.contains("/Subtype /Image"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
pub(super) fn dots_dashed_and_zigzag_line_fills_render_undiagnosed_in_every_format() {
    // The three fillers ported for #137 render through the same Rough
    // `Drawing` as the other four, so every format paints them and none
    // diagnoses the value; before the port each was substituted by `solid`
    // under `compatibility.unsupported-property-value`, as a value outside
    // the domain still is.
    let scene = SceneDocument::parse(br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"dots","type":"rectangle","x":10,"y":10,"width":80,"height":50,"backgroundColor":"#e03131","fillStyle":"dots","seed":41,"updated":1},
        {"id":"dashed","type":"ellipse","x":110,"y":10,"width":80,"height":50,"backgroundColor":"#2f9e44","fillStyle":"dashed","seed":42,"updated":1},
        {"id":"zigzag-line","type":"diamond","x":210,"y":10,"width":80,"height":50,"backgroundColor":"#1971c2","fillStyle":"zigzag-line","seed":43,"updated":1},
        {"id":"unknown","type":"rectangle","x":310,"y":10,"width":80,"height":50,"backgroundColor":"#f08c00","fillStyle":"stipple","seed":44,"updated":1}
      ],
      "appState":{"viewBackgroundColor":"#ffffff"},"files":{}
    }"##)
    .expect("filler scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        let diagnostics = outcome.report().diagnostics();
        assert_eq!(
            diagnostics.len(),
            1,
            "{format:?}: only the unknown value is diagnosed"
        );
        assert_eq!(
            diagnostics[0].code(),
            DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE
        );
        assert_eq!(diagnostics[0].json_pointer(), "/elements/3/fillStyle");

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                // Three sketched fills stroked at the fill weight, one solid
                // fill for the substituted value, and four outlines.
                assert_eq!(svg.matches("<path ").count(), 8);
                assert_eq!(svg.matches("stroke-width=\"1\"").count(), 3);
                let paths = svg
                    .split("<path d=\"")
                    .skip(1)
                    .map(|tail| tail.split_once('"').unwrap().0)
                    .collect::<Vec<_>>();
                // The three fills are the vectors
                // `render::tests::dots_dashed_and_zigzag_line_fills_match_the_pinned_roughjs_vectors`
                // pins for the same elements against the npm Rough.js.
                assert_eq!(
                    format!("{:x}", Sha256::digest(paths[0])),
                    "3e71799dc22c6afd54dfbe1034b6f7104760ebfcf3d2a814f21f923f73cea877"
                );
                assert_eq!(
                    format!("{:x}", Sha256::digest(paths[2])),
                    "86d629cf4d59ca89c11c073e8279b5f6a09eb11f0d47c920f49439f7c614b0cf"
                );
                assert_eq!(
                    format!("{:x}", Sha256::digest(paths[4])),
                    "b323939a39be484b021100620064bab064c5bb7ef4fa8121e1182710c52e30b5"
                );
            }
            OutputFormat::Png => {
                let decoded = decode_png(outcome.artifact().bytes());
                let pixels = decoded.rgba.as_chunks::<4>().0.iter();
                assert!(pixels.clone().any(|pixel| pixel[0] > pixel[1]));
                assert!(pixels.clone().any(|pixel| pixel[1] > pixel[0]));
                assert!(pixels.clone().any(|pixel| pixel[2] > pixel[0]));
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                // The page background and the substituted solid fill are the
                // only fills; the three sketched fills and four outlines
                // stroke.
                assert_eq!(pdf.matches("\nf\n").count(), 2);
                assert!(pdf.matches("\nS\n").count() >= 7);
                assert!(!pdf.contains("/Subtype /Image"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
pub(super) fn patterned_curved_boundaries_match_pinned_js_sampling() {
    let scene = SceneDocument::parse(br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"rounded-path","type":"rectangle","x":10,"y":10,"width":80,"height":50,"roundness":{"type":3},"backgroundColor":"#e03131","fillStyle":"hachure","seed":31,"updated":1},
        {"id":"rounded-curve","type":"line","x":110,"y":10,"width":80,"height":50,"points":[[0,0],[80,0],[80,50],[0,50],[4,3]],"roundness":{"type":2},"backgroundColor":"#e03131","fillStyle":"hachure","seed":32,"updated":1}
      ],
      "appState":{},"files":{}
    }"##).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let paths = svg
        .split("<path d=\"")
        .skip(1)
        .map(|tail| tail.split_once('"').unwrap().0)
        .collect::<Vec<_>>();

    assert!(outcome.report().is_empty());
    assert_eq!(paths.len(), 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(paths[0])),
        "c211c4346fa692dc825164e947714aaed5b66e99ae2fcecdfe43ef656683bfb0",
        "rounded path fill was generated independently with pinned Rough.js 4.6.4",
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(paths[2])),
        "9176fe003ade6804f0f3e9fe2b79aa67835300b4999ac2c72e43e639d0748f06",
        "rounded curve fill was generated independently with pinned Rough.js 4.6.4",
    );
}

#[test]
fn solid_fill_rules_and_joins_follow_each_backend() {
    let scene = SceneDocument::parse(br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"rectangle","type":"rectangle","x":10,"y":10,"width":40,"height":30,"backgroundColor":"#e03131","fillStyle":"solid","seed":41,"updated":1},
        {"id":"rounded-path","type":"rectangle","x":60,"y":10,"width":40,"height":30,"roundness":{"type":3},"backgroundColor":"#e03131","fillStyle":"solid","seed":42,"updated":1},
        {"id":"ellipse","type":"ellipse","x":110,"y":10,"width":40,"height":30,"backgroundColor":"#e03131","fillStyle":"solid","seed":43,"updated":1},
        {"id":"polygon","type":"diamond","x":160,"y":10,"width":40,"height":30,"backgroundColor":"#e03131","fillStyle":"solid","seed":44,"updated":1},
        {"id":"near-loop","type":"line","x":210,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[6.4,4.8]],"backgroundColor":"#e03131","fillStyle":"solid","seed":45,"updated":1},
        {"id":"curve","type":"line","x":260,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[6.4,4.8]],"roundness":{"type":2},"backgroundColor":"#e03131","fillStyle":"solid","seed":46,"updated":1}
      ],
      "appState":{},"files":{}
    }"##).unwrap();

    let svg = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Svg).build().unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(svg.artifact().bytes()).unwrap();
    assert_eq!(svg.matches("fill-rule=\"evenodd\"").count(), 3);
    assert_eq!(svg.matches("stroke-linecap=\"round\"").count(), 6);
    assert!(!svg.contains("stroke-linejoin"));

    let pdf = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Pdf).build().unwrap(),
        )
        .unwrap();
    let pdf = pdf_text(pdf.artifact().bytes());
    assert_eq!(pdf.matches("\nf*\n").count(), 4);
    assert!(pdf.matches("\n1 J\n").count() >= 6);
    assert!(!pdf.contains("\n1 j\n"));
}

#[test]
fn rounded_solid_fill_geometry_budget_counts_only_retained_operations() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"rounded-solid","type":"rectangle","width":80,"height":50,
            "roundness":{"type":3},"backgroundColor":"#e03131","fillStyle":"solid",
            "seed":81,"updated":1}],"appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let request = |maximum| {
        RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(
                ResourceLimits::builder()
                    .max_geometry_segments_per_element(maximum)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    };

    let equality = Renderer::new().render(&scene, &request(41)).unwrap();
    assert!(equality.report().is_empty());

    let crossing = Renderer::new().render(&scene, &request(40)).unwrap();
    let diagnostic = crossing
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
        })
        .expect("the first retained operation crossing has a resource diagnostic");
    let descriptor = serde_json::to_value(diagnostic).unwrap();
    assert_eq!(descriptor["details"]["configured"], 40);
    assert_eq!(descriptor["details"]["measurement"]["value"], 41);
}
