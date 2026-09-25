//! Revision-1 geometry/style surface ownership. Fixtures and expected defaults,
//! participation and diagnostics are authored rather than read from runtime policy.
use base64::Engine;
use excalidraw_render::{
    ColorMode, FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest,
    Renderer, SceneDocument,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const KINDS: &[&str] = &[
    "rectangle",
    "diamond",
    "ellipse",
    "line",
    "arrow",
    "draw",
    "freedraw",
    "text",
    "image",
    "frame",
    "magicframe",
    "iframe",
    "embeddable",
    "selection",
];

fn scene(kind: &str) -> Value {
    let mut element = json!({"id":"paint","type":kind,"x":10,"y":10,"width":40,"height":30,
        "seed":1,"updated":0});
    if matches!(kind, "line" | "arrow" | "draw" | "freedraw") {
        element["points"] = json!([[0, 0], [40, 0], [40, 30], [0, 0]]);
    }
    if kind == "text" {
        element["text"] = json!("A");
        element["originalText"] = json!("A");
        element["fontFamily"] = json!(3);
        element["fontSize"] = json!(16);
        element["lineHeight"] = json!(1.25);
    }
    let mut files = json!({});
    if kind == "image" {
        element["fileId"] = json!("asset");
        let png =
            crate::common::codec_fixtures::png_rgba(2, 1, &[[255, 0, 0, 255], [0, 255, 0, 128]]);
        files = json!({"asset":{"id":"asset","created":0,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png))}});
    }
    json!({"type":"excalidraw","version":2,"elements":[
        {"id":"anchor","type":"rectangle","x":-10,"y":-10,"width":5,"height":5,"seed":1,"updated":0},
        element],"appState":{},"files":files})
}

fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}

fn wrong_types() -> Vec<Value> {
    vec![
        Value::Null,
        json!(false),
        json!(true),
        json!(""),
        json!("wrong"),
        json!([]),
        json!([1, 2]),
        json!({}),
        json!({"future":true}),
    ]
}

fn ignored(kind: &str, field: &str) -> bool {
    match kind {
        "selection" => true,
        "text" => [
            "backgroundColor",
            "fillStyle",
            "strokeWidth",
            "strokeStyle",
            "roundness",
            "roughness",
            "seed",
        ]
        .contains(&field),
        "image" => [
            "strokeColor",
            "backgroundColor",
            "fillStyle",
            "strokeWidth",
            "strokeStyle",
            "roughness",
            "seed",
        ]
        .contains(&field),
        "frame" | "magicframe" => [
            "strokeColor",
            "backgroundColor",
            "fillStyle",
            "strokeWidth",
            "strokeStyle",
            "roundness",
            "roughness",
            "seed",
        ]
        .contains(&field),
        "ellipse" | "freedraw" => field == "roundness",
        "arrow" => ["backgroundColor", "fillStyle"].contains(&field),
        _ => false,
    }
}

fn expected(format: OutputFormat, field: &str, color: bool) -> Value {
    json!({"code":if color {"color.invalid"} else {"compatibility.unsupported-property-value"},
        "severity":"warning","effects":["input_repair","visual"],"format":format.as_str(),
        "element_id":"paint","json_pointer":format!("/elements/1/{field}"),
        "details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}})
}

fn assert_report(outcome: &RenderOutcome, rows: Value) {
    let mut actual = serde_json::to_value(outcome.report()).unwrap();
    for row in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    assert_eq!(actual, json!({"schema_version":1,"diagnostics":rows}));
}

fn assert_equivalent(source: &Value, clean: &Value, format: OutputFormat, rows: Value) {
    let request = RenderRequest::builder(format).build().unwrap();
    let document = parse(source);
    let first = Renderer::new().render(&document, &request).unwrap();
    let baseline = Renderer::new().render(&parse(clean), &request).unwrap();
    assert!(
        baseline.report().is_empty(),
        "clean: {:?}",
        baseline.report()
    );
    assert_eq!(
        first.artifact().bytes(),
        baseline.artifact().bytes(),
        "{format:?}: {source}"
    );
    assert_report(&first, rows.clone());
    let second = Renderer::new().render(&document, &request).unwrap();
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    let metadata = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let embedded = Renderer::new().render(&document, &metadata).unwrap();
    assert_report(&embedded, rows);
    assert_eq!(
        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
}

fn assert_frame_equivalent(source: &Value, clean: &Value, format: OutputFormat, rows: Value) {
    let mut source = source.clone();
    let mut clean = clean.clone();
    // A separate outer frame keeps zero-sized repaired content in view. Target
    // discovery remains source-ordinal based for frame/magicframe probes.
    let outer =
        json!({"id":"outer","type":"frame","x":-100,"y":-100,"width":250,"height":250,"updated":0});
    source["elements"]
        .as_array_mut()
        .unwrap()
        .push(outer.clone());
    clean["elements"].as_array_mut().unwrap().push(outer);
    source["elements"][1]["frameId"] = json!("outer");
    clean["elements"][1]["frameId"] = json!("outer");
    let document = parse(&source);
    let clean_document = parse(&clean);
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let frame = document.frames().last().unwrap();
    let clean_frame = clean_document.frames().last().unwrap();
    let renderer = Renderer::new();
    let direct = renderer.render_frame(frame, &request).unwrap();
    let baseline = renderer.render_frame(clean_frame, &request).unwrap();
    assert!(baseline.report().is_empty());
    assert_eq!(direct.artifact().bytes(), baseline.artifact().bytes());
    assert_report(&direct, rows);
    let replay = renderer
        .render_frames(&document, [frame], &request)
        .unwrap()
        .next()
        .unwrap()
        .into_result()
        .unwrap();
    assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
    assert_eq!(direct.report(), replay.report());
    // Excluding the source removes its normalization warning independently of
    // the value being repaired. Deleted values are likewise silent/preserved.
    source["elements"][1]["frameId"] = json!("elsewhere");
    source["elements"][1]["x"] = json!(1000);
    let excluded = parse(&source);
    let outside = renderer
        .render_frame(excluded.frames().last().unwrap(), &request)
        .unwrap();
    assert!(
        outside.report().is_empty(),
        "excluded {format:?}: {:?}",
        outside.report()
    );
    source["elements"][1]["isDeleted"] = json!(true);
    clean["elements"][1]["isDeleted"] = json!(true);
    assert_equivalent(&source, &clean, format, json!([]));
}

#[test]
fn coordinates_dimensions_and_rotation_have_exact_defaults_and_recovery() {
    signed_dimensions_and_linear_point_authority();
    for kind in KINDS {
        for field in ["x", "y", "width", "height", "angle"] {
            let original = scene(kind);
            let mut clean = original.clone();
            clean["elements"][1][field] = json!(0);
            let mut missing = original.clone();
            missing["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove(field);
            for format in FORMATS {
                assert_equivalent(&missing, &clean, format, json!([]));
                for value in wrong_types() {
                    let mut source = original.clone();
                    source["elements"][1][field] = value;
                    let rows = if ignored(kind, field) {
                        json!([])
                    } else {
                        json!([expected(format, field, false)])
                    };
                    assert_equivalent(&source, &clean, format, rows.clone());
                    assert_frame_equivalent(&source, &clean, format, rows);
                    source["elements"][1]["isDeleted"] = json!(true);
                    let mut deleted = clean.clone();
                    deleted["elements"][1]["isDeleted"] = json!(true);
                    assert_equivalent(&source, &deleted, format, json!([]));
                }
            }
        }
    }
}

fn signed_dimensions_and_linear_point_authority() {
    for kind in KINDS {
        for field in ["width", "height"] {
            let mut source = scene(kind);
            source["elements"][1][field] = json!(-20.5);
            let mut clean = source.clone();
            if !matches!(*kind, "line" | "arrow" | "draw") {
                clean["elements"][1][field] = json!(20.5);
                clean["elements"][1][if field == "width" { "x" } else { "y" }] = json!(-10.5);
            } else {
                // Stored extents do not replace the normalized linear points.
                clean["elements"][1][field] = json!(999);
            }
            for format in FORMATS {
                assert_equivalent(&source, &clean, format, json!([]));
                assert_frame_equivalent(&source, &clean, format, json!([]));
            }
        }
    }
    // Independent semantic geometry oracle, with a fixed anchor so translation
    // and dimensions cannot disappear into whole-scene auto-fitting.
    let mut source = scene("rectangle");
    source["elements"][1]["roughness"] = json!(0);
    for format in FORMATS {
        let plain = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).build().unwrap(),
            )
            .unwrap();
        for (field, value) in [
            ("x", 30.5),
            ("y", 30.5),
            ("width", 60.5),
            ("height", 50.5),
            ("angle", 0.5),
        ] {
            let mut changed = source.clone();
            changed["elements"][1][field] = json!(value);
            let result = Renderer::new()
                .render(
                    &parse(&changed),
                    &RenderRequest::builder(format).build().unwrap(),
                )
                .unwrap();
            assert!(result.report().is_empty());
            assert_ne!(
                plain.artifact().bytes(),
                result.artifact().bytes(),
                "{format:?} {field} must participate"
            );
        }
    }
}

#[test]
fn style_domains_default_recover_and_respect_kind_participation() {
    // Existing independently pinned Rough.js paths and consumer semantics are
    // the positive oracle; domain/recovery matrices below do not invent goldens.
    super::rough_style_render::rough_fills_and_stroke_variants_render_directly_in_every_format();
    super::rough_style_render::dots_dashed_and_zigzag_line_fills_render_undiagnosed_in_every_format(
    );
    super::whole_scene_controls::element_opacity_multiplies_color_alpha_in_every_projection();
    for (field, default, valid, invalid) in [
        (
            "strokeWidth",
            json!(2),
            vec![json!(0.5), json!(1), json!(4)],
            vec![json!(0), json!(-1)],
        ),
        (
            "roughness",
            json!(1),
            vec![json!(0), json!(0.5), json!(2), json!(3)],
            vec![json!(-1)],
        ),
        (
            "opacity",
            json!(100),
            vec![json!(0), json!(0.5), json!(50), json!(100)],
            vec![json!(-1), json!(100.1)],
        ),
        (
            "fillStyle",
            json!("solid"),
            [
                "solid",
                "hachure",
                "cross-hatch",
                "zigzag",
                "zigzag-line",
                "dots",
                "dashed",
            ]
            .into_iter()
            .map(|v| json!(v))
            .collect(),
            vec![json!(0)],
        ),
        (
            "strokeStyle",
            json!("solid"),
            ["solid", "dashed", "dotted"]
                .into_iter()
                .map(|v| json!(v))
                .collect(),
            vec![json!(0)],
        ),
    ] {
        for kind in KINDS {
            let mut original = scene(kind);
            if !ignored(kind, "backgroundColor") {
                original["elements"][1]["backgroundColor"] = json!("#ff0000");
            }
            let mut clean = original.clone();
            clean["elements"][1][field] = default.clone();
            let mut missing = original.clone();
            missing["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove(field);
            for format in FORMATS {
                assert_equivalent(&missing, &clean, format, json!([]));
                for value in wrong_types().into_iter().chain(invalid.clone()) {
                    let mut source = original.clone();
                    source["elements"][1][field] = value;
                    let rows = if ignored(kind, field) {
                        json!([])
                    } else {
                        json!([expected(format, field, false)])
                    };
                    assert_equivalent(&source, &clean, format, rows.clone());
                    assert_frame_equivalent(&source, &clean, format, rows);
                }
                for value in &valid {
                    let mut source = original.clone();
                    source["elements"][1][field] = value.clone();
                    let result = Renderer::new()
                        .render(
                            &parse(&source),
                            &RenderRequest::builder(format).build().unwrap(),
                        )
                        .unwrap();
                    assert!(
                        result.report().is_empty(),
                        "{kind} {field}: {:?}",
                        result.report()
                    );
                    if ignored(kind, field) {
                        assert_equivalent(&source, &clean, format, json!([]));
                    }
                    // All admitted variants must preserve their actual source.
                    assert_equivalent(&source, &source, format, json!([]));
                }
            }
        }
    }
}

#[test]
fn color_fields_have_canonical_values_recovery_and_request_precedence() {
    background_preservation_and_global_frame_reports();
    super::whole_scene_controls::chromium_css_colors_are_canonicalized_for_every_projection();
    super::whole_scene_controls::canonical_color_alpha_is_preserved_by_every_projection();
    super::whole_scene_controls::dark_mode_lowers_scene_colors_before_every_projection();
    for (field, default) in [
        ("strokeColor", "#1e1e1e"),
        ("backgroundColor", "transparent"),
    ] {
        for kind in KINDS {
            let original = scene(kind);
            let mut clean = original.clone();
            clean["elements"][1][field] = json!(default);
            let mut missing = original.clone();
            missing["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove(field);
            for format in FORMATS {
                assert_equivalent(&missing, &clean, format, json!([]));
                for value in wrong_types().into_iter().chain([
                    json!(0),
                    json!("rgb(broken)"),
                    json!("url(https://invalid)"),
                ]) {
                    let mut source = original.clone();
                    source["elements"][1][field] = value;
                    let rows = if ignored(kind, field) {
                        json!([])
                    } else {
                        json!([expected(format, field, true)])
                    };
                    assert_equivalent(&source, &clean, format, rows.clone());
                    assert_frame_equivalent(&source, &clean, format, rows);
                }
                for (color, canonical) in [
                    ("red", "#ff0000"),
                    ("ReBeccAPurple", "#663399"),
                    ("rgb(10 20 30 / 50%)", "#0a141e80"),
                    ("hsl(120 100% 50%)", "#00ff00"),
                    ("#abc", "#aabbcc"),
                ] {
                    let mut source = original.clone();
                    source["elements"][1][field] = json!(color);
                    let mut clean = source.clone();
                    clean["elements"][1][field] = json!(canonical);
                    assert_equivalent(&source, &clean, format, json!([]));
                    assert_frame_equivalent(&source, &clean, format, json!([]));
                }
            }
        }
    }
    for format in FORMATS {
        for background in [true, false] {
            for mode in [ColorMode::Light, ColorMode::Dark] {
                let request = RenderRequest::builder(format)
                    .include_background(background)
                    .color_mode(mode)
                    .build()
                    .unwrap();
                for value in std::iter::once(None).chain(
                    wrong_types()
                        .into_iter()
                        .chain([
                            json!(0),
                            json!("red"),
                            json!("#ffffff"),
                            json!("transparent"),
                        ])
                        .map(Some),
                ) {
                    let mut source = scene("rectangle");
                    let mut clean = source.clone();
                    clean["appState"]["viewBackgroundColor"] = json!("#ffffff");
                    let valid = value.as_ref().is_none_or(|v| {
                        matches!(v.as_str(), Some("red" | "#ffffff" | "transparent"))
                    });
                    if let Some(value) = &value {
                        source["appState"]["viewBackgroundColor"] = value.clone();
                    }
                    if valid && value.is_some() {
                        clean["appState"]["viewBackgroundColor"] = value.clone().unwrap();
                    }
                    let result = Renderer::new().render(&parse(&source), &request).unwrap();
                    let baseline = Renderer::new().render(&parse(&clean), &request).unwrap();
                    assert_eq!(result.artifact().bytes(), baseline.artifact().bytes());
                    let mut row = expected(format, "", true);
                    row["element_id"] = Value::Null;
                    row["json_pointer"] = json!("/appState/viewBackgroundColor");
                    assert_report(&result, if valid { json!([]) } else { json!([row]) });
                }
            }
        }
    }
}

fn background_preservation_and_global_frame_reports() {
    for empty in [true, false] {
        let mut source = scene("rectangle");
        if empty {
            source["elements"] = json!([]);
        }
        source["elements"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"outer","type":"frame","width":80,"height":60,"updated":0}));
        source["appState"]["viewBackgroundColor"] = json!({"invalid":"color"});
        let mut clean = source.clone();
        clean["appState"]["viewBackgroundColor"] = json!("#ffffff");
        for format in FORMATS {
            let mut row = expected(format, "", true);
            row["element_id"] = Value::Null;
            row["json_pointer"] = json!("/appState/viewBackgroundColor");
            assert_equivalent(&source, &clean, format, json!([row.clone()]));
            let document = parse(&source);
            let clean_document = parse(&clean);
            let request = FrameRenderRequest::builder(format).build().unwrap();
            let frame = document.frames().last().unwrap();
            let renderer = Renderer::new();
            let direct = renderer.render_frame(frame, &request).unwrap();
            let baseline = renderer
                .render_frame(clean_document.frames().last().unwrap(), &request)
                .unwrap();
            assert_eq!(direct.artifact().bytes(), baseline.artifact().bytes());
            assert_report(&direct, json!([row]));
            let replay = renderer
                .render_frames(&document, [frame], &request)
                .unwrap()
                .next()
                .unwrap()
                .into_result()
                .unwrap();
            assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
            assert_eq!(direct.report(), replay.report());
        }
    }
}

#[test]
fn roundness_and_legacy_sharpness_have_narrow_recovery_and_precedence() {
    explicit_adaptive_radius_changes_corners_and_clamps_to_proportional_radius();
    super::rough_style_render::patterned_curved_boundaries_match_pinned_js_sampling();
    for kind in KINDS {
        let original = scene(kind);
        let mut square = original.clone();
        square["elements"][1]["roundness"] = Value::Null;
        for format in FORMATS {
            assert_equivalent(&original, &square, format, json!([]));
            for value in wrong_types()
                .into_iter()
                .filter(|v| !v.is_null() && !v.is_object())
                .chain([json!(0), json!(3)])
            {
                let mut source = original.clone();
                source["elements"][1]["roundness"] = value;
                let rows = if ignored(kind, "roundness") {
                    json!([])
                } else {
                    json!([expected(format, "roundness", false)])
                };
                assert_equivalent(&source, &square, format, rows.clone());
                assert_frame_equivalent(&source, &square, format, rows);
            }
            for field in ["type", "value"] {
                let invalid: Vec<_> = wrong_types()
                    .into_iter()
                    .chain(if field == "type" {
                        vec![json!(0), json!(4), json!(1.5)]
                    } else {
                        vec![json!(-1)]
                    })
                    .collect();
                for value in invalid {
                    let mut source = original.clone();
                    source["elements"][1]["roundness"] = json!({"type":3,"value":10});
                    source["elements"][1]["roundness"][field] = value;
                    let rows = if ignored(kind, "roundness") {
                        json!([])
                    } else {
                        json!([expected(format, &format!("roundness/{field}"), false)])
                    };
                    assert_equivalent(&source, &square, format, rows.clone());
                    assert_frame_equivalent(&source, &square, format, rows);
                }
            }
            for (value, twin) in [
                (json!({}), Value::Null),
                (json!({"value":10}), Value::Null),
                (json!({"type":1}), json!({"type":2})),
                (json!({"type":2,"value":0}), json!({"type":1})),
                (json!({"type":3}), json!({"type":3,"value":32})),
                (json!({"type":3,"value":0}), json!({"type":3,"value":0})),
            ] {
                let mut source = original.clone();
                source["elements"][1]["roundness"] = value;
                let mut clean = original.clone();
                clean["elements"][1]["roundness"] = twin;
                assert_equivalent(&source, &clean, format, json!([]));
                assert_frame_equivalent(&source, &clean, format, json!([]));
            }
            for value in wrong_types().into_iter().chain([
                json!(0),
                json!("round"),
                json!("sharp"),
                json!("future"),
            ]) {
                let mut source = original.clone();
                source["elements"][1]["strokeSharpness"] = value.clone();
                let mut clean = square.clone();
                if value == "round" && !ignored(kind, "roundness") {
                    clean["elements"][1]["roundness"] = json!({"type":2});
                }
                let rows = if value.is_string() || *kind == "selection" {
                    json!([])
                } else {
                    json!([expected(format, "strokeSharpness", false)])
                };
                assert_equivalent(&source, &clean, format, rows.clone());
                assert_frame_equivalent(&source, &clean, format, rows);
            }
            let mut source = original.clone();
            source["elements"][1]["strokeSharpness"] = json!("round");
            source["elements"][1]["roundness"] = json!({"type":3,"value":5});
            let mut clean = source.clone();
            clean["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove("strokeSharpness");
            assert_equivalent(&source, &clean, format, json!([]));
        }
    }
}

fn explicit_adaptive_radius_changes_corners_and_clamps_to_proportional_radius() {
    for kind in ["rectangle", "diamond", "image", "iframe", "embeddable"] {
        let mut small = scene(kind);
        small["elements"][1]["roundness"] = json!({"type":3,"value":1});
        let mut large = small.clone();
        large["elements"][1]["roundness"] = json!({"type":3,"value":1000});
        let mut proportional = small.clone();
        proportional["elements"][1]["roundness"] = json!({"type":2});
        for format in FORMATS {
            let request = RenderRequest::builder(format).build().unwrap();
            let small_artifact = Renderer::new().render(&parse(&small), &request).unwrap();
            let large_artifact = Renderer::new().render(&parse(&large), &request).unwrap();
            assert!(small_artifact.report().is_empty());
            assert!(large_artifact.report().is_empty());
            assert_ne!(
                small_artifact.artifact().bytes(),
                large_artifact.artifact().bytes(),
                "{kind} {format:?} explicit radius must participate"
            );
            assert_equivalent(&large, &proportional, format, json!([]));
            assert_frame_equivalent(&large, &proportional, format, json!([]));
        }
    }
}

#[test]
fn rough_seed_defaults_zero_repairs_and_invalid_values_are_target_stable() {
    // Independent hashlib evaluation of the frozen revision/identity transcript
    // for the literal identity "paint"; never captured from renderer output.
    const PAINT_SEED: i32 = 438_155_844;
    for kind in KINDS {
        let mut original = scene(kind);
        if !ignored(kind, "backgroundColor") {
            original["elements"][1]["backgroundColor"] = json!("red");
            original["elements"][1]["fillStyle"] = json!("hachure");
        }
        for format in FORMATS {
            let mut clean = original.clone();
            clean["elements"][1]["seed"] = json!(1);
            let mut missing = original.clone();
            missing["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove("seed");
            assert_equivalent(&missing, &clean, format, json!([]));
            for value in wrong_types().into_iter().chain([json!(1.5)]) {
                let mut source = original.clone();
                source["elements"][1]["seed"] = value;
                let rows = if ignored(kind, "seed") {
                    json!([])
                } else {
                    json!([expected(format, "seed", false)])
                };
                assert_equivalent(&source, &clean, format, rows.clone());
                assert_frame_equivalent(&source, &clean, format, rows);
            }
            let mut zero = original.clone();
            zero["elements"][1]["seed"] = json!(0);
            let mut repaired = original.clone();
            repaired["elements"][1]["seed"] = json!(PAINT_SEED);
            let mut row = expected(format, "seed", false);
            row["code"] = json!("normalization.zero-rough-seed");
            row["details"]["recovery"] = json!("repaired");
            let rows = if ignored(kind, "seed") {
                json!([])
            } else {
                json!([row])
            };
            assert_equivalent(&zero, &repaired, format, rows.clone());
            assert_frame_equivalent(&zero, &repaired, format, rows);
            for seed in [-2147483647, -1, 1, 2147483647] {
                let mut source = original.clone();
                source["elements"][1]["seed"] = json!(seed);
                assert_equivalent(&source, &source, format, json!([]));
            }
        }
    }
}
