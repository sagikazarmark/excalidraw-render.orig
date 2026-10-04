//! Text field ownership through the public facade. Authored defaults, source
//! preservation and clean-scene oracles complement existing typography vectors.
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
fn scene() -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"frame","type":"frame","width":120,"height":100,"updated":0},
    {"id":"text","type":"text","x":10,"y":10,"width":60,"height":40,"text":"AB\nCD","originalText":"AB\nCD","fontFamily":3,"fontSize":20,"lineHeight":1,"frameId":"frame","updated":0},
    {"id":"other","type":"magicframe","x":200,"width":120,"height":100,"updated":0}],"appState":{},"files":{}})
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
        json!("invalid"),
        json!([]),
        json!([1, 2]),
        json!({}),
        json!({"unknown":true}),
    ]
}
fn repair(field: &str) -> Value {
    json!({"code":"compatibility.unsupported-property-value","severity":"warning",
    "effects":if ["fontFamily","text","containerId"].contains(&field){json!(["input_repair","visual","semantic"])}else{json!(["input_repair","visual"])},
    "element_id":"text","json_pointer":format!("/elements/1/{field}"),"details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}})
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
    let request = RenderRequest::builder(format).build().unwrap();
    let first = Renderer::new().render(&document, &request).unwrap();
    let second = Renderer::new().render(&document, &request).unwrap();
    let baseline = Renderer::new().render(&clean_document, &request).unwrap();
    assert_eq!(
        first.artifact().bytes(),
        baseline.artifact().bytes(),
        "{format:?}: {source}"
    );
    assert_report(&first, format, rows.clone());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    let metadata = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::default().with_embedded_scene(true))
        .build()
        .unwrap();
    let embedded = Renderer::new().render(&document, &metadata).unwrap();
    assert_report(&embedded, format, rows.clone());
    assert_eq!(
        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
    let renderer = Renderer::new();
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let frames: Vec<_> = document.frames().collect();
    let clean_frames: Vec<_> = clean_document.frames().collect();
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for (index, (frame, clean_frame)) in frames.iter().zip(clean_frames).enumerate() {
        let direct = renderer.render_frame(*frame, &request).unwrap();
        let baseline = renderer.render_frame(clean_frame, &request).unwrap();
        let replay = batch.next().unwrap().into_result().unwrap();
        assert_eq!(direct.artifact().bytes(), baseline.artifact().bytes());
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
    let result = Renderer::new()
        .render(
            &parse(&deleted),
            &RenderRequest::builder(format).build().unwrap(),
        )
        .unwrap();
    assert_report(&result, format, json!([]));
}

#[test]
fn text_size_family_and_alignment_defaults_and_invalid_values_are_exact() {
    valid_text_layout_and_container_participation();
    for (field, default) in [
        ("fontSize", json!(20)),
        ("fontFamily", json!(5)),
        ("textAlign", json!("left")),
        ("verticalAlign", json!("top")),
        ("containerId", Value::Null),
    ] {
        let original = scene();
        let mut clean = original.clone();
        clean["elements"][1][field] = default;
        let mut absent = original.clone();
        absent["elements"][1].as_object_mut().unwrap().remove(field);
        let mut invalid = values();
        if field == "fontSize" {
            invalid.retain(|v| v != &json!(0.5));
        }
        if field == "containerId" {
            invalid.retain(|v| !v.is_null() && !v.is_string());
        }
        if field == "fontFamily" {
            invalid.extend([json!(4), json!(10), json!(3.0)]);
        }
        for format in FORMATS {
            assert_contract(&absent, &clean, format, json!([]));
            for value in &invalid {
                let mut source = original.clone();
                source["elements"][1][field] = value.clone();
                assert_contract(&source, &clean, format, json!([repair(field)]));
            }
        }
    }
}

fn valid_text_layout_and_container_participation() {
    alignment_and_container_positions_have_independent_oracles();
    super::bound_text_render::arrow_bound_text_renders_at_the_arrow_midpoint_in_every_format();
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let original = scene();
        let baseline = Renderer::new().render(&parse(&original), &request).unwrap();
        for (field, value) in [
            ("fontSize", json!(10.5)),
            ("fontSize", json!(32)),
            ("textAlign", json!("center")),
            ("textAlign", json!("right")),
        ] {
            let mut source = original.clone();
            source["elements"][1][field] = value;
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            assert_ne!(
                result.artifact().bytes(),
                baseline.artifact().bytes(),
                "{format:?} {field}"
            );
            assert_contract(&source, &source, format, json!([]));
        }
        for value in ["top", "middle", "bottom"] {
            let mut source = original.clone();
            source["elements"][1]["verticalAlign"] = json!(value);
            assert_contract(&source, &original, format, json!([]));
        }
        let mut family_outputs = std::collections::BTreeSet::new();
        for family in [1, 2, 3, 5, 6, 7, 8, 9] {
            let mut source = original.clone();
            source["elements"][1]["fontFamily"] = json!(family);
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            family_outputs.insert(result.artifact().bytes().to_vec());
            assert_contract(&source, &source, format, font_notice(family));
        }
        assert!(
            family_outputs.len() >= 7,
            "distinct admitted font faces participate"
        );
        for container in [Value::Null, json!(""), json!("absent")] {
            let mut source = original.clone();
            source["elements"][1]["containerId"] = container;
            assert_contract(&source, &original, format, json!([]));
        }
        for reciprocal in [false, true] {
            let mut source = original.clone();
            source["elements"][1]["containerId"] = json!("box");
            let mut container = json!({"id":"box","type":"rectangle","x":5,"y":5,"width":80,"height":70,"angle":0.4,"frameId":"frame","seed":1,"updated":0});
            if reciprocal {
                container["boundElements"] = json!([{"id":"text","type":"text"}]);
            }
            source["elements"].as_array_mut().unwrap().push(container);
            let mut clean = source.clone();
            if !reciprocal {
                clean["elements"][1]["text"] = json!("");
            }
            assert_contract(&source, &clean, format, json!([]));
            if reciprocal {
                let mut unbound = source.clone();
                unbound["elements"][1]["containerId"] = Value::Null;
                let result = Renderer::new().render(&parse(&source), &request).unwrap();
                let plain = Renderer::new().render(&parse(&unbound), &request).unwrap();
                assert_ne!(result.artifact().bytes(), plain.artifact().bytes());
            }
        }
    }
}

#[test]
fn text_and_original_text_recover_independently_and_preserve_original_values() {
    original_text_demands_extra_subset_characters_without_changing_display_text();
    for field in ["text", "originalText"] {
        let original = scene();
        let mut clean = original.clone();
        clean["elements"][1][field] = if field == "text" {
            json!("")
        } else {
            original["elements"][1]["text"].clone()
        };
        let mut absent = original.clone();
        absent["elements"][1].as_object_mut().unwrap().remove(field);
        for format in FORMATS {
            assert_contract(&absent, &clean, format, json!([]));
            for value in values().into_iter().filter(|v| !v.is_string()) {
                let mut source = original.clone();
                source["elements"][1][field] = value;
                assert_contract(&source, &clean, format, json!([repair(field)]));
            }
        }
    }
    for text in ["", "A", "A\n\nB", "A\r\nB\rC", "A\tB", "אב A"] {
        let mut source = scene();
        source["elements"][1]["text"] = json!(text);
        source["elements"][1]["originalText"] = json!(text);
        for format in FORMATS {
            let rows = if text == "אב A" {
                json!([1488,1489].map(|cp|json!({"code":"font.missing-glyph","severity":"warning","effects":["visual"],"element_id":"text","json_pointer":"/elements/1","details":{"kind":"missing_glyph","codepoint":cp,"recovery":"substituted"}})))
            } else {
                json!([])
            };
            assert_contract(&source, &source, format, rows);
        }
    }
    // originalText chooses direct-SVG font demand; changing it never rewrites
    // the stored text painted by PNG/PDF or carried as source text in SVG.
    let mut source = scene();
    source["elements"][1]["originalText"] = json!("WXYZ");
    let clean = scene();
    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        assert_contract(&source, &clean, format, json!([]));
    }
    let result = Renderer::new()
        .render(
            &parse(&source),
            &RenderRequest::builder(OutputFormat::Svg).build().unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
    assert!(svg.contains("data-source-text=\"AB"));
    assert!(!svg.contains("data-source-text=\"WXYZ"));
}

fn font_notice(family: i32) -> Value {
    if family == 2 {
        json!([{"code":"font.liberation-sans-substituted","severity":"info","effects":["visual"],"element_id":"text","json_pointer":"/elements/1","details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}}])
    } else {
        json!([])
    }
}

fn original_text_demands_extra_subset_characters_without_changing_display_text() {
    use base64::Engine;
    let mut source = scene();
    source["elements"][1]["originalText"] = json!("AB\nCDZ");
    let result = Renderer::new()
        .render(
            &parse(&source),
            &RenderRequest::builder(OutputFormat::Svg).build().unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
    let fonts: Vec<_> = svg
        .split("data:font/woff2;base64,")
        .skip(1)
        .map(|tail| {
            let encoded = tail.split(['\'', '"', ')']).next().unwrap();
            let woff = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap();
            wuff::decompress_woff2(&woff).unwrap()
        })
        .collect();
    assert!(!fonts.is_empty());
    for character in ['A', 'B', 'C', 'D', 'Z'] {
        assert!(
            fonts.iter().any(|bytes| ttf_parser::Face::parse(bytes, 0)
                .unwrap()
                .glyph_index(character)
                .is_some()),
            "demanded {character}"
        );
    }
    assert!(fonts.iter().all(|bytes| ttf_parser::Face::parse(bytes, 0)
        .unwrap()
        .glyph_index('Q')
        .is_none()));
    assert!(svg.contains("data-source-text=\"AB"));
    assert!(!svg.contains("data-source-text=\"AB\nCDZ"));
}

fn alignment_and_container_positions_have_independent_oracles() {
    // Cascadia's advance is 1200/2048 em: two A glyphs at 20px occupy
    // 23.4375px of the authored 60px text box.
    let mut source = scene();
    source["elements"][1]["text"] = json!("AA");
    source["elements"][1]["originalText"] = json!("AA");
    source["elements"][1]["height"] = json!(20);
    for (align, offset) in [("left", 0.0), ("center", 18.28125), ("right", 36.5625)] {
        let mut aligned = source.clone();
        aligned["elements"][1]["textAlign"] = json!(align);
        for format in FORMATS {
            let request = RenderRequest::builder(format)
                .frame_options(
                    excalidraw_render::FrameOptions::default()
                        .with_enabled(false)
                        .with_clip(false)
                        .with_name(false)
                        .with_outline(false),
                )
                .build()
                .unwrap();
            let actual = Renderer::new().render(&parse(&aligned), &request).unwrap();
            if format == OutputFormat::Png {
                let ink = |outcome: &RenderOutcome| {
                    let png = crate::common::raster::decode_png(outcome.artifact().bytes());
                    let mut min = png.width;
                    let mut max = 0;
                    for y in 0..png.height {
                        for x in 0..png.width {
                            let p = png.pixel(x as usize, y as usize);
                            if p[0] < 128 && p[1] < 128 && p[2] < 128 {
                                min = min.min(x);
                                max = max.max(x);
                            }
                        }
                    }
                    (min, max)
                };
                let a = ink(&actual);
                let b = (11 + offset as u32, 32 + offset as u32);
                assert!(
                    a.0.abs_diff(b.0) <= 1 && a.1.abs_diff(b.1) <= 1,
                    "{align}: {a:?} vs {b:?}"
                );
            }
            if format == OutputFormat::Svg {
                let xml = roxmltree::Document::parse(
                    std::str::from_utf8(actual.artifact().bytes()).unwrap(),
                )
                .unwrap();
                let tspan = xml
                    .descendants()
                    .find(|n| n.has_tag_name("tspan") && n.text() == Some("AA"))
                    .unwrap();
                let x = tspan
                    .attribute("x")
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                let anchor = match align {
                    "left" => 0.0,
                    "center" => 30.0,
                    _ => 60.0,
                };
                assert!((x - anchor).abs() < 0.001, "{align}: {x}");
            }
            if format == OutputFormat::Pdf {
                let pdf = crate::common::pdf_inspect::pdf_text(actual.artifact().bytes());
                let matrix = pdf
                    .lines()
                    .find(|line| line.starts_with("1 0 0 -1 ") && line.ends_with(" Tm"))
                    .expect("text matrix");
                let x: f64 = matrix.split_whitespace().nth(4).unwrap().parse().unwrap();
                assert!((x - offset).abs() < 0.001, "{align} matrix {matrix}");
            }
        }
    }
    // A shape-bound label inherits its container's rotation about the
    // container center, while retaining authored coordinates when unrotated.
    let mut bound = source.clone();
    bound["elements"][1]["containerId"] = json!("box");
    bound["elements"].as_array_mut().unwrap().push(json!({"id":"box","type":"rectangle","x":5,"y":5,"width":80,"height":70,"frameId":"frame","boundElements":[{"id":"text","type":"text"}],"seed":1,"updated":0}));
    let mut unbound = bound.clone();
    unbound["elements"][1]["containerId"] = Value::Null;
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let a = Renderer::new().render(&parse(&bound), &request).unwrap();
        let b = Renderer::new().render(&parse(&unbound), &request).unwrap();
        if format == OutputFormat::Png {
            assert_eq!(a.artifact().bytes(), b.artifact().bytes());
        }
        if format == OutputFormat::Svg {
            let xml =
                roxmltree::Document::parse(std::str::from_utf8(a.artifact().bytes()).unwrap())
                    .unwrap();
            let text = xml
                .descendants()
                .find(|n| n.has_tag_name("text") && n.attribute("data-source-text") == Some("AA"))
                .unwrap();
            // Scene frame-name margin is 20.5px above the frame, in addition
            // to padding 10 and authored y=10.
            assert!(text
                .ancestors()
                .any(|n| n.attribute("transform") == Some("translate(20 40.5) rotate(0 30 10)")));
        }
        if format == OutputFormat::Pdf {
            let pdf = crate::common::pdf_inspect::pdf_text(a.artifact().bytes());
            // translate(20 40.5) under the page mapping 0.75 0 0 -0.75 0 105.375,
            // which Krilla writes composed (ADR-0021).
            assert!(pdf.contains("0.75 0 0 -0.75 15 75 cm"));
        }
    }
}

#[test]
fn line_height_derives_from_wrapped_lines_or_source_family_metrics() {
    for (family, metric) in [
        (1, 1.25),
        (2, 1.15),
        (3, 1.2),
        (5, 1.25),
        (6, 1.35),
        (7, 1.15),
        (8, 1.25),
        (9, 1.15),
    ] {
        for height in [0.0, 40.0, -40.0] {
            for text in ["AB\nCD", "AB\r\nCD", "AB\rCD", "AB\n\nCD"] {
                let mut original = scene();
                original["elements"][1]["fontFamily"] = json!(family);
                original["elements"][1]["height"] = json!(height);
                original["elements"][1]["text"] = json!(text);
                original["elements"][1]["originalText"] = json!(text);
                let mut clean = original.clone();
                clean["elements"][1]["lineHeight"] = json!(if height == 0.0 {
                    metric
                } else if text == "AB\n\nCD" {
                    0.6666666666666667
                } else {
                    1.0
                });
                for value in [
                    None,
                    Some(json!(0)),
                    Some(json!(-1)),
                    Some(json!(null)),
                    Some(json!("bad")),
                ] {
                    let mut source = original.clone();
                    if let Some(value) = &value {
                        source["elements"][1]["lineHeight"] = value.clone();
                    } else {
                        source["elements"][1]
                            .as_object_mut()
                            .unwrap()
                            .remove("lineHeight");
                    }
                    let mut rows = font_notice(family);
                    if value.is_some() {
                        rows.as_array_mut().unwrap().push(repair("lineHeight"));
                    }
                    for format in FORMATS {
                        assert_contract(&source, &clean, format, rows.clone());
                    }
                }
            }
        }
    }
}

#[test]
fn legacy_font_overrides_modern_fields_and_invalid_font_falls_back() {
    for (font, size, family) in [
        ("28px Virgil", 28.0, 1),
        ("16px Helvetica", 16.0, 2),
        ("24px Cascadia", 24.0, 3),
        ("12px Excalifont", 12.0, 5),
        ("18px Nunito", 18.0, 6),
        ("20px Unknown", 20.0, 5),
        ("20px Comic Shanns", 20.0, 5),
        ("20", 20.0, 5),
        ("2e1px Virgil", 20.0, 1),
    ] {
        let original = scene();
        let mut source = original.clone();
        source["elements"][1]["font"] = json!(font);
        let mut clean = original.clone();
        clean["elements"][1]["fontSize"] = json!(size);
        clean["elements"][1]["fontFamily"] = json!(family);
        for format in FORMATS {
            assert_contract(&source, &clean, format, font_notice(family));
        }
    }
    for value in values().into_iter().chain([
        json!("0px Virgil"),
        json!("-1px Virgil"),
        json!("Infinitypx Virgil"),
    ]) {
        let clean = scene();
        let mut source = clean.clone();
        source["elements"][1]["font"] = value;
        for format in FORMATS {
            assert_contract(&source, &clean, format, json!([repair("font")]));
        }
    }
}
