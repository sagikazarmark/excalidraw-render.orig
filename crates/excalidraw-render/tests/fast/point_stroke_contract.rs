//! Point, pressure and arrow field contracts at the public facade.
use crate::common::lower_hex;
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
fn scene(kind: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"frame","type":"frame","width":100,"height":80,"updated":0},
    {"id":"stroke","type":kind,"x":10,"y":10,"width":40,"height":30,"points":[[0,0],[20,0],[20,30],[40,30]],"seed":1,"updated":0},
    {"id":"other","type":"magicframe","x":200,"width":100,"height":80,"updated":0}],"appState":{},"files":{}})
}
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn values() -> Vec<Value> {
    vec![
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(-1),
        json!(0.5),
        json!(""),
        json!("bad"),
        json!([]),
        json!([1]),
        json!({}),
    ]
}
fn repair(field: &str, recovery: &str) -> Value {
    json!({"code":"compatibility.unsupported-property-value","severity":"warning","effects":if recovery=="placeholder"{json!(["visual"])}else{json!(["input_repair","visual"])},"element_id":"stroke","json_pointer":format!("/elements/1/{field}"),"details":{"kind":"compatibility","recovery":recovery,"recipe_id":if recovery=="placeholder"{json!("renderer.known-element-unavailable.v1")}else{Value::Null}}})
}
fn assert_report(outcome: &RenderOutcome, format: OutputFormat, mut rows: Value) {
    let mut actual = serde_json::to_value(outcome.report()).unwrap();
    for row in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    for row in rows.as_array_mut().unwrap() {
        row["format"] = json!(format.as_str());
    }
    assert_eq!(actual, json!({"schema_version":1,"diagnostics":rows}));
}
fn assert_contract(source: &Value, clean: &Value, format: OutputFormat, rows: Value) {
    let document = parse(source);
    let clean_document = parse(clean);
    let renderer = Renderer::new();
    let request = RenderRequest::builder(format).build().unwrap();
    let first = renderer.render(&document, &request).unwrap();
    let second = Renderer::new().render(&document, &request).unwrap();
    let baseline = renderer.render(&clean_document, &request).unwrap();
    assert_eq!(
        first.artifact().bytes(),
        baseline.artifact().bytes(),
        "{format:?}: {source}"
    );
    assert_report(&first, format, rows.clone());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let request = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let embedded = renderer.render(&document, &request).unwrap();
    assert_report(&embedded, format, rows.clone());
    assert_eq!(
        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
    let frames: Vec<_> = document.frames().collect();
    let clean_frames: Vec<_> = clean_document.frames().collect();
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for (index, (frame, clean_frame)) in frames.iter().zip(clean_frames).enumerate() {
        let direct = renderer.render_frame(*frame, &request).unwrap();
        let clean = renderer.render_frame(clean_frame, &request).unwrap();
        let replay = batch.next().unwrap().into_result().unwrap();
        assert_eq!(direct.artifact().bytes(), clean.artifact().bytes());
        assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
        assert_eq!(direct.report(), replay.report());
        assert_report(
            &direct,
            format,
            if index == 0 { rows.clone() } else { json!([]) },
        );
    }
    assert!(batch.next().is_none());
    let mut deleted = source.clone();
    deleted["elements"][1]["isDeleted"] = json!(true);
    let result = renderer
        .render(
            &parse(&deleted),
            &RenderRequest::builder(format).build().unwrap(),
        )
        .unwrap();
    assert_report(&result, format, json!([]));
}

#[test]
fn linear_points_default_short_geometry_rebase_origin_and_migrate_draw() {
    malformed_point_components_have_narrow_placeholder_recovery();
    for kind in ["line", "arrow", "draw"] {
        let original = scene(kind);
        let mut default = original.clone();
        default["elements"][1]["points"] = json!([[0, 0], [40, 30]]);
        for value in
            std::iter::once(None).chain(values().into_iter().chain([json!([[8, 9]])]).map(Some))
        {
            let mut source = original.clone();
            if let Some(value) = &value {
                source["elements"][1]["points"] = value.clone();
            } else {
                source["elements"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("points");
            }
            let rows = if value.as_ref().is_none_or(Value::is_array) {
                json!([])
            } else {
                json!([repair("points", "substituted")])
            };
            for format in FORMATS {
                assert_contract(&source, &default, format, rows.clone());
            }
        }
        let mut source = original.clone();
        source["elements"][1]["points"] = json!([[5, -5], [25, 10], [-5, 25]]);
        let mut clean = original.clone();
        clean["elements"][1]["x"] = json!(15);
        clean["elements"][1]["y"] = json!(5);
        clean["elements"][1]["points"] = json!([[0, 0], [20, 15], [-10, 30]]);
        clean["elements"][1]["width"] = json!(30);
        clean["elements"][1]["height"] = json!(30);
        for format in FORMATS {
            assert_contract(&source, &clean, format, json!([]));
        }
        if kind == "draw" {
            let mut clean = original.clone();
            clean["elements"][1]["type"] = json!("line");
            for format in FORMATS {
                assert_contract(&original, &clean, format, json!([]));
            }
        }
    }
}

#[test]
fn arrowheads_and_elbows_have_exact_defaults_recovery_and_valid_geometry() {
    super::arrow_render::admitted_arrows_bindings_elbows_and_arrowheads_render_in_every_format();
    for (field, default) in [
        ("startArrowhead", Value::Null),
        ("endArrowhead", json!("arrow")),
        ("elbowed", json!(false)),
    ] {
        let original = scene("arrow");
        let mut clean = original.clone();
        clean["elements"][1][field] = default;
        for format in FORMATS {
            assert_contract(&original, &clean, format, json!([]));
        }
        for value in values() {
            if field == "elbowed" && value.is_boolean() || field != "elbowed" && value.is_null() {
                continue;
            }
            let mut source = original.clone();
            source["elements"][1][field] = value;
            for format in FORMATS {
                assert_contract(
                    &source,
                    &clean,
                    format,
                    json!([repair(field, "substituted")]),
                );
            }
        }
    }
    for elbowed in [false, true] {
        for head in [
            None,
            Some("arrow"),
            Some("bar"),
            Some("dot"),
            Some("circle"),
            Some("circle_outline"),
            Some("triangle"),
            Some("triangle_outline"),
            Some("diamond"),
            Some("diamond_outline"),
            Some("crowfoot_one"),
            Some("crowfoot_many"),
            Some("crowfoot_one_or_many"),
        ] {
            let mut source = scene("arrow");
            source["elements"][1]["elbowed"] = json!(elbowed);
            source["elements"][1]["startArrowhead"] = json!(head);
            source["elements"][1]["endArrowhead"] = json!(head);
            for format in FORMATS {
                assert_contract(&source, &source, format, json!([]));
            }
        }
    }
}

fn placeholder_rows(field: &str) -> Value {
    json!([repair(field,"placeholder"),{"code":"font.liberation-sans-substituted","severity":"info","effects":["visual"],"element_id":"stroke","json_pointer":"/elements/1","details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}}])
}

#[test]
fn malformed_point_components_have_narrow_placeholder_recovery() {
    placeholder_has_the_specified_visual_grammar();
    for kind in ["line", "arrow", "draw", "freedraw"] {
        for (point, suffix) in [
            (json!([]), "/0"),
            (json!([1]), "/1"),
            (json!([null, 2]), "/0"),
            (json!([1, false]), "/1"),
            (json!({}), ""),
        ] {
            let mut source = scene(kind);
            source["elements"][1]["points"] = json!([[0, 0], point]);
            // Every malformed point produces the same known-kind placeholder
            // for the same bounds; compare different malformed source values.
            let mut placeholder = source.clone();
            placeholder["elements"][1]["points"] = json!([[0, 0], null]);
            for format in FORMATS {
                assert_contract(
                    &source,
                    &placeholder,
                    format,
                    placeholder_rows(&format!("points/1{suffix}")),
                );
            }
        }
        let mut extra = scene(kind);
        extra["elements"][1]["points"] = json!([[0, 0, 99], [40, 30]]);
        let mut clean = extra.clone();
        clean["elements"][1]["points"] = json!([[0, 0], [40, 30]]);
        let row = json!({"code":"compatibility.unsupported-property","severity":"warning","effects":["visual","semantic","metadata"],"element_id":"stroke","json_pointer":"/elements/1/points/0/2","details":{"kind":"compatibility","recovery":"ignored","recipe_id":null}});
        for format in FORMATS {
            assert_contract(&extra, &clean, format, json!([row.clone()]));
        }
    }
}

#[test]
fn freedraw_points_pressure_and_simulation_follow_their_independent_domains() {
    explicit_pressure_vector_matches_the_qualified_geometry();
    malformed_points_keep_pressure_length_based_on_source_occurrences();
    super::freedraw_render::simulated_pressure_freedraw_renders_directly_in_every_format();
    super::freedraw_render::closed_freedraw_paints_rough_background_before_the_pressure_stroke();
    malformed_point_components_have_narrow_placeholder_recovery();
    let original = scene("freedraw");
    for format in FORMATS {
        let mut absent = original.clone();
        absent["elements"][1]
            .as_object_mut()
            .unwrap()
            .remove("points");
        let mut empty = original.clone();
        empty["elements"][1]["points"] = json!([]);
        assert_contract(&absent, &empty, format, json!([]));
        for value in values().into_iter().filter(|v| !v.is_array()) {
            let mut source = original.clone();
            source["elements"][1]["points"] = value;
            assert_contract(&source, &source, format, placeholder_rows("points"));
        }
        for simulation in [false, true] {
            let mut clean = original.clone();
            clean["elements"][1]["simulatePressure"] = json!(simulation);
            clean["elements"][1]["pressures"] = json!([]);
            let mut absent = clean.clone();
            absent["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove("pressures");
            assert_contract(&absent, &clean, format, json!([]));
            for value in values().into_iter().chain([
                json!([0, 0.25, 0.75, 1]),
                json!([0.5, 0.5]),
                json!([-0.1, 0, 0, 0]),
                json!([0, 1.1, 0, 0]),
            ]) {
                let valid = value.as_array().is_some_and(|a| {
                    (simulation || a.len() == 4)
                        && a.iter()
                            .all(|v| v.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
                });
                let mut source = clean.clone();
                source["elements"][1]["pressures"] = value;
                assert_contract(
                    &source,
                    if valid { &source } else { &clean },
                    format,
                    if valid {
                        json!([])
                    } else {
                        json!([repair("pressures", "substituted")])
                    },
                );
            }
        }
        let mut clean = original.clone();
        clean["elements"][1]["simulatePressure"] = json!(true);
        assert_contract(&original, &clean, format, json!([]));
        for value in values().into_iter().filter(|v| !v.is_boolean()) {
            let mut source = original.clone();
            source["elements"][1]["simulatePressure"] = value;
            assert_contract(
                &source,
                &clean,
                format,
                json!([repair("simulatePressure", "substituted")]),
            );
        }
        let mut real = original.clone();
        real["elements"][1]["simulatePressure"] = json!(false);
        real["elements"][1]["pressures"] = json!([0, 0.25, 0.75, 1]);
        let mut simulated = real.clone();
        simulated["elements"][1]["simulatePressure"] = json!(true);
        let request = RenderRequest::builder(format).build().unwrap();
        let a = Renderer::new().render(&parse(&real), &request).unwrap();
        let b = Renderer::new()
            .render(&parse(&simulated), &request)
            .unwrap();
        assert_ne!(a.artifact().bytes(), b.artifact().bytes());
    }
}

fn explicit_pressure_vector_matches_the_qualified_geometry() {
    use sha2::{Digest, Sha256};
    let source=br#"{"type":"excalidraw","version":2,"elements":[{"id":"pressure","type":"freedraw","width":50,"height":30,"points":[[0,0],[15,20],[35,10],[50,30]],"pressures":[0.1,0.4,0.9,0.3],"simulatePressure":false,"strokeWidth":3,"updated":1}],"appState":{},"files":{}}"#;
    let document = SceneDocument::parse(source).unwrap();
    for format in FORMATS {
        let result = Renderer::new()
            .render(&document, &RenderRequest::builder(format).build().unwrap())
            .unwrap();
        assert_report(&result, format, json!([]));
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
            let xml = roxmltree::Document::parse(svg).unwrap();
            let path = xml
                .descendants()
                .find(|n| n.has_tag_name("path"))
                .unwrap()
                .attribute("d")
                .unwrap();
            assert_eq!(
                lower_hex(&Sha256::digest(path)),
                "0bb4552d75b1f4b8a39f1d5721b052c288db07d3d51e7b5303ab3c6ffffd0f1e"
            );
        }
    }
}

#[test]
fn malformed_points_keep_pressure_length_based_on_source_occurrences() {
    for pressures in [json!([0.2, 0.8]), json!([0.2])] {
        let mut source = scene("freedraw");
        source["elements"][1]["points"] = json!([[0, 0], [null, 30]]);
        source["elements"][1]["simulatePressure"] = json!(false);
        source["elements"][1]["pressures"] = pressures.clone();
        let mut rows = placeholder_rows("points/1/0");
        if pressures.as_array().unwrap().len() == 1 {
            rows.as_array_mut()
                .unwrap()
                .insert(1, repair("pressures", "substituted"));
        }
        for format in FORMATS {
            assert_contract(&source, &source, format, rows.clone());
        }
    }
}

fn placeholder_has_the_specified_visual_grammar() {
    let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"stroke","type":"freedraw","width":200,"height":80,"points":false,"updated":0}],"appState":{},"files":{}});
    for format in FORMATS {
        let result = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).padding(0.0).build().unwrap(),
            )
            .unwrap();
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
                assert!(svg.contains("width=\"200\" height=\"80\" fill=\"#f1f3f5\""));
                assert!(svg.contains("M0 0 L200 80 M200 0 L0 80"));
                assert!(svg.contains("stroke=\"#868e96\""));
                assert!(svg.contains("stroke-dasharray=\"6 4\""));
                assert!(svg.contains("Unavailable: freedraw"));
            }
            OutputFormat::Png => {
                let png = crate::common::raster::decode_png(result.artifact().bytes());
                assert_eq!((png.width, png.height), (200, 80));
                assert_eq!(png.pixel(20, 30), [241, 243, 245, 255]);
                assert_ne!(png.pixel(100, 40), [241, 243, 245, 255]);
            }
            OutputFormat::Pdf => {
                let pdf = crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
                assert!(pdf.contains("0 0 m\n200 0 l\n200 80 l\n0 80 l\nh\n"));
                assert!(pdf.contains("[6 4] 0 d"));
                assert!(pdf.contains("200 80 l"));
                assert!(!pdf.contains("/Subtype /Image"));
            }
            _ => unreachable!(),
        }
    }
}
