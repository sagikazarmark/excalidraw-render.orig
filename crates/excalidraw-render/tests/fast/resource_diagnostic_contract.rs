//! Local resource diagnostics: independently authored complete reports and
//! boundary evidence, through the public scene/frame facade.
use base64::Engine;
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn image_scene(bytes: &[u8], mime: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"image","type":"image","x":5,"y":5,"width":20,"height":20,"fileId":"asset/~","updated":0}],
    "appState":{},"files":{"asset/~":{"id":"asset/~","created":0,"dataURL":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}}})
}
fn report(outcome: &RenderOutcome, format: OutputFormat, mut rows: Value) {
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
fn expected(
    code: &str,
    field: &str,
    configured: Value,
    attempted: Value,
    kind: &str,
    pointer: &str,
    recovery: &str,
) -> Value {
    json!({"code":code,"severity":"warning","effects":["visual"],"element_id":"image","json_pointer":pointer,
        "details":{"kind":"resource_limit","field":field,"configured":configured,"value_kind":kind,"measurement":{"kind":"attempted","value":attempted},"recovery":recovery}})
}
fn assert_repeat(source: &Value, request: &RenderRequest, rows: Value) -> RenderOutcome {
    let document = parse(source);
    let first = Renderer::new().render(&document, request).unwrap();
    let second = Renderer::new().render(&document, request).unwrap();
    report(&first, request.format(), rows);
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    first
}

#[test]
fn top_level_asset_resource_boundaries_have_exact_reports_and_authority() {
    let bytes = crate::common::codec_fixtures::png_rgba(2, 2, &[[255, 0, 0, 255]; 4]);
    for format in FORMATS {
        for (field, code, n, kind) in [
            (
                "max_asset_bytes",
                "resource.asset-bytes-exceeded",
                bytes.len() as u64,
                "bytes",
            ),
            (
                "max_raster_dimension",
                "resource.image-dimensions-exceeded",
                2,
                "dimension",
            ),
            (
                "max_decoded_pixels_per_asset",
                "resource.image-pixels-exceeded",
                4,
                "pixels",
            ),
        ] {
            for configured in [n - 1, n, n + 1] {
                let limits = match field {
                    "max_asset_bytes" => ResourceLimits::builder()
                        .max_asset_bytes(configured)
                        .build()
                        .unwrap(),
                    "max_raster_dimension" => ResourceLimits::builder()
                        .max_raster_dimension(configured as u32)
                        .build()
                        .unwrap(),
                    _ => ResourceLimits::builder()
                        .max_decoded_pixels_per_asset(configured)
                        .build()
                        .unwrap(),
                };
                // Dimension also limits the destination canvas: a one-pixel
                // scene allows probing the two-pixel intrinsic asset boundary.
                let mut source = image_scene(&bytes, "image/png");
                source["elements"][0]["width"] = json!(1);
                source["elements"][0]["height"] = json!(1);
                for supplemental in [false, true] {
                    let mut source = source.clone();
                    let assets = if supplemental {
                        source["files"] = json!({});
                        SupplementalAssets::builder()
                            .asset("asset/~", bytes.clone())
                            .build()
                            .unwrap()
                    } else {
                        SupplementalAssets::default()
                    };
                    let request = RenderRequest::builder(format)
                        .padding(0.0)
                        .supplemental_assets(assets)
                        .resource_limits(limits.clone())
                        .build()
                        .unwrap();
                    let rows = if configured < n
                        && (format != OutputFormat::Svg || field == "max_asset_bytes")
                    {
                        json!([expected(
                            code,
                            field,
                            json!(configured),
                            json!(n),
                            kind,
                            if supplemental {
                                ""
                            } else {
                                "/files/asset~1~0/dataURL"
                            },
                            if format == OutputFormat::Png {
                                "placeholder"
                            } else {
                                "omitted"
                            }
                        )])
                    } else {
                        json!([])
                    };
                    assert_repeat(&source, &request, rows);
                    source["elements"][0]["isDeleted"] = json!(true);
                    let request = RenderRequest::builder(format)
                        .padding(0.0)
                        .resource_limits(limits.clone())
                        .build()
                        .unwrap();
                    assert_repeat(&source, &request, json!([]));
                }
            }
        }
    }
}

fn svg_url(svg: &str) -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    )
}

#[test]
fn top_level_svg_work_exhaustion_uses_ordinary_image_recovery() {
    // XML comments add source bytes but no semantic records. The root has
    // one node and two attributes: six records at 16 work units each.
    for (comment_bytes, attempted) in [(9000, 9076), (8050, 8222)] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><!--{}--></svg>"#,
            "x".repeat(comment_bytes)
        );
        for format in FORMATS {
            for status in ["saved", "error"] {
                let mut source = image_scene(svg.as_bytes(), "image/svg+xml");
                source["elements"][0]["status"] = json!(status);
                let request = RenderRequest::builder(format)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_asset_work_units(8192)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let result = assert_repeat(
                    &source,
                    &request,
                    json!([expected(
                        "resource.asset-work-exceeded",
                        "max_asset_work_units",
                        json!(8192),
                        json!(attempted),
                        "work_units",
                        "/files/asset~1~0/dataURL",
                        if format == OutputFormat::Png {
                            "placeholder"
                        } else {
                            "omitted"
                        }
                    )]),
                );
                // Missing-file recovery independently selects the same ordinary
                // status recipe. Optional diagnostic metadata is disabled by default.
                source["files"] = json!({});
                let missing = Renderer::new().render(&parse(&source), &request).unwrap();
                assert_eq!(result.artifact().bytes(), missing.artifact().bytes());
            }
        }
    }
}

#[test]
fn nested_resource_limits_report_the_rejected_source_slot() {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;
    let child = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(leaf)
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(&child)
    );
    let source = image_scene(parent.as_bytes(), "image/svg+xml");
    for format in FORMATS {
        for configured in [1, 2, 3] {
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_nested_asset_depth(configured)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            assert_repeat(
                &source,
                &request,
                if configured == 1 {
                    json!([{
                        "code":"resource.nested-asset-depth-exceeded","severity":"warning","effects":if format == OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                        "details":{"kind":"nested_resource_limit","field":"max_nested_asset_depth","configured":1,"value_kind":"depth","measurement":{"kind":"attempted","value":2},
                            "asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}
                    }])
                } else {
                    json!([])
                },
            );
        }
    }
}

#[test]
fn nested_depth_contract_covers_owners_paths_frames_and_silence() {
    nested_resource_limits_report_the_rejected_source_slot();
    rejected_nested_payload_effects_follow_signatures_not_media_claims();
    nested_depth_recovery_depends_on_current_edge_not_cached_parent();
    nested_depth_recovery_envelope_and_owner_order_are_exact();
    nested_depth_placeholder_matches_manifested_geometry_and_local_inheritance();
    nested_depth_stops_discovery_and_ignores_conflicting_payload();
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;
    let child = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(leaf),
        svg_url(leaf)
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(&child)
    );
    for format in FORMATS {
        for supplemental in [false, true] {
            let mut source = image_scene(parent.as_bytes(), "image/svg+xml");
            // Repeated references discover and diagnose this owner only once.
            let mut second = source["elements"][0].clone();
            second["id"] = json!("second");
            second["x"] = json!(30);
            source["elements"].as_array_mut().unwrap().push(second);
            source["elements"].as_array_mut().unwrap().extend([
                json!({"id":"frame","type":"frame","width":60,"height":40,"updated":0}),
                json!({"id":"other","type":"frame","x":100,"width":60,"height":40,"updated":0}),
            ]);
            let assets = if supplemental {
                source["files"] = json!({});
                SupplementalAssets::builder()
                    .asset("asset/~", parent.as_bytes())
                    .build()
                    .unwrap()
            } else {
                SupplementalAssets::default()
            };
            let limits = ResourceLimits::builder()
                .max_nested_asset_depth(1)
                .build()
                .unwrap();
            let rows: Vec<_> = [1,2].into_iter().map(|ordinal|json!({
                "code":"resource.nested-asset-depth-exceeded","severity":"warning","effects":if format == OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":if supplemental {""} else {"/files/asset~1~0/dataURL"},
                "details":{"kind":"nested_resource_limit","field":"max_nested_asset_depth","configured":1,"value_kind":"depth","measurement":{"kind":"attempted","value":2},
                    "asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":ordinal,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}
            })).collect();
            let request = RenderRequest::builder(format)
                .resource_limits(limits.clone())
                .supplemental_assets(assets.clone())
                .build()
                .unwrap();
            assert_repeat(&source, &request, json!(rows));
            let frame_request = FrameRenderRequest::builder(format)
                .resource_limits(limits)
                .supplemental_assets(assets)
                .build()
                .unwrap();
            let document = parse(&source);
            let renderer = Renderer::new();
            let frames: Vec<_> = document.frames().collect();
            let mut batch = renderer
                .render_frames(&document, frames.iter().copied(), &frame_request)
                .unwrap();
            for (i, frame) in frames.iter().enumerate() {
                let direct = renderer.render_frame(*frame, &frame_request).unwrap();
                let replay = batch.next().unwrap().into_result().unwrap();
                report(
                    &direct,
                    format,
                    if i == 0 { json!(rows) } else { json!([]) },
                );
                assert_eq!(direct.report(), replay.report());
                assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
            }
            for element in &mut source["elements"].as_array_mut().unwrap()[..2] {
                element["isDeleted"] = json!(true);
            }
            assert_repeat(&source, &request, json!([]));
        }
    }
}

#[test]
fn nested_depth_stops_discovery_and_ignores_conflicting_payload() {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="data:image/png;base64,bm90IGFuIGltYWdl"/></svg>"#;
    let child = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(leaf)
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(&child)
    );
    for format in FORMATS {
        let source = image_scene(parent.as_bytes(), "image/svg+xml");
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_nested_asset_depth(1)
                    .max_asset_entries(3)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        assert_repeat(
            &source,
            &request,
            json!([{
                "code":"resource.nested-asset-depth-exceeded","severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                "details":{"kind":"nested_resource_limit","field":"max_nested_asset_depth","configured":1,"value_kind":"depth","measurement":{"kind":"attempted","value":2},"asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}
            }]),
        );
        let clean = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
        let source = image_scene(clean.as_bytes(), "image/svg+xml");
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_nested_asset_depth(1)
                    .build()
                    .unwrap(),
            )
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset("asset/~", parent.as_bytes())
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        assert_repeat(
            &source,
            &request,
            json!([{"code":"asset.supplement-conflict","severity":"warning","effects":["input_repair"],"element_id":null,"json_pointer":"/files/asset~1~0","details":{"kind":"asset","asset_id":"asset/~","claimed_media_type":null,"validated_media_type":null,"recovery":"ignored","recipe_id":null}}]),
        );
    }
}

#[test]
fn nested_depth_placeholder_matches_manifested_geometry_and_local_inheritance() {
    // Independent oracle from the frozen recipe, not the recovery producer.
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../tests/oracle/scene-surfaces/revision-1.json"
    ))
    .unwrap();
    let recipe = &manifest["fallbackRecipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["content"]["kind"] == "image-error")
        .unwrap()["content"];
    let mut paths = String::new();
    for path in recipe["glyph"]["paths"].as_array().unwrap() {
        let matrix = path["transform"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        paths.push_str(&format!(
            r#"<path fill="{}" transform="matrix({matrix})" d="{}"/>"#,
            path["fill"].as_str().unwrap(),
            path["d"].as_str().unwrap()
        ));
    }
    // Worked 30x20 viewport: size=8, centred at (11,6), in a 668-unit glyph box.
    let replacement = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20" viewBox="0 0 30 20"><rect width="30" height="20" fill="#E7E7E7"/><svg x="11" y="6" width="8" height="8" viewBox="0 0 668 668">{paths}</svg></svg>"##
    );
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><rect width="30" height="20" fill="red"/></svg>"#;
    let child = |payload: &str| {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><defs><clipPath id="cut"><rect x="0" y="0" width="25" height="40"/></clipPath></defs><g clip-path="url(#cut)"><image x="5" y="3" width="30" height="20" transform="translate(2 1)" opacity="0.5" href="{}"/></g><rect x="40" y="5" width="10" height="10" fill="blue"/></svg>"#,
            svg_url(payload)
        )
    };
    let parent = |payload: &str| {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><image width="60" height="40" href="{}"/></svg>"#,
            svg_url(&child(payload))
        )
    };
    for format in FORMATS {
        let mut source = image_scene(parent(leaf).as_bytes(), "image/svg+xml");
        source["elements"][0]["width"] = json!(60);
        source["elements"][0]["height"] = json!(40);
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .resource_limits(
                ResourceLimits::builder()
                    .max_nested_asset_depth(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let actual = Renderer::new().render(&parse(&source), &request).unwrap();
        source["files"]["asset/~"]["dataURL"] = json!(svg_url(&parent(&replacement)));
        let expected = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).padding(0.0).build().unwrap(),
            )
            .unwrap();
        assert!(expected.report().is_empty());
        match format {
            OutputFormat::Png => {
                let a = crate::common::raster::decode_png(actual.artifact().bytes());
                let b = crate::common::raster::decode_png(expected.artifact().bytes());
                for y in 0..40 {
                    for x in 0..60 {
                        assert_eq!(a.pixel(x, y), b.pixel(x, y), "pixel {x},{y}");
                    }
                }
                assert_eq!(a.pixel(10, 8), [243, 243, 243, 255]);
                assert_eq!(a.pixel(30, 8), [255, 255, 255, 255]);
                assert_eq!(a.pixel(45, 8), [0, 0, 255, 255]);
            }
            OutputFormat::Svg => {
                fn canonical_svg(text: &str) -> Value {
                    fn node_value(node: roxmltree::Node<'_, '_>) -> Value {
                        let attrs: serde_json::Map<String, Value> = node
                            .attributes()
                            .map(|a| {
                                let value = if let Some(encoded) =
                                    a.value().strip_prefix("data:image/svg+xml;base64,")
                                {
                                    let bytes = base64::engine::general_purpose::STANDARD
                                        .decode(encoded)
                                        .unwrap();
                                    canonical_svg(std::str::from_utf8(&bytes).unwrap())
                                } else {
                                    json!(a.value())
                                };
                                (a.name().to_owned(), value)
                            })
                            .collect();
                        json!({"name":node.tag_name().name(),"attributes":attrs,"children":node.children().filter(|n|n.is_element()).map(node_value).collect::<Vec<_>>()})
                    }
                    node_value(roxmltree::Document::parse(text).unwrap().root_element())
                }
                fn root_image(bytes: &[u8]) -> Value {
                    let text = std::str::from_utf8(bytes).unwrap();
                    let document = roxmltree::Document::parse(text).unwrap();
                    let url = document
                        .descendants()
                        .find(|n| n.has_tag_name("image"))
                        .unwrap()
                        .attribute("href")
                        .unwrap();
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(url.strip_prefix("data:image/svg+xml;base64,").unwrap())
                        .unwrap();
                    canonical_svg(std::str::from_utf8(&bytes).unwrap())
                }
                assert_eq!(
                    root_image(actual.artifact().bytes()),
                    root_image(expected.artifact().bytes())
                );
                let text = std::str::from_utf8(actual.artifact().bytes()).unwrap();
                assert!(text.contains("font-awesome-free-5.15.4/solid/image@"));
                assert!(text.contains("font-awesome-free-5.15.4/solid/ban@"));
            }
            OutputFormat::Pdf => {
                let a = crate::common::pdf_inspect::pdf_text(actual.artifact().bytes());
                let b = crate::common::pdf_inspect::pdf_text(expected.artifact().bytes());
                // Compare every drawing stream, including form streams, while
                // retaining separate assertions for mandatory notice metadata.
                let drawing_streams = |text: &str| {
                    text.split("\nstream\n")
                        .skip(1)
                        .filter_map(|part| {
                            part.split_once("\nendstream")
                                .map(|(body, _)| body.to_owned())
                        })
                        .filter(|body| body.contains(" cm\n") || body.contains(" rg\n"))
                        .collect::<Vec<_>>()
                };
                assert_eq!(drawing_streams(&a), drawing_streams(&b));
                assert!(!drawing_streams(&a).is_empty());
                assert!(!a.contains("/Subtype /Image"));
                assert!(a.contains("font-awesome-free-5.15.4/solid/image@"));
                assert!(a.contains("font-awesome-free-5.15.4/solid/ban@"));
            }
            _ => unreachable!("revision-1 formats"),
        }
    }
}

#[test]
fn nested_depth_recovery_envelope_and_owner_order_are_exact() {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;
    let child = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(leaf)
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(&child)
    );
    let empty_child = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2"/></svg>"#;
    let omitted = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(empty_child)
    );
    for format in FORMATS {
        for work in [8191, 8192, 8193] {
            let source = json!({"type":"excalidraw","version":2,"elements":[
                {"id":"first","type":"image","width":20,"height":20,"fileId":"z/~","updated":0},
                {"id":"second","type":"image","x":30,"width":20,"height":20,"fileId":"a","updated":0}
            ],"appState":{},"files":{"z/~":{"id":"z/~","created":0,"dataURL":svg_url(&parent)},"a":{"id":"a","created":0,"dataURL":svg_url(&parent)},"unused":{"id":"unused","created":0,"dataURL":svg_url(&parent)}}});
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_nested_asset_depth(1)
                        .max_asset_work_units(work)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let rows:Vec<_>=[("a","/files/a/dataURL"),("z/~","/files/z~1~0/dataURL")].into_iter().map(|(id,pointer)|json!({
                "code":"resource.nested-asset-depth-exceeded","severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":pointer,
                "details":{"kind":"nested_resource_limit","field":"max_nested_asset_depth","configured":1,"value_kind":"depth","measurement":{"kind":"attempted","value":2},"asset_chain":[{"asset_id":id,"occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":if work<8192 {"omitted"} else {"placeholder"},"recipe_id":if work<8192 {Value::Null} else {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")}}
            })).collect();
            let result = assert_repeat(&source, &request, json!(rows));
            if work < 8192 {
                let mut expected_source = source.clone();
                for id in ["a", "z/~"] {
                    expected_source["files"][id]["dataURL"] = json!(svg_url(&omitted));
                }
                let expected_result = Renderer::new()
                    .render(
                        &parse(&expected_source),
                        &RenderRequest::builder(format).build().unwrap(),
                    )
                    .unwrap();
                assert_eq!(
                    result.artifact().bytes(),
                    expected_result.artifact().bytes()
                );
            }
        }
    }
}

#[test]
fn nested_depth_recovery_depends_on_current_edge_not_cached_parent() {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;
    let shared = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(leaf)
    );
    let wrapper = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
        svg_url(&shared)
    );
    for format in FORMATS {
        for deep_first in [false, true] {
            let (first, second) = if deep_first {
                (&wrapper, &shared)
            } else {
                (&shared, &wrapper)
            };
            let parent = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><image width="2" height="2" href="{}"/><image x="2" width="2" height="2" href="{}"/></svg>"#,
                svg_url(first),
                svg_url(second)
            );
            let mut source = image_scene(parent.as_bytes(), "image/svg+xml");
            source["elements"][0]["width"] = json!(40);
            source["elements"][0]["height"] = json!(20);
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_nested_asset_depth(2)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let result = assert_repeat(
                &source,
                &request,
                json!([{
                    "code":"resource.nested-asset-depth-exceeded","severity":"warning","effects":if format == OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                    "details":{"kind":"nested_resource_limit","field":"max_nested_asset_depth","configured":2,"value_kind":"depth","measurement":{"kind":"attempted","value":3},
                        "asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":if deep_first {1} else {2},"name":null},{"kind":"attribute","ordinal":if deep_first {2} else {3},"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}
                }]),
            );
            if format == OutputFormat::Png {
                let pixels = crate::common::raster::decode_png(result.artifact().bytes());
                let (red_x, gray_x) = if deep_first { (25, 5) } else { (5, 25) };
                assert_eq!(pixels.pixel(red_x, 5), [255, 0, 0, 255]);
                assert_eq!(pixels.pixel(gray_x, 5), [231, 231, 231, 255]);
            }
        }
    }
}

#[test]
fn rejected_nested_payload_effects_follow_signatures_not_media_claims() {
    let raster = crate::common::codec_fixtures::png_rgba(2, 2, &[[255, 0, 0, 255]; 4]);
    let vector = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    for format in FORMATS {
        for bounded in [false, true] {
            for (bytes, claim, semantic) in [
                (vector.as_slice(), "image/svg+xml", true),
                (vector.as_slice(), "image/png", true),
                (raster.as_slice(), "image/png", false),
                (raster.as_slice(), "image/svg+xml", false),
                (b"unidentified".as_slice(), "image/png", true),
            ] {
                let url = format!(
                    "data:{claim};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                );
                let child = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="{}" height="2" href="{url}"/></svg>"#,
                    u8::from(bounded)
                );
                let parent = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
                    svg_url(&child)
                );
                let source = image_scene(parent.as_bytes(), "image/svg+xml");
                let request = RenderRequest::builder(format)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_nested_asset_depth(1)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                assert_repeat(
                    &source,
                    &request,
                    json!([{
                        "code":"resource.nested-asset-depth-exceeded","severity":"warning","effects":if semantic && format != OutputFormat::Png {json!(["visual","semantic"])} else {json!(["visual"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                        "details":{"kind":"nested_resource_limit","field":"max_nested_asset_depth","configured":1,"value_kind":"depth","measurement":{"kind":"attempted","value":2},
                            "asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":if bounded {"placeholder"} else {"omitted"},"recipe_id":if bounded {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")} else {Value::Null}}
                    }]),
                );
            }
        }
    }
}

#[test]
fn nested_raster_limits_preserve_exact_paths_and_local_recovery() {
    let bytes = crate::common::codec_fixtures::png_rgba(2, 2, &[[255, 0, 0, 255]; 4]);
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    for format in FORMATS {
        for bounded in [false, true] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image width="{}" height="1" href="{url}"/><image width="{}" height="1" href="{url}"/></svg>"#,
                u8::from(bounded),
                u8::from(bounded)
            );
            let mut source = image_scene(svg.as_bytes(), "image/svg+xml");
            source["elements"][0]["width"] = json!(1);
            source["elements"][0]["height"] = json!(1);
            for (field, code, n, kind) in [
                (
                    "max_raster_dimension",
                    "resource.image-dimensions-exceeded",
                    2,
                    "dimension",
                ),
                (
                    "max_decoded_pixels_per_asset",
                    "resource.image-pixels-exceeded",
                    4,
                    "pixels",
                ),
            ] {
                for configured in [n - 1, n, n + 1] {
                    let limits = if field == "max_raster_dimension" {
                        ResourceLimits::builder()
                            .max_raster_dimension(configured as u32)
                            .build()
                            .unwrap()
                    } else {
                        ResourceLimits::builder()
                            .max_decoded_pixels_per_asset(configured)
                            .build()
                            .unwrap()
                    };
                    let request = RenderRequest::builder(format)
                        .padding(0.0)
                        .resource_limits(limits)
                        .build()
                        .unwrap();
                    // Direct SVG validates and preserves the raster payload;
                    // decoded dimension/pixel ceilings apply only to PNG/PDF.
                    let rows: Vec<_> = if configured < n && format != OutputFormat::Svg {
                        [1, 2].into_iter().map(|ordinal| json!({
                            "code":code,"severity":"warning","effects":["visual"],"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                            "details":{"kind":"nested_resource_limit","field":field,"configured":configured,"value_kind":kind,"measurement":{"kind":"attempted","value":n},
                                "asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":ordinal,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],
                                "recovery":if bounded {"placeholder"} else {"omitted"},"recipe_id":if bounded {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")} else {Value::Null}}
                        })).collect()
                    } else {
                        vec![]
                    };
                    assert_repeat(&source, &request, json!(rows));
                }
            }
        }
    }
}

#[test]
fn nested_raster_work_rejection_has_exact_measurements_and_recovery_budget() {
    // The PNG has few encoded bytes but 4096 pixels, so raster normalization
    // work crosses independently of the enclosing SVG's sanitization work.
    let bytes = crate::common::codec_fixtures::png_rgba(64, 64, &[[255, 0, 0, 255]; 4096]);
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    // Revision-1 normalization: source validation plus five source-byte
    // passes and four work units per decoded pixel; no animation-frame work.
    let n = bytes.len() as u64 * 6 + 4096 * 4;
    for format in FORMATS {
        for bounded in [false, true] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="{}" height="2" href="{url}"/></svg>"#,
                u8::from(bounded)
            );
            let source = image_scene(svg.as_bytes(), "image/svg+xml");
            for configured in [8191, 8192, n - 1, n, n + 1] {
                let request = RenderRequest::builder(format)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_asset_work_units(configured)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let placeholder = bounded && configured >= 8192;
                let rows = if configured < n && format != OutputFormat::Svg {
                    json!([{
                        "code":"resource.asset-work-exceeded","severity":"warning","effects":["visual"],"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                        "details":{"kind":"nested_resource_limit","field":"max_asset_work_units","configured":configured,"value_kind":"work_units","measurement":{"kind":"attempted","value":n},
                            "asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":if placeholder {"placeholder"} else {"omitted"},"recipe_id":if placeholder {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")} else {Value::Null}}
                    }])
                } else {
                    json!([])
                };
                assert_repeat(&source, &request, rows);
            }
        }
    }
}

#[test]
fn optional_metadata_output_boundary_retains_lower_priority_contract() {
    let source = json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}});
    for format in FORMATS {
        let metadata = MetadataOptions::new(true, true, true);
        let request = RenderRequest::builder(format)
            .metadata_options(metadata.clone())
            .build()
            .unwrap();
        let baseline = Renderer::new().render(&parse(&source), &request).unwrap();
        let n = baseline.artifact().bytes().len() as u64;
        for configured in [n - 1, n, n + 1] {
            let request = RenderRequest::builder(format)
                .metadata_options(metadata.clone())
                .resource_limits(
                    ResourceLimits::builder()
                        .max_output_bytes(configured)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let mut row = expected(
                "resource.optional-metadata-omitted",
                "max_output_bytes",
                json!(configured),
                json!(n),
                "bytes",
                "",
                "metadata_omitted",
            );
            row["element_id"] = Value::Null;
            row["effects"] = json!(["metadata"]);
            let result = assert_repeat(
                &source,
                &request,
                if configured < n {
                    json!([row])
                } else {
                    json!([])
                },
            );
            if configured >= n {
                assert_eq!(result.artifact().bytes(), baseline.artifact().bytes());
            }
        }
    }
}

#[test]
fn resource_reports_are_scoped_and_replayed_for_independent_frames() {
    let bytes = crate::common::codec_fixtures::png_rgba(2, 2, &[[255, 0, 0, 255]; 4]);
    let mut source = image_scene(&bytes, "image/png");
    source["elements"].as_array_mut().unwrap().extend([
        json!({"id":"frame","type":"frame","width":40,"height":40,"updated":0}),
        json!({"id":"other","type":"frame","x":100,"width":40,"height":40,"updated":0}),
    ]);
    for format in FORMATS {
        let limits = ResourceLimits::builder()
            .max_asset_bytes(bytes.len() as u64 - 1)
            .build()
            .unwrap();
        let request = FrameRenderRequest::builder(format)
            .resource_limits(limits)
            .build()
            .unwrap();
        let document = parse(&source);
        let renderer = Renderer::new();
        let frames: Vec<_> = document.frames().collect();
        let mut batch = renderer
            .render_frames(&document, frames.iter().copied(), &request)
            .unwrap();
        for (i, frame) in frames.iter().enumerate() {
            let direct = renderer.render_frame(*frame, &request).unwrap();
            let replay = batch.next().unwrap().into_result().unwrap();
            assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
            assert_eq!(direct.report(), replay.report());
            report(
                &direct,
                format,
                if i == 0 {
                    json!([expected(
                        "resource.asset-bytes-exceeded",
                        "max_asset_bytes",
                        json!(bytes.len() - 1),
                        json!(bytes.len()),
                        "bytes",
                        "/files/asset~1~0/dataURL",
                        if format == OutputFormat::Png {
                            "placeholder"
                        } else {
                            "omitted"
                        }
                    )])
                } else {
                    json!([])
                },
            );
        }
    }
}

#[test]
fn local_outline_resource_recovery_reports_exact_crossings() {
    for format in FORMATS {
        for bounded in [false, true] {
            let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"rectangle","x":21,"width":if bounded{20}else{0},"height":20,"seed":1,"updated":0}],"appState":{},"files":{}});
            for configured in [20, 21, 22] {
                let request = RenderRequest::builder(format)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_coordinate_magnitude(f64::from(configured))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let mut rows = vec![];
                if configured < 21 {
                    // The candidate placeholder would cross the same coordinate
                    // ceiling at x=21, so recovery omits even positive bounds.
                    let mut row = expected(
                        "resource.coordinate-range-exceeded",
                        "max_coordinate_magnitude",
                        json!(configured),
                        json!(21),
                        "coordinate",
                        "/elements/0/x",
                        "omitted",
                    );
                    if bounded && format != OutputFormat::Png {
                        row["effects"] = json!(["visual", "semantic"]);
                    }
                    rows.push(row);
                }
                assert_repeat(&source, &request, json!(rows));
            }
        }
    }
}

#[test]
fn omitted_vector_content_reports_semantic_loss_only_in_vector_formats() {
    let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"rectangle","x":21,"width":20,"height":20,"seed":1,"updated":0}],"appState":{},"files":{}});
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(20.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut row = expected(
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            json!(20),
            json!(21),
            "coordinate",
            "/elements/0/x",
            "omitted",
        );
        if format != OutputFormat::Png {
            row["effects"] = json!(["visual", "semantic"]);
        }
        assert_repeat(&source, &request, json!([row]));
    }
}

#[test]
fn rejected_empty_content_does_not_claim_lost_link_semantics() {
    for format in FORMATS {
        for link in [
            Value::Null,
            json!("https://example.com"),
            json!("javascript:alert(1)"),
        ] {
            // Empty geometry has no output area or link, even with a source URL.
            let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"rectangle","x":21,"width":0,"height":20,"seed":1,"updated":0,"link":link}],"appState":{},"files":{}});
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_coordinate_magnitude(20.0)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let mut rows = vec![];
            if format != OutputFormat::Png && link == "javascript:alert(1)" {
                rows.push(json!({"code":"link.unsafe-omitted","severity":"warning","effects":["semantic"],"element_id":"image","json_pointer":"/elements/0/link","details":{"kind":"compatibility","recovery":"omitted","recipe_id":null}}));
            }
            rows.push(expected(
                "resource.coordinate-range-exceeded",
                "max_coordinate_magnitude",
                json!(20),
                json!(21),
                "coordinate",
                "/elements/0/x",
                "omitted",
            ));
            assert_repeat(&source, &request, json!(rows));
        }
    }
}

#[test]
fn empty_text_work_recovery_does_not_claim_lost_source_semantics() {
    for format in FORMATS {
        for width in [0, 20] {
            for link in [Value::Null, json!("https://example.com")] {
                let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"text","text":"","originalText":"","fontFamily":5,"fontSize":20,"lineHeight":1.25,"width":width,"height":20,"updated":0,"link":link}],"appState":{},"files":{}});
                for configured in [31, 32, 33] {
                    let request = RenderRequest::builder(format)
                        .resource_limits(
                            ResourceLimits::builder()
                                .max_element_work_units(configured)
                                .build()
                                .unwrap(),
                        )
                        .build()
                        .unwrap();
                    let rows = if configured < 32 {
                        json!([expected(
                            "resource.element-work-exceeded",
                            "max_element_work_units",
                            json!(configured),
                            json!(32),
                            "work_units",
                            "/elements/0",
                            "omitted"
                        )])
                    } else {
                        json!([])
                    };
                    assert_repeat(&source, &request, rows);
                }
            }
        }
    }
}

#[test]
fn nonreciprocal_bound_text_recovery_does_not_claim_lost_text() {
    let source = json!({"type":"excalidraw","version":2,"elements":[
        {"id":"container","type":"rectangle","width":100,"height":100,"seed":1,"updated":0},
        {"id":"image","type":"text","text":"AB","originalText":"AB","fontFamily":5,"fontSize":20,"lineHeight":1.25,"width":0,"height":20,"containerId":"container","updated":0}
    ],"appState":{},"files":{}});
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_text_scalars_per_element(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        assert_repeat(
            &source,
            &request,
            json!([expected(
                "resource.element-complexity-replaced",
                "max_text_scalars_per_element",
                json!(1),
                json!(2),
                "count",
                "/elements/1",
                "omitted"
            )]),
        );
    }
}

#[test]
fn nested_placeholder_uses_current_reference_bounds_and_local_omission() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    let mut source = image_scene(svg.as_bytes(), "image/svg+xml");
    let mut first = source["elements"][0].clone();
    first["id"] = json!("deleted");
    first["width"] = json!(0);
    first["isDeleted"] = json!(true);
    source["elements"].as_array_mut().unwrap().insert(0, first);
    for format in FORMATS {
        for max_work in [1024, 8192, 8193] {
            let limits = ResourceLimits::builder()
                .max_nested_svg_nodes(1)
                .max_asset_work_units(max_work)
                .build()
                .unwrap();
            let request = RenderRequest::builder(format)
                .resource_limits(limits)
                .build()
                .unwrap();
            let result = Renderer::new().render(&parse(&source), &request).unwrap();
            let row = serde_json::to_value(&result.report().diagnostics()[0]).unwrap();
            assert_eq!(row["code"], "resource.nested-svg-complexity-exceeded");
            assert_eq!(row["details"]["kind"], "nested_resource_limit");
            assert_eq!(row["details"]["configured"], 1);
            assert_eq!(
                row["details"]["measurement"],
                json!({"kind":"attempted","value":2})
            );
            assert_eq!(row["details"]["paths"], json!([{"segments":[]}]));
            assert_eq!(
                row["details"]["recovery"],
                if max_work < 8192 {
                    "omitted"
                } else {
                    "placeholder"
                }
            );
            if max_work >= 8192 {
                if format == OutputFormat::Svg {
                    assert!(std::str::from_utf8(result.artifact().bytes())
                        .unwrap()
                        .contains("font-awesome"));
                }
                if format == OutputFormat::Pdf {
                    assert!(
                        crate::common::pdf_inspect::pdf_text(result.artifact().bytes())
                            .contains("font-awesome")
                    );
                }
            }
        }
    }
}
