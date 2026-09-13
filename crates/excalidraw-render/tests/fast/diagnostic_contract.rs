//! Public-facade diagnostic ownership: exact format, severity, effects, details,
//! occurrence order and silence. Expected contracts are authored, not generated
//! from the production diagnostic registry.
use excalidraw_render::{
    FrameOptions, FrameRenderRequest, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

#[test]
fn image_authorities_have_exact_ordered_diagnostics() {
    missing_images_report_referring_owners_and_format_specific_recovery();
    malformed_file_records_report_each_authority_once_and_unreferenced_are_silent();
    unsupported_payloads_report_each_file_once_and_unreferenced_are_silent();
    image_authority_precedence_and_valid_silence();
    for format in FORMATS {
        for status in ["saved", "error"] {
            let mut a = image("a-ref", "a/~");
            let mut z = image("z-ref", "z/~");
            a["status"] = json!(status);
            z["status"] = json!(status);
            let mut again = a.clone();
            again["id"] = json!("again");
            let elements = json!([a, z, again]);
            // File-source z,a differs from first-reference and lexical a,z.
            for (files, code, owner, suffix) in [
                (
                    r#"{"z/~":null,"a/~":null,"unused":null}"#,
                    "asset.malformed-file-record",
                    false,
                    "",
                ),
                (
                    r#"{"z/~":{"id":"z/~","created":1},"a/~":{"id":"a/~","created":1},"unused":{}}"#,
                    "asset.malformed-file-record",
                    false,
                    "/dataURL",
                ),
                (
                    r#"{"z/~":{"id":"z/~","created":1,"mimeType":"image/png","dataURL":"data:image/png;base64,aW52YWxpZA=="},"a/~":{"id":"a/~","created":1,"mimeType":"image/png","dataURL":"data:image/png;base64,aW52YWxpZA=="}}"#,
                    "image.unsupported-format",
                    true,
                    "/dataURL",
                ),
            ] {
                let source = SceneDocument::parse(format!(r#"{{"type":"excalidraw","version":2,"elements":{elements},"appState":{{}},"files":{files}}}"#).as_bytes()).unwrap();
                let rows: Vec<_> = [("z/~", "z-ref", "z~1~0"), ("a/~", "a-ref", "a~1~0")]
                    .into_iter()
                    .map(|(id, element, escaped)| {
                        image_expected(
                            code,
                            if owner { json!(element) } else { Value::Null },
                            &format!("/files/{escaped}{suffix}"),
                            id,
                            status,
                            format,
                        )
                    })
                    .collect();
                assert_report(&source, format, json!(rows));
                let empty = SceneDocument::parse(format!(r#"{{"type":"excalidraw","version":2,"elements":[],"appState":{{}},"files":{files}}}"#).as_bytes()).unwrap();
                assert_report(&empty, format, json!([]));
            }
            let assets = SupplementalAssets::builder()
                .asset_with_media_type("z/~", b"invalid".to_vec(), "image/png")
                .asset("a/~", b"invalid".to_vec())
                .asset("unused", b"invalid".to_vec())
                .build()
                .unwrap();
            let request = RenderRequest::builder(format)
                .supplemental_assets(assets)
                .build()
                .unwrap();
            assert_report_with_request(
                &image_scene(json!([z, a, again]), json!({})),
                &request,
                json!([
                    image_expected(
                        "image.unsupported-format",
                        json!("a-ref"),
                        "",
                        "a/~",
                        status,
                        format
                    ),
                    image_expected(
                        "image.unsupported-format",
                        json!("z-ref"),
                        "",
                        "z/~",
                        status,
                        format
                    )
                ]),
            );
            assert_report_with_request(&scene(json!([])), &request, json!([]));
        }
    }
}

fn image_authority_precedence_and_valid_silence() {
    use base64::Engine;
    let data = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let png = base64::engine::general_purpose::STANDARD
        .decode(data.split_once(',').unwrap().1)
        .unwrap();
    for format in FORMATS {
        for status in ["saved", "error"] {
            let mut element = image("image", "file/~");
            element["status"] = json!(status);
            let assets = SupplementalAssets::builder()
                .asset("file/~", png.clone())
                .build()
                .unwrap();
            let request = RenderRequest::builder(format)
                .supplemental_assets(assets)
                .build()
                .unwrap();
            assert_report_with_request(
                &image_scene(json!([element]), json!({})),
                &request,
                json!([]),
            );
            let valid = image_scene(
                json!([element]),
                json!({"file/~":{"id":"file/~","created":1,"mimeType":"image/png","dataURL":data}}),
            );
            assert_report(&valid, format, json!([]));
            let conflict = expected(
                "asset.supplement-conflict",
                "warning",
                json!(["input_repair"]),
                Value::Null,
                "/files/file~1~0",
                json!({"kind":"asset","asset_id":"file/~","claimed_media_type":null,"validated_media_type":null,"recovery":"ignored","recipe_id":null}),
            );
            assert_report_with_request(&valid, &request, json!([conflict]));
            for (record, code, pointer) in [
                (
                    Value::Null,
                    "asset.malformed-file-record",
                    "/files/file~1~0",
                ),
                (
                    json!({"id":"file/~","created":1}),
                    "asset.malformed-file-record",
                    "/files/file~1~0/dataURL",
                ),
                (
                    json!({"id":"file/~","created":1,"dataURL":"data:image/png;base64,aW52YWxpZA=="}),
                    "image.unsupported-format",
                    "/files/file~1~0/dataURL",
                ),
            ] {
                let source = image_scene(json!([element]), json!({"file/~":record}));
                let diagnostic = image_expected(
                    code,
                    if code == "asset.malformed-file-record" {
                        Value::Null
                    } else {
                        json!("image")
                    },
                    pointer,
                    "file/~",
                    status,
                    format,
                );
                let rows = if record.is_null() {
                    json!([diagnostic, conflict])
                } else {
                    json!([conflict, diagnostic])
                };
                assert_report_with_request(&source, &request, rows);
                assert_report_with_request(
                    &image_scene(json!([]), json!({"file/~":record})),
                    &request,
                    json!([]),
                );
            }
        }
    }
}

#[test]
fn missing_image_contract_in_every_format() {
    missing_images_report_referring_owners_and_format_specific_recovery();
    image_authority_precedence_and_valid_silence();
    for format in FORMATS {
        for status in ["saved", "error"] {
            let mut first = image("z-first", "missing/~");
            first["status"] = json!(status);
            let mut second = first.clone();
            second["id"] = json!("a-second");
            for file in [None, Some(Value::Null), Some(json!(""))] {
                let mut no_id = image("no-id", "unused");
                if let Some(file) = file {
                    no_id["fileId"] = file;
                } else {
                    no_id.as_object_mut().unwrap().remove("fileId");
                }
                no_id["status"] = json!(status);
                assert_report(
                    &scene(json!([no_id])),
                    format,
                    json!([image_expected(
                        "image.missing-file",
                        json!("no-id"),
                        "/elements/0/fileId",
                        "",
                        status,
                        format
                    )]),
                );
            }
            assert_report(
                &scene(json!([first, second])),
                format,
                json!([
                    image_expected(
                        "image.missing-file",
                        json!("z-first"),
                        "/elements/0/fileId",
                        "missing/~",
                        status,
                        format
                    ),
                    image_expected(
                        "image.missing-file",
                        json!("a-second"),
                        "/elements/1/fileId",
                        "missing/~",
                        status,
                        format
                    )
                ]),
            );
            let mut outside = image("outside", "missing/~");
            outside["x"] = json!(1000);
            let framed = scene(json!([
                {"id":"frame","type":"frame","width":100,"height":100,"updated":1}, first, outside
            ]));
            assert_frame_reports(
                &framed,
                format,
                &[json!([image_expected(
                    "image.missing-file",
                    json!("z-first"),
                    "/elements/1/fileId",
                    "missing/~",
                    status,
                    format
                )])],
            );
        }
    }
}

#[test]
fn deleted_error_image_does_not_select_live_file_recovery_recipe() {
    let live = image("live", "file");
    let mut deleted = image("deleted", "file");
    deleted["status"] = json!("error");
    deleted["isDeleted"] = json!(true);
    for format in FORMATS {
        assert_report(
            &image_scene(json!([live, deleted]), json!({"file":null})),
            format,
            json!([image_expected(
                "asset.malformed-file-record",
                Value::Null,
                "/files/file",
                "file",
                "saved",
                format
            )]),
        );
    }
}

fn image_expected(
    code: &str,
    owner: Value,
    pointer: &str,
    id: &str,
    status: &str,
    format: OutputFormat,
) -> Value {
    let mut details = asset_details(id, Value::Null, Value::Null, format);
    if status == "error" && format == OutputFormat::Png {
        details["recipe_id"] = json!("official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9");
    }
    expected(code, "warning", json!(["visual"]), owner, pointer, details)
}

#[test]
fn font_diagnostics_cover_text_generated_labels_and_silence() {
    local_font_substitution_is_informational_per_occurrence_in_all_formats();
    embeddable_emoji_diagnostics_identify_the_offline_recipe();
    frame_missing_glyph_reports_follow_visible_names();
    font_omission_and_zero_extent_contract();
    for format in FORMATS {
        let content = "😀🎉\u{10001}\u{10000}\u{10001}\u{200d}\u{fe0f}";
        let source = scene(json!([
            font_text("z-first", content, 2),
            font_text("a-second", content, 9)
        ]));
        let mut rows = vec![font_substitution(
            "font.liberation-sans-substituted",
            "z-first",
            "/elements/0",
            Value::Null,
        )];
        for (id, pointer) in [("z-first", "/elements/0"), ("a-second", "/elements/1")] {
            rows.push(font_substitution(
                "font.noto-emoji-substituted",
                id,
                pointer,
                Value::Null,
            ));
            for codepoint in [0x10001, 0x10000, 0x10001] {
                rows.push(missing_glyph(id, pointer, codepoint));
            }
        }
        assert_report(&source, format, json!(rows));
        let framed = scene(json!([
            {"id":"frame","type":"frame","width":1000,"height":200,"updated":1},
            font_text("inside", content, 2),
            {"id":"outside","type":"text","x":2000,"width":500,"height":40,"fontFamily":2,"fontSize":20,"text":content,"updated":1}
        ]));
        assert_frame_reports(
            &framed,
            format,
            &[json!([
                font_substitution(
                    "font.liberation-sans-substituted",
                    "inside",
                    "/elements/1",
                    Value::Null
                ),
                font_substitution(
                    "font.noto-emoji-substituted",
                    "inside",
                    "/elements/1",
                    Value::Null
                ),
                missing_glyph("inside", "/elements/1", 0x10001),
                missing_glyph("inside", "/elements/1", 0x10000),
                missing_glyph("inside", "/elements/1", 0x10001)
            ])],
        );
        let controls = "A\u{200d}\u{fe0f}B";
        // Authored endpoints for the pinned shaper's default-ignorable
        // classes (§8.3), not values generated by the implementation predicate.
        for control in [
            '\u{ad}',
            '\u{34f}',
            '\u{61c}',
            '\u{17b4}',
            '\u{17b5}',
            '\u{180b}',
            '\u{180e}',
            '\u{200b}',
            '\u{200f}',
            '\u{202a}',
            '\u{202e}',
            '\u{2060}',
            '\u{206f}',
            '\u{fe00}',
            '\u{fe0f}',
            '\u{feff}',
            '\u{fff0}',
            '\u{fff8}',
            '\u{1d173}',
            '\u{1d17a}',
            '\u{e0000}',
            '\u{e0fff}',
        ] {
            assert_report(
                &scene(json!([font_text("control", &format!("A{control}B"), 9)])),
                format,
                json!([]),
            );
        }
        for scalar in ['\u{115f}', '\u{1160}', '\u{3164}', '\u{ffa0}', '\u{2029}'] {
            assert_report(
                &scene(json!([font_text(
                    "not-ignorable",
                    &format!("A{scalar}B"),
                    9
                )])),
                format,
                json!([missing_glyph("not-ignorable", "/elements/0", scalar as u32)]),
            );
        }
        let mut omitted = font_text("unbound", content, 2);
        omitted["containerId"] = json!("box");
        let mut deleted = font_text("deleted", content, 2);
        deleted["isDeleted"] = json!(true);
        assert_report(
            &scene(json!([
                font_text("direct", controls, 9),
                font_text("empty", "", 2),
                rectangle("box"),
                omitted,
                deleted
            ])),
            format,
            json!([]),
        );
        // Renderer-owned chrome is intentionally silent for Liberation Sans
        // (§8.3/#175); frame labels are hidden in frame exports.
        let chrome = scene(json!([
            {"id":"frame","type":"frame","name":"Plain frame","width":300,"height":100,"updated":1},
            {"id":"magic","type":"magicframe","name":"","x":400,"width":300,"height":100,"updated":1},
            {"id":"embed","type":"embeddable","link":"https://example.com/plain","width":300,"height":100,"seed":1,"updated":1},
            {"id":"iframe","type":"iframe","width":300,"height":100,"seed":1,"updated":1}
        ]));
        assert_report(&chrome, format, json!([]));
        assert_frame_reports(&chrome, format, &[json!([]), json!([])]);
        let embed = scene(
            json!([{"id":"embed","type":"embeddable","link":"https://example.com/\u{10001}\u{10000}\u{10001}","width":1000,"height":100,"seed":1,"updated":1}]),
        );
        assert_report(
            &embed,
            format,
            json!([
                missing_glyph("embed", "/elements/0", 0x10001),
                missing_glyph("embed", "/elements/0", 0x10000),
                missing_glyph("embed", "/elements/0", 0x10001)
            ]),
        );
        let fallback = scene(
            json!([{"id":"fallback","type":"future\u{10001}\u{10000}","width":1000,"height":100,"updated":1}]),
        );
        assert_report(
            &fallback,
            format,
            json!([
                expected(
                    "compatibility.unsupported-element-type",
                    "warning",
                    json!(["visual", "semantic"]),
                    json!("fallback"),
                    "/elements/0",
                    json!({"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"})
                ),
                font_substitution(
                    "font.liberation-sans-substituted",
                    "fallback",
                    "/elements/0",
                    Value::Null
                ),
                missing_glyph("fallback", "/elements/0", 0x10001),
                missing_glyph("fallback", "/elements/0", 0x10000)
            ]),
        );
        let tiny = scene(
            json!([{"id":"fallback","type":"future\u{10000}","width":10,"height":5,"updated":1}]),
        );
        assert_report(
            &tiny,
            format,
            json!([expected(
                "compatibility.unsupported-element-type",
                "warning",
                json!(["visual", "semantic"]),
                json!("fallback"),
                "/elements/0",
                json!({"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"})
            )]),
        );
    }
}

fn font_omission_and_zero_extent_contract() {
    for format in FORMATS {
        let embedded = scene(
            json!([{"id":"embed","type":"embeddable","link":"https://example.com/😀\u{10000}","width":1000,"height":100,"seed":1,"updated":1,"roughness":0}]),
        );
        for (limits, code, field, configured, attempted, kind) in [
            (
                ResourceLimits::builder()
                    .max_geometry_segments_per_element(20)
                    .build()
                    .unwrap(),
                "resource.element-complexity-replaced",
                "max_geometry_segments_per_element",
                20,
                716,
                "count",
            ),
            (
                ResourceLimits::builder()
                    .max_element_work_units(2000)
                    .build()
                    .unwrap(),
                "resource.element-work-exceeded",
                "max_element_work_units",
                2000,
                25152,
                "work_units",
            ),
        ] {
            let request = RenderRequest::builder(format)
                .resource_limits(limits)
                .build()
                .unwrap();
            let row = |value| {
                expected(
                    code,
                    "warning",
                    json!(["visual"]),
                    json!("embed"),
                    "/elements/0",
                    json!({"kind":"resource_limit","field":field,"configured":configured,"value_kind":kind,"measurement":{"kind":"attempted","value":value},"recovery":"omitted"}),
                )
            };
            let mut rows = vec![row(attempted)];
            // PNG additionally rejects the remaining outline's pixel work.
            if format == OutputFormat::Png && kind == "work_units" {
                rows.push(row(1635904));
            }
            assert_report_with_request(&embedded, &request, json!(rows));
        }
        let zero_width = scene(
            json!([{"id":"embed","type":"embeddable","link":"https://example.com/😀\u{10000}","width":0,"height":100,"seed":1,"updated":1,"roughness":0}]),
        );
        let rows = if format == OutputFormat::Png {
            json!([])
        } else {
            json!([
            font_substitution("font.noto-emoji-substituted", "embed", "/elements/0", json!("official.embeddable-offline@sha256:d1d9207be2a4022886b9916ef0068b654c47f64d9ea59cd6d31d6432c5eecb01")),
            missing_glyph("embed", "/elements/0", 0x10000)
        ])
        };
        assert_report(&zero_width, format, rows);
        // Zero-height PNG embeddables omit their generated label. SVG/PDF
        // still emit it, including its font diagnostics.
        let source = scene(
            json!([{"id":"embed","type":"embeddable","link":"https://example.com/😀\u{10000}","width":1000,"height":0,"seed":1,"updated":1}]),
        );
        let rows = if format == OutputFormat::Png {
            json!([])
        } else {
            json!([
            font_substitution("font.noto-emoji-substituted", "embed", "/elements/0", json!("official.embeddable-offline@sha256:d1d9207be2a4022886b9916ef0068b654c47f64d9ea59cd6d31d6432c5eecb01")),
            missing_glyph("embed", "/elements/0", 0x10000)
        ])
        };
        assert_report(&source, format, rows);
        // Both the original text and the replacement label exceed the ceiling.
        // Retain the box and both resource diagnostics, but no font diagnostic.
        let text = scene(json!([font_text("text", "😀\u{10000}", 2)]));
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_text_scalars_per_element(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        assert_report_with_request(
            &text,
            &request,
            json!([
                expected(
                    "resource.element-complexity-replaced",
                    "warning",
                    json!(["visual"]),
                    json!("text"),
                    "/elements/0",
                    json!({"kind":"resource_limit","field":"max_text_scalars_per_element","configured":1,"value_kind":"count","measurement":{"kind":"attempted","value":2},"recovery":"placeholder"})
                ),
                expected(
                    "resource.element-complexity-replaced",
                    "warning",
                    json!(["visual"]),
                    json!("text"),
                    "/elements/0",
                    json!({"kind":"resource_limit","field":"max_text_scalars_per_element","configured":1,"value_kind":"count","measurement":{"kind":"attempted","value":17},"recovery":"omitted"})
                )
            ]),
        );
        // Post-layout rejection must likewise precede all font diagnostics.
        // The ceiling admits the source preflight and replacement box but not
        // either affected glyph outline.
        let glyph_limited = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_geometry_segments_per_element(10)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        assert_report_with_request(
            &text,
            &glyph_limited,
            json!([expected(
                "resource.element-complexity-replaced",
                "warning",
                json!(["visual"]),
                json!("text"),
                "/elements/0",
                json!({"kind":"resource_limit","field":"max_geometry_segments_per_element","configured":10,"value_kind":"count","measurement":{"kind":"attempted","value":144},"recovery":"omitted"})
            )]),
        );
    }
}

fn font_text(id: &str, text: &str, family: u8) -> Value {
    json!({"id":id,"type":"text","text":text,"fontFamily":family,"fontSize":20,"width":500,"height":40,"updated":1})
}

fn frame_missing_glyph_reports_follow_visible_names() {
    let source = scene(
        json!([{"id":"frame","type":"frame","name":"A\u{10001}\u{10000}\u{10001}B","width":500,"height":100,"updated":1}]),
    );
    for format in FORMATS {
        assert_report(
            &source,
            format,
            json!([
                missing_glyph("frame", "/elements/0", 0x10001),
                missing_glyph("frame", "/elements/0", 0x10000),
                missing_glyph("frame", "/elements/0", 0x10001)
            ]),
        );
        assert_frame_reports(&source, format, &[json!([])]);
        let hidden = RenderRequest::builder(format)
            .frame_options(FrameOptions::new(true, true, false, true))
            .build()
            .unwrap();
        assert_report_with_request(&source, &hidden, json!([]));
        let truncated = scene(
            json!([{"id":"frame","type":"frame","name":"A long ordinary prefix then \u{10000}","width":60,"height":100,"updated":1}]),
        );
        assert_report(&truncated, format, json!([]));
    }
}

fn font_substitution(code: &str, id: &str, pointer: &str, recipe: Value) -> Value {
    expected(
        code,
        "info",
        json!(["visual"]),
        json!(id),
        pointer,
        json!({"kind":"compatibility","recovery":"substituted","recipe_id":recipe}),
    )
}

fn missing_glyph(id: &str, pointer: &str, codepoint: u32) -> Value {
    expected(
        "font.missing-glyph",
        "warning",
        json!(["visual"]),
        json!(id),
        pointer,
        json!({"kind":"missing_glyph","codepoint":codepoint,"recovery":"substituted"}),
    )
}

fn embeddable_emoji_diagnostics_identify_the_offline_recipe() {
    let source = scene(
        json!([{"id":"embed","type":"embeddable","link":"https://example.com/😀🎉","width":500,"height":100,"seed":1,"updated":1}]),
    );
    for format in FORMATS {
        assert_report(
            &source,
            format,
            json!([expected(
                "font.noto-emoji-substituted",
                "info",
                json!(["visual"]),
                json!("embed"),
                "/elements/0",
                json!({"kind":"compatibility","recovery":"substituted","recipe_id":"official.embeddable-offline@sha256:d1d9207be2a4022886b9916ef0068b654c47f64d9ea59cd6d31d6432c5eecb01"})
            )]),
        );
    }
}

#[test]
fn relationship_cycle_contract_in_every_format() {
    disabled_frame_cycle_repairs_input_without_claiming_output_effects();
    cycle_effects_follow_text_and_binding_consumers();
    relationship_cycles_report_each_closing_edge_once();
    mixed_reciprocal_cycles_and_fallbacks_have_exact_reports();
    frame_opacity_cycle_affects_output_even_when_clipping_is_disabled();
    frame_cycle_reports_and_binding_selection();
}

fn mixed_reciprocal_cycles_and_fallbacks_have_exact_reports() {
    for format in FORMATS {
        let effects = if format == OutputFormat::Png {
            json!(["input_repair", "visual"])
        } else {
            json!(["input_repair", "visual", "semantic"])
        };
        for field in ["boundElements", "boundElementIds"] {
            let mut container = rectangle("container");
            container["frameId"] = json!("text");
            container[field] = if field == "boundElements" {
                json!([{"id":"text","type":"text"}])
            } else {
                json!(["text"])
            };
            let text = json!({"id":"text","type":"text","text":"Hello","fontFamily":9,"fontSize":20,"width":80,"height":25,"updated":1,"containerId":"container"});
            let pointer = if field == "boundElements" {
                "/elements/0/boundElements/0/id"
            } else {
                "/elements/0/boundElementIds/0"
            };
            assert_report(
                &scene(json!([container, text])),
                format,
                json!([
                    expected(
                        "normalization.relationship-cycle",
                        "warning",
                        json!(["input_repair"]),
                        json!("container"),
                        pointer,
                        compatibility("removed")
                    ),
                    expected(
                        "normalization.relationship-cycle",
                        "warning",
                        effects.clone(),
                        json!("text"),
                        "/elements/1/containerId",
                        compatibility("removed")
                    )
                ]),
            );
            // A genuine reciprocal pair is two projections of the same
            // dependency direction, not a cycle. Removing the opposing frame
            // edge therefore makes both reference encodings silent.
            container.as_object_mut().unwrap().remove("frameId");
            assert_report(&scene(json!([container, text])), format, json!([]));
        }
        let frame = json!({"id":"frame","type":"frame","width":100,"height":100,"updated":1,"frameId":"future"});
        let fallback = json!({"id":"future","type":"future","width":40,"height":40,"updated":1,"frameId":"frame"});
        let source = scene(json!([frame, fallback]));
        let diagnostics = json!([
            expected(
                "compatibility.unsupported-element-type",
                "warning",
                json!(["visual", "semantic"]),
                json!("future"),
                "/elements/1",
                json!({"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"})
            ),
            expected(
                "normalization.relationship-cycle",
                "warning",
                effects,
                json!("future"),
                "/elements/1/frameId",
                compatibility("removed")
            ),
            expected(
                "font.liberation-sans-substituted",
                "info",
                json!(["visual"]),
                json!("future"),
                "/elements/1",
                compatibility("substituted")
            )
        ]);
        assert_report(&source, format, diagnostics.clone());
        assert_frame_reports(&source, format, std::slice::from_ref(&diagnostics));
        let disabled = RenderRequest::builder(format)
            .frame_options(FrameOptions::new(false, false, false, false))
            .build()
            .unwrap();
        let mut disabled_diagnostics = diagnostics.clone();
        disabled_diagnostics[1]["effects"] = json!(["input_repair"]);
        assert_report_with_request(&source, &disabled, disabled_diagnostics);
        // A placeholder outside the selected viewport is silent for that frame.
        let outside = json!({"id":"future","type":"future","x":200,"width":40,"height":40,"updated":1,"frameId":"frame"});
        assert_frame_reports(&scene(json!([frame, outside])), format, &[json!([])]);
        // With no safe bounds the rejected source is omitted. Its source cycle
        // still needs an input repair, but has no output channels to affect.
        let omitted = json!({"id":"future","type":"future","x":null,"y":null,"width":null,"height":null,"updated":1,"frameId":"frame"});
        let omitted_source = scene(json!([frame, omitted]));
        assert_report(
            &omitted_source,
            format,
            json!([
                expected(
                    "compatibility.unsupported-element-type",
                    "warning",
                    json!(["visual", "semantic"]),
                    json!("future"),
                    "/elements/1",
                    compatibility("omitted")
                ),
                expected(
                    "normalization.relationship-cycle",
                    "warning",
                    json!(["input_repair"]),
                    json!("future"),
                    "/elements/1/frameId",
                    compatibility("removed")
                )
            ]),
        );
        assert_frame_reports(&omitted_source, format, &[json!([])]);
    }
}

fn frame_opacity_cycle_affects_output_even_when_clipping_is_disabled() {
    let source = scene(json!([
        {"id":"parent","type":"frame","width":100,"height":100,"opacity":50,"updated":1,"frameId":"child"},
        {"id":"child","type":"rectangle","width":40,"height":40,"seed":1,"updated":1,"frameId":"parent"}
    ]));
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .frame_options(FrameOptions::new(false, false, false, false))
            .build()
            .unwrap();
        let effects = if format == OutputFormat::Png {
            json!(["input_repair", "visual"])
        } else {
            json!(["input_repair", "visual", "semantic"])
        };
        assert_report_with_request(
            &source,
            &request,
            json!([expected(
                "normalization.relationship-cycle",
                "warning",
                effects,
                json!("child"),
                "/elements/1/frameId",
                compatibility("removed")
            )]),
        );
    }
}

fn assert_frame_reports(scene: &SceneDocument, format: OutputFormat, expected: &[Value]) {
    let renderer = Renderer::new();
    let frames: Vec<_> = scene.frames().collect();
    assert_eq!(frames.len(), expected.len());
    let request = FrameRenderRequest::builder(format).build().unwrap();
    let mut batch = renderer
        .render_frames(scene, frames.iter().copied(), &request)
        .unwrap();
    for (frame, expected) in frames.iter().zip(expected) {
        let first = renderer.render_frame(*frame, &request).unwrap();
        let second = Renderer::new().render_frame(*frame, &request).unwrap();
        let item = batch.next().unwrap();
        let replayed = item.result().as_ref().unwrap();
        assert_eq!(first.artifact().bytes(), replayed.artifact().bytes());
        assert_eq!(
            serde_json::to_vec(first.report()).unwrap(),
            serde_json::to_vec(replayed.report()).unwrap()
        );
        assert_outcomes(&first, &second, format, expected.clone());
    }
    assert!(batch.next().is_none());
}

fn cycle_expected(id: &str, pointer: &str, effects: Value) -> Value {
    expected(
        "normalization.relationship-cycle",
        "warning",
        effects,
        json!(id),
        pointer,
        compatibility("removed"),
    )
}

fn frame_cycle_reports_and_binding_selection() {
    for format in FORMATS {
        let effects = if format == OutputFormat::Png {
            json!(["input_repair", "visual"])
        } else {
            json!(["input_repair", "visual", "semantic"])
        };
        let frames = scene(json!([
            {"id":"first","type":"frame","width":40,"height":40,"updated":1,"frameId":"first"},
            {"id":"second","type":"magicframe","x":150,"width":40,"height":40,"updated":1,"frameId":"second"}
        ]));
        assert_frame_reports(
            &frames,
            format,
            &[
                json!([cycle_expected(
                    "first",
                    "/elements/0/frameId",
                    effects.clone()
                )]),
                json!([cycle_expected(
                    "second",
                    "/elements/1/frameId",
                    effects.clone()
                )]),
            ],
        );
        for field in [
            "startBinding",
            "endBinding",
            "containerId",
            "boundElements",
            "boundElementIds",
        ] {
            let mut outside = json!({"id":"outside","type":"arrow","x":200,"width":40,"height":20,"points":[[0,0],[40,20]],"seed":1,"updated":1});
            let mut inside = if field == "containerId" {
                json!({"id":"inside","type":"text","text":"","fontFamily":9,"fontSize":20,"x":10,"width":40,"height":20,"updated":1})
            } else {
                json!({"id":"inside","type":"arrow","x":10,"width":40,"height":20,"points":[[0,0],[40,20]],"seed":1,"updated":1})
            };
            let pointer = match field {
                "boundElements" | "boundElementIds" => {
                    // Reverse bindings create outside -> inside dependencies;
                    // close them with a prior inside -> outside dependency.
                    outside["boundElements"] = json!([{"id":"inside","type":"arrow"}]);
                    inside[field] = if field == "boundElements" {
                        json!([{"id":"outside","type":"arrow"}])
                    } else {
                        json!(["outside"])
                    };
                    if field == "boundElements" {
                        "/elements/2/boundElements/0/id"
                    } else {
                        "/elements/2/boundElementIds/0"
                    }
                }
                "containerId" => {
                    outside["startBinding"] = json!({"elementId":"inside","focus":0,"gap":0});
                    inside[field] = json!("outside");
                    "/elements/2/containerId"
                }
                _ => {
                    outside[field] = json!({"elementId":"inside","focus":0,"gap":0});
                    inside[field] = json!({"elementId":"outside","focus":0,"gap":0});
                    if field == "startBinding" {
                        "/elements/2/startBinding/elementId"
                    } else {
                        "/elements/2/endBinding/elementId"
                    }
                }
            };
            let source = scene(json!([
                {"id":"frame","type":"frame","width":100,"height":100,"updated":1}, outside, inside
            ]));
            assert_report(
                &source,
                format,
                json!([cycle_expected("inside", pointer, json!(["input_repair"]))]),
            );
            assert_frame_reports(
                &source,
                format,
                &[json!([cycle_expected("inside", pointer, effects.clone())])],
            );
        }
    }
}

fn disabled_frame_cycle_repairs_input_without_claiming_output_effects() {
    let mut element = rectangle("self");
    element["frameId"] = json!("self");
    for format in FORMATS {
        for options in [
            FrameOptions::new(false, true, true, true),
            FrameOptions::new(true, false, true, true),
        ] {
            let request = RenderRequest::builder(format)
                .frame_options(options)
                .build()
                .unwrap();
            assert_report_with_request(
                &scene(json!([element])),
                &request,
                json!([expected(
                    "normalization.relationship-cycle",
                    "warning",
                    json!(["input_repair"]),
                    json!("self"),
                    "/elements/0/frameId",
                    compatibility("removed")
                )]),
            );
        }
        for app_state in [
            json!({"frameRendering":{"enabled":false}}),
            json!({"frameRendering":{"clip":false}}),
        ] {
            let source = SceneDocument::parse(&serde_json::to_vec(&json!({"type":"excalidraw","version":2,"elements":[element],"appState":app_state,"files":{}})).unwrap()).unwrap();
            assert_report(
                &source,
                format,
                json!([expected(
                    "normalization.relationship-cycle",
                    "warning",
                    json!(["input_repair"]),
                    json!("self"),
                    "/elements/0/frameId",
                    compatibility("removed")
                )]),
            );
        }
    }
}

fn cycle_effects_follow_text_and_binding_consumers() {
    for format in FORMATS {
        let visual_effects = if format == OutputFormat::Png {
            json!(["input_repair", "visual"])
        } else {
            json!(["input_repair", "visual", "semantic"])
        };
        for (text, effects) in [
            ("Hello", visual_effects.clone()),
            ("", json!(["input_repair"])),
        ] {
            let source = json!({"id":"text","type":"text","text":text,"fontFamily":9,"fontSize":20,"width":80,"height":25,"updated":1,"containerId":"text"});
            assert_report(
                &scene(json!([source])),
                format,
                json!([expected(
                    "normalization.relationship-cycle",
                    "warning",
                    effects,
                    json!("text"),
                    "/elements/0/containerId",
                    compatibility("removed")
                )]),
            );
        }
        // Endpoint bindings do not change stored scene geometry.
        for binding in ["startBinding", "endBinding"] {
            let mut arrow = json!({"id":"arrow","type":"arrow","width":40,"height":20,"points":[[0,0],[40,20]],"seed":1,"updated":1});
            arrow[binding] = json!({"elementId":"arrow","focus":0,"gap":0});
            assert_report(
                &scene(json!([arrow])),
                format,
                json!([expected(
                    "normalization.relationship-cycle",
                    "warning",
                    json!(["input_repair"]),
                    json!("arrow"),
                    &format!("/elements/0/{binding}/elementId"),
                    compatibility("removed")
                )]),
            );
        }
        for field in ["boundElements", "boundElementIds"] {
            for (text, effects) in [
                (false, json!(["input_repair"])),
                (true, json!(["input_repair"])),
            ] {
                let mut element = if text {
                    json!({"id":"self","type":"text","text":"Hello","fontFamily":9,"fontSize":20,"width":80,"height":25,"updated":1})
                } else {
                    rectangle("self")
                };
                element[field] = if field == "boundElements" {
                    json!([{"id":"self","type":"text"}])
                } else {
                    json!(["self"])
                };
                let pointer = if field == "boundElements" {
                    "/elements/0/boundElements/0/id"
                } else {
                    "/elements/0/boundElementIds/0"
                };
                assert_report(
                    &scene(json!([element])),
                    format,
                    json!([expected(
                        "normalization.relationship-cycle",
                        "warning",
                        effects,
                        json!("self"),
                        pointer,
                        compatibility("removed")
                    )]),
                );
            }
        }
    }
}

fn relationship_cycles_report_each_closing_edge_once() {
    for format in FORMATS {
        let effects = if format == OutputFormat::Png {
            json!(["input_repair", "visual"])
        } else {
            json!(["input_repair", "visual", "semantic"])
        };
        for pairs in [
            vec![("a", "b"), ("b", "a")],
            vec![("a", "b"), ("b", "a"), ("c", "d"), ("d", "c")],
            vec![("z", "a"), ("a", "z"), ("y", "b"), ("b", "y")],
        ] {
            let elements: Vec<_> = pairs
                .iter()
                .map(|(id, target)| {
                    let mut element = rectangle(id);
                    element["frameId"] = json!(target);
                    element
                })
                .collect();
            let diagnostics: Vec<_> = pairs
                .iter()
                .enumerate()
                .filter(|(index, _)| index % 2 == 1)
                .map(|(index, (id, _))| {
                    expected(
                        "normalization.relationship-cycle",
                        "warning",
                        effects.clone(),
                        json!(id),
                        &format!("/elements/{index}/frameId"),
                        compatibility("removed"),
                    )
                })
                .collect();
            assert_report(&scene(json!(elements)), format, json!(diagnostics));
        }
        // Two distinct source properties on one owner must remain distinct,
        // even though each projects to two graph edges.
        let text = json!({"id":"text","type":"text","text":"Hello","fontFamily":9,"fontSize":20,"width":80,"height":25,"updated":1,"frameId":"text","containerId":"text"});
        assert_report(
            &scene(json!([text])),
            format,
            json!([
                expected(
                    "normalization.relationship-cycle",
                    "warning",
                    effects.clone(),
                    json!("text"),
                    "/elements/0/containerId",
                    compatibility("removed")
                ),
                expected(
                    "normalization.relationship-cycle",
                    "warning",
                    effects.clone(),
                    json!("text"),
                    "/elements/0/frameId",
                    compatibility("removed")
                )
            ]),
        );
        let mut acyclic = rectangle("a");
        acyclic["frameId"] = json!("b");
        let mut deleted = rectangle("deleted");
        deleted["frameId"] = json!("deleted");
        deleted["isDeleted"] = json!(true);
        assert_report(
            &scene(json!([acyclic, rectangle("b"), deleted])),
            format,
            json!([]),
        );
    }
}

fn scene(elements: Value) -> SceneDocument {
    image_scene(elements, json!({}))
}

fn rectangle(id: &str) -> Value {
    json!({"id":id,"type":"rectangle","x":0,"y":0,"width":40,"height":40,"seed":1,"updated":1})
}

fn expected(
    code: &str,
    severity: &str,
    effects: Value,
    element: Value,
    pointer: &str,
    details: Value,
) -> Value {
    json!({"code":code,"severity":severity,"effects":effects,"element_id":element,
        "json_pointer":pointer,"details":details})
}

fn compatibility(recovery: &str) -> Value {
    json!({"kind":"compatibility","recovery":recovery,"recipe_id":null})
}

/// Compare every machine-readable field and the complete ordered report. The
/// explanatory message must be nonempty and repeats byte-for-byte; its prose
/// is not used to infer the diagnostic's semantics.
fn assert_report(scene: &SceneDocument, format: OutputFormat, expected: Value) {
    let request = RenderRequest::builder(format).build().unwrap();
    assert_report_with_request(scene, &request, expected);
}

fn assert_report_with_request(scene: &SceneDocument, request: &RenderRequest, expected: Value) {
    let format = request.format();
    let first = Renderer::new().render(scene, request).unwrap();
    let second = Renderer::new().render(scene, request).unwrap();
    assert_outcomes(&first, &second, format, expected);
}

fn assert_outcomes(
    first: &RenderOutcome,
    second: &RenderOutcome,
    format: OutputFormat,
    mut expected: Value,
) {
    let bytes = serde_json::to_vec(first.report()).unwrap();
    assert_eq!(bytes, serde_json::to_vec(second.report()).unwrap());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    let mut actual = serde_json::to_value(first.report()).unwrap();
    assert_eq!(actual["schema_version"], 1);
    for row in expected.as_array_mut().unwrap() {
        row["format"] = json!(format.as_str());
    }
    for row in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    assert_eq!(
        actual,
        json!({"schema_version":1,"diagnostics":expected}),
        "{format:?}"
    );
}

#[test]
fn missing_timestamps_are_repaired_in_source_order_and_valid_or_deleted_are_silent() {
    let mut first = rectangle("first");
    first.as_object_mut().unwrap().remove("updated");
    let valid = rectangle("valid");
    let mut second = rectangle("second");
    second.as_object_mut().unwrap().remove("updated");
    let source = scene(json!([first,valid,second,{"type":"rectangle","isDeleted":true}]));
    for format in FORMATS {
        assert_report(
            &source,
            format,
            json!([
                expected(
                    "normalization.missing-timestamp",
                    "warning",
                    json!(["input_repair"]),
                    json!("first"),
                    "/elements/0/updated",
                    compatibility("repaired")
                ),
                expected(
                    "normalization.missing-timestamp",
                    "warning",
                    json!(["input_repair"]),
                    json!("second"),
                    "/elements/2/updated",
                    compatibility("repaired")
                )
            ]),
        );
        assert_report(&scene(json!([rectangle("valid")])), format, json!([]));
        let data = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
        // File source order z,a is opposite both lexical and reference order a,z.
        let source = format!(
            r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"a-ref","type":"image","width":40,"height":30,"fileId":"a","status":"saved","scale":[1,1],"updated":1}},{{"id":"z-ref","type":"image","width":40,"height":30,"fileId":"z","status":"saved","scale":[1,1],"updated":1}}],"appState":{{}},"files":{{"z":{{"id":"z","mimeType":"image/png","dataURL":"{data}"}},"a":{{"id":"a","mimeType":"image/png","dataURL":"{data}"}},"unused":{{"id":"unused","mimeType":"image/png","dataURL":"{data}"}}}}}}"#
        );
        let source = SceneDocument::parse(source.as_bytes()).unwrap();
        assert_report(
            &source,
            format,
            json!([
                expected(
                    "normalization.missing-timestamp",
                    "warning",
                    json!(["input_repair"]),
                    Value::Null,
                    "/files/z/created",
                    compatibility("repaired")
                ),
                expected(
                    "normalization.missing-timestamp",
                    "warning",
                    json!(["input_repair"]),
                    Value::Null,
                    "/files/a/created",
                    compatibility("repaired")
                )
            ]),
        );
        assert_report(
            &image_scene(
                json!([image("valid", "valid")]),
                json!({"valid":{"id":"valid","mimeType":"image/png","dataURL":data,"created":1}}),
            ),
            format,
            json!([]),
        );
    }
}

#[test]
fn zero_rough_seeds_are_repaired_once_per_participating_shape() {
    let mut first = rectangle("first");
    first["seed"] = json!(0);
    let mut second = rectangle("second");
    second["seed"] = json!(0);
    for format in FORMATS {
        assert_report(
            &scene(json!([first, rectangle("valid"), second])),
            format,
            json!([
                expected(
                    "normalization.zero-rough-seed",
                    "warning",
                    json!(["input_repair", "visual"]),
                    json!("first"),
                    "/elements/0/seed",
                    compatibility("repaired")
                ),
                expected(
                    "normalization.zero-rough-seed",
                    "warning",
                    json!(["input_repair", "visual"]),
                    json!("second"),
                    "/elements/2/seed",
                    compatibility("repaired")
                )
            ]),
        );
        assert_report(
            &scene(json!([{"type":"rectangle","isDeleted":true,"seed":0},rectangle("valid")])),
            format,
            json!([]),
        );
    }
}

#[test]
fn unknown_properties_report_each_source_subtree_and_ignore_opaque_metadata() {
    let mut first = rectangle("first");
    first["future"] = json!({"deep":{"unknown":true}});
    first["customData"] = json!({"future":true});
    let mut second = rectangle("second");
    second["future"] = json!(true);
    for format in FORMATS {
        assert_report(
            &scene(json!([first,second,{"type":"selection","future":true}])),
            format,
            json!([
                expected(
                    "compatibility.unsupported-property",
                    "warning",
                    json!(["visual", "semantic", "metadata"]),
                    json!("first"),
                    "/elements/0/future",
                    compatibility("ignored")
                ),
                expected(
                    "compatibility.unsupported-property",
                    "warning",
                    json!(["visual", "semantic", "metadata"]),
                    json!("second"),
                    "/elements/1/future",
                    compatibility("ignored")
                )
            ]),
        );
        assert_report(&scene(json!([rectangle("valid")])), format, json!([]));
    }
}

#[test]
fn invalid_colors_substitute_in_member_order_and_valid_colors_are_silent() {
    // Source member order is intentional and must survive diagnostic assembly.
    let source=SceneDocument::parse(br#"{"type":"excalidraw","version":2,"elements":[{"id":"r","type":"rectangle","width":40,"height":40,"strokeColor":"not-a-color","backgroundColor":"also-invalid","seed":1,"updated":1}],"appState":{},"files":{}}"#).unwrap();
    for format in FORMATS {
        assert_report(
            &source,
            format,
            json!([
                expected(
                    "color.invalid",
                    "warning",
                    json!(["input_repair", "visual"]),
                    json!("r"),
                    "/elements/0/strokeColor",
                    compatibility("substituted")
                ),
                expected(
                    "color.invalid",
                    "warning",
                    json!(["input_repair", "visual"]),
                    json!("r"),
                    "/elements/0/backgroundColor",
                    compatibility("substituted")
                )
            ]),
        );
        let mut valid = rectangle("r");
        valid["strokeColor"] = json!("#e03131");
        valid["backgroundColor"] = json!("transparent");
        assert_report(&scene(json!([valid])), format, json!([]));
    }
}

#[test]
fn unsafe_links_are_omitted_in_semantic_formats_and_png_is_silent() {
    let mut first = rectangle("first");
    first["link"] = json!("javascript:alert(1)");
    let mut second = rectangle("second");
    second["link"] = json!("data:text/html,hello");
    let mut valid = rectangle("valid");
    valid["link"] = json!("https://example.com/");
    for format in FORMATS {
        let diagnostics = if format == OutputFormat::Png {
            json!([])
        } else {
            json!([
                expected(
                    "link.unsafe-omitted",
                    "warning",
                    json!(["semantic"]),
                    json!("first"),
                    "/elements/0/link",
                    compatibility("omitted")
                ),
                expected(
                    "link.unsafe-omitted",
                    "warning",
                    json!(["semantic"]),
                    json!("second"),
                    "/elements/2/link",
                    compatibility("omitted")
                )
            ])
        };
        assert_report(&scene(json!([first, valid, second])), format, diagnostics);
        assert_report(&scene(json!([valid])), format, json!([]));
    }
}

#[test]
fn local_font_substitution_is_informational_per_occurrence_in_all_formats() {
    let text = |id: &str, family: u8| json!({"id":id,"type":"text","width":120,"height":30,"fontSize":20,"fontFamily":family,"text":"Office","updated":1});
    for format in FORMATS {
        assert_report(
            &scene(json!([
                text("first", 2),
                text("direct", 9),
                text("second", 2)
            ])),
            format,
            json!([
                expected(
                    "font.liberation-sans-substituted",
                    "info",
                    json!(["visual"]),
                    json!("first"),
                    "/elements/0",
                    compatibility("substituted")
                ),
                expected(
                    "font.liberation-sans-substituted",
                    "info",
                    json!(["visual"]),
                    json!("second"),
                    "/elements/2",
                    compatibility("substituted")
                )
            ]),
        );
        let mut empty = text("empty", 2);
        empty["text"] = json!("");
        assert_report(&scene(json!([text("direct", 9), empty])), format, json!([]));
    }
}

#[test]
fn unknown_elements_cover_bounded_placeholders_and_unbounded_omissions() {
    let bounded = json!({"id":"bounded","type":"future","width":40,"height":40,"updated":1,"future":{"nested":true}});
    let unbounded = json!({"id":"unbounded","type":"future","x":null,"y":null,"width":null,"height":null,"updated":1});
    for format in FORMATS {
        assert_report(
            &scene(json!([bounded,unbounded,{"type":"future","isDeleted":true}])),
            format,
            json!([
                expected(
                    "compatibility.unsupported-element-type",
                    "warning",
                    json!(["visual", "semantic"]),
                    json!("bounded"),
                    "/elements/0",
                    json!({"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"})
                ),
                expected(
                    "font.liberation-sans-substituted",
                    "info",
                    json!(["visual"]),
                    json!("bounded"),
                    "/elements/0",
                    compatibility("substituted")
                ),
                expected(
                    "compatibility.unsupported-element-type",
                    "warning",
                    json!(["visual", "semantic"]),
                    json!("unbounded"),
                    "/elements/1",
                    compatibility("omitted")
                )
            ]),
        );
        assert_report(&scene(json!([rectangle("valid")])), format, json!([]));
    }
}

#[test]
fn malformed_elements_cover_bounded_placeholders_and_unbounded_omissions() {
    let bounded = json!({"id":"bounded","type":"","width":40,"height":40,"updated":1});
    for format in FORMATS {
        assert_report(
            &scene(json!([bounded,null,{"type":"","isDeleted":true}])),
            format,
            json!([
                expected(
                    "compatibility.malformed-element",
                    "warning",
                    json!(["visual"]),
                    json!("bounded"),
                    "/elements/0",
                    json!({"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.malformed-element.v1"})
                ),
                expected(
                    "font.liberation-sans-substituted",
                    "info",
                    json!(["visual"]),
                    json!("bounded"),
                    "/elements/0",
                    compatibility("substituted")
                ),
                expected(
                    "compatibility.malformed-element",
                    "warning",
                    json!(["visual"]),
                    Value::Null,
                    "/elements/1",
                    compatibility("omitted")
                )
            ]),
        );
        assert_report(&scene(json!([rectangle("valid")])), format, json!([]));
    }
}

fn image(id: &str, file: &str) -> Value {
    json!({"id":id,"type":"image","width":40,"height":30,"fileId":file,"status":"saved","scale":[1,1],"updated":1})
}

fn image_scene(elements: Value, files: Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(&json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":files})).unwrap()).unwrap()
}

fn asset_details(id: &str, claimed: Value, validated: Value, format: OutputFormat) -> Value {
    let png = format == OutputFormat::Png;
    json!({"kind":"asset","asset_id":id,"claimed_media_type":claimed,"validated_media_type":validated,
        "recovery":if png {"placeholder"}else{"omitted"},
        "recipe_id":if png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")}else{Value::Null}})
}

/// Repeat an image recovery with both source statuses. Only PNG changes its
/// selected recipe; the format-neutral code/owner/occurrence contract remains.
fn assert_image_statuses(elements: Value, files: Value, format: OutputFormat, diagnostics: Value) {
    assert_report(
        &image_scene(elements.clone(), files.clone()),
        format,
        diagnostics.clone(),
    );
    let mut error_elements = elements;
    for element in error_elements.as_array_mut().unwrap() {
        element["status"] = json!("error");
    }
    let mut error_diagnostics = diagnostics;
    if format == OutputFormat::Png {
        for row in error_diagnostics.as_array_mut().unwrap() {
            row["details"]["recipe_id"]=json!("official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9");
        }
    }
    assert_report(
        &image_scene(error_elements, files),
        format,
        error_diagnostics,
    );
}

#[test]
fn missing_images_report_referring_owners_and_format_specific_recovery() {
    for format in FORMATS {
        assert_image_statuses(
            json!([image("first", "absent"), image("second", "absent")]),
            json!({}),
            format,
            json!([
                expected(
                    "image.missing-file",
                    "warning",
                    json!(["visual"]),
                    json!("first"),
                    "/elements/0/fileId",
                    asset_details("absent", Value::Null, Value::Null, format)
                ),
                expected(
                    "image.missing-file",
                    "warning",
                    json!(["visual"]),
                    json!("second"),
                    "/elements/1/fileId",
                    asset_details("absent", Value::Null, Value::Null, format)
                )
            ]),
        );
        let mut deleted = image("deleted", "absent");
        deleted["isDeleted"] = json!(true);
        assert_report(&image_scene(json!([deleted]), json!({})), format, json!([]));
    }
}

#[test]
fn malformed_file_records_report_each_authority_once_and_unreferenced_are_silent() {
    let files = json!({"a":null,"b":null,"unreferenced":null});
    for format in FORMATS {
        assert_image_statuses(
            json!([
                image("first", "a"),
                image("same", "a"),
                image("second", "b")
            ]),
            files.clone(),
            format,
            json!([
                expected(
                    "asset.malformed-file-record",
                    "warning",
                    json!(["visual"]),
                    Value::Null,
                    "/files/a",
                    asset_details("a", Value::Null, Value::Null, format)
                ),
                expected(
                    "asset.malformed-file-record",
                    "warning",
                    json!(["visual"]),
                    Value::Null,
                    "/files/b",
                    asset_details("b", Value::Null, Value::Null, format)
                )
            ]),
        );
        assert_report(&image_scene(json!([]), files.clone()), format, json!([]));
    }
}

#[test]
fn unsupported_payloads_report_each_file_once_and_unreferenced_are_silent() {
    let files = json!({"a":{"id":"a","mimeType":"image/png","dataURL":"data:image/png;base64,aW52YWxpZA==","created":1},
        "b":{"id":"b","mimeType":"image/png","dataURL":"data:image/png;base64,aW52YWxpZA==","created":1}});
    for format in FORMATS {
        assert_image_statuses(
            json!([
                image("first", "a"),
                image("same", "a"),
                image("second", "b")
            ]),
            files.clone(),
            format,
            json!([
                expected(
                    "image.unsupported-format",
                    "warning",
                    json!(["visual"]),
                    json!("first"),
                    "/files/a/dataURL",
                    asset_details("a", Value::Null, Value::Null, format)
                ),
                expected(
                    "image.unsupported-format",
                    "warning",
                    json!(["visual"]),
                    json!("second"),
                    "/files/b/dataURL",
                    asset_details("b", Value::Null, Value::Null, format)
                )
            ]),
        );
        assert_report(&image_scene(json!([]), files.clone()), format, json!([]));
    }
}
