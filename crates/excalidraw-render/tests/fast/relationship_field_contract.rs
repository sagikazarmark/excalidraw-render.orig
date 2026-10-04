//! Relationship-field judgments at the public renderer boundary.
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
fn scene(kind: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"frame","type":"frame","width":100,"height":80,"updated":0},
    {"id":"owner","type":kind,"x":10,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30]],"seed":1,"updated":0},
    {"id":"target","type":"rectangle","x":50,"y":10,"width":20,"height":20,"seed":1,"updated":0},
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
fn repair(pointer: &str, recovery: &str, effects: Value) -> Value {
    json!({"code":"compatibility.unsupported-property-value","severity":"warning","effects":effects,"element_id":"owner","json_pointer":format!("/elements/1/{pointer}"),"details":{"kind":"compatibility","recovery":recovery,"recipe_id":null}})
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
    let renderer = Renderer::new();
    let first = renderer.render(&document, &request).unwrap();
    let baseline = renderer.render(&clean_document, &request).unwrap();
    let second = Renderer::new().render(&document, &request).unwrap();
    assert_eq!(
        first.artifact().bytes(),
        baseline.artifact().bytes(),
        "{format:?}: {source}"
    );
    assert_report(&first, format, rows.clone());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let metadata = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::default().with_embedded_scene(true))
        .build()
        .unwrap();
    let embedded = renderer.render(&document, &metadata).unwrap();
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
fn endpoint_bindings_and_nested_members_recover_at_the_narrowest_boundary() {
    omitted_bindings_do_not_select_outside_targets_and_valid_siblings_survive();
    super::duplicate_identity_contract::identity_and_duplicate_relationship_contract_in_every_format();
    for kind in ["line", "arrow", "draw"] {
        for elbowed in [false, true] {
            if elbowed && kind != "arrow" {
                continue;
            }
            let mut original = scene(kind);
            if elbowed {
                original["elements"][1]["elbowed"] = json!(true);
            }
            for field in ["startBinding", "endBinding"] {
                let mut clean = original.clone();
                clean["elements"][1][field] = Value::Null;
                for format in FORMATS {
                    assert_contract(&original, &clean, format, json!([]));
                    for value in values()
                        .into_iter()
                        .filter(|v| !v.is_null() && !v.is_object())
                    {
                        let mut source = original.clone();
                        source["elements"][1][field] = value;
                        assert_contract(
                            &source,
                            &clean,
                            format,
                            json!([repair(
                                field,
                                "substituted",
                                json!(["input_repair", "visual"])
                            )]),
                        );
                    }
                }
                for member in ["elementId", "focus", "gap"] {
                    for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
                        let mut source = original.clone();
                        source["elements"][1][field] =
                            json!({"elementId":"target","focus":0,"gap":1});
                        if elbowed {
                            source["elements"][1][field]["fixedPoint"] = json!([0, 0]);
                        }
                        if let Some(value) = &value {
                            source["elements"][1][field][member] = value.clone();
                        } else {
                            source["elements"][1][field]
                                .as_object_mut()
                                .unwrap()
                                .remove(member);
                        }
                        let valid = match member {
                            "elementId" => value
                                .as_ref()
                                .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty())),
                            "gap" => value.as_ref().is_some_and(Value::is_number),
                            _ => value.as_ref().is_none_or(|v| {
                                v.is_number() || v.is_null() || v == false || v == ""
                            }),
                        };
                        let rows = if valid {
                            json!([])
                        } else {
                            json!([repair(
                                &format!("{field}/{member}"),
                                "omitted",
                                json!(["input_repair", "visual"])
                            )])
                        };
                        // Ordinary binding focus/gap do not replace stored scene points;
                        // even omitted bindings must not remove or replace the owner.
                        for format in FORMATS {
                            assert_contract(&source, &clean, format, rows.clone());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn fixed_point_binding_normalization_defaults_center_components_and_recovery() {
    let mut original = scene("arrow");
    original["elements"][1]["elbowed"] = json!(true);
    for field in ["startBinding", "endBinding"] {
        let mut unbound = original.clone();
        unbound["elements"][1][field] = Value::Null;
        for value in std::iter::once(None).chain(
            values()
                .into_iter()
                .chain([
                    json!([0, 0]),
                    json!([0.5, 0.5]),
                    json!([0.50005, 0.49995]),
                    json!([0, 1, 2]),
                    json!(["bad", 0]),
                ])
                .map(Some),
        ) {
            let mut source = original.clone();
            source["elements"][1][field] = json!({"elementId":"target","focus":0,"gap":1});
            if let Some(value) = &value {
                source["elements"][1][field]["fixedPoint"] = value.clone();
            }
            let mut clean = source.clone();
            let mut rows = json!([]);
            match &value {
                None | Some(Value::Null) => clean = unbound.clone(),
                Some(value)
                    if value
                        .as_array()
                        .is_some_and(|a| a.len() == 2 && a.iter().all(Value::is_number)) =>
                {
                    if value != &json!([0, 0]) {
                        clean["elements"][1][field]["fixedPoint"] = json!([0.5001, 0.5001]);
                    }
                }
                Some(value) => {
                    clean["elements"][1][field]["fixedPoint"] = json!([0, 0]);
                    let suffix = match value.as_array() {
                        Some(a) if a.len() < 2 => format!("/{}", a.len()),
                        Some(a) if !a[0].is_number() => "/0".into(),
                        _ => String::new(),
                    };
                    rows = json!([repair(
                        &format!("{field}/fixedPoint{suffix}"),
                        "substituted",
                        json!(["input_repair", "visual"])
                    )]);
                }
            }
            for format in FORMATS {
                assert_contract(&source, &clean, format, rows.clone());
            }
        }
    }
}

#[test]
fn grouping_and_frame_membership_default_and_recover_without_replacing_content() {
    image_and_frame_grouping_recovery();
    super::frame_render::nested_frames_clip_direct_geometry_in_every_format();
    super::frame_render::group_membership_preserves_direct_paint_order_in_every_format();
    super::diagnostic_contract::relationship_cycle_contract_in_every_format();
    for kind in [
        "rectangle",
        "diamond",
        "ellipse",
        "line",
        "arrow",
        "draw",
        "freedraw",
        "image",
        "frame",
        "magicframe",
        "iframe",
        "embeddable",
        "text",
        "selection",
    ] {
        let mut original = scene(kind);
        if !matches!(kind, "line" | "arrow" | "draw" | "freedraw") {
            original["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove("points");
        }
        if kind == "text" {
            original["elements"][1]["text"] = json!("A");
            original["elements"][1]["fontFamily"] = json!(3);
            original["elements"][1]["fontSize"] = json!(16);
            original["elements"][1]["lineHeight"] = json!(1.25);
        }
        if kind == "image" {
            continue;
        }
        // Avoid adding a target frame to this helper's two-target fixture.
        if matches!(kind, "frame" | "magicframe") {
            continue;
        }
        for (field, default) in [("frameId", Value::Null), ("groupIds", json!([]))] {
            let mut clean = original.clone();
            clean["elements"][1][field] = default;
            for format in FORMATS {
                assert_contract(&original, &clean, format, json!([]));
            }
            for value in values().into_iter().chain([
                json!(["group"]),
                json!(["one", "two"]),
                json!(["one", 3]),
            ]) {
                let valid = if field == "frameId" {
                    value.is_null() || value.is_string()
                } else {
                    value
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string))
                };
                let mut source = original.clone();
                source["elements"][1][field] = value;
                let rows = if valid || kind == "selection" {
                    json!([])
                } else {
                    json!([repair(
                        field,
                        "substituted",
                        if field == "frameId" {
                            json!(["input_repair", "visual"])
                        } else {
                            json!(["input_repair"])
                        }
                    )])
                };
                for format in FORMATS {
                    assert_contract(&source, &clean, format, rows.clone());
                }
            }
        }
    }
}

fn image_and_frame_grouping_recovery() {
    use base64::Engine;
    for kind in ["image", "frame", "magicframe"] {
        let mut original = scene(kind);
        original["elements"][1]
            .as_object_mut()
            .unwrap()
            .remove("points");
        if kind == "image" {
            original["elements"][1]["fileId"] = json!("asset");
            let png = crate::common::codec_fixtures::png_rgba(1, 1, &[[255, 0, 0, 255]]);
            original["files"] = json!({"asset":{"id":"asset","created":0,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png))}});
        }
        for field in ["groupIds", "frameId"] {
            for value in values() {
                let valid = if field == "frameId" {
                    value.is_null() || value.is_string()
                } else {
                    value == json!([])
                };
                let mut source = original.clone();
                source["elements"][1][field] = value;
                let mut clean = original.clone();
                clean["elements"][1][field] = if field == "frameId" {
                    Value::Null
                } else {
                    json!([])
                };
                for format in FORMATS {
                    let request = RenderRequest::builder(format)
                        .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                        .build()
                        .unwrap();
                    let document = parse(&source);
                    let result = Renderer::new().render(&document, &request).unwrap();
                    let rows = if valid {
                        json!([])
                    } else {
                        json!([repair(
                            field,
                            "substituted",
                            if field == "frameId" {
                                json!(["input_repair", "visual"])
                            } else {
                                json!(["input_repair"])
                            }
                        )])
                    };
                    assert_report(&result, format, rows.clone());
                    assert_eq!(
                        super::artifact_metadata::extract(format, result.artifact().bytes()).1,
                        serde_json::to_vec(&source).unwrap()
                    );
                    let request = FrameRenderRequest::builder(format).build().unwrap();
                    let renderer = Renderer::new();
                    let frame = document.frames().next().unwrap();
                    let actual = renderer.render_frame(frame, &request).unwrap();
                    let clean_document = parse(&clean);
                    let expected = renderer
                        .render_frame(clean_document.frames().next().unwrap(), &request)
                        .unwrap();
                    assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
                    assert_report(&actual, format, rows);
                    let replay = renderer
                        .render_frames(&document, [frame], &request)
                        .unwrap()
                        .next()
                        .unwrap()
                        .into_result()
                        .unwrap();
                    assert_eq!(actual.report(), replay.report());
                    assert_eq!(actual.artifact().bytes(), replay.artifact().bytes());
                }
            }
        }
    }
}

#[test]
fn bound_element_records_and_legacy_ids_have_narrow_recovery_and_precedence() {
    omitted_bindings_do_not_select_outside_targets_and_valid_siblings_survive();
    bound_arrays_preserve_valid_siblings_and_modern_presence_wins();
    let mut original = scene("rectangle");
    original["elements"][1]
        .as_object_mut()
        .unwrap()
        .remove("points");
    for field in ["boundElements", "boundElementIds"] {
        let mut clean = original.clone();
        clean["elements"][1][field] = json!([]);
        for format in FORMATS {
            assert_contract(&original, &clean, format, json!([]));
        }
        for value in values() {
            let valid = value == json!([]) || (field == "boundElements" && value.is_null());
            if valid {
                continue;
            }
            let mut source = original.clone();
            source["elements"][1][field] = value;
            for format in FORMATS {
                assert_contract(
                    &source,
                    &clean,
                    format,
                    json!([repair(
                        field,
                        "substituted",
                        json!(["input_repair", "visual", "semantic"])
                    )]),
                );
            }
        }
    }
    for member in ["id", "type"] {
        for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
            if member == "id"
                && value
                    .as_ref()
                    .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
            {
                continue;
            }
            let mut source = original.clone();
            source["elements"][1]["boundElements"] = json!([{"id":"target","type":"arrow"}]);
            if let Some(value) = value {
                source["elements"][1]["boundElements"][0][member] = value;
            } else {
                source["elements"][1]["boundElements"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove(member);
            }
            let mut clean = original.clone();
            clean["elements"][1]["boundElements"] = json!([]);
            for format in FORMATS {
                assert_contract(
                    &source,
                    &clean,
                    format,
                    json!([repair(
                        &format!("boundElements/0/{member}"),
                        "omitted",
                        json!(["input_repair", "visual", "semantic"])
                    )]),
                );
            }
        }
    }
    let mut legacy = original.clone();
    legacy["elements"][1]["boundElementIds"] = json!(["target"]);
    let mut modern = original.clone();
    modern["elements"][1]["boundElements"] = json!([{"id":"target","type":"arrow"}]);
    for format in FORMATS {
        assert_contract(&legacy, &modern, format, json!([]));
    }
}

fn bound_arrays_preserve_valid_siblings_and_modern_presence_wins() {
    let mut original = scene("rectangle");
    original["elements"][1]
        .as_object_mut()
        .unwrap()
        .remove("points");
    for format in FORMATS {
        let mut mixed = original.clone();
        mixed["elements"][1]["boundElements"] = json!([42, {}]);
        let mut empty = original.clone();
        empty["elements"][1]["boundElements"] = json!([]);
        assert_contract(
            &mixed,
            &empty,
            format,
            json!([repair(
                "boundElements",
                "substituted",
                json!(["input_repair", "visual", "semantic"])
            )]),
        );
        for modern in [Value::Null, json!([]), json!(false), json!({})] {
            let mut both = original.clone();
            both["elements"][1]["boundElements"] = modern.clone();
            both["elements"][1]["boundElementIds"] = json!(["owner"]);
            let rows = if modern.is_null() || modern.is_array() {
                json!([])
            } else {
                json!([repair(
                    "boundElements",
                    "substituted",
                    json!(["input_repair", "visual", "semantic"])
                )])
            };
            assert_contract(&both, &empty, format, rows);
        }
        let mut source = original.clone();
        source["elements"][1]["boundElementIds"] = json!(["", "target"]);
        source["elements"][2]["x"] = json!(150);
        source["elements"][2]
            .as_object_mut()
            .unwrap()
            .remove("updated");
        let document = parse(&source);
        let request = FrameRenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new()
            .render_frame(document.frames().next().unwrap(), &request)
            .unwrap();
        assert_report(
            &outcome,
            format,
            json!([{"code":"normalization.missing-timestamp","severity":"warning","effects":["input_repair"],"element_id":"target","json_pointer":"/elements/2/updated","details":{"kind":"compatibility","recovery":"repaired","recipe_id":null}}]),
        );
    }
}

#[test]
fn omitted_bindings_do_not_select_outside_targets_and_valid_siblings_survive() {
    for field in ["startBinding", "endBinding"] {
        let mut original = scene("arrow");
        original["elements"][2]["x"] = json!(150);
        original["elements"][2]
            .as_object_mut()
            .unwrap()
            .remove("updated");
        let mut source = original.clone();
        source["elements"][1][field] = json!({"elementId":"target","focus":"invalid","gap":1});
        for format in FORMATS {
            let document = parse(&source);
            let renderer = Renderer::new();
            let request = FrameRenderRequest::builder(format).build().unwrap();
            let frame = document.frames().next().unwrap();
            let result = renderer.render_frame(frame, &request).unwrap();
            assert_report(
                &result,
                format,
                json!([repair(
                    &format!("{field}/focus"),
                    "omitted",
                    json!(["input_repair", "visual"])
                )]),
            );
        }
    }
    let mut source = scene("rectangle");
    source["elements"][1]
        .as_object_mut()
        .unwrap()
        .remove("points");
    source["elements"][1]["boundElements"] =
        json!([{"id":"target","type":"arrow"},{"id":null,"type":"arrow"}]);
    source["elements"][2]["x"] = json!(150);
    source["elements"][2]
        .as_object_mut()
        .unwrap()
        .remove("updated");
    for format in FORMATS {
        let document = parse(&source);
        let request = FrameRenderRequest::builder(format).build().unwrap();
        let renderer = Renderer::new();
        let result = renderer
            .render_frame(document.frames().next().unwrap(), &request)
            .unwrap();
        let rows = json!([repair("boundElements/1/id","omitted",json!(["input_repair","visual","semantic"])),
            {"code":"normalization.missing-timestamp","severity":"warning","effects":["input_repair"],"element_id":"target","json_pointer":"/elements/2/updated","details":{"kind":"compatibility","recovery":"repaired","recipe_id":null}}]);
        assert_report(&result, format, rows);
        let replay = renderer
            .render_frames(&document, document.frames(), &request)
            .unwrap()
            .next()
            .unwrap()
            .into_result()
            .unwrap();
        assert_eq!(result.report(), replay.report());
        assert_eq!(result.artifact().bytes(), replay.artifact().bytes());
    }
}
