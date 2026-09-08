use std::io::Cursor;

use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use png::{Decoder, Transformations};
use sha2::{Digest, Sha256};

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
fn rough_fills_and_stroke_variants_render_directly_in_every_format() {
    let scene = SceneDocument::parse(SCENE).expect("style scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let first = Renderer::new().render(&scene, &request).unwrap();
        let second = Renderer::new().render(&scene, &request).unwrap();

        assert!(
            first.report().is_empty(),
            "unexpected {format:?} diagnostics"
        );
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());

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
                let mut decoder = Decoder::new(Cursor::new(first.artifact().bytes()));
                decoder.set_transformations(Transformations::IDENTITY);
                let mut reader = decoder.read_info().unwrap();
                let mut rgba = vec![0; reader.output_buffer_size()];
                let frame = reader.next_frame(&mut rgba).unwrap();
                let pixels = rgba[..frame.buffer_size()].chunks_exact(4);
                assert!(pixels.clone().any(|pixel| pixel[0] > pixel[1]));
                assert!(pixels.clone().any(|pixel| pixel[1] > pixel[0]));
                assert!(pixels.clone().any(|pixel| pixel[2] > pixel[0]));
            }
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(first.artifact().bytes());
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
fn rough_fill_vectors_match_the_pinned_geometry_families() {
    let scene = SceneDocument::parse(br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"rounded","type":"rectangle","x":10,"y":10,"width":80,"height":50,"roundness":{"type":3},"backgroundColor":"#e03131","fillStyle":"solid","seed":21,"updated":1},
        {"id":"ellipse","type":"ellipse","x":110,"y":10,"width":80,"height":50,"backgroundColor":"#e03131","fillStyle":"hachure","seed":22,"updated":1},
        {"id":"diamond","type":"diamond","x":210,"y":10,"width":80,"height":50,"backgroundColor":"#e03131","fillStyle":"cross-hatch","seed":23,"updated":1},
        {"id":"loop","type":"line","x":310,"y":10,"width":80,"height":50,"points":[[0,0],[80,0],[80,50],[0,50],[0,0]],"backgroundColor":"#e03131","fillStyle":"zigzag","seed":24,"updated":1}
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
    assert_eq!(paths.len(), 8);
    assert_eq!(
        format!("{:x}", Sha256::digest(paths.join("\n"))),
        // Moved with the diamond vertex fix: the scene carries two diamonds.
        "d4e7528ff1a10f3ebbfbb8f3b404728ce7bef805930d56384652c2be6dc53f6d"
    );
}

#[test]
fn patterned_curved_boundaries_match_pinned_js_sampling() {
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
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert_eq!(pdf.matches("\nf*\n").count(), 4);
    assert!(pdf.matches("\n1 J\n").count() >= 6);
    assert!(!pdf.contains("\n1 j\n"));
}

#[test]
fn line_fill_uses_the_exact_export_zoom_loop_threshold() {
    let scene = SceneDocument::parse(br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"at-threshold","type":"line","x":10,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[6.4,4.8]],"backgroundColor":"#e03131","fillStyle":"solid","seed":51,"updated":1},
        {"id":"outside-threshold","type":"line","x":60,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[6.408,4.806]],"backgroundColor":"#e03131","fillStyle":"solid","seed":52,"updated":1}
      ],
      "appState":{},"files":{}
    }"##).unwrap();
    let outcome = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Svg).build().unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();

    assert_eq!(svg.matches("<path ").count(), 3);
    assert_eq!(svg.matches("fill-rule=\"evenodd\"").count(), 1);
}

#[test]
fn repeated_loop_endpoint_keeps_the_polygon_generators_extra_close() {
    let scene = SceneDocument::parse(br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"repeated","type":"line","x":10,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[0,0]],"backgroundColor":"#e03131","fillStyle":"solid","seed":61,"updated":1}
      ],
      "appState":{},"files":{}
    }"##).unwrap();
    let outcome = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Svg).build().unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let outline = svg
        .split("<path d=\"")
        .nth(2)
        .and_then(|tail| tail.split_once('"'))
        .unwrap()
        .0;

    assert!(
        outline.ends_with("M0 0 C0 0, 0 0, 0 0 M0 0 C0 0, 0 0, 0 0"),
        "unexpected repeated-close outline: {outline}",
    );
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
