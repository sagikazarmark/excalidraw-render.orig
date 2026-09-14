//! Duplicate identity judgments use explicit disambiguated scenes as their
//! artifact oracle and independently authored complete diagnostic fields.
use excalidraw_render::{
    FrameOptions, FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest,
    Renderer, SceneDocument,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn scene(elements: Value) -> Value {
    json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":{}})
}
fn rectangle(id: &str, x: u32) -> Value {
    json!({"id":id,"type":"rectangle","x":x,"y":5,"width":30,"height":20,"seed":1,"updated":0})
}

fn expected_duplicate(format: OutputFormat, id: &str, index: usize, visual: bool) -> Value {
    let effects = if !visual {
        json!(["input_repair"])
    } else if format == OutputFormat::Png {
        json!(["input_repair", "visual"])
    } else {
        json!(["input_repair", "visual", "semantic"])
    };
    json!({"code":"normalization.duplicate-element-id","severity":"warning","effects":effects,
        "format":format.as_str(),"element_id":id,"json_pointer":format!("/elements/{index}/id"),
        "details":{"kind":"compatibility","recovery":"repaired","recipe_id":null}})
}

fn assert_report(outcome: &RenderOutcome, rows: Value) {
    let mut actual = serde_json::to_value(outcome.report()).unwrap();
    for row in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    assert_eq!(actual, json!({"schema_version":1,"diagnostics":rows}));
}

fn duplicate_frame_scene() -> Value {
    let mut child = rectangle("child", 65);
    child["frameId"] = json!("duplicate");
    scene(json!([
        {"id":"duplicate","type":"frame","width":80,"height":60,"updated":0},
        {"id":"duplicate","type":"magicframe","x":200,"width":80,"height":60,"updated":0},child
    ]))
}

#[test]
fn duplicate_frame_identity_reports_the_channels_consuming_disambiguation() {
    let source = duplicate_frame_scene();
    for format in FORMATS {
        for clip in [true, false] {
            let request = RenderRequest::builder(format)
                .frame_options(FrameOptions::new(true, clip, false, false))
                .build()
                .unwrap();
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            let id = result.report().diagnostics()[0].element_id().unwrap();
            assert_report(&result, json!([expected_duplicate(format, id, 1, clip)]));
        }
    }
}

fn assert_frames(source: &Value, clean: &Value, format: OutputFormat, rows: &[Value]) {
    let document = parse(source);
    let clean_document = parse(clean);
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let renderer = Renderer::new();
    let frames: Vec<_> = document.frames().collect();
    let clean_frames: Vec<_> = clean_document.frames().collect();
    assert_eq!(frames.len(), rows.len());
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for ((frame, clean_frame), rows) in frames.iter().zip(clean_frames).zip(rows) {
        let direct = renderer.render_frame(*frame, &request).unwrap();
        let baseline = Renderer::new().render_frame(clean_frame, &request).unwrap();
        let replay = batch.next().unwrap().into_result().unwrap();
        assert!(baseline.report().is_empty());
        assert_eq!(direct.artifact().bytes(), baseline.artifact().bytes());
        assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
        assert_eq!(
            serde_json::to_vec(direct.report()).unwrap(),
            serde_json::to_vec(replay.report()).unwrap()
        );
        assert_report(&direct, rows.clone());
    }
    assert!(batch.next().is_none());
}

#[test]
fn endpoint_disambiguation_changes_frame_selection_but_not_stored_scene_geometry() {
    for field in ["startBinding", "endBinding"] {
        let mut first = rectangle("duplicate", 120);
        first["frameId"] = json!("frame");
        let second = rectangle("duplicate", 240);
        let mut arrow = json!({"id":"arrow","type":"arrow","x":5,"y":5,"width":30,"height":20,"points":[[0,0],[30,20]],"frameId":"frame","seed":1,"updated":0});
        arrow[field] = json!({"elementId":"duplicate","focus":0,"gap":0});
        let source = scene(json!([
            {"id":"frame","type":"frame","width":80,"height":60,"updated":0},first,second,arrow
        ]));
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            let id = result.report().diagnostics()[0].element_id().unwrap();
            let mut clean = source.clone();
            clean["elements"][2]["id"] = json!(id);
            assert_scene_equivalence(
                &source,
                &clean,
                &request,
                json!([expected_duplicate(format, id, 2, false)]),
            );
            // The duplicate itself is outside the selected frame, so its repair
            // remains target-silent even though the binding selects the first.
            assert_frames(&source, &clean, format, &[json!([])]);
            let mut selected = source.clone();
            selected["elements"][2]["frameId"] = json!("frame");
            selected["elements"][2]["x"] = json!(40);
            let selected_document = parse(&selected);
            let first = Renderer::new()
                .render(&selected_document, &request)
                .unwrap();
            let id = first.report().diagnostics()[0].element_id().unwrap();
            let mut selected_clean = selected.clone();
            selected_clean["elements"][2]["id"] = json!(id);
            assert_frames(
                &selected,
                &selected_clean,
                format,
                &[json!([expected_duplicate(format, id, 2, true)])],
            );
        }
    }
}

fn assert_scene_equivalence(
    source: &Value,
    clean: &Value,
    request: &RenderRequest,
    expected: Value,
) {
    let renderer = Renderer::new();
    let document = parse(source);
    let first = renderer.render(&document, request).unwrap();
    let second = Renderer::new().render(&document, request).unwrap();
    let baseline = Renderer::new().render(&parse(clean), request).unwrap();
    assert!(baseline.report().is_empty(), "{:?}", baseline.report());
    assert_eq!(first.artifact().bytes(), baseline.artifact().bytes());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    assert_report(&first, expected.clone());
    let metadata = RenderRequest::builder(request.format())
        .frame_options(request.frame_options().clone())
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let embedded = renderer.render(&document, &metadata).unwrap();
    assert_report(&embedded, expected);
    assert_eq!(
        super::artifact_metadata::extract(request.format(), embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
}

#[test]
fn duplicate_bound_container_reports_visual_and_semantic_disambiguation() {
    let mut first = rectangle("duplicate", 0);
    first["angle"] = json!(0.4);
    first["boundElements"] = json!([{"id":"label","type":"text"}]);
    let mut second = rectangle("duplicate", 100);
    second["boundElements"] = json!([{"id":"label","type":"text"}]);
    let source = scene(json!([first,second,
        {"id":"label","type":"text","x":5,"y":5,"width":20,"height":20,"text":"A",
         "fontFamily":3,"fontSize":16,"lineHeight":1.25,"containerId":"duplicate","updated":0}
    ]));
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let result = Renderer::new().render(&parse(&source), &request).unwrap();
        let id = result.report().diagnostics()[0].element_id().unwrap();
        let mut clean = source.clone();
        clean["elements"][1]["id"] = json!(id);
        assert_scene_equivalence(
            &source,
            &clean,
            &request,
            json!([expected_duplicate(format, id, 1, true)]),
        );
    }
}

#[test]
fn reverse_bound_text_disambiguation_and_silence_follow_reciprocity() {
    for legacy in [false, true] {
        for reciprocal in [false, true] {
            for (text, later_text) in [("", ""), ("A", "A"), ("", "A"), ("A", "")] {
                let mut container = rectangle("container", 0);
                if legacy {
                    container["boundElementIds"] = json!(["duplicate"]);
                } else {
                    container["boundElements"] = json!([{"id":"duplicate","type":"text"}]);
                }
                let mut first = json!({"id":"duplicate","type":"text","x":5,"y":5,"width":20,"height":20,"text":text,
                    "fontFamily":3,"fontSize":16,"lineHeight":1.25,"updated":0});
                if reciprocal {
                    first["containerId"] = json!("container");
                }
                let mut second = first.clone();
                second["x"] = json!(100);
                second["text"] = json!(later_text);
                let source = scene(json!([container, first, second]));
                for format in FORMATS {
                    let request = RenderRequest::builder(format).build().unwrap();
                    let result = Renderer::new().render(&parse(&source), &request).unwrap();
                    let id = result.report().diagnostics()[0].element_id().unwrap();
                    let mut clean = source.clone();
                    clean["elements"][2]["id"] = json!(id);
                    assert_scene_equivalence(
                        &source,
                        &clean,
                        &request,
                        json!([expected_duplicate(
                            format,
                            id,
                            2,
                            reciprocal && (!text.is_empty() || !later_text.is_empty())
                        )]),
                    );
                }
            }
        }
    }
}

#[test]
fn duplicate_occurrences_are_independent_and_preserved_in_source_order() {
    for ignored in ["deleted", "selection"] {
        let mut first = rectangle("duplicate", 0);
        first["groupIds"] = json!(["duplicate"]); // Group namespace is not an element reference.
        let second = rectangle("duplicate", 50);
        let third = rectangle("duplicate", 100);
        let ignored = if ignored == "deleted" {
            json!({"id":"duplicate","type":"rectangle","isDeleted":true})
        } else {
            json!({"id":"duplicate","type":"selection"})
        };
        let source = scene(json!([ignored, first, second, third]));
        let mut identities = None;
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            assert_eq!(result.report().diagnostics().len(), 2);
            let ids: Vec<_> = result
                .report()
                .diagnostics()
                .iter()
                .map(|d| d.element_id().unwrap().to_owned())
                .collect();
            assert_ne!(ids[0], ids[1]);
            assert!(ids.iter().all(|id| id != "duplicate"));
            if let Some(expected) = &identities {
                assert_eq!(&ids, expected);
            } else {
                identities = Some(ids.clone());
            }
            let mut clean = source.clone();
            clean["elements"][2]["id"] = json!(ids[0]);
            clean["elements"][3]["id"] = json!(ids[1]);
            assert_scene_equivalence(
                &source,
                &clean,
                &request,
                json!([
                    expected_duplicate(format, &ids[0], 2, false),
                    expected_duplicate(format, &ids[1], 3, false)
                ]),
            );
        }
    }
}

#[test]
fn frame_opacity_and_target_scope_select_duplicate_effects() {
    for opacity in [50, 100] {
        let mut source = duplicate_frame_scene();
        source["elements"][0]["opacity"] = json!(opacity);
        for format in FORMATS {
            let request = RenderRequest::builder(format)
                .frame_options(FrameOptions::new(false, false, false, false))
                .build()
                .unwrap();
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            let id = result.report().diagnostics()[0].element_id().unwrap();
            let mut clean = source.clone();
            clean["elements"][1]["id"] = json!(id);
            assert_scene_equivalence(
                &source,
                &clean,
                &request,
                json!([expected_duplicate(format, id, 1, opacity != 100)]),
            );
            assert_frames(
                &source,
                &clean,
                format,
                &[json!([]), json!([expected_duplicate(format, id, 1, false)])],
            );
        }
    }
}

#[test]
fn empty_text_does_not_consume_disambiguated_frame_opacity() {
    let mut source = duplicate_frame_scene();
    source["elements"][0]["opacity"] = json!(50);
    source["elements"][2] = json!({"id":"child","type":"text","x":5,"y":5,"width":30,"height":20,"text":"","fontFamily":3,"fontSize":16,"lineHeight":1.25,"frameId":"duplicate","updated":0});
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .frame_options(FrameOptions::new(false, false, false, false))
            .build()
            .unwrap();
        let result = Renderer::new().render(&parse(&source), &request).unwrap();
        let id = result.report().diagnostics()[0].element_id().unwrap();
        let mut clean = source.clone();
        clean["elements"][1]["id"] = json!(id);
        assert_scene_equivalence(
            &source,
            &clean,
            &request,
            json!([expected_duplicate(format, id, 1, false)]),
        );
        let mut outside = source.clone();
        outside["elements"][2]["x"] = json!(500);
        let clip_request = RenderRequest::builder(format)
            .frame_options(FrameOptions::new(true, true, false, false))
            .build()
            .unwrap();
        let clipped = Renderer::new()
            .render(&parse(&outside), &clip_request)
            .unwrap();
        assert_report(&clipped, json!([expected_duplicate(format, id, 1, true)]));
        outside["elements"][2]
            .as_object_mut()
            .unwrap()
            .remove("frameId");
        let unclipped = Renderer::new()
            .render(&parse(&outside), &clip_request)
            .unwrap();
        assert_ne!(clipped.artifact().bytes(), unclipped.artifact().bytes());
    }
}

#[test]
fn nonreciprocal_first_container_disambiguation_reports_text_loss() {
    let first = rectangle("duplicate", 0);
    let mut second = rectangle("duplicate", 100);
    second["boundElements"] = json!([{"id":"label","type":"text"}]);
    let source = scene(
        json!([first,second,{"id":"label","type":"text","x":5,"y":5,"width":20,"height":20,"text":"A","fontFamily":3,"fontSize":16,"lineHeight":1.25,"containerId":"duplicate","updated":0}]),
    );
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let result = Renderer::new().render(&parse(&source), &request).unwrap();
        let id = result.report().diagnostics()[0].element_id().unwrap();
        let mut clean = source.clone();
        clean["elements"][1]["id"] = json!(id);
        assert_scene_equivalence(
            &source,
            &clean,
            &request,
            json!([expected_duplicate(format, id, 1, true)]),
        );
    }
}

#[test]
fn retained_placeholders_consume_disambiguated_frame_opacity() {
    let mut source = duplicate_frame_scene();
    source["elements"][0]["opacity"] = json!(50);
    source["elements"][2]["type"] = json!("future");
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .frame_options(FrameOptions::new(false, false, false, false))
            .build()
            .unwrap();
        let result = Renderer::new().render(&parse(&source), &request).unwrap();
        let id = result.report().diagnostics()[0].element_id().unwrap();
        let rows = json!([
            expected_duplicate(format,id,1,true),
            {"code":"compatibility.unsupported-element-type","severity":"warning","effects":["visual","semantic"],"format":format.as_str(),"element_id":"child","json_pointer":"/elements/2","details":{"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"}},
            {"code":"font.liberation-sans-substituted","severity":"info","effects":["visual"],"format":format.as_str(),"element_id":"child","json_pointer":"/elements/2","details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}}
        ]);
        assert_report(&result, rows);
        let mut unrelated = source.clone();
        unrelated["elements"][2]
            .as_object_mut()
            .unwrap()
            .remove("frameId");
        let without = Renderer::new()
            .render(&parse(&unrelated), &request)
            .unwrap();
        assert_ne!(result.artifact().bytes(), without.artifact().bytes());
    }
}

#[test]
fn text_loss_and_placeholder_effects_repeat_through_frame_batches() {
    for placeholder in [false, true] {
        let mut source = if placeholder {
            let mut source = duplicate_frame_scene();
            source["elements"][0]["opacity"] = json!(50);
            source["elements"][2]["type"] = json!("future");
            source
        } else {
            let first = rectangle("duplicate", 5);
            let mut second = rectangle("duplicate", 45);
            second["boundElements"] = json!([{"id":"label","type":"text"}]);
            scene(
                json!([first,second,{"id":"label","type":"text","x":5,"y":5,"width":20,"height":20,"text":"A","fontFamily":3,"fontSize":16,"lineHeight":1.25,"containerId":"duplicate","updated":0}]),
            )
        };
        // The encompassing third frame retains both duplicate occurrences and
        // the affected consumer; it also gives placeholder/text-loss a common
        // standalone/batch target.
        source["elements"].as_array_mut().unwrap().push(json!({"id":"outer","type":"frame","x":-10,"y":-10,"width":400,"height":100,"updated":0}));
        if !placeholder {
            for index in 0..3 {
                source["elements"][index]["frameId"] = json!("outer");
            }
        }
        for format in FORMATS {
            let document = parse(&source);
            let request = FrameRenderRequest::builder(format).build().unwrap();
            let renderer = Renderer::new();
            let frames: Vec<_> = document.frames().collect();
            let mut batch = renderer
                .render_frames(&document, frames.iter().copied(), &request)
                .unwrap();
            for frame in &frames {
                let direct = renderer.render_frame(*frame, &request).unwrap();
                let replay = batch.next().unwrap().into_result().unwrap();
                assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
                assert_eq!(
                    serde_json::to_vec(direct.report()).unwrap(),
                    serde_json::to_vec(replay.report()).unwrap()
                );
            }
            assert!(batch.next().is_none());
            // Reversed fresh target order must not depend on journal/cache use.
            for frame in frames.iter().rev() {
                let first = Renderer::new().render_frame(*frame, &request).unwrap();
                let second = Renderer::new().render_frame(*frame, &request).unwrap();
                assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                assert_eq!(first.report(), second.report());
            }
        }
    }
}
