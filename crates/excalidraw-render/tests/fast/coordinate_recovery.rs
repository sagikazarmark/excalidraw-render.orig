//! Derived-coordinate regressions through the public facade. Every fixture's
//! source coordinates fit its limit; only a subsequent semantic operation fails.
use excalidraw_render::{
    FrameOptions, FrameRenderRequest, OutputFormat, RenderRequest, Renderer, ResourceLimits,
    SceneDocument, SupplementalAssets,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn scene(elements: Value) -> SceneDocument {
    SceneDocument::parse(
        &serde_json::to_vec(
            &json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":{}}),
        )
        .unwrap(),
    )
    .unwrap()
}

fn request(format: OutputFormat, limit: f64) -> RenderRequest {
    RenderRequest::builder(format)
        .padding(0.0)
        .frame_options(FrameOptions::default().with_show_name(false))
        .resource_limits(
            ResourceLimits::builder()
                .max_coordinate_magnitude(limit)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
}

fn coordinates(outcome: &excalidraw_render::RenderOutcome) -> Vec<Value> {
    outcome
        .report()
        .diagnostics()
        .iter()
        .filter(|d| d.code().as_str() == "resource.coordinate-range-exceeded")
        .map(|d| serde_json::to_value(d).unwrap())
        .collect()
}

#[test]
fn translated_bounds_are_rejected_and_do_not_expand_the_surviving_canvas() {
    for format in FORMATS {
        let request = request(format, 100.0);
        let good = json!({"id":"good","type":"rectangle","x":0,"y":0,"width":10,"height":10,"roughness":0,"seed":1,"updated":0});
        let bad = json!({"id":"bad","type":"rectangle","x":75,"y":0,"width":50,"height":10,"roughness":0,"seed":1,"updated":0});
        let renderer = Renderer::new();
        let actual = renderer
            .render(&scene(json!([bad, good])), &request)
            .unwrap();
        let expected = renderer.render(&scene(json!([good])), &request).unwrap();
        assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        let diagnostics = coordinates(&actual);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["json_pointer"], "/elements/0");
        assert_eq!(
            diagnostics[0]["details"],
            json!({"kind":"resource_limit","field":"max_coordinate_magnitude","configured":100,"value_kind":"coordinate","measurement":{"kind":"attempted","value":125},"recovery":"omitted"})
        );
    }
}

#[test]
fn generated_controls_and_scanline_intermediates_use_safe_placeholders() {
    for format in FORMATS {
        for bad in [
            json!({"id":"bad","type":"line","points":[[0,0],[100,0],[100,100]],"width":100,"height":100,"roundness":{"type":2},"roughness":0,"seed":1,"updated":0}),
            json!({"id":"bad","type":"rectangle","width":80,"height":80,"fillStyle":"hachure","backgroundColor":"red","roughness":0,"seed":1,"updated":0}),
            json!({"id":"bad","type":"freedraw","width":10,"height":10,"points":[[90,90],[95,95]],"strokeWidth":20,"simulatePressure":false,"pressures":[1,1],"roughness":0,"seed":1,"updated":0}),
        ] {
            let request = request(format, 100.0);
            let source = scene(json!([bad]));
            let first = Renderer::new().render(&source, &request).unwrap();
            let second = Renderer::new().render(&source, &request).unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(first.report(), second.report());
            let diagnostics = coordinates(&first);
            assert_eq!(diagnostics.len(), 1, "{bad} {format:?}");
            assert_eq!(
                diagnostics[0]["details"]["recovery"], "placeholder",
                "{bad} {format:?}"
            );
            assert!(
                diagnostics[0]["details"]["measurement"]["value"]
                    .as_f64()
                    .unwrap()
                    > 100.0
            );
        }
    }
}

#[test]
fn opposite_admitted_origins_cannot_escape_via_export_translation() {
    for format in FORMATS {
        let request = request(format, 100.0);
        let left = json!({"id":"left","type":"rectangle","x":-90,"width":10,"height":10,"roughness":0,"seed":1,"updated":0});
        let right = json!({"id":"right","type":"rectangle","x":90,"width":10,"height":10,"roughness":0,"seed":1,"updated":0});
        let actual = Renderer::new()
            .render(&scene(json!([left, right])), &request)
            .unwrap();
        let expected = Renderer::new()
            .render(&scene(json!([left])), &request)
            .unwrap();
        assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        let diagnostics = coordinates(&actual);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["element_id"], "right");
        assert_eq!(
            diagnostics[0]["details"]["measurement"],
            json!({"kind":"attempted","value":180})
        );
    }
}

#[test]
fn derived_frame_bounds_omit_the_clip_subtree_and_fail_direct_and_prepared_targets() {
    for format in FORMATS {
        for angle in [0.0, std::f64::consts::FRAC_PI_4] {
            let source = scene(json!([
                {"id":"child","type":"rectangle","x":80,"y":10,"width":5,"height":5,"frameId":"bad","roughness":0,"seed":1,"updated":0},
                {"id":"bad","type":"frame","x":75,"width":50,"height":50,"angle":angle,"updated":0}
            ]));
            let request = request(format, 100.0);
            let renderer = Renderer::new();
            let actual = renderer.render(&source, &request).unwrap();
            let empty = renderer.render(&scene(json!([])), &request).unwrap();
            assert_eq!(actual.artifact().bytes(), empty.artifact().bytes());
            assert_eq!(coordinates(&actual).len(), 1);
            let request = FrameRenderRequest::builder(format)
                .resource_limits(request.resource_limits().clone())
                .build()
                .unwrap();
            let frame = source.frames().next().unwrap();
            let direct = renderer.render_frame(frame, &request).unwrap_err();
            let prepared = renderer
                .render_frames(&source, [frame], &request)
                .unwrap()
                .next()
                .unwrap()
                .into_result()
                .unwrap_err();
            assert_eq!(direct.code().as_str(), "resource.coordinate-range-exceeded");
            assert_eq!(direct.json_pointer(), "/elements/1");
            assert_eq!(direct.descriptor(), prepared.descriptor());
            assert_eq!(direct.report(), prepared.report());
        }
    }
}

#[test]
fn crop_expansion_and_nonfinite_derived_values_recover_before_encoding() {
    for format in FORMATS {
        for crop_width in [1.0, 1e-320] {
            let source = scene(
                json!([{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","updated":0,
                "crop":{"x":0,"y":0,"width":crop_width,"height":100,"naturalWidth":100,"naturalHeight":100}}]),
            );
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .supplemental_assets(
                    SupplementalAssets::builder()
                        .asset(
                            "asset",
                            super::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]),
                        )
                        .build()
                        .unwrap(),
                )
                .resource_limits(
                    ResourceLimits::builder()
                        .max_coordinate_magnitude(100.0)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let outcome = Renderer::new().render(&source, &request).unwrap();
            let diagnostics = coordinates(&outcome);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0]["details"]["recovery"], "omitted");
            assert_eq!(
                diagnostics[0]["details"]["measurement"]["kind"],
                if crop_width == 1.0 {
                    "attempted"
                } else {
                    "at_least"
                }
            );
            assert!(
                diagnostics[0]["details"]["measurement"]["value"]
                    .as_f64()
                    .unwrap()
                    > 100.0
            );
        }
    }
}

#[test]
fn layout_glyph_positions_cannot_exceed_the_coordinate_ceiling() {
    for format in FORMATS {
        let source = scene(
            json!([{"id":"text","type":"text","width":10,"height":10,"text":"A\nA\nA","fontSize":50,"lineHeight":2,"fontFamily":2,"updated":0}]),
        );
        let outcome = Renderer::new()
            .render(&source, &request(format, 100.0))
            .unwrap();
        let diagnostics = coordinates(&outcome);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["details"]["recovery"], "omitted");
        assert_eq!(diagnostics[0]["element_id"], "text");
    }
}

#[test]
fn omitted_bound_text_does_not_leave_a_mask_or_expand_bounds() {
    for format in FORMATS {
        for late in [false, true] {
            let arrow = json!({"id":"arrow","type":"arrow","x":if late {0}else{-80},"width":10,"height":1,"points":[[0,0],[10,1]],"endArrowhead":null,"roughness":0,"seed":1,"updated":0,"boundElements":[{"id":"text","type":"text"}]});
            let text = json!({"id":"text","type":"text","x":0,"width":if late {10}else{100},"height":10,"containerId":"arrow","text":"A\nA\nA","fontSize":if late {50}else{10},"lineHeight":2,"fontFamily":2,"updated":0});
            let actual = Renderer::new()
                .render(&scene(json!([arrow, text])), &request(format, 100.0))
                .unwrap();
            let expected = Renderer::new()
                .render(&scene(json!([arrow])), &request(format, 100.0))
                .unwrap();
            assert_eq!(coordinates(&actual).len(), 1, "{format:?}, late={late}");
            assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
            if format == OutputFormat::Svg {
                assert!(!std::str::from_utf8(actual.artifact().bytes())
                    .unwrap()
                    .contains("<mask"));
            }
        }
    }
}

#[test]
fn rotated_viewport_extent_is_checked_before_frame_selection() {
    for format in FORMATS {
        let source = scene(
            json!([{"id":"frame","type":"frame","x":-40,"y":-40,"width":80,"height":80,"angle":std::f64::consts::FRAC_PI_4,"updated":0}]),
        );
        let request = FrameRenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render_frame(source.frames().next().unwrap(), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.coordinate-range-exceeded");
        assert_eq!(error.json_pointer(), "/elements/0");
    }
}

#[test]
fn doomed_text_cannot_displace_a_valid_sibling_and_aggregate_fatal_still_wins() {
    for format in FORMATS {
        let text = json!({"id":"text","type":"text","x":-90,"width":10,"height":10,"text":"A\nA\nA","fontSize":50,"lineHeight":2,"fontFamily":2,"updated":0});
        let rectangle = json!({"id":"rectangle","type":"rectangle","x":90,"width":10,"height":10,"roughness":0,"seed":1,"updated":0});
        let actual = Renderer::new()
            .render(&scene(json!([text, rectangle])), &request(format, 100.0))
            .unwrap();
        let expected = Renderer::new()
            .render(&scene(json!([rectangle])), &request(format, 100.0))
            .unwrap();
        assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        assert_eq!(coordinates(&actual).len(), 1);
        let constrained = RenderRequest::builder(format)
            .padding(0.0)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .max_geometry_segments_per_element(1)
                    .max_total_geometry_segments(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&scene(json!([text])), &constrained)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.total-geometry-exceeded");
    }
}

#[test]
fn nested_svg_paths_transforms_and_clip_subroots_obey_coordinate_limits() {
    for format in FORMATS {
        for content in [
            r#"<g transform="translate(60 0)"><path d="M60 0 L70 0 L70 10 Z"/></g>"#,
            r#"<path d="M80 0 l30 0 l0 10 Z"/>"#,
            r#"<polygon points="0,0 1e100,0 0,10"/>"#,
            r#"<defs><filter id="f"><feOffset dx="100"/></filter></defs><rect x="20" width="10" height="10" filter="url(#f)"/>"#,
            r#"<path d="M0 0 A1e100 1e100 0 1 0 10 10"/>"#,
            r#"<path d="M0 0 A1e-300 1 0 1 0 10 10"/>"#,
            r#"<svg width="10" height="10" viewBox="0 0 200 200"><rect width="5" height="5"/></svg>"#,
            r#"<defs><clipPath id="c" clipPathUnits="objectBoundingBox"><path d="M0 0L2 0L2 1L0 1Z"/></clipPath></defs><rect width="100" height="10" clip-path="url(#c)"/>"#,
            r#"<defs><clipPath id="c"><path d="M0 0 C0 120 10 120 10 0Z"/></clipPath></defs><path clip-path="url(#c)" d="M0 0L10 0L10 10Z"/>"#,
        ] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">{content}</svg>"#
            );
            let source = scene(
                json!([{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","updated":0}]),
            );
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .supplemental_assets(
                    SupplementalAssets::builder()
                        .asset("asset", svg.into_bytes())
                        .build()
                        .unwrap(),
                )
                .resource_limits(
                    ResourceLimits::builder()
                        .max_coordinate_magnitude(100.0)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let outcome = Renderer::new().render(&source, &request).unwrap();
            let diagnostics = coordinates(&outcome);
            assert_eq!(diagnostics.len(), 1, "{content} {format:?}");
            assert!(
                diagnostics[0]["details"]["measurement"]["value"]
                    .as_f64()
                    .unwrap()
                    > 100.0
            );
        }
    }
}

#[test]
fn rejected_nested_placeholder_does_not_escalate_to_parent_loss() {
    use base64::Engine;
    let child=br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0L120 0L120 10Z"/></svg>"#;
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="survivor" width="10" height="10" fill="green"/><image x="20" width="10" height="10" href="data:image/svg+xml;base64,{}"/></svg>"#,
        base64::engine::general_purpose::STANDARD.encode(child)
    );
    for format in FORMATS {
        let source = scene(
            json!([{"id":"image","type":"image","width":100,"height":100,"fileId":"asset","updated":0}]),
        );
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset("asset", parent.as_bytes().to_vec())
                    .build()
                    .unwrap(),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let outcome = Renderer::new().render(&source, &request).unwrap();
        let diagnostics = coordinates(&outcome);
        assert_eq!(diagnostics.len(), 1, "{format:?}");
        assert_eq!(diagnostics[0]["details"]["kind"], "nested_resource_limit");
        assert_eq!(diagnostics[0]["details"]["recovery"], "omitted");
    }
}

#[test]
fn intrinsic_image_coordinates_are_admitted_before_export_origin_selection() {
    for format in FORMATS {
        let bad = json!({"id":"image","type":"image","x":-90,"width":10,"height":10,"fileId":"asset","updated":0});
        let good = json!({"id":"rectangle","type":"rectangle","x":90,"width":10,"height":10,"roughness":0,"seed":1,"updated":0});
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset(
                        "asset",
                        super::common::codec_fixtures::solid_png(200, 1, [255, 0, 0, 255]),
                    )
                    .build()
                    .unwrap(),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let actual = Renderer::new()
            .render(&scene(json!([bad, good])), &request)
            .unwrap();
        let expected = Renderer::new()
            .render(&scene(json!([good])), &request)
            .unwrap();
        assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        assert_eq!(coordinates(&actual).len(), 1);
    }
}
