use std::io::Cursor;

use excalidraw_render::{
    ErrorCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use png::{ColorType, Decoder, Transformations};
use sha2::{Digest, Sha256};

const SCENE: &[u8] = br##"{
  "type":"excalidraw","version":2,
  "elements":[
    {"id":"rectangle","type":"rectangle","x":10,"y":10,"width":80,"height":50,"seed":1,"updated":1},
    {"id":"rounded-rectangle","type":"rectangle","x":110,"y":10,"width":80,"height":50,"roundness":{"type":3},"seed":2,"updated":1},
    {"id":"ellipse","type":"ellipse","x":210,"y":10,"width":80,"height":50,"seed":3,"updated":1},
    {"id":"diamond","type":"diamond","x":310,"y":10,"width":80,"height":50,"seed":4,"updated":1},
    {"id":"rounded-diamond","type":"diamond","x":410,"y":10,"width":80,"height":50,"roundness":{"type":2},"seed":5,"updated":1},
    {"id":"line","type":"line","x":10,"y":90,"width":160,"height":40,"points":[[0,0],[80,40],[160,10]],"seed":6,"updated":1},
    {"id":"rounded-line","type":"line","x":210,"y":90,"width":160,"height":40,"points":[[0,0],[80,40],[160,10]],"roundness":{"type":2},"seed":7,"updated":1}
  ],
  "appState":{"viewBackgroundColor":"#ffffff"},"files":{}
}"##;

#[test]
fn admitted_outline_shape_families_render_directly_in_every_format() {
    let scene = SceneDocument::parse(SCENE).expect("outline family scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("default render request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("all outline families render");
        let expected_digest = match format {
            OutputFormat::Svg => "541c9f3ffc1b4363e24a8bb243302bee8a9e2939572ab0e8e19ecb334561afdf",
            OutputFormat::Png => "05f6ffb03045d532fd1450443603ee9ddf62691cfc1310f3061d0ed5cef1e1a8",
            OutputFormat::Pdf => "a2eb805c63e670500764fd8658d73553afa07d1e06c41b7dafc17d6f363ab186",
            _ => unreachable!("the test enumerates every revision-1 output format"),
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
            expected_digest,
            "exact {format:?} focused-case artifact changed"
        );

        assert!(
            outcome.report().is_empty(),
            "direct outline rendering must not use compatibility fallbacks for {format:?}: {:?}",
            outcome.report().diagnostics()
        );
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                assert_eq!(svg.matches("<path ").count(), 7);
                assert!(!svg.contains("Unavailable:"));
            }
            OutputFormat::Png => {
                let mut decoder = Decoder::new(Cursor::new(outcome.artifact().bytes()));
                decoder.set_transformations(Transformations::IDENTITY);
                let mut reader = decoder.read_info().unwrap();
                assert_eq!(reader.info().color_type, ColorType::Rgba);
                let mut rgba = vec![0; reader.output_buffer_size()];
                let frame = reader.next_frame(&mut rgba).unwrap();
                assert!(rgba[..frame.buffer_size()]
                    .chunks_exact(4)
                    .any(|pixel| pixel != [255, 255, 255, 255]));
            }
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
                assert_eq!(pdf.matches("\nS\nQ\n").count(), 7);
                assert!(!pdf.contains("Unavailable:"));
            }
            _ => unreachable!("the test enumerates every revision-1 output format"),
        }
    }
}

#[test]
fn svg_outline_operations_match_the_pinned_roughjs_vectors() {
    let scene = SceneDocument::parse(SCENE).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let paths: Vec<_> = svg
        .split("<path d=\"")
        .skip(1)
        .map(|tail| tail.split_once("\" stroke=").unwrap().0)
        .collect();

    assert_eq!(paths.len(), 7);
    assert_eq!(
        paths
            .iter()
            .map(|path| path.matches(['M', 'C']).count())
            .collect::<Vec<_>>(),
        [16, 32, 24, 16, 32, 8, 6]
    );
    let digest = Sha256::digest(paths.join("\n"));
    assert_eq!(
        format!("{digest:x}"),
        "c7f7fecb0ae243c8a19d0f650cdd937e917292559449cc40b99783e8ffbfadd6",
        "vectors were generated independently with pinned Rough.js 4.6.4"
    );
}

#[test]
fn outline_geometry_accounting_uses_each_generated_operation_count() {
    let scene = SceneDocument::parse(SCENE).unwrap();
    let request = |maximum| {
        RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(
                ResourceLimits::builder()
                    .max_geometry_segments_per_element(32)
                    .max_total_geometry_segments(maximum)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    };

    Renderer::new()
        .render(&scene, &request(134))
        .expect("the exact total Rough operation count is admitted");
    let error = Renderer::new()
        .render(&scene, &request(133))
        .expect_err("the first operation beyond the aggregate limit is rejected");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("rounded-line"));
}

#[test]
fn architect_ellipse_uses_the_single_core_roughjs_stroke() {
    let source = std::str::from_utf8(SCENE).unwrap().replacen(
        "\"seed\":3,\"updated\"",
        "\"roughness\":0,\"seed\":3,\"updated\"",
        1,
    );
    let scene = SceneDocument::parse(source.as_bytes()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let ellipse = svg
        .split("<path d=\"")
        .nth(3)
        .and_then(|tail| tail.split_once("\" stroke="))
        .unwrap()
        .0;

    assert_eq!(ellipse.matches(['M', 'C']).count(), 42);
    assert_eq!(
        format!("{:x}", Sha256::digest(ellipse)),
        "d74c0380e58abcacfb14e495406d5f38af852f32f37a0ea528ecb0f0454bec05"
    );
}

#[test]
fn cartoonist_outline_consumes_vertex_offsets_in_roughjs_order() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"cartoonist","type":"rectangle","width":8,"height":5,
            "roughness":2,"seed":8,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let path = svg
        .split_once("<path d=\"")
        .and_then(|(_, tail)| tail.split_once("\" stroke="))
        .unwrap()
        .0;

    assert_eq!(
        format!("{:x}", Sha256::digest(path)),
        "fe33b396a5e457111305917fe234efdc6f856b635e7a55ab7fc5e5ed3083c19b"
    );
}

#[test]
fn cartoonist_rounded_outline_preserves_continuous_path_vertices() {
    let source = std::str::from_utf8(SCENE).unwrap().replacen(
        "\"seed\":2,\"updated\"",
        "\"roughness\":2,\"seed\":2,\"updated\"",
        1,
    );
    let scene = SceneDocument::parse(source.as_bytes()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
    let rounded_rectangle = svg
        .split("<path d=\"")
        .nth(2)
        .and_then(|tail| tail.split_once("\" stroke="))
        .unwrap()
        .0;

    assert_eq!(
        format!("{:x}", Sha256::digest(rounded_rectangle)),
        "e9be3be60300e30a0f37ea1c58c8a7b18ddf966d468a0afb6629b3fc5f81aa7f"
    );
}

#[test]
fn negative_linear_points_define_bounds_and_rotation_center() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"negative-line","type":"line","x":100,"y":10,
            "width":80,"height":40,"angle":1.5707963267948966,
            "points":[[0,0],[-80,40]],"seed":9,"updated":1}],
        "appState":{},"files":{}
    }"##,
    )
    .unwrap();
    let request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();

    assert!(
        svg.contains("viewBox=\"0 0 60 100\""),
        "unexpected bounds: {svg}"
    );
    assert!(
        svg.contains("transform=\"translate(70 30) rotate(90 -40 20)\""),
        "unexpected linear transform: {svg}"
    );
}

#[test]
fn ellipse_ignores_inapplicable_roundness_without_consuming_randomness() {
    let rounded_source = std::str::from_utf8(SCENE).unwrap().replacen(
        "\"seed\":3,\"updated\"",
        "\"roundness\":{\"type\":2},\"seed\":3,\"updated\"",
        1,
    );
    let plain = SceneDocument::parse(SCENE).unwrap();
    let rounded = SceneDocument::parse(rounded_source.as_bytes()).unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let plain = Renderer::new().render(&plain, &request).unwrap();
        let rounded = Renderer::new().render(&rounded, &request).unwrap();
        assert_eq!(plain.artifact().bytes(), rounded.artifact().bytes());
        assert_eq!(plain.report(), rounded.report());
    }
}
