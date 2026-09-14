//! Admission and source-identity contracts through the public facade. Expected
//! diagnostics and repair vectors are authored independently of runtime policy.
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn scene(elements: Value) -> Value {
    json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":{}})
}

fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}

fn rectangle(id: &str) -> Value {
    json!({"id":id,"type":"rectangle","x":5,"y":5,"width":30,"height":20,"seed":1,"updated":1})
}

fn participating_kinds() -> Value {
    use base64::Engine;
    let png = crate::common::codec_fixtures::png_rgba(1, 1, &[[255, 0, 0, 255]]);
    let kinds = [
        "rectangle",
        "diamond",
        "ellipse",
        "text",
        "line",
        "arrow",
        "freedraw",
        "image",
        "frame",
        "magicframe",
        "iframe",
        "embeddable",
        "draw",
    ];
    let elements: Vec<_> = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            let mut element = rectangle(kind);
            element["type"] = json!(kind);
            element["x"] = json!(index * 50);
            if *kind == "text" {
                element["text"] = json!("A");
                element["fontFamily"] = json!(3);
                element["fontSize"] = json!(16);
                element["lineHeight"] = json!(1.25);
            }
            if matches!(*kind, "line" | "arrow" | "freedraw" | "draw") {
                element["points"] = json!([[0, 0], [30, 20]]);
            }
            if *kind == "image" {
                element["fileId"] = json!("file");
            }
            element
        })
        .collect();
    let mut source = scene(json!(elements));
    source["files"] = json!({"file":{"id":"file","created":0,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png))}});
    source
}

fn values() -> Vec<Value> {
    vec![
        Value::Null,
        json!(false),
        json!(true),
        json!(-1),
        json!(0),
        json!(1),
        json!(2),
        json!(2.0),
        json!(0.5),
        json!(""),
        json!("2"),
        json!("future"),
        json!("excalidraw"),
        json!([]),
        json!([null, false, {"future":true}]),
        json!({}),
        json!({"future":{"nested":[null, 1]}}),
    ]
}

fn envelope_error(bytes: &[u8], pointer: &str) {
    let first = SceneDocument::parse(bytes).unwrap_err();
    let second = SceneDocument::parse(bytes).unwrap_err();
    assert_eq!(
        serde_json::to_vec(&first.descriptor()).unwrap(),
        serde_json::to_vec(&second.descriptor()).unwrap()
    );
    let mut descriptor = serde_json::to_value(first.descriptor()).unwrap();
    assert!(descriptor["message"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    descriptor.as_object_mut().unwrap().remove("message");
    assert_eq!(
        descriptor,
        json!({"family":"scene_parse","category":null,
        "code":"scene.invalid-envelope","format":null,"element_id":null,
        "json_pointer":pointer,"details":null})
    );
}

#[test]
fn envelope_fields_admit_only_the_version_two_contract_and_preserve_nested_values() {
    let baseline = scene(json!([]));
    for field in ["type", "version", "elements", "appState", "files"] {
        for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
            let mut source = baseline.clone();
            let valid = value.as_ref().is_some_and(|v| match field {
                "type" => v == "excalidraw",
                "version" => v.as_u64() == Some(2),
                "elements" => v.is_array(),
                "appState" | "files" => v.is_object(),
                _ => unreachable!(),
            });
            if let Some(value) = value {
                source[field] = value;
            } else {
                source.as_object_mut().unwrap().remove(field);
            }
            if valid {
                let document = parse(&source);
                for format in FORMATS {
                    let request = RenderRequest::builder(format)
                        .metadata_options(MetadataOptions::new(true, false, false))
                        .build()
                        .unwrap();
                    let first = Renderer::new().render(&document, &request).unwrap();
                    let second = Renderer::new().render(&document, &request).unwrap();
                    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                    assert_eq!(first.report(), second.report());
                    let expected = match field {
                        "elements" => (0..source["elements"].as_array().unwrap().len())
                            .map(|index| {
                                let mut row = diagnostic(
                                    "compatibility.malformed-element",
                                    Value::Null,
                                    &format!("/elements/{index}"),
                                    "omitted",
                                );
                                row["effects"] = json!(["visual"]);
                                row
                            })
                            .collect::<Vec<_>>(),
                        "appState" if source["appState"].get("future").is_some() => {
                            let mut row = diagnostic(
                                "compatibility.unsupported-property",
                                Value::Null,
                                "/appState/future",
                                "ignored",
                            );
                            row["effects"] = json!(["visual", "semantic", "metadata"]);
                            vec![row]
                        }
                        _ => vec![],
                    };
                    assert_report(&first, format, json!(expected));
                    let (_, original) =
                        super::artifact_metadata::extract(format, first.artifact().bytes());
                    assert_eq!(original, serde_json::to_vec(&source).unwrap());
                }
            } else {
                envelope_error(&serde_json::to_vec(&source).unwrap(), &format!("/{field}"));
            }
        }
    }
    // Envelope validity is checked in schema order, not input member order.
    for bytes in [
        b"{}".as_slice(),
        br#"{"files":null,"appState":[],"elements":0,"version":3,"type":"future"}"#,
    ] {
        envelope_error(bytes, "/type");
    }
    for token in ["null", "false", "0", "\"scene\"", "[]"] {
        envelope_error(token.as_bytes(), "");
    }
    for field in ["type", "version", "elements", "appState", "files"] {
        let bytes = format!(
            "{},\"{field}\":null}}",
            String::from_utf8(serde_json::to_vec(&baseline).unwrap())
                .unwrap()
                .trim_end_matches('}')
        );
        let error = SceneDocument::parse(bytes.as_bytes()).unwrap_err();
        assert_eq!(
            error.code(),
            excalidraw_render::ErrorCode::SCENE_DUPLICATE_OBJECT_KEY
        );
        assert_eq!(error.json_pointer(), Some(format!("/{field}").as_str()));
    }
    for token in ["2.0", "2e0", "2E+0", "-2", "3", "200000000000000000000"] {
        let source = format!(
            r#"{{"type":"excalidraw","version":{token},"elements":[],"appState":{{}},"files":{{}}}}"#
        );
        envelope_error(source.as_bytes(), "/version");
    }
    let original = br#"{"files":{"bad":false},"appState":{"future":1.00e+02},"elements":[null,false,42,"scalar",[],{"isDeleted":true,"unknown":1}],"version":2,"type":"excalidraw"}"#;
    let mut spaced = b" \n".to_vec();
    spaced.extend(original);
    let document = SceneDocument::parse(&spaced).unwrap();
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .metadata_options(MetadataOptions::new(true, false, false))
            .build()
            .unwrap();
        let rendered = Renderer::new().render(&document, &request).unwrap();
        let mut rows = vec![];
        let mut unknown = diagnostic(
            "compatibility.unsupported-property",
            Value::Null,
            "/appState/future",
            "ignored",
        );
        unknown["effects"] = json!(["visual", "semantic", "metadata"]);
        rows.push(unknown);
        for index in 0..5 {
            let mut row = diagnostic(
                "compatibility.malformed-element",
                Value::Null,
                &format!("/elements/{index}"),
                "omitted",
            );
            row["effects"] = json!(["visual"]);
            rows.push(row);
        }
        assert_report(&rendered, format, json!(rows));
        assert_eq!(
            super::artifact_metadata::extract(format, rendered.artifact().bytes()).1,
            original
        );
    }
}

fn diagnostic(code: &str, id: Value, pointer: &str, recovery: &str) -> Value {
    json!({"code":code,"severity":"warning","effects":["input_repair"],
        "element_id":id,"json_pointer":pointer,
        "details":{"kind":"compatibility","recovery":recovery,"recipe_id":null}})
}

fn assert_report(outcome: &RenderOutcome, format: OutputFormat, mut expected: Value) {
    let mut actual = serde_json::to_value(outcome.report()).unwrap();
    for row in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    for row in expected.as_array_mut().unwrap() {
        row["format"] = json!(format.as_str());
    }
    assert_eq!(
        actual,
        json!({"schema_version":1,"diagnostics":expected}),
        "{format:?}"
    );
}

fn assert_scene(source: &Value, clean: &Value, format: OutputFormat, expected: Value) {
    let document = parse(source);
    let request = RenderRequest::builder(format).build().unwrap();
    let first = Renderer::new().render(&document, &request).unwrap();
    let second = Renderer::new().render(&document, &request).unwrap();
    let baseline = Renderer::new().render(&parse(clean), &request).unwrap();
    assert!(
        baseline.report().is_empty(),
        "baseline: {:?}",
        baseline.report()
    );
    assert_eq!(first.artifact().bytes(), baseline.artifact().bytes());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    assert_report(&first, format, expected.clone());
    let request = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let embedded = Renderer::new().render(&document, &request).unwrap();
    assert_report(&embedded, format, expected);
    assert_eq!(
        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
}

fn framed(element: Value) -> Value {
    let mut element = element;
    element["frameId"] = json!("frame");
    scene(json!([
        {"id":"frame","type":"frame","width":80,"height":60,"updated":0},
        element,
        {"id":"other","type":"magicframe","x":200,"width":80,"height":60,"updated":0}
    ]))
}

fn assert_frames(source: &Value, clean: &Value, format: OutputFormat, expected: &[Value]) {
    let document = parse(source);
    let clean_document = parse(clean);
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let renderer = Renderer::new();
    let frames: Vec<_> = document.frames().collect();
    let clean_frames: Vec<_> = clean_document.frames().collect();
    assert_eq!(frames.len(), expected.len());
    assert_eq!(clean_frames.len(), expected.len());
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for ((frame, clean_frame), expected) in frames.iter().zip(clean_frames).zip(expected) {
        let direct = renderer.render_frame(*frame, &request).unwrap();
        let baseline = Renderer::new().render_frame(clean_frame, &request).unwrap();
        let entry = batch.next().unwrap().into_result().unwrap();
        assert!(baseline.report().is_empty());
        assert_eq!(direct.artifact().bytes(), baseline.artifact().bytes());
        assert_eq!(direct.artifact().bytes(), entry.artifact().bytes());
        assert_eq!(
            serde_json::to_vec(direct.report()).unwrap(),
            serde_json::to_vec(entry.report()).unwrap()
        );
        assert_report(&direct, format, expected.clone());
    }
    assert!(batch.next().is_none());
}

#[test]
fn element_timestamps_repair_only_absence_and_preserve_invalid_originals() {
    assert_nonparticipating_identity_boundaries();
    let clean = framed(rectangle("paint"));
    for value in std::iter::once(None).chain(
        values()
            .into_iter()
            .chain([json!(9007199254740991_u64), json!(1e20)])
            .map(Some),
    ) {
        let mut source = clean.clone();
        let missing = value.is_none();
        if let Some(value) = value {
            source["elements"][1]["updated"] = value;
        } else {
            source["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove("updated");
        }
        let rows = if missing {
            json!([diagnostic(
                "normalization.missing-timestamp",
                json!("paint"),
                "/elements/1/updated",
                "repaired"
            )])
        } else {
            json!([])
        };
        for format in FORMATS {
            assert_scene(&source, &clean, format, rows.clone());
            assert_frames(&source, &clean, format, &[rows.clone(), json!([])]);
            let mut deleted = source.clone();
            deleted["elements"][1]["isDeleted"] = json!(true);
            let mut deleted_clean = clean.clone();
            deleted_clean["elements"][1]["isDeleted"] = json!(true);
            assert_scene(&deleted, &deleted_clean, format, json!([]));
            assert_frames(&deleted, &deleted_clean, format, &[json!([]), json!([])]);
        }
    }
    let clean = participating_kinds();
    for value in [
        None,
        Some(json!(null)),
        Some(json!(-1)),
        Some(json!(0)),
        Some(json!("invalid")),
    ] {
        let mut source = clean.clone();
        let mut rows = Vec::new();
        for (index, element) in source["elements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            if let Some(value) = &value {
                element["updated"] = value.clone();
            } else {
                element.as_object_mut().unwrap().remove("updated");
                rows.push(diagnostic(
                    "normalization.missing-timestamp",
                    element["id"].clone(),
                    &format!("/elements/{index}/updated"),
                    "repaired",
                ));
            }
        }
        for format in FORMATS {
            assert_scene(&source, &clean, format, json!(rows));
        }
    }
}

#[test]
fn file_timestamps_repair_once_per_authoritative_participating_record() {
    use base64::Engine;
    let png = crate::common::codec_fixtures::png_rgba(1, 1, &[[255, 0, 0, 255]]);
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    let mut image = rectangle("paint");
    image["type"] = json!("image");
    image["fileId"] = json!("a/b~c");
    let mut clean = framed(image.clone());
    image["id"] = json!("again");
    image["frameId"] = json!("frame");
    clean["elements"].as_array_mut().unwrap().push(image);
    clean["files"] = json!({"a/b~c":{"id":"a/b~c","created":0,"dataURL":data},
        "unused":{"id":"unused","dataURL":data},"malformed":false});
    for value in std::iter::once(None).chain(
        values()
            .into_iter()
            .chain([json!(9007199254740991_u64), json!(1e20)])
            .map(Some),
    ) {
        let missing = value.is_none();
        let mut source = clean.clone();
        if let Some(value) = value {
            source["files"]["a/b~c"]["created"] = value;
        } else {
            source["files"]["a/b~c"]
                .as_object_mut()
                .unwrap()
                .remove("created");
        }
        let rows = if missing {
            json!([diagnostic(
                "normalization.missing-timestamp",
                Value::Null,
                "/files/a~1b~0c/created",
                "repaired"
            )])
        } else {
            json!([])
        };
        for format in FORMATS {
            assert_scene(&source, &clean, format, rows.clone());
            assert_frames(&source, &clean, format, &[rows.clone(), json!([])]);
            let mut deleted = source.clone();
            let mut deleted_clean = clean.clone();
            for index in [1, 3] {
                deleted["elements"][index]["isDeleted"] = json!(true);
                deleted_clean["elements"][index]["isDeleted"] = json!(true);
            }
            assert_scene(&deleted, &deleted_clean, format, json!([]));
            assert_frames(&deleted, &deleted_clean, format, &[json!([]), json!([])]);
        }
    }
    // Wholly malformed authorities have one narrow cause, never an additional
    // timestamp repair. Empty-key records are ignored even when populated.
    for value in [json!(null), json!(false), json!(3), json!("bad"), json!([])] {
        let mut source = clean.clone();
        source["files"]["a/b~c"] = value;
        source["files"][""] = json!({"dataURL":data});
        for format in FORMATS {
            let document = parse(&source);
            let request = RenderRequest::builder(format)
                .metadata_options(MetadataOptions::new(true, false, false))
                .build()
                .unwrap();
            let outcome = Renderer::new().render(&document, &request).unwrap();
            let mut row = diagnostic(
                "asset.malformed-file-record",
                Value::Null,
                "/files/a~1b~0c",
                "omitted",
            );
            row["effects"] = json!(["visual"]);
            row["details"] = json!({"kind":"asset","asset_id":"a/b~c","claimed_media_type":null,"validated_media_type":null,
                "recovery":if format==OutputFormat::Png {"placeholder"} else {"omitted"},
                "recipe_id":if format==OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")} else {Value::Null}});
            assert_report(&outcome, format, json!([row]));
            assert_eq!(
                super::artifact_metadata::extract(format, outcome.artifact().bytes()).1,
                serde_json::to_vec(&source).unwrap()
            );
        }
    }
}

#[test]
fn deletion_controls_paint_diagnostics_assets_and_frame_discovery() {
    assert_nonparticipating_identity_boundaries();
    let clean = framed(rectangle("paint"));
    for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
        let mut source = clean.clone();
        if let Some(value) = &value {
            source["elements"][1]["isDeleted"] = value.clone();
        }
        let deleted = value == Some(json!(true));
        let invalid = value.as_ref().is_some_and(|v| !v.is_boolean());
        let mut baseline = clean.clone();
        if deleted {
            baseline["elements"][1]["isDeleted"] = json!(true);
        }
        let rows = if invalid {
            json!([diagnostic(
                "compatibility.unsupported-property-value",
                json!("paint"),
                "/elements/1/isDeleted",
                "substituted"
            )])
        } else {
            json!([])
        };
        for format in FORMATS {
            assert_scene(&source, &baseline, format, rows.clone());
            assert_frames(&source, &baseline, format, &[rows.clone(), json!([])]);
        }
    }
    let mut deleted = clean.clone();
    deleted["elements"].as_array_mut().unwrap().extend([
        json!({"type":"future","isDeleted":true,"seed":0,"unknown":true}),
        json!({"type":"rectangle","isDeleted":true,"id":null,"width":"bad","updated":null}),
        json!({"type":"image","isDeleted":true,"fileId":"bad"}),
        json!({"type":"text","isDeleted":true,"text":"bad","fontFamily":999}),
        json!({"type":"frame","isDeleted":true,"name":"Hidden"}),
        json!({"type":"magicframe","isDeleted":true,"name":"Hidden magic"}),
    ]);
    deleted["files"] = json!({"bad":{"dataURL":"https://example.invalid/image"}});
    assert_eq!(parse(&deleted).frames().count(), 2);
    for format in FORMATS {
        assert_scene(&deleted, &clean, format, json!([]));
        assert_frames(&deleted, &clean, format, &[json!([]), json!([])]);
    }
    let clean = participating_kinds();
    for value in [
        None,
        Some(json!(false)),
        Some(json!(true)),
        Some(json!(null)),
        Some(json!(1)),
        Some(json!("true")),
    ] {
        let mut source = clean.clone();
        let mut rows = Vec::new();
        for (index, element) in source["elements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            if let Some(value) = &value {
                element["isDeleted"] = value.clone();
                if !value.is_boolean() {
                    rows.push(diagnostic(
                        "compatibility.unsupported-property-value",
                        element["id"].clone(),
                        &format!("/elements/{index}/isDeleted"),
                        "substituted",
                    ));
                }
            }
        }
        let is_deleted = value == Some(json!(true));
        let baseline = if is_deleted {
            scene(json!([]))
        } else {
            clean.clone()
        };
        assert_eq!(
            parse(&source).frames().count(),
            if is_deleted { 0 } else { 2 }
        );
        for format in FORMATS {
            assert_scene(&source, &baseline, format, json!(rows));
        }
    }
}

fn generated_identity_vectors_and_duplicate_collisions_are_stable() {
    // Independent hashlib calculations of the frozen domain/LE transcript;
    // never captured from renderer diagnostics. Text/frame and lexical-number
    // distinctions supplement the previously qualified rectangle vectors.
    for (element, id) in [
        (
            r#"{"type":"text","text":"A","fontFamily":3,"fontSize":16,"width":30,"height":20,"updated":1}"#,
            "generated-2b6a138a3b76fd4f275a6d6d29c3bbeb3b2a6ffa71c69ce14db409daf6f72500",
        ),
        (
            r#"{"type":"frame","width":80,"height":60,"updated":1}"#,
            "generated-d5573840640af866306cb1a2bee4407f6a64b9b270be6f3233176e73b758e245",
        ),
        (
            r#"{"type":"rectangle","width":30,"height":20,"updated":1}"#,
            "generated-40d206f25fa560aa51a85a48dc00817182d9d0a3219e25d49026baa30a3863fb",
        ),
        (
            r#"{"type":"rectangle","width":30.0,"height":20,"updated":1}"#,
            "generated-c265fd968e53c85a7beeab374d8a07c9aee2e37c5b000e8ad01b87dcdf3b7283",
        ),
        (
            r#"{"width":30,"type":"rectangle","height":20,"updated":1}"#,
            "generated-2b09133be614a0bf7845c9292f3aff88c6df71c7442d59403ad5f5e07ddda6c8",
        ),
    ] {
        let source = format!(
            r#"{{"type":"excalidraw","version":2,"elements":[{element}],"appState":{{}},"files":{{}}}}"#
        );
        let clean = source.replace(element, &format!("{{\"id\":\"{id}\",{}", &element[1..]));
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let outcome = Renderer::new()
                .render(&SceneDocument::parse(source.as_bytes()).unwrap(), &request)
                .unwrap();
            let baseline = Renderer::new()
                .render(&SceneDocument::parse(clean.as_bytes()).unwrap(), &request)
                .unwrap();
            assert!(baseline.report().is_empty());
            assert_eq!(outcome.artifact().bytes(), baseline.artifact().bytes());
            assert_report(
                &outcome,
                format,
                json!([diagnostic(
                    "normalization.missing-element-id",
                    json!(id),
                    "/elements/0/id",
                    "repaired"
                )]),
            );
        }
    }
    // Previously qualified exact vectors, lifted to the public facade. Keeping
    // source bytes literal matters: member order is part of original content.
    let vectors = [
        (
            r#"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"#,
            r#"{"type":"excalidraw","version":2,"elements":[{"id":"generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792","type":"rectangle","width":180,"height":100,"seed":642336616,"updated":0}],"appState":{},"files":{}}"#,
            "generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792",
            0,
            "normalization.missing-element-id",
            true,
        ),
        (
            r#"{"type":"excalidraw","version":2,"elements":[{"id":"duplicate","type":"rectangle","x":0,"y":0,"width":80,"height":60,"seed":1,"updated":1},{"id":"duplicate","type":"rectangle","x":100,"y":0,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"#,
            r#"{"type":"excalidraw","version":2,"elements":[{"id":"duplicate","type":"rectangle","x":0,"y":0,"width":80,"height":60,"seed":1,"updated":1},{"id":"generated-3180035c77cb940ec4c79c4729441c64bad5072d494bd981721e7e1b9c85c3a3","type":"rectangle","x":100,"y":0,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"#,
            "generated-3180035c77cb940ec4c79c4729441c64bad5072d494bd981721e7e1b9c85c3a3",
            1,
            "normalization.duplicate-element-id",
            false,
        ),
        (
            r#"{"type":"excalidraw","version":2,"elements":[{"id":"generated-d02dc903e43157ace063528d673d5657c6e6433aa467e643d1b7a2cb9296386a","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"type":"rectangle","x":100,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"#,
            r#"{"type":"excalidraw","version":2,"elements":[{"id":"generated-d02dc903e43157ace063528d673d5657c6e6433aa467e643d1b7a2cb9296386a","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"id":"generated-604f36b7127cb8ae0e242c4cf18cbb0f437f585790ef41c6f0edaa07a7676e63","type":"rectangle","x":100,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"#,
            "generated-604f36b7127cb8ae0e242c4cf18cbb0f437f585790ef41c6f0edaa07a7676e63",
            1,
            "normalization.missing-element-id",
            false,
        ),
    ];
    for (source, clean, id, index, code, seed_and_timestamp) in vectors {
        let document = SceneDocument::parse(source.as_bytes()).unwrap();
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let first = Renderer::new().render(&document, &request).unwrap();
            let baseline = Renderer::new()
                .render(&SceneDocument::parse(clean.as_bytes()).unwrap(), &request)
                .unwrap();
            assert!(baseline.report().is_empty());
            assert_eq!(first.artifact().bytes(), baseline.artifact().bytes());
            let mut rows = vec![diagnostic(
                code,
                json!(id),
                &format!("/elements/{index}/id"),
                "repaired",
            )];
            if seed_and_timestamp {
                let mut seed = diagnostic(
                    "normalization.zero-rough-seed",
                    json!(id),
                    "/elements/0/seed",
                    "repaired",
                );
                seed["effects"] = json!(["input_repair", "visual"]);
                rows.push(seed);
                rows.push(diagnostic(
                    "normalization.missing-timestamp",
                    json!(id),
                    "/elements/0/updated",
                    "repaired",
                ));
            }
            assert_report(&first, format, json!(rows));
            let alternate = source
                .replace("\"type\"", "\"\\u0074ype\"")
                .replace(',', ",\n ");
            let second = Renderer::new()
                .render(
                    &SceneDocument::parse(alternate.as_bytes()).unwrap(),
                    &request,
                )
                .unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
            let metadata = RenderRequest::builder(format)
                .metadata_options(MetadataOptions::new(true, false, false))
                .build()
                .unwrap();
            let result = Renderer::new().render(&document, &metadata).unwrap();
            assert_eq!(
                super::artifact_metadata::extract(format, result.artifact().bytes()).1,
                source.as_bytes()
            );
        }
    }
}

fn assert_nonparticipating_identity_boundaries() {
    let initial = scene(json!([
        {"type":"selection","id":null,"isDeleted":"invalid","updated":null,"unknown":true},
        {"type":"future","id":"unknown","isDeleted":false,"updated":null,"unknown":true},
        {"type":null,"id":"malformed","isDeleted":false,"updated":null,"unknown":true}
    ]));
    for missing in [false, true] {
        let mut source = initial.clone();
        if missing {
            for element in source["elements"].as_array_mut().unwrap() {
                element.as_object_mut().unwrap().remove("id");
                element.as_object_mut().unwrap().remove("updated");
            }
        }
        for format in FORMATS {
            let mut unknown = diagnostic(
                "compatibility.unsupported-element-type",
                if missing {
                    Value::Null
                } else {
                    json!("unknown")
                },
                "/elements/1",
                "omitted",
            );
            unknown["effects"] = json!(["visual", "semantic"]);
            let mut malformed = diagnostic(
                "compatibility.malformed-element",
                if missing {
                    Value::Null
                } else {
                    json!("malformed")
                },
                "/elements/2",
                "omitted",
            );
            malformed["effects"] = json!(["visual"]);
            let document = parse(&source);
            let request = RenderRequest::builder(format)
                .metadata_options(MetadataOptions::new(true, false, false))
                .build()
                .unwrap();
            let outcome = Renderer::new().render(&document, &request).unwrap();
            assert_report(&outcome, format, json!([unknown, malformed]));
            assert_eq!(
                super::artifact_metadata::extract(format, outcome.artifact().bytes()).1,
                serde_json::to_vec(&source).unwrap()
            );
        }
    }
}

#[test]
fn duplicate_ids_retain_first_occurrence_and_resolve_frame_references() {
    let source = scene(json!([
        {"id":"frame","type":"frame","width":80,"height":60,"updated":0},
        {"id":"frame","type":"magicframe","x":200,"width":80,"height":60,"updated":0},
        {"id":"child","type":"rectangle","x":5,"y":5,"width":30,"height":20,"frameId":"frame","seed":1,"updated":0}
    ]));
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let result = Renderer::new().render(&parse(&source), &request).unwrap();
        let mut clean = source.clone();
        let id = result.report().diagnostics()[0].element_id().unwrap();
        clean["elements"][1]["id"] = json!(id);
        let rows = json!([diagnostic(
            "normalization.duplicate-element-id",
            json!(id),
            "/elements/1/id",
            "repaired"
        )]);
        assert_scene(&source, &clean, format, rows.clone());
        assert_frames(&source, &clean, format, &[json!([]), rows]);
    }
}

#[test]
fn missing_and_invalid_ids_have_exact_recovery_and_target_participation() {
    assert_nonparticipating_identity_boundaries();
    generated_identity_vectors_and_duplicate_collisions_are_stable();
    let mut ids = std::collections::BTreeSet::new();
    for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
        let mut source = framed(rectangle("paint"));
        let valid = value
            .as_ref()
            .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
        let missing = value.is_none();
        if let Some(value) = &value {
            source["elements"][1]["id"] = value.clone();
        } else {
            source["elements"][1].as_object_mut().unwrap().remove("id");
        }
        let mut identity = None;
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let first = Renderer::new().render(&parse(&source), &request).unwrap();
            let id = if valid {
                value.as_ref().unwrap().as_str().unwrap().to_owned()
            } else {
                assert_eq!(first.report().diagnostics().len(), 1);
                let id = first.report().diagnostics()[0]
                    .element_id()
                    .unwrap()
                    .to_owned();
                assert!(id.starts_with("generated-") && id.len() == 74);
                assert!(id[10..]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
                id
            };
            if let Some(expected) = &identity {
                assert_eq!(&id, expected);
            } else {
                identity = Some(id.clone());
            }
            let mut clean = source.clone();
            clean["elements"][1]["id"] = json!(id);
            let rows = if valid {
                json!([])
            } else {
                json!([diagnostic(
                    if missing {
                        "normalization.missing-element-id"
                    } else {
                        "compatibility.unsupported-property-value"
                    },
                    json!(id),
                    "/elements/1/id",
                    "repaired"
                )])
            };
            assert_scene(&source, &clean, format, rows.clone());
            assert_frames(&source, &clean, format, &[rows, json!([])]);
            let mut deleted = source.clone();
            deleted["elements"][1]["isDeleted"] = json!(true);
            let mut deleted_clean = clean;
            deleted_clean["elements"][1]["isDeleted"] = json!(true);
            assert_scene(&deleted, &deleted_clean, format, json!([]));
            assert_frames(&deleted, &deleted_clean, format, &[json!([]), json!([])]);
        }
        if !valid {
            assert!(
                ids.insert(identity.unwrap()),
                "distinct original values have distinct generated identities"
            );
        }
    }
    // Every traversed revision-1 family uses the identity contract, including
    // both frame kinds and the legacy draw migration.
    for value in [None, Some(json!(null)), Some(json!(""))] {
        let mut source = participating_kinds();
        for element in source["elements"].as_array_mut().unwrap() {
            if let Some(value) = &value {
                element["id"] = value.clone();
            } else {
                element.as_object_mut().unwrap().remove("id");
            }
        }
        let mut expected_ids = None;
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            let mut clean = source.clone();
            assert_eq!(
                result.report().diagnostics().len(),
                clean["elements"].as_array().unwrap().len()
            );
            let ids: Vec<_> = result
                .report()
                .diagnostics()
                .iter()
                .map(|d| d.element_id().unwrap().to_owned())
                .collect();
            assert_eq!(
                ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
                ids.len()
            );
            if let Some(expected) = &expected_ids {
                assert_eq!(&ids, expected);
            } else {
                expected_ids = Some(ids.clone());
            }
            let mut rows = Vec::new();
            for (index, id) in ids.iter().enumerate() {
                clean["elements"][index]["id"] = json!(id);
                rows.push(diagnostic(
                    if value.is_none() {
                        "normalization.missing-element-id"
                    } else {
                        "compatibility.unsupported-property-value"
                    },
                    json!(id),
                    &format!("/elements/{index}/id"),
                    "repaired",
                ));
            }
            assert_scene(&source, &clean, format, json!(rows));
        }
    }
    // Position is part of identity even when the preceding occurrence is not
    // rendered; deleting content does not renumber source occurrences.
    let mut shifted = scene(json!([{"type":"rectangle","isDeleted":true},
        {"type":"rectangle","width":30,"height":20,"seed":1,"updated":1}]));
    let unshifted =
        scene(json!([{ "type":"rectangle","width":30,"height":20,"seed":1,"updated":1}]));
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let first = Renderer::new()
            .render(&parse(&unshifted), &request)
            .unwrap();
        let second = Renderer::new().render(&parse(&shifted), &request).unwrap();
        assert_ne!(
            first.report().diagnostics()[0].element_id(),
            second.report().diagnostics()[0].element_id()
        );
        shifted["elements"][0]["id"] = json!("unused");
        let third = Renderer::new().render(&parse(&shifted), &request).unwrap();
        assert_eq!(second.artifact().bytes(), third.artifact().bytes());
        assert_eq!(second.report(), third.report());
    }
}
