//! Final surface owners: type discrimination, semantic links, local SVG resources.
use base64::Engine;
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
fn scene() -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"frame","type":"frame","width":250,"height":140,"updated":0},
    {"id":"subject","type":"rectangle","x":10,"y":10,"width":200,"height":80,"seed":1,"updated":0},
    {"id":"other","type":"magicframe","x":400,"width":100,"height":100,"updated":0}],"appState":{},"files":{}})
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
        json!(1.5),
        json!(""),
        json!([]),
        json!({}),
    ]
}
fn diagnostic(code: &str, effects: Value, pointer: &str, recovery: &str, recipe: Value) -> Value {
    json!({"code":code,"severity":"warning","effects":effects,"element_id":"subject","json_pointer":pointer,"details":{"kind":"compatibility","recovery":recovery,"recipe_id":recipe}})
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
    let renderer = Renderer::new();
    let request = RenderRequest::builder(format).build().unwrap();
    let first = renderer.render(&document, &request).unwrap();
    let second = Renderer::new().render(&document, &request).unwrap();
    let baseline = renderer.render(&clean_document, &request).unwrap();
    assert_eq!(first.artifact().bytes(), baseline.artifact().bytes());
    assert_report(&first, format, rows.clone());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let metadata = RenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let embedded = renderer.render(&document, &metadata).unwrap();
    assert_report(&embedded, format, rows.clone());
    assert_eq!(
        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
        serde_json::to_vec(source).unwrap()
    );
    let request = FrameRenderRequest::builder(format)
        .metadata_options(MetadataOptions::new(true, false, false))
        .build()
        .unwrap();
    let frames: Vec<_> = document.frames().collect();
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for (index, frame) in frames.iter().enumerate() {
        let direct = renderer.render_frame(*frame, &request).unwrap();
        let replay = batch.next().unwrap().into_result().unwrap();
        assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
        assert_eq!(direct.report(), replay.report());
        assert_report(
            &direct,
            format,
            if index == 0 && source["elements"][1].get("width").is_some() {
                rows.clone()
            } else {
                json!([])
            },
        );
        assert_eq!(
            super::artifact_metadata::extract(format, direct.artifact().bytes()).1,
            serde_json::to_vec(source).unwrap()
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
fn type_discrimination_preserves_known_unknown_and_malformed_occurrences() {
    super::compatibility_fallback_render::unknown_element_renders_the_exact_transformed_svg_placeholder();
    super::diagnostic_contract::unknown_elements_cover_bounded_placeholders_and_unbounded_omissions(
    );
    super::diagnostic_contract::malformed_elements_cover_bounded_placeholders_and_unbounded_omissions();
    for value in std::iter::once(None).chain(
        values()
            .into_iter()
            .chain([json!("future"), json!("Rectangle")])
            .map(Some),
    ) {
        for bounded in [false, true] {
            let mut source = scene();
            if let Some(value) = &value {
                source["elements"][1]["type"] = value.clone();
            } else {
                source["elements"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("type");
            }
            if !bounded {
                source["elements"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("width");
                source["elements"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("height");
            }
            let unknown = value
                .as_ref()
                .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
            let code = if unknown {
                "compatibility.unsupported-element-type"
            } else {
                "compatibility.malformed-element"
            };
            let recipe = if unknown {
                "renderer.unknown-element.v1"
            } else {
                "renderer.malformed-element.v1"
            };
            let mut rows = vec![diagnostic(
                code,
                if unknown {
                    json!(["visual", "semantic"])
                } else {
                    json!(["visual"])
                },
                "/elements/1",
                if bounded { "placeholder" } else { "omitted" },
                if bounded { json!(recipe) } else { Value::Null },
            )];
            if bounded {
                rows.push(json!({"code":"font.liberation-sans-substituted","severity":"info","effects":["visual"],"element_id":"subject","json_pointer":"/elements/1","details":{"kind":"compatibility","recovery":"substituted","recipe_id":null}}));
            }
            for format in FORMATS {
                assert_contract(&source, &source, format, json!(rows));
                let request = RenderRequest::builder(format).build().unwrap();
                let result = Renderer::new().render(&parse(&source), &request).unwrap();
                if !bounded {
                    let mut omitted = source.clone();
                    omitted["elements"].as_array_mut().unwrap().remove(1);
                    let baseline = Renderer::new().render(&parse(&omitted), &request).unwrap();
                    assert_eq!(result.artifact().bytes(), baseline.artifact().bytes());
                } else {
                    match format {
                        OutputFormat::Svg => {
                            let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
                            assert!(svg.contains("width=\"200\" height=\"80\" fill=\"#f1f3f5\""));
                            assert!(svg.contains("M0 0 L200 80 M200 0 L0 80"));
                            assert!(svg.contains(if unknown {
                                "Unsupported:"
                            } else {
                                "Malformed element"
                            }));
                        }
                        OutputFormat::Png => {
                            let png = crate::common::raster::decode_png(result.artifact().bytes());
                            assert_eq!(png.pixel(40, 70), [241, 243, 245, 255]);
                        }
                        OutputFormat::Pdf => {
                            let pdf =
                                crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
                            assert!(pdf.contains("0 0 m\n200 0 l\n200 80 l\n0 80 l\nh\n"));
                            assert!(pdf.contains("[6 4] 0 d"));
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
    for kind in [
        "rectangle",
        "diamond",
        "ellipse",
        "line",
        "arrow",
        "freedraw",
        "draw",
        "text",
        "image",
        "iframe",
        "embeddable",
        "selection",
    ] {
        let mut source = scene();
        source["elements"][1]["type"] = json!(kind);
        if matches!(kind, "line" | "arrow" | "freedraw" | "draw") {
            source["elements"][1]["points"] = json!([[0, 0], [100, 40], [200, 0]]);
        }
        if kind == "text" {
            source["elements"][1]["text"] = json!("A");
            source["elements"][1]["fontFamily"] = json!(3);
            source["elements"][1]["fontSize"] = json!(20);
            source["elements"][1]["lineHeight"] = json!(1.2);
        }
        if kind == "image" {
            source["elements"][1]["fileId"] = json!("asset");
            let png = crate::common::codec_fixtures::png_rgba(1, 1, &[[255, 0, 0, 255]]);
            source["files"] = json!({"asset":{"id":"asset","created":0,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png))}});
        }
        for format in FORMATS {
            assert_contract(&source, &source, format, json!([]));
        }
    }
}

#[test]
fn links_have_exact_semantic_recovery_canonical_urls_and_png_silence() {
    pdf_link_regions_follow_rotation_and_frame_clipping();
    super::compatibility_fallback_render::pinned_link_sanitizer_preserves_its_safe_canonical_value_and_omits_active_actions();
    super::compatibility_fallback_render::supported_linked_elements_keep_navigation_in_semantic_formats();
    super::compatibility_fallback_render::pdf_link_annotations_are_ordered_from_topmost_to_bottommost();
    let clean = scene();
    for value in std::iter::once(None).chain(
        values()
            .into_iter()
            .chain([
                json!("javascript:alert(1)"),
                json!("data:text/html,unsafe"),
                json!("vbscript:unsafe"),
            ])
            .map(Some),
    ) {
        let mut source = clean.clone();
        if let Some(value) = &value {
            source["elements"][1]["link"] = value.clone();
        }
        for format in FORMATS {
            let invalid = value
                .as_ref()
                .is_some_and(|v| !v.is_null() && !v.is_string());
            let unsafe_link = value.as_ref().is_some_and(Value::is_string);
            let rows = if format == OutputFormat::Png || (!invalid && !unsafe_link) {
                json!([])
            } else {
                json!([diagnostic(
                    if invalid {
                        "compatibility.unsupported-property-value"
                    } else {
                        "link.unsafe-omitted"
                    },
                    json!(["semantic"]),
                    "/elements/1/link",
                    "omitted",
                    Value::Null
                )])
            };
            assert_contract(&source, &clean, format, rows);
        }
    }
    for (url, canonical, pdf_url) in [
        (
            "  https://example.com/a\tb  ",
            "https://example.com/ab",
            "https://example.com/ab",
        ),
        ("/docs", "/docs", "http://reference.invalid/docs"),
        (
            "mailto:a@example.com",
            "mailto:a@example.com",
            "mailto:a@example.com",
        ),
    ] {
        let mut source = clean.clone();
        source["elements"][1]["link"] = json!(url);
        let mut expected = clean.clone();
        expected["elements"][1]["link"] = json!(canonical);
        for format in FORMATS {
            assert_contract(&source, &expected, format, json!([]));
            let result = Renderer::new()
                .render(
                    &parse(&source),
                    &RenderRequest::builder(format).build().unwrap(),
                )
                .unwrap();
            match format {
                OutputFormat::Svg => assert!(std::str::from_utf8(result.artifact().bytes())
                    .unwrap()
                    .contains(&format!("<a href=\"{canonical}\">"))),
                OutputFormat::Pdf => assert!(crate::common::pdf_inspect::pdf_text(
                    result.artifact().bytes()
                )
                .contains(&format!("/URI ({pdf_url})"))),
                OutputFormat::Png => {
                    let plain = Renderer::new()
                        .render(
                            &parse(&clean),
                            &RenderRequest::builder(format).build().unwrap(),
                        )
                        .unwrap();
                    assert_eq!(result.artifact().bytes(), plain.artifact().bytes());
                }
                _ => unreachable!(),
            }
        }
    }
}

fn pdf_link_regions_follow_rotation_and_frame_clipping() {
    let source = json!({"type":"excalidraw","version":2,"elements":[
        {"id":"frame","type":"frame","width":100,"height":100,"updated":0},
        {"id":"link","type":"rectangle","x":80,"y":20,"width":40,"height":20,"angle":std::f64::consts::FRAC_PI_2,"frameId":"frame","link":"https://example.com/visible","seed":1,"updated":0},
        {"id":"gone","type":"rectangle","x":150,"y":20,"width":20,"height":20,"frameId":"frame","link":"https://example.com/gone","seed":1,"updated":0}],"appState":{},"files":{}});
    let document = parse(&source);
    let request = FrameRenderRequest::builder(OutputFormat::Pdf)
        .build()
        .unwrap();
    let result = Renderer::new()
        .render_frame(document.frames().next().unwrap(), &request)
        .unwrap();
    let pdf = crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
    assert_eq!(pdf.matches("/Subtype /Link").count(), 1);
    assert!(pdf.contains("/Border [0 0 0]"));
    assert!(!pdf.contains("https://example.com/gone"));
    let array = |key: &str| -> Vec<f64> {
        pdf.split_once(key)
            .unwrap()
            .1
            .split_once(']')
            .unwrap()
            .0
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect()
    };
    // The quadrilaterals' bounds, widened by the 0.001 pt Krilla adds so
    // Acrobat does not ignore quad points on the rectangle's edge (ADR-0021).
    let rect = array("/Rect [");
    let expected = [67.499, 37.499, 75.001, 67.501];
    assert_eq!(rect.len(), 4);
    for (a, b) in rect.iter().zip(expected) {
        assert!((a - b).abs() < 1e-5, "Rect {rect:?}");
    }
    let quad = array("/QuadPoints [");
    assert_eq!(quad.len(), 8);
    // PDF quadrilateral vertices are top-left/top-right/bottom-left/bottom-right
    // in the rotated source traversal, after clipping to the frame's right edge.
    for (actual, expected) in quad
        .iter()
        .zip([75.0, 67.5, 75.0, 37.5, 67.5, 67.5, 67.5, 37.5])
    {
        assert!((actual - expected).abs() < 1e-9, "QuadPoints {quad:?}");
    }
    let mut overlap = source.clone();
    overlap["elements"][2] = overlap["elements"][1].clone();
    overlap["elements"][2]["id"] = json!("top");
    overlap["elements"][2]["link"] = json!("https://example.com/top");
    let document = parse(&overlap);
    let result = Renderer::new()
        .render_frame(document.frames().next().unwrap(), &request)
        .unwrap();
    let pdf = crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
    let refs = pdf
        .split_once("/Annots [")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0
        .split_whitespace()
        .collect::<Vec<_>>();
    assert_eq!(refs.len(), 6);
    for (reference, url) in refs
        .chunks_exact(3)
        .zip(["https://example.com/top", "https://example.com/visible"])
    {
        assert_eq!(&reference[1..], &["0", "R"]);
        let marker = format!("\n{} 0 obj\n", reference[0]);
        let object = pdf
            .split_once(&marker)
            .unwrap()
            .1
            .split_once("endobj")
            .unwrap()
            .0;
        assert!(object.contains(&format!("/URI ({url})")));
    }
}

#[test]
fn nested_svg_resource_contract_is_local_bounded_source_located_and_preserved() {
    super::nested_svg_asset_render::assert_local_resource_contract();
    super::diagnostic_contract::unsafe_svg_categories_follow_static_effects();
    super::diagnostic_contract::unsafe_svg_attributes_and_safe_styles_are_explicit();
    super::diagnostic_contract::unsafe_nested_siblings_sort_by_full_source_path();
    super::diagnostic_contract::nested_failure_and_removal_share_one_source_order();
    super::diagnostic_contract::unsafe_svg_removals_have_exact_effects_and_silence();
    super::diagnostic_contract::nested_event_handler_removal_keeps_its_full_path_and_semantic_effect();
    super::diagnostic_contract::decode_failures_cover_every_codec_and_authority();
    super::diagnostic_contract::unsupported_image_contract_in_every_format();
    for (svg, unsafe_content) in [
        (
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><defs><path id="local" d="M0 0H40V30H0Z"/></defs><use href="#local" fill="red"/></svg>"##,
            false,
        ),
        (
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect width="40" height="30" fill="red" onload="bad()"/></svg>"##,
            true,
        ),
    ] {
        let mut source = scene();
        source["elements"][1]["type"] = json!("image");
        source["elements"][1]["fileId"] = json!("outer/~");
        source["files"] = json!({"outer/~":{"id":"outer/~","created":0,"dataURL":format!("data:image/svg+xml;base64,{}",base64::engine::general_purpose::STANDARD.encode(svg))}});
        let mut clean = source.clone();
        clean["files"]["outer/~"]["dataURL"] = json!(format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg.replace(" onload=\"bad()\"", ""))
        ));
        let rows = if !unsafe_content {
            json!([])
        } else {
            json!([{"code":"image.unsafe-svg-content-removed","severity":"warning","effects":["semantic"],"element_id":null,"json_pointer":"/files/outer~1~0/dataURL","details":{"kind":"nested_source","asset_chain":[{"asset_id":"outer/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":3,"name":"onload"}]}],"recovery":"removed"}}])
        };
        for format in FORMATS {
            assert_contract(&source, &clean, format, rows.clone());
            if !unsafe_content {
                let result = Renderer::new()
                    .render(
                        &parse(&source),
                        &RenderRequest::builder(format).build().unwrap(),
                    )
                    .unwrap();
                match format {
                    OutputFormat::Svg => {
                        let svg = std::str::from_utf8(result.artifact().bytes()).unwrap();
                        let encoded = svg
                            .split_once("data:image/svg+xml;base64,")
                            .unwrap()
                            .1
                            .split('"')
                            .next()
                            .unwrap();
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(encoded)
                            .unwrap();
                        let nested = std::str::from_utf8(&bytes).unwrap();
                        let xml = roxmltree::Document::parse(nested).unwrap();
                        let use_node = xml.descendants().find(|n| n.has_tag_name("use")).unwrap();
                        let reference = use_node
                            .attribute("href")
                            .or_else(|| {
                                use_node.attribute(("http://www.w3.org/1999/xlink", "href"))
                            })
                            .unwrap();
                        let id = reference.strip_prefix('#').unwrap();
                        assert!(xml
                            .descendants()
                            .any(|n| n.attribute("id") == Some(id) && n.has_tag_name("path")));
                    }
                    OutputFormat::Png => {
                        let png = crate::common::raster::decode_png(result.artifact().bytes());
                        assert_eq!(png.pixel(100, 70), [255, 0, 0, 255]);
                    }
                    OutputFormat::Pdf => {
                        let pdf = crate::common::pdf_inspect::pdf_text(result.artifact().bytes());
                        assert!(pdf.contains("1 0 0 rg") || pdf.contains("1 0 0 scn"));
                        assert!(!pdf.contains("/Subtype /Image"));
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}
