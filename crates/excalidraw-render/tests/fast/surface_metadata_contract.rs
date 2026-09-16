//! Authored surface contracts: editor state is preserved, but never rendered.
//! Field lists come from the reviewed revision-1 authority, not runtime policy.
use excalidraw_render::{
    ColorMode, FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest,
    Renderer, SceneDocument, Sizing,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
pub(super) fn unsupported_value_contract() {
    persisted_request_controls_validate_but_never_override_the_request();
    line_arrowheads_are_validated_but_do_not_paint_or_change_frame_exports();
}

const EDITOR_FIELDS: &[&str] = &[
    "contextMenu",
    "showWelcomeScreen",
    "isLoading",
    "errorMessage",
    "activeEmbeddable",
    "newElement",
    "resizingElement",
    "multiElement",
    "selectionElement",
    "isBindingEnabled",
    "startBoundElement",
    "suggestedBindings",
    "frameToHighlight",
    "editingFrame",
    "elementsToHighlight",
    "editingTextElement",
    "editingLinearElement",
    "activeTool",
    "penMode",
    "penDetected",
    "currentItemStrokeColor",
    "currentItemBackgroundColor",
    "currentItemFillStyle",
    "currentItemStrokeWidth",
    "currentItemStrokeStyle",
    "currentItemRoughness",
    "currentItemOpacity",
    "currentItemFontFamily",
    "currentItemFontSize",
    "currentItemTextAlign",
    "currentItemStartArrowhead",
    "currentItemEndArrowhead",
    "currentHoveredFontFamily",
    "currentItemRoundness",
    "currentItemArrowType",
    "scrollX",
    "scrollY",
    "cursorButton",
    "scrolledOutside",
    "name",
    "isResizing",
    "isRotating",
    "zoom",
    "openMenu",
    "openPopup",
    "openSidebar",
    "openDialog",
    "defaultSidebarDockedPreference",
    "lastPointerDownWith",
    "selectedElementIds",
    "hoveredElementIds",
    "previousSelectedElementIds",
    "selectedElementsAreBeingDragged",
    "shouldCacheIgnoreZoom",
    "toast",
    "zenModeEnabled",
    "theme",
    "gridSize",
    "gridStep",
    "gridModeEnabled",
    "viewModeEnabled",
    "selectedGroupIds",
    "editingGroupId",
    "width",
    "height",
    "offsetTop",
    "offsetLeft",
    "fileHandle",
    "collaborators",
    "stats",
    "currentChartType",
    "pasteDialog",
    "pendingImageElementId",
    "showHyperlinkPopup",
    "selectedLinearElement",
    "snapLines",
    "originSnapOffset",
    "objectsSnapModeEnabled",
    "userToFollow",
    "followedBy",
    "isCropping",
    "croppingElementId",
    "searchMatches",
    "isSidebarDocked",
];

fn source() -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
        {"id":"frame","type":"frame","x":0,"y":0,"width":80,"height":60,"updated":1},
        {"id":"paint","type":"rectangle","x":5,"y":5,"width":30,"height":20,
         "frameId":"frame","seed":1,"updated":1,"backgroundColor":"#ff0000","fillStyle":"solid"}
    ],"appState":{},"files":{}})
}

fn editor_scene() -> Value {
    use base64::Engine;
    let mut scene = source();
    let elements = scene["elements"].as_array_mut().unwrap();
    elements[0]["name"] = json!("Board");
    elements.push(
        json!({"id":"label","type":"text","x":5,"y":30,"width":25,"height":20,
        "text":"ABC","fontSize":16,"fontFamily":3,"lineHeight":1.25,"frameId":"frame","updated":1}),
    );
    elements.push(
        json!({"id":"arrow","type":"arrow","x":40,"y":5,"width":25,"height":30,
        "points":[[0,0],[25,30]],"startArrowhead":null,"endArrowhead":"arrow",
        "frameId":"frame","seed":3,"updated":1}),
    );
    elements.push(
        json!({"id":"image","type":"image","x":40,"y":40,"width":10,"height":10,
        "fileId":"asset","frameId":"frame","updated":1}),
    );
    let png = crate::common::codec_fixtures::png_rgba(1, 1, &[[0, 255, 0, 255]]);
    scene["files"] = json!({"asset":{"id":"asset","created":1,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png))}});
    scene
}

fn parse(value: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}

// Includes every JSON type, nested names which are rendering-affecting outside
// opaque metadata, nontrivial viewport values and supported enum spellings.
fn values() -> Vec<Value> {
    vec![
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(-125.5),
        json!(300),
        json!(1),
        json!(3),
        json!(5),
        json!(""),
        json!("dark"),
        json!("hachure"),
        json!("dashed"),
        json!("right"),
        json!("arrow"),
        json!("round"),
        json!("elbow"),
        json!("mouse"),
        json!("down"),
        json!("bar"),
        json!([]),
        json!([900, -800.5]),
        json!(["paint", {"unknown": true}]),
        json!({}),
        json!({"type":"text","value":4,"strokeColor":"red","isDeleted":true,
            "paint":true,"frame":true,"nested":{"future/render~field":[null,1,"x"]}}),
    ]
}

fn editor_values(field: &str) -> Vec<Value> {
    let mut probes = values();
    let strings: &[&str] = match field {
        "openMenu" => &["canvas", "shape"],
        "openPopup" => &[
            "canvasBackground",
            "elementBackground",
            "elementStroke",
            "fontFamily",
        ],
        "showHyperlinkPopup" => &["info", "editor"],
        "lastPointerDownWith" => &["mouse", "pen", "touch"],
        "currentItemFillStyle" => &[
            "hachure",
            "cross-hatch",
            "solid",
            "zigzag",
            "zigzag-line",
            "dots",
            "dashed",
        ],
        "currentItemStrokeStyle" => &["solid", "dashed", "dotted"],
        "currentItemTextAlign" => &["left", "center", "right"],
        "currentItemStartArrowhead" | "currentItemEndArrowhead" => &[
            "arrow",
            "bar",
            "dot",
            "circle",
            "circle_outline",
            "triangle",
            "triangle_outline",
            "diamond",
            "diamond_outline",
            "crowfoot_one",
            "crowfoot_many",
            "crowfoot_one_or_many",
        ],
        "currentItemRoundness" | "currentItemArrowType" => &["round", "sharp", "elbow"],
        "currentChartType" => &["bar", "line"],
        "cursorButton" => &["up", "down"],
        "theme" => &["light", "dark"],
        "currentItemStrokeColor" | "currentItemBackgroundColor" => &["#ff0000", "transparent"],
        _ => &[],
    };
    probes.extend(strings.iter().map(|s| json!(s)));
    if matches!(field, "currentItemFontFamily" | "currentHoveredFontFamily") {
        probes.extend([1, 2, 3, 5, 6, 7, 8, 9].into_iter().map(|n| json!(n)));
    }
    probes
}

#[test]
fn editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports() {
    let baseline = editor_scene();
    for format in FORMATS {
        let renderer = Renderer::new();
        let request = RenderRequest::builder(format).build().unwrap();
        let plain = renderer.render(&parse(&baseline), &request).unwrap();
        assert!(plain.report().is_empty());
        // One field at a time: a later ignored field cannot mask an earlier
        // field accidentally consumed by normalization or projection.
        for field in EDITOR_FIELDS {
            for value in editor_values(field) {
                let mut changed = baseline.clone();
                changed["appState"][field] = value;
                let result = Renderer::new().render(&parse(&changed), &request).unwrap();
                assert_eq!(
                    result.artifact().bytes(),
                    plain.artifact().bytes(),
                    "{format:?} {field}: {changed}"
                );
                assert_eq!(
                    serde_json::to_vec(result.report()).unwrap(),
                    serde_json::to_vec(plain.report()).unwrap(),
                    "{format:?} {field}"
                );
                // Generic values are covered in the combined preservation/frame
                // matrix below. Exercise additional field-specific valid values
                // here too, where their active editor meaning could matter.
                if !values().contains(&changed["appState"][field]) {
                    assert_preservation_and_frames(&baseline, &changed, format);
                }
            }
        }
        // Exercise preservation and frame selection with the whole editor state
        // present, including descendants that would warn at a consumed boundary.
        for value in values() {
            let mut changed = baseline.clone();
            for field in EDITOR_FIELDS {
                changed["appState"][field] = value.clone();
            }
            assert_preservation_and_frames(&baseline, &changed, format);
        }
    }
}

fn assert_preservation_and_frames(baseline: &Value, changed: &Value, format: OutputFormat) {
    let scene = parse(changed);
    let request = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let first = Renderer::new().render(&scene, &request).unwrap();
    let second = Renderer::new().render(&scene, &request).unwrap();
    assert!(
        first.report().is_empty(),
        "{format:?}: {:?}",
        first.report()
    );
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    let (_, embedded) = super::artifact_metadata::extract(format, first.artifact().bytes());
    assert_eq!(
        embedded,
        serde_json::to_vec(changed).unwrap(),
        "preserve original {format:?}"
    );
    let baseline_scene = parse(baseline);
    let frame_request = FrameRenderRequest::builder(format).build().unwrap();
    let base_frame = baseline_scene.frames().next().unwrap();
    let changed_frame = scene.frames().next().unwrap();
    let base = Renderer::new()
        .render_frame(base_frame, &frame_request)
        .unwrap();
    let actual = Renderer::new()
        .render_frame(changed_frame, &frame_request)
        .unwrap();
    assert_eq!(base.artifact().bytes(), actual.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(base.report()).unwrap(),
        serde_json::to_vec(actual.report()).unwrap()
    );
    let batch_renderer = Renderer::new();
    let batch = batch_renderer
        .render_frames(&scene, scene.frames(), &frame_request)
        .unwrap()
        .next()
        .unwrap()
        .into_result()
        .unwrap();
    assert_eq!(actual.artifact().bytes(), batch.artifact().bytes());
    assert_eq!(actual.report(), batch.report());
}

#[test]
fn scene_metadata_is_preserved_without_changing_static_exports() {
    let baseline = source();
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
        assert!(plain.report().is_empty());
        for field in ["source", "scrollToContent", "libraryItems"] {
            for value in values() {
                let mut changed = baseline.clone();
                changed[field] = value;
                let result = Renderer::new().render(&parse(&changed), &request).unwrap();
                assert_eq!(
                    result.artifact().bytes(),
                    plain.artifact().bytes(),
                    "{format:?} {field}"
                );
                assert!(result.report().is_empty());
                assert_preservation_and_frames(&baseline, &changed, format);
            }
        }
    }
}

#[test]
fn element_editor_metadata_is_preserved_without_changing_static_exports() {
    for (kind, fields) in [
        (
            "rectangle",
            &["version", "versionNonce", "locked", "customData"][..],
        ),
        ("text", &["autoResize", "rawText", "baseline"]),
        ("line", &["lastCommittedPoint"]),
        (
            "arrow",
            &[
                "lastCommittedPoint",
                "fixedSegments",
                "startIsSpecial",
                "endIsSpecial",
            ],
        ),
        ("freedraw", &["lastCommittedPoint"]),
        ("embeddable", &["validated"]),
        ("iframe", &["customData"]),
    ] {
        let mut baseline = source();
        let element = &mut baseline["elements"][1];
        element["type"] = json!(kind);
        if kind == "text" {
            element["text"] = json!("A");
            element["originalText"] = json!("A");
            element["fontFamily"] = json!(3);
            element["fontSize"] = json!(16);
            element["lineHeight"] = json!(1.25);
        }
        if matches!(kind, "line" | "arrow" | "freedraw") {
            element["points"] = json!([[0, 0], [0, 20], [30, 20]]);
        }
        if kind == "arrow" {
            element["elbowed"] = json!(true);
        }
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
            assert!(
                plain.report().is_empty(),
                "{kind} {format:?}: {:?}",
                plain.report()
            );
            for field in fields {
                let mut probes = values();
                probes.push(json!([{"start":[900,800],"end":[700,600],"index":1}]));
                probes.push(json!({"generationData":{"status":"done","html":"<script>unsafe()</script>","message":"x","code":"x","unknown":{"future":true}}}));
                for value in probes {
                    let mut changed = baseline.clone();
                    if kind == "iframe" {
                        changed["elements"][1]["customData"] = json!({"generationData":value});
                    } else {
                        changed["elements"][1][field] = value;
                    }
                    let result = Renderer::new().render(&parse(&changed), &request).unwrap();
                    assert_eq!(
                        result.artifact().bytes(),
                        plain.artifact().bytes(),
                        "{kind} {format:?} {field}"
                    );
                    assert!(
                        result.report().is_empty(),
                        "{kind} {field}: {:?}",
                        result.report()
                    );
                    assert_preservation_and_frames(&baseline, &changed, format);
                }
            }
        }
    }
}

#[test]
fn file_editor_metadata_is_preserved_for_referenced_and_unreferenced_records() {
    use base64::Engine;
    let png = crate::common::codec_fixtures::png_rgba(1, 1, &[[255, 0, 0, 255]]);
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    let mut baseline = source();
    baseline["elements"][1]["type"] = json!("image");
    baseline["elements"][1]["fileId"] = json!("file");
    baseline["files"] = json!({"file":{"id":"file","created":1,"dataURL":data}});
    for referenced in [true, false] {
        if !referenced {
            baseline["elements"][1]["isDeleted"] = json!(true);
        }
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
            assert!(plain.report().is_empty());
            for field in ["lastRetrieved", "version"] {
                for value in values() {
                    let mut changed = baseline.clone();
                    changed["files"]["file"][field] = value;
                    let result = Renderer::new().render(&parse(&changed), &request).unwrap();
                    assert_eq!(plain.artifact().bytes(), result.artifact().bytes());
                    assert!(result.report().is_empty());
                    assert_preservation_and_frames(&baseline, &changed, format);
                }
            }
        }
    }
}

#[test]
fn common_editor_metadata_and_fractional_indices_preserve_source_order() {
    let mut baseline = editor_scene();
    // Overlap makes source-order changes observable; stored indices deliberately
    // conflict with the array order. Common metadata also covers deleted and
    // placeholder records, whose participation differs from ordinary elements.
    baseline["elements"].as_array_mut().unwrap().extend([
        json!({"id":"overlap","type":"ellipse","x":5,"y":5,"width":30,"height":20,
            "frameId":"frame","backgroundColor":"blue","seed":7,"updated":1}),
        json!({"id":"deleted","type":"diamond","x":5,"y":5,"width":30,"height":20,
            "isDeleted":true,"seed":8,"updated":1}),
    ]);
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
        assert!(plain.report().is_empty());
        for field in ["version", "versionNonce", "locked", "customData", "index"] {
            let mut probes = values();
            probes.extend([json!("a0"), json!("a1"), json!("not an index")]);
            for value in probes {
                let mut changed = baseline.clone();
                for element in changed["elements"].as_array_mut().unwrap() {
                    element[field] = value.clone();
                }
                // Duplicate/descending indices must not reorder paint.
                if field == "index" && value == "a0" {
                    changed["elements"][1][field] = json!("a2");
                }
                let result = Renderer::new().render(&parse(&changed), &request).unwrap();
                assert_eq!(
                    result.artifact().bytes(),
                    plain.artifact().bytes(),
                    "{field} {format:?}"
                );
                assert!(result.report().is_empty());
                assert_preservation_and_frames(&baseline, &changed, format);
            }
        }
    }
}

#[test]
fn nested_generation_and_fixed_segment_metadata_is_preserved_and_silent() {
    for (kind, container, fields, initial) in [
        (
            "iframe",
            "generationData",
            &["status", "html", "message", "code"][..],
            json!({"status":"done","html":"<p>Hello</p>","message":"message","code":"code"}),
        ),
        (
            "arrow",
            "fixedSegments",
            &["start", "end", "index"][..],
            json!({"start":[100,200],"end":[300,400],"index":1}),
        ),
    ] {
        let mut baseline = source();
        baseline["elements"][1]["type"] = json!(kind);
        if kind == "arrow" {
            baseline["elements"][1]["points"] = json!([[0, 0], [0, 20], [30, 20]]);
            baseline["elements"][1]["elbowed"] = json!(true);
        }
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
            assert!(plain.report().is_empty());
            for field in fields {
                for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
                    let mut nested = initial.clone();
                    if let Some(value) = value {
                        nested[field] = value;
                    } else {
                        nested.as_object_mut().unwrap().remove(*field);
                    }
                    let mut changed = baseline.clone();
                    if kind == "iframe" {
                        changed["elements"][1]["customData"] = json!({container:nested});
                    } else {
                        changed["elements"][1][container] = json!([nested]);
                    }
                    let result = Renderer::new().render(&parse(&changed), &request).unwrap();
                    assert_eq!(
                        result.artifact().bytes(),
                        plain.artifact().bytes(),
                        "{kind} {field} {format:?}"
                    );
                    assert!(result.report().is_empty());
                    assert_preservation_and_frames(&baseline, &changed, format);
                }
            }
        }
    }
}

fn assert_property_report(
    outcome: &RenderOutcome,
    format: OutputFormat,
    pointer: &str,
    owner: Value,
    effects: Value,
) {
    let mut report = serde_json::to_value(outcome.report()).unwrap();
    for row in report["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    assert_eq!(
        report,
        json!({"schema_version":1,"diagnostics":[{
            "code":"compatibility.unsupported-property-value","severity":"warning",
            "effects":effects,"format":format.as_str(),"element_id":owner,"json_pointer":pointer,
            "details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}
        }]})
    );
}

fn assert_invalid_preservation_and_frames(
    changed: &Value,
    format: OutputFormat,
    pointer: &str,
    owner: Value,
    effects: Value,
) {
    let scene = parse(changed);
    let request = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let rendered = Renderer::new().render(&scene, &request).unwrap();
    assert_property_report(&rendered, format, pointer, owner.clone(), effects.clone());
    let (_, embedded) = super::artifact_metadata::extract(format, rendered.artifact().bytes());
    assert_eq!(embedded, serde_json::to_vec(changed).unwrap());
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let renderer = Renderer::new();
    let frame = scene.frames().next().unwrap();
    let direct = renderer.render_frame(frame, &request).unwrap();
    let mut clean = changed.clone();
    let (parent, field) = pointer.rsplit_once('/').unwrap();
    clean
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(field);
    let clean_scene = parse(&clean);
    let baseline = Renderer::new()
        .render_frame(clean_scene.frames().next().unwrap(), &request)
        .unwrap();
    assert!(baseline.report().is_empty());
    assert_eq!(direct.artifact().bytes(), baseline.artifact().bytes());
    assert_property_report(&direct, format, pointer, owner, effects);
    let batch = renderer
        .render_frames(&scene, scene.frames(), &request)
        .unwrap()
        .next()
        .unwrap()
        .into_result()
        .unwrap();
    assert_eq!(direct.artifact().bytes(), batch.artifact().bytes());
    assert_eq!(direct.report(), batch.report());
}

#[test]
fn persisted_request_controls_validate_but_never_override_the_request() {
    let baseline = source();
    for format in FORMATS {
        for field in [
            "exportBackground",
            "exportEmbedScene",
            "exportScale",
            "exportWithDarkMode",
        ] {
            let alternate = RenderRequest::builder(format)
                .include_background(false)
                .color_mode(ColorMode::Dark)
                .sizing(Sizing::Scale(2.0))
                .build()
                .unwrap();
            for request in [RenderRequest::builder(format).build().unwrap(), alternate] {
                let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
                assert!(plain.report().is_empty());
                for value in std::iter::once(None).chain(values().into_iter().map(Some)) {
                    let mut changed = baseline.clone();
                    let valid = value.as_ref().is_none_or(|v| {
                        if field == "exportScale" {
                            v.as_f64().is_some_and(|n| n > 0.0)
                        } else {
                            v.is_boolean()
                        }
                    });
                    if let Some(value) = value {
                        changed["appState"][field] = value;
                    }
                    let scene = parse(&changed);
                    let first = Renderer::new().render(&scene, &request).unwrap();
                    let second = Renderer::new().render(&scene, &request).unwrap();
                    assert_eq!(
                        plain.artifact().bytes(),
                        first.artifact().bytes(),
                        "{field} {format:?}"
                    );
                    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                    assert_eq!(
                        serde_json::to_vec(first.report()).unwrap(),
                        serde_json::to_vec(second.report()).unwrap()
                    );
                    if valid {
                        assert!(first.report().is_empty());
                    } else {
                        let effects = if field == "exportEmbedScene" {
                            json!(["input_repair", "metadata"])
                        } else {
                            json!(["input_repair", "visual"])
                        };
                        assert_property_report(
                            &first,
                            format,
                            &format!("/appState/{field}"),
                            Value::Null,
                            if field == "exportEmbedScene" {
                                json!(["input_repair", "metadata"])
                            } else {
                                json!(["input_repair", "visual"])
                            },
                        );
                        assert_invalid_preservation_and_frames(
                            &changed,
                            format,
                            &format!("/appState/{field}"),
                            Value::Null,
                            effects,
                        );
                    }
                }
            }
            // Explicit embedding wins even over a persisted false. It preserves
            // the source field rather than embedding a repaired/request value.
            let mut changed = baseline.clone();
            changed["appState"][field] = if field == "exportScale" {
                json!(0.5)
            } else {
                json!(false)
            };
            assert_preservation_and_frames(&baseline, &changed, format);
        }
    }
}

#[test]
fn line_arrowheads_are_validated_but_do_not_paint_or_change_frame_exports() {
    let mut baseline = source();
    baseline["elements"][1]["type"] = json!("line");
    baseline["elements"][1]["points"] = json!([[0, 0], [30, 20]]);
    let heads = [
        "arrow",
        "bar",
        "dot",
        "triangle",
        "triangle_outline",
        "circle",
        "circle_outline",
        "diamond",
        "diamond_outline",
        "crowfoot_one",
        "crowfoot_many",
        "crowfoot_one_or_many",
    ];
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let plain = Renderer::new().render(&parse(&baseline), &request).unwrap();
        assert!(plain.report().is_empty());
        for field in ["startArrowhead", "endArrowhead"] {
            for value in std::iter::once(None).chain(
                values()
                    .into_iter()
                    .chain(heads.iter().map(|s| json!(s)))
                    .map(Some),
            ) {
                let valid = value
                    .as_ref()
                    .is_none_or(|v| v.is_null() || v.as_str().is_some_and(|s| heads.contains(&s)));
                let mut changed = baseline.clone();
                if let Some(value) = value {
                    changed["elements"][1][field] = value;
                }
                let scene = parse(&changed);
                let first = Renderer::new().render(&scene, &request).unwrap();
                let second = Renderer::new().render(&scene, &request).unwrap();
                assert_eq!(first.artifact().bytes(), plain.artifact().bytes());
                assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                assert_eq!(first.report(), second.report());
                if valid {
                    assert!(first.report().is_empty());
                    assert_preservation_and_frames(&baseline, &changed, format);
                } else {
                    assert_property_report(
                        &first,
                        format,
                        &format!("/elements/1/{field}"),
                        json!("paint"),
                        json!(["input_repair", "visual"]),
                    );
                    assert_invalid_preservation_and_frames(
                        &changed,
                        format,
                        &format!("/elements/1/{field}"),
                        json!("paint"),
                        json!(["input_repair", "visual"]),
                    );
                    let mut outside = changed.clone();
                    outside["elements"][1]["frameId"] = Value::Null;
                    outside["elements"][1]["x"] = json!(500);
                    let outside_scene = parse(&outside);
                    let frame_request = FrameRenderRequest::builder(format).build().unwrap();
                    assert!(Renderer::new()
                        .render_frame(outside_scene.frames().next().unwrap(), &frame_request)
                        .unwrap()
                        .report()
                        .is_empty());
                    changed["elements"][1]["isDeleted"] = json!(true);
                    assert!(Renderer::new()
                        .render(&parse(&changed), &request)
                        .unwrap()
                        .report()
                        .is_empty());
                }
            }
        }
    }
}
