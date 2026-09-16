//! Frame surface contracts through scene and frame public requests.
use excalidraw_render::{
    FrameOptions, FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest,
    Renderer, SceneDocument,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
pub(super) fn unsupported_value_contract() {
    frame_rendering_container_and_members_have_global_exact_recovery();
    frame_names_validate_only_when_painted_and_preserve_source_metadata();
}
fn scene(kind: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"frame","type":kind,"x":0,"y":0,"width":80,"height":60,"name":"Board","updated":0},
    {"id":"paint","type":"rectangle","x":60,"y":20,"width":40,"height":20,"frameId":"frame","strokeColor":"red","backgroundColor":"red","roughness":0,"seed":1,"updated":0},
    {"id":"other","type":"frame","x":200,"width":80,"height":60,"updated":0}],"appState":{},"files":{}})
}
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn wrong() -> Vec<Value> {
    vec![
        Value::Null,
        json!(0),
        json!(-1),
        json!(0.5),
        json!(""),
        json!("false"),
        json!([]),
        json!([true]),
        json!({}),
        json!({"unknown":true}),
    ]
}
fn repair(pointer: &str, owner: Value) -> Value {
    json!({"code":"compatibility.unsupported-property-value","severity":"warning","effects":["input_repair","visual"],"element_id":owner,"json_pointer":pointer,"details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}})
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
fn assert_scene(
    source: &Value,
    clean: &Value,
    format: OutputFormat,
    options: FrameOptions,
    rows: Value,
) {
    let document = parse(source);
    let request = RenderRequest::builder(format)
        .frame_options(options.clone())
        .build()
        .unwrap();
    let first = Renderer::new().render(&document, &request).unwrap();
    let baseline = Renderer::new().render(&parse(clean), &request).unwrap();
    let second = Renderer::new().render(&document, &request).unwrap();
    assert_eq!(first.artifact().bytes(), baseline.artifact().bytes());
    assert_report(&first, format, rows.clone());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let metadata = RenderRequest::builder(format)
        .frame_options(options)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let result = Renderer::new().render(&document, &metadata).unwrap();
    assert_report(&result, format, rows);
    assert_eq!(
        super::artifact_metadata::extract(format, result.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
}
fn assert_frames(source: &Value, clean: &Value, format: OutputFormat, rows: Value) {
    let document = parse(source);
    let baseline = parse(clean);
    let renderer = Renderer::new();
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let frames: Vec<_> = document.frames().collect();
    let clean_frames: Vec<_> = baseline.frames().collect();
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for (frame, clean_frame) in frames.iter().zip(clean_frames) {
        let direct = renderer.render_frame(*frame, &request).unwrap();
        let clean = renderer.render_frame(clean_frame, &request).unwrap();
        let entry = batch.next().unwrap();
        assert_eq!(direct.artifact().bytes(), clean.artifact().bytes());
        assert_eq!(
            direct.artifact().bytes(),
            entry.result().as_ref().unwrap().artifact().bytes()
        );
        assert_eq!(direct.report(), entry.result().as_ref().unwrap().report());
        assert_report(&direct, format, rows.clone());
        let metadata = FrameRenderRequest::builder(format)
            .metadata_options(MetadataOptions::new(true, false, false))
            .build()
            .unwrap();
        let embedded = renderer.render_frame(*frame, &metadata).unwrap();
        assert_report(&embedded, format, rows.clone());
        assert_eq!(
            super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
            serde_json::to_vec(source).unwrap()
        );
        let replay = renderer
            .render_frames(&document, [*frame], &metadata)
            .unwrap()
            .next()
            .unwrap()
            .into_result()
            .unwrap();
        assert_eq!(embedded.artifact().bytes(), replay.artifact().bytes());
        assert_eq!(embedded.report(), replay.report());
    }
    assert!(batch.next().is_none());
}

#[test]
fn frame_rendering_container_and_members_have_global_exact_recovery() {
    frame_controls_combine_with_requests_and_frame_exports_force_their_scope();
    for format in FORMATS {
        let mut empty = scene("frame");
        empty["elements"] = json!([]);
        let clean = empty.clone();
        empty["appState"]["frameRendering"] =
            json!({"enabled":false,"clip":"bad","name":true,"outline":true});
        assert_scene(
            &empty,
            &clean,
            format,
            FrameOptions::new(false, false, false, false),
            json!([repair("/appState/frameRendering/clip", Value::Null)]),
        );
    }
    for kind in ["frame", "magicframe"] {
        let original = scene(kind);
        let mut clean = original.clone();
        clean["appState"]["frameRendering"] =
            json!({"enabled":true,"clip":true,"name":true,"outline":true});
        for format in FORMATS {
            assert_scene(
                &original,
                &clean,
                format,
                FrameOptions::default(),
                json!([]),
            );
            for value in wrong()
                .into_iter()
                .filter(|v| !v.is_object())
                .chain([json!(false), json!(true)])
            {
                let mut source = original.clone();
                source["appState"]["frameRendering"] = value;
                let rows = json!([repair("/appState/frameRendering", Value::Null)]);
                assert_scene(
                    &source,
                    &clean,
                    format,
                    FrameOptions::default(),
                    rows.clone(),
                );
                assert_frames(&source, &clean, format, rows);
            }
            for field in ["enabled", "clip", "name", "outline"] {
                let mut absent = clean.clone();
                absent["appState"]["frameRendering"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                assert_scene(&absent, &clean, format, FrameOptions::default(), json!([]));
                for value in wrong() {
                    let mut source = clean.clone();
                    source["appState"]["frameRendering"][field] = value;
                    let rows = json!([repair(
                        &format!("/appState/frameRendering/{field}"),
                        Value::Null
                    )]);
                    assert_scene(
                        &source,
                        &clean,
                        format,
                        FrameOptions::default(),
                        rows.clone(),
                    );
                    assert_frames(&source, &clean, format, rows);
                }
            }
            let mut unknown = clean.clone();
            unknown["appState"]["frameRendering"]["future/~"] = json!({"child":true});
            let row = json!({"code":"compatibility.unsupported-property","severity":"warning","effects":["visual","semantic","metadata"],"element_id":null,"json_pointer":"/appState/frameRendering/future~1~0","details":{"kind":"compatibility","recovery":"ignored","recipe_id":null}});
            assert_scene(
                &unknown,
                &clean,
                format,
                FrameOptions::default(),
                json!([row.clone()]),
            );
            assert_frames(&unknown, &clean, format, json!([row]));
        }
    }
}

fn frame_controls_combine_with_requests_and_frame_exports_force_their_scope() {
    super::frame_render::nested_frames_clip_direct_geometry_in_every_format();
    super::frame_render::frame_clipping_uses_the_rounded_frame_boundary();
    for kind in ["frame", "magicframe"] {
        let original = scene(kind);
        for mask in 0..16 {
            let flags = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
            let mut source = original.clone();
            source["appState"]["frameRendering"] =
                json!({"enabled":flags[0],"clip":flags[1],"name":flags[2],"outline":flags[3]});
            for format in FORMATS {
                // Persisted false gates the corresponding request control;
                // request false can independently disable persisted true.
                let request = RenderRequest::builder(format)
                    .frame_options(FrameOptions::new(flags[0], flags[1], flags[2], flags[3]))
                    .build()
                    .unwrap();
                let explicit = Renderer::new().render(&parse(&original), &request).unwrap();
                let persisted = Renderer::new()
                    .render(
                        &parse(&source),
                        &RenderRequest::builder(format).build().unwrap(),
                    )
                    .unwrap();
                assert_eq!(explicit.artifact().bytes(), persisted.artifact().bytes());
                assert_report(&persisted, format, json!([]));
                assert_frames(&source, &original, format, json!([]));
                for request_flags in [
                    [true, false, true, false],
                    [true, true, false, true],
                    [false, true, true, true],
                ] {
                    let mixed = RenderRequest::builder(format)
                        .frame_options(FrameOptions::new(
                            request_flags[0],
                            request_flags[1],
                            request_flags[2],
                            request_flags[3],
                        ))
                        .build()
                        .unwrap();
                    let combined = RenderRequest::builder(format)
                        .frame_options(FrameOptions::new(
                            flags[0] && request_flags[0],
                            flags[1] && request_flags[1],
                            flags[2] && request_flags[2],
                            flags[3] && request_flags[3],
                        ))
                        .build()
                        .unwrap();
                    let mixed_result = Renderer::new().render(&parse(&source), &mixed).unwrap();
                    let combined_result = Renderer::new()
                        .render(&parse(&original), &combined)
                        .unwrap();
                    assert_eq!(
                        mixed_result.artifact().bytes(),
                        combined_result.artifact().bytes()
                    );
                    assert_report(&mixed_result, format, json!([]));
                }
                if format == OutputFormat::Svg {
                    let svg = std::str::from_utf8(persisted.artifact().bytes()).unwrap();
                    assert_eq!(
                        svg.contains("data-frame-name=\"frame\""),
                        flags[0] && flags[2]
                    );
                    assert_eq!(
                        svg.contains("data-frame-outline=\"frame\""),
                        flags[0] && flags[3]
                    );
                    assert_eq!(svg.contains("<clipPath"), flags[0] && flags[1]);
                }
            }
        }
    }
}

#[test]
fn frame_names_validate_only_when_painted_and_preserve_source_metadata() {
    super::diagnostic_contract::font_diagnostics_cover_text_generated_labels_and_silence();
    super::frame_render::frame_name_is_painted_grey_above_the_frame_in_png();
    for kind in ["frame", "magicframe"] {
        let original = scene(kind);
        let mut clean = original.clone();
        clean["elements"][0]["name"] = Value::Null;
        for format in FORMATS {
            let mut absent = original.clone();
            absent["elements"][0]
                .as_object_mut()
                .unwrap()
                .remove("name");
            assert_scene(&absent, &clean, format, FrameOptions::default(), json!([]));
            for value in wrong()
                .into_iter()
                .filter(|v| !v.is_null() && !v.is_string())
                .chain([json!(false), json!(true)])
            {
                let mut source = original.clone();
                source["elements"][0]["name"] = value;
                assert_scene(
                    &source,
                    &clean,
                    format,
                    FrameOptions::default(),
                    json!([repair("/elements/0/name", json!("frame"))]),
                );
                for options in [
                    FrameOptions::new(false, true, true, true),
                    FrameOptions::new(true, true, false, true),
                ] {
                    assert_scene(&source, &clean, format, options, json!([]));
                }
                assert_frames(&source, &clean, format, json!([]));
                for field in ["enabled", "name"] {
                    let mut disabled = source.clone();
                    disabled["appState"]["frameRendering"] = json!({field:false});
                    let mut disabled_clean = clean.clone();
                    disabled_clean["appState"]["frameRendering"] = json!({field:false});
                    assert_scene(
                        &disabled,
                        &disabled_clean,
                        format,
                        FrameOptions::default(),
                        json!([]),
                    );
                    disabled["elements"][0]["name"] = json!("\u{10ffff}");
                    assert_scene(
                        &disabled,
                        &disabled_clean,
                        format,
                        FrameOptions::default(),
                        json!([]),
                    );
                    assert_frames(&disabled, &disabled_clean, format, json!([]));
                }
                let mut deleted = source.clone();
                deleted["elements"][0]["isDeleted"] = json!(true);
                let mut deleted_clean = clean.clone();
                deleted_clean["elements"][0]["isDeleted"] = json!(true);
                assert_scene(
                    &deleted,
                    &deleted_clean,
                    format,
                    FrameOptions::default(),
                    json!([]),
                );
            }
            for name in [
                Value::Null,
                json!(""),
                json!("Board"),
                json!("A very long frame name that cannot fit inside this small frame"),
            ] {
                let mut source = original.clone();
                source["elements"][0]["name"] = name;
                assert_scene(&source, &source, format, FrameOptions::default(), json!([]));
                assert_frames(&source, &clean, format, json!([]));
                if format == OutputFormat::Svg {
                    let result = Renderer::new()
                        .render(
                            &parse(&source),
                            &RenderRequest::builder(format).build().unwrap(),
                        )
                        .unwrap();
                    let xml = roxmltree::Document::parse(
                        std::str::from_utf8(result.artifact().bytes()).unwrap(),
                    )
                    .unwrap();
                    let label = xml
                        .descendants()
                        .find(|n| n.attribute("data-frame-name") == Some("frame"));
                    if source["elements"][0]["name"] == "Board" {
                        assert!(label
                            .unwrap()
                            .descendants()
                            .filter_map(|n| n.text())
                            .any(|s| s.contains("Board")));
                    }
                    if source["elements"][0]["name"]
                        .as_str()
                        .is_some_and(|s| s.len() > 40)
                    {
                        assert!(label
                            .unwrap()
                            .descendants()
                            .filter_map(|n| n.text())
                            .any(|s| s.contains("...")));
                    }
                }
            }
        }
    }
}
