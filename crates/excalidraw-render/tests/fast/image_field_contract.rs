//! Image field ownership at the public facade; authored field expectations are
//! independent of the renderer's normalization and diagnostic registries.
use base64::Engine;
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    SceneDocument, SupplementalAssets,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
pub(super) fn unsupported_value_contract() {
    crop_members_recover_at_narrow_paths_with_scene_or_supplemental_authority();
    image_scale_and_status_default_recover_and_preserve_valid_payloads();
    file_id_and_status_cover_missing_invalid_and_authoritative_payloads();
    file_keys_and_record_ids_preserve_map_authority_and_repair_only_bad_ids();
    payload_and_mime_fields_bind_validation_recovery_and_original_bytes();
}
const KEY: &str = "a/b~c";
fn png() -> Vec<u8> {
    crate::common::codec_fixtures::png_rgba(
        2,
        2,
        &[
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ],
    )
}
fn scene() -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
        {"id":"frame","type":"frame","width":80,"height":60,"updated":0},
        {"id":"image","type":"image","x":5,"y":5,"width":20,"height":20,"frameId":"frame","fileId":KEY,"updated":0},
        {"id":"other","type":"magicframe","x":200,"width":80,"height":60,"updated":0}
    ],"appState":{},"files":{KEY:{"id":KEY,"created":0,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png()))}}})
}
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn wrong() -> Vec<Value> {
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
        json!({"unknown":[1]}),
    ]
}

fn crop_has_independent_pixel_and_vector_semantics() {
    super::image_viewport_rounding::a_flipped_fractional_image_mirrors_its_rounded_viewport_about_the_unrounded_center();
    super::image_viewport_rounding::a_cropped_flipped_fractional_image_keeps_the_reference_mask_viewport_and_unrounded_content();
    let mut source = scene();
    source["elements"] = json!([source["elements"][1].clone()]);
    source["elements"][0]
        .as_object_mut()
        .unwrap()
        .remove("frameId");
    source["elements"][0]["crop"] =
        json!({"x":1,"y":0,"width":1,"height":1,"naturalWidth":2,"naturalHeight":2});
    let request = RenderRequest::builder(OutputFormat::Png)
        .padding(0.0)
        .build()
        .unwrap();
    let result = Renderer::new().render(&parse(&source), &request).unwrap();
    let pixels = crate::common::raster::decode_png(result.artifact().bytes());
    assert_eq!((pixels.width, pixels.height), (20, 20));
    assert_eq!(pixels.pixel(15, 5), [0, 255, 0, 255]);
    for format in FORMATS {
        let result = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).build().unwrap(),
            )
            .unwrap();
        assert_report(&result, format, json!([]));
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
            assert!(svg.contains("width=\"40\" height=\"40\""));
            assert!(svg.contains("overflow=\"hidden\""));
        }
        if format == OutputFormat::Pdf {
            let pdf = crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
            assert!(pdf.contains("/Subtype /Image"));
            assert!(pdf.contains("/Im0 Do"));
        }
    }
    // Nonzero X and Y offsets: source 2x2 cropped to its lower-right texel.
    source["elements"][0]["crop"]["y"] = json!(1);
    source["elements"][0]["scale"] = json!([-1, -1]);
    for format in [OutputFormat::Svg, OutputFormat::Pdf] {
        let result = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).build().unwrap(),
            )
            .unwrap();
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
            assert!(svg.contains("translate(20 20) scale(-1 -1)"));
            assert!(svg.contains(
                "<svg x=\"-20\" y=\"-20\" width=\"40\" height=\"40\" overflow=\"hidden\">"
            ));
        } else {
            let pdf = crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
            assert!(pdf.contains("-40 0 0 -40 40 40 cm\n/Im0 Do"));
        }
    }
}
fn property(pointer: &str, owner: Value, recovery: &str, effects: Value) -> Value {
    json!({"code":"compatibility.unsupported-property-value","severity":"warning","effects":effects,"element_id":owner,"json_pointer":pointer,"details":{"kind":"compatibility","recovery":recovery,"recipe_id":null}})
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
    assert_eq!(actual, json!({"schema_version":1,"diagnostics":expected}));
}
fn assert_contract(
    source: &Value,
    clean: &Value,
    format: OutputFormat,
    rows: Value,
    supplemental: bool,
) {
    let assets = if supplemental {
        SupplementalAssets::builder()
            .asset(KEY, png())
            .build()
            .unwrap()
    } else {
        SupplementalAssets::default()
    };
    let document = parse(source);
    let clean_document = parse(clean);
    let request = RenderRequest::builder(format)
        .supplemental_assets(assets.clone())
        .build()
        .unwrap();
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
        .supplemental_assets(assets.clone())
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let embedded = Renderer::new().render(&document, &metadata).unwrap();
    assert_report(&embedded, format, rows.clone());
    assert_eq!(
        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
    let request = FrameRenderRequest::builder(format)
        .supplemental_assets(assets)
        .build()
        .unwrap();
    let renderer = Renderer::new();
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
fn crop_members_recover_at_narrow_paths_with_scene_or_supplemental_authority() {
    crop_has_independent_pixel_and_vector_semantics();
    for supplemental in [false, true] {
        let mut original = scene();
        if supplemental {
            original["files"] = json!({});
        }
        let clean = original.clone();
        for field in ["x", "y", "width", "height", "naturalWidth", "naturalHeight"] {
            let mut invalid = wrong();
            if matches!(field, "x" | "y") {
                invalid.retain(|v| v != &json!(0) && v != &json!(0.5));
            } else {
                invalid.retain(|v| v != &json!(0.5));
            }
            for value in std::iter::once(None).chain(invalid.into_iter().map(Some)) {
                let mut source = original.clone();
                source["elements"][1]["crop"] =
                    json!({"x":0,"y":0,"width":1,"height":1,"naturalWidth":2,"naturalHeight":2});
                if let Some(value) = value {
                    source["elements"][1]["crop"][field] = value;
                } else {
                    source["elements"][1]["crop"]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                }
                for format in FORMATS {
                    assert_contract(
                        &source,
                        &clean,
                        format,
                        json!([property(
                            &format!("/elements/1/crop/{field}"),
                            json!("image"),
                            "omitted",
                            json!(["visual"])
                        )]),
                        supplemental,
                    );
                }
            }
        }
        for value in wrong()
            .into_iter()
            .filter(|v| !v.is_null() && !v.is_object())
        {
            let mut source = original.clone();
            source["elements"][1]["crop"] = value;
            for format in FORMATS {
                assert_contract(
                    &source,
                    &clean,
                    format,
                    json!([property(
                        "/elements/1/crop",
                        json!("image"),
                        "substituted",
                        json!(["input_repair", "visual"])
                    )]),
                    supplemental,
                );
            }
        }
        for format in FORMATS {
            let mut source = original.clone();
            source["elements"][1]["crop"] = Value::Null;
            assert_contract(&source, &clean, format, json!([]), supplemental);
            source["elements"][1]["crop"] =
                json!({"x":0.5,"y":0.5,"width":1,"height":1,"naturalWidth":2,"naturalHeight":2});
            let mut scaled = source.clone();
            scaled["elements"][1]["crop"] =
                json!({"x":1,"y":1,"width":2,"height":2,"naturalWidth":4,"naturalHeight":4});
            assert_contract(&source, &scaled, format, json!([]), supplemental);
        }
    }
}

#[test]
fn image_scale_and_status_default_recover_and_preserve_valid_payloads() {
    crop_has_independent_pixel_and_vector_semantics();
    for supplemental in [false, true] {
        let mut original = scene();
        if supplemental {
            original["files"] = json!({});
        }
        for (field, default) in [("scale", json!([1, 1])), ("status", json!("pending"))] {
            let mut clean = original.clone();
            clean["elements"][1][field] = default;
            for format in FORMATS {
                assert_contract(&original, &clean, format, json!([]), supplemental);
                let mut invalid = wrong();
                if field == "scale" {
                    invalid.extend([
                        json!([0, 1]),
                        json!([1, 0]),
                        json!([-1, 2]),
                        json!([1, 1, 1]),
                        json!([1, "bad"]),
                    ]);
                }
                for value in invalid {
                    let mut source = original.clone();
                    source["elements"][1][field] = value;
                    assert_contract(
                        &source,
                        &clean,
                        format,
                        json!([property(
                            &format!("/elements/1/{field}"),
                            json!("image"),
                            "substituted",
                            json!(["input_repair", "visual"])
                        )]),
                        supplemental,
                    );
                }
                if field == "status" {
                    for status in ["pending", "saved", "error"] {
                        let mut source = original.clone();
                        source["elements"][1][field] = json!(status);
                        assert_contract(&source, &clean, format, json!([]), supplemental);
                    }
                }
            }
        }
    }
    // Four independently known pixel arrangements establish both flip axes.
    for (scale, pixels) in [
        (
            [1, 1],
            [
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 0, 255],
            ],
        ),
        (
            [-1, 1],
            [
                [0, 255, 0, 255],
                [255, 0, 0, 255],
                [255, 255, 0, 255],
                [0, 0, 255, 255],
            ],
        ),
        (
            [1, -1],
            [
                [0, 0, 255, 255],
                [255, 255, 0, 255],
                [255, 0, 0, 255],
                [0, 255, 0, 255],
            ],
        ),
        (
            [-1, -1],
            [
                [255, 255, 0, 255],
                [0, 0, 255, 255],
                [0, 255, 0, 255],
                [255, 0, 0, 255],
            ],
        ),
    ] {
        let mut source = scene();
        source["elements"][1]["scale"] = json!(scale);
        for format in FORMATS {
            assert_contract(&source, &source, format, json!([]), false);
        }
        let mut pixel_source = source.clone();
        pixel_source["elements"] = json!([source["elements"][1].clone()]);
        pixel_source["elements"][0]
            .as_object_mut()
            .unwrap()
            .remove("frameId");
        let result = Renderer::new()
            .render(
                &parse(&pixel_source),
                &RenderRequest::builder(OutputFormat::Png)
                    .padding(0.0)
                    .build()
                    .unwrap(),
            )
            .unwrap();
        let decoded = crate::common::raster::decode_png(result.artifact().bytes());
        assert_eq!((decoded.width, decoded.height), (20, 20));
        for ((x, y), expected) in [(2, 2), (17, 2), (2, 17), (17, 17)].into_iter().zip(pixels) {
            assert_eq!(decoded.pixel(x, y), expected);
        }
    }
}

fn missing(format: OutputFormat, id: &str, status: &str) -> Value {
    json!({"code":"image.missing-file","severity":"warning","effects":["visual"],"element_id":"image","json_pointer":"/elements/1/fileId",
        "details":{"kind":"asset","asset_id":id,"claimed_media_type":null,"validated_media_type":null,
            "recovery":if format==OutputFormat::Png{"placeholder"}else{"omitted"},
            "recipe_id":if format!=OutputFormat::Png{Value::Null}else if status=="error"{json!("official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9")}else{json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")}}})
}

#[test]
fn file_id_and_status_cover_missing_invalid_and_authoritative_payloads() {
    super::diagnostic_contract::missing_image_contract_in_every_format();
    super::diagnostic_contract::supplement_conflicts_report_authority_once_in_every_format();
    for status in ["pending", "saved", "error"] {
        for format in FORMATS {
            let mut original = scene();
            original["elements"][1]["status"] = json!(status);
            for value in std::iter::once(None).chain(
                wrong()
                    .into_iter()
                    .chain([json!(KEY), json!("absent")])
                    .map(Some),
            ) {
                let mut source = original.clone();
                if let Some(value) = &value {
                    source["elements"][1]["fileId"] = value.clone();
                } else {
                    source["elements"][1]
                        .as_object_mut()
                        .unwrap()
                        .remove("fileId");
                }
                let mut clean = source.clone();
                let rows = match &value {
                    Some(value) if !value.is_null() && !value.is_string() => {
                        clean["elements"][1]["fileId"] = Value::Null;
                        json!([property(
                            "/elements/1/fileId",
                            json!("image"),
                            "substituted",
                            json!(["input_repair", "visual"])
                        )])
                    }
                    Some(value) if value == KEY => json!([]),
                    _ => json!([missing(
                        format,
                        value.as_ref().and_then(Value::as_str).unwrap_or(""),
                        status
                    )]),
                };
                assert_contract(&source, &clean, format, rows, false);
            }
            let mut unavailable = original.clone();
            unavailable["files"] = json!({});
            unavailable["elements"][1]["crop"] = json!({"x":"bad"});
            let mut no_crop = unavailable.clone();
            no_crop["elements"][1]
                .as_object_mut()
                .unwrap()
                .remove("crop");
            assert_contract(
                &unavailable,
                &no_crop,
                format,
                json!([missing(format, KEY, status)]),
                false,
            );
            for value in wrong() {
                let mut source = no_crop.clone();
                source["elements"][1]["status"] = value;
                let mut clean = source.clone();
                clean["elements"][1]["status"] = json!("pending");
                assert_contract(
                    &source,
                    &clean,
                    format,
                    json!([
                        missing(format, KEY, "pending"),
                        property(
                            "/elements/1/status",
                            json!("image"),
                            "substituted",
                            json!(["input_repair", "visual"])
                        )
                    ]),
                    false,
                );
            }
        }
    }
}

#[test]
fn file_keys_and_record_ids_preserve_map_authority_and_repair_only_bad_ids() {
    for format in FORMATS {
        let original = scene();
        for value in std::iter::once(None).chain(
            wrong()
                .into_iter()
                .chain([json!(KEY), json!("different"), json!("💠")])
                .map(Some),
        ) {
            let mut source = original.clone();
            if let Some(value) = &value {
                source["files"][KEY]["id"] = value.clone();
            } else {
                source["files"][KEY].as_object_mut().unwrap().remove("id");
            }
            let valid = value
                .as_ref()
                .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
            assert_contract(
                &source,
                &original,
                format,
                if valid {
                    json!([])
                } else {
                    json!([property(
                        "/files/a~1b~0c/id",
                        Value::Null,
                        "repaired",
                        json!(["input_repair"])
                    )])
                },
                false,
            );
        }
        let mut empty = original.clone();
        empty["files"][""] = empty["files"][KEY].clone();
        empty["files"].as_object_mut().unwrap().remove(KEY);
        empty["elements"][1]["fileId"] = json!("");
        let mut absent = empty.clone();
        absent["files"] = json!({});
        let mut row = missing(format, "", "pending");
        row["code"] = json!("asset.malformed-file-record");
        row["element_id"] = Value::Null;
        row["json_pointer"] = json!("/files/");
        assert_contract(&empty, &absent, format, json!([row]), false);
    }
}

#[test]
fn payload_and_mime_fields_bind_validation_recovery_and_original_bytes() {
    super::diagnostic_contract::media_type_contract_in_every_format();
    super::diagnostic_contract::malformed_file_contract_in_every_format();
    super::diagnostic_contract::unsupported_image_contract_in_every_format();
    super::diagnostic_contract::decode_failures_cover_every_codec_and_authority();
    super::diagnostic_contract::forbidden_resource_reports_bounds_selected_recovery();
    for format in FORMATS {
        let original = scene();
        for value in std::iter::once(None).chain(
            wrong()
                .into_iter()
                .chain([
                    json!("image/png"),
                    json!("image/jpeg"),
                    json!("application/octet-stream"),
                    json!("IMAGE/PNG"),
                ])
                .map(Some),
        ) {
            let mut source = original.clone();
            if let Some(value) = &value {
                source["files"][KEY]["mimeType"] = value.clone();
            }
            let rows = if value.is_none() || value == Some(json!("image/png")) {
                json!([])
            } else {
                json!([{"code":"asset.media-type-mismatch","severity":"warning","effects":["input_repair"],"element_id":null,"json_pointer":"/files/a~1b~0c/mimeType",
                    "details":{"kind":"asset","asset_id":KEY,"claimed_media_type":value.as_ref().and_then(Value::as_str),"validated_media_type":"image/png","recovery":"repaired","recipe_id":null}}])
            };
            assert_contract(&source, &original, format, rows, false);
        }
        for value in
            std::iter::once(None).chain(wrong().into_iter().filter(|v| !v.is_string()).map(Some))
        {
            let mut source = original.clone();
            let absent = value.is_none();
            if let Some(value) = value {
                source["files"][KEY]["dataURL"] = value;
            } else {
                source["files"][KEY]
                    .as_object_mut()
                    .unwrap()
                    .remove("dataURL");
            }
            let mut clean = original.clone();
            clean["files"][KEY]
                .as_object_mut()
                .unwrap()
                .remove("dataURL");
            let row = if absent {
                let mut row = missing(format, KEY, "pending");
                row["code"] = json!("asset.malformed-file-record");
                row["element_id"] = Value::Null;
                row["json_pointer"] = json!("/files/a~1b~0c/dataURL");
                row
            } else {
                property(
                    "/files/a~1b~0c/dataURL",
                    Value::Null,
                    if format == OutputFormat::Png {
                        "placeholder"
                    } else {
                        "omitted"
                    },
                    json!(["visual"]),
                )
            };
            assert_contract(&source, &clean, format, json!([row]), false);
        }
        // Revision-1 admission requires the canonical base64 data-URL envelope.
        // A non-base64 body remains a malformed payload, even with PNG octets.
        let bytes = png();
        let mut source = original.clone();
        source["files"][KEY]["dataURL"] = json!(format!(
            "data:image/png,{}",
            bytes
                .iter()
                .map(|b| format!("%{b:02X}"))
                .collect::<String>()
        ));
        let mut clean = original.clone();
        clean["files"][KEY]
            .as_object_mut()
            .unwrap()
            .remove("dataURL");
        let mut row = missing(format, KEY, "pending");
        row["code"] = json!("asset.malformed-file-record");
        row["json_pointer"] = json!("/files/a~1b~0c/dataURL");
        assert_contract(&source, &clean, format, json!([row]), false);
    }
}
