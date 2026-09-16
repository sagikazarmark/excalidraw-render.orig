//! Exact compatibility-recovery reports and independently authored drawing views.
use excalidraw_render::{
    FrameOptions, FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest,
    Renderer, ResourceLimits, SceneDocument,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
fn scene(elements: Value) -> Value {
    json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":{}})
}
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn request(format: OutputFormat) -> RenderRequest {
    RenderRequest::builder(format)
        .frame_options(FrameOptions::new(true, true, false, true))
        .build()
        .unwrap()
}
fn row(
    code: &str,
    effects: Value,
    id: Value,
    pointer: &str,
    recovery: &str,
    recipe: Value,
) -> Value {
    json!({"code":code,"severity":"warning","effects":effects,"element_id":id,"json_pointer":pointer,"details":{"kind":"compatibility","recovery":recovery,"recipe_id":recipe}})
}
fn report(report: &excalidraw_render::CompatibilityReport, format: OutputFormat, mut rows: Value) {
    let mut actual = serde_json::to_value(report).unwrap();
    for row in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(row["message"].as_str().is_some_and(|s| !s.is_empty()));
        row.as_object_mut().unwrap().remove("message");
    }
    for row in rows.as_array_mut().unwrap() {
        row["format"] = json!(format.as_str());
    }
    assert_eq!(actual, json!({"schema_version":1,"diagnostics":rows}));
}
fn checked(source: &Value, request: &RenderRequest, rows: Value) -> RenderOutcome {
    let document = parse(source);
    let first = Renderer::new().render(&document, request).unwrap();
    let second = Renderer::new().render(&document, request).unwrap();
    assert_eq!(
        first.report().diagnostics().len(),
        rows.as_array().unwrap().len(),
        "{source}"
    );
    report(first.report(), request.format(), rows);
    assert_eq!(first.report(), second.report());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    first
}
fn malformed(id: Value, index: usize, bounded: bool, semantic: bool) -> Value {
    row(
        "compatibility.malformed-element",
        if semantic {
            json!(["visual", "semantic"])
        } else {
            json!(["visual"])
        },
        id,
        &format!("/elements/{index}"),
        if bounded { "placeholder" } else { "omitted" },
        if bounded {
            json!("renderer.malformed-element.v1")
        } else {
            Value::Null
        },
    )
}

#[test]
fn malformed_element_contract_covers_recovery_and_semantic_loss() {
    for format in FORMATS {
        for kind in [
            None,
            Some(Value::Null),
            Some(json!("")),
            Some(json!(false)),
            Some(json!(17)),
            Some(json!([])),
            Some(json!({"nested":"opaque"})),
        ] {
            for bounded in [false, true] {
                for link in [
                    Value::Null,
                    json!("https://example.com/kept"),
                    json!("javascript:never()"),
                ] {
                    let mut element = json!({"id":"malformed","updated":1,"link":link,"opaque":{"type":"text","fontFamily":"invalid"}});
                    if let Some(kind) = &kind {
                        element["type"] = kind.clone();
                    }
                    if bounded {
                        element["width"] = json!(10);
                        element["height"] = json!(10);
                    }
                    let source = scene(json!([element]));
                    let mut rows = vec![malformed(
                        json!("malformed"),
                        0,
                        bounded,
                        !bounded
                            && format != OutputFormat::Png
                            && link == "https://example.com/kept",
                    )];
                    if format != OutputFormat::Png && link == "javascript:never()" {
                        rows.push(row(
                            "link.unsafe-omitted",
                            json!(["semantic"]),
                            json!("malformed"),
                            "/elements/0/link",
                            "omitted",
                            Value::Null,
                        ));
                    }
                    let actual = checked(&source, &request(format), json!(rows.clone()));
                    let oracle = if bounded {
                        let mut oracle = source.clone();
                        oracle["elements"][0]["type"] = json!("future");
                        oracle
                    } else {
                        scene(json!([]))
                    };
                    let reference = Renderer::new()
                        .render(&parse(&oracle), &request(format))
                        .unwrap();
                    assert_eq!(actual.artifact().bytes(), reference.artifact().bytes());
                    if bounded && link == "https://example.com/kept" {
                        match format {
                            OutputFormat::Svg => {
                                assert!(std::str::from_utf8(actual.artifact().bytes())
                                    .unwrap()
                                    .contains("<a href=\"https://example.com/kept\">"))
                            }
                            OutputFormat::Pdf => assert!(crate::common::pdf_inspect::pdf_text(
                                actual.artifact().bytes()
                            )
                            .contains("/URI (https://example.com/kept)")),
                            OutputFormat::Png => {}
                            _ => unreachable!(),
                        }
                    }
                    let metadata = RenderRequest::builder(format)
                        .metadata_options(MetadataOptions::new(true, false, false))
                        .build()
                        .unwrap();
                    let embedded = checked(&source, &metadata, json!(rows));
                    assert_eq!(
                        super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
                        serde_json::to_vec(&source).unwrap()
                    );
                    let mut deleted = source;
                    deleted["elements"][0]["isDeleted"] = json!(true);
                    checked(&deleted, &request(format), json!([]));
                }
            }
        }
        let values = json!([
            null,
            false,
            23,
            "https://example.invalid/not-an-element",
            [],
            [{}]
        ]);
        let rows: Vec<_> = (0..values.as_array().unwrap().len())
            .map(|i| malformed(Value::Null, i, false, false))
            .collect();
        checked(&scene(values), &request(format), json!(rows));
    }
    malformed_frame_and_report_prefix();
    malformed_label_and_transform_oracle();
    malformed_transform_opacity_and_clip_match_authored_geometry();
    malformed_frame_rejection_reports_lost_navigation();
}

#[test]
fn malformed_frame_rejection_reports_lost_navigation() {
    for format in FORMATS {
        for link in [
            Value::Null,
            json!("https://example.com/kept"),
            json!("javascript:never()"),
        ] {
            let source = scene(json!([
                {"id":"frame","type":"frame","x":21,"width":10,"height":10,"updated":1},
                {"id":"child","type":null,"width":5,"height":5,"frameId":"frame","link":link,"updated":1}
            ]));
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_coordinate_magnitude(20.0)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let mut rows = vec![
                json!({"code":"resource.coordinate-range-exceeded","severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])}else{json!(["visual","semantic"])},"element_id":"frame","json_pointer":"/elements/0/x","details":{"kind":"resource_limit","field":"max_coordinate_magnitude","configured":20,"value_kind":"coordinate","measurement":{"kind":"attempted","value":21},"recovery":"omitted"}}),
                malformed(
                    json!("child"),
                    1,
                    false,
                    format != OutputFormat::Png && link == "https://example.com/kept",
                ),
            ];
            if format != OutputFormat::Png && link == "javascript:never()" {
                rows.push(row(
                    "link.unsafe-omitted",
                    json!(["semantic"]),
                    json!("child"),
                    "/elements/1/link",
                    "omitted",
                    Value::Null,
                ));
            }
            let actual = checked(&source, &request, json!(rows));
            let empty = checked(&scene(json!([])), &request, json!([]));
            assert_eq!(actual.artifact().bytes(), empty.artifact().bytes());
        }
    }
}

fn malformed_frame_and_report_prefix() {
    let source = scene(json!([
        {"id":"z","type":null,"x":5,"y":5,"width":10,"height":10,"updated":1},
        {"id":"a","type":false,"x":20,"y":5,"width":10,"height":10,"updated":1},
        {"id":"frame","type":"frame","width":40,"height":30,"updated":1},
        {"id":"other","type":"frame","x":100,"width":40,"height":30,"updated":1}
    ]));
    for format in FORMATS {
        let rows = json!([
            malformed(json!("z"), 0, true, false),
            malformed(json!("a"), 1, true, false)
        ]);
        checked(&source, &request(format), rows.clone());
        let document = parse(&source);
        let frames: Vec<_> = document.frames().collect();
        let renderer = Renderer::new();
        let request = FrameRenderRequest::builder(format).build().unwrap();
        let mut batch = renderer
            .render_frames(&document, frames.iter().copied(), &request)
            .unwrap();
        for (i, frame) in frames.into_iter().enumerate() {
            let direct = renderer.render_frame(frame, &request).unwrap();
            let prepared = batch.next().unwrap().into_result().unwrap();
            report(
                direct.report(),
                format,
                if i == 0 { rows.clone() } else { json!([]) },
            );
            assert_eq!(direct.report(), prepared.report());
            assert_eq!(direct.artifact().bytes(), prepared.artifact().bytes());
        }
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_diagnostics(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = renderer.render(&document, &request).unwrap_err();
        assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
        report(error.report(), format, json!([rows[0]]));
    }
}

fn malformed_label_and_transform_oracle() {
    let source = scene(
        json!([{"id":"malformed","type":null,"x":10,"y":20,"width":240,"height":60,"angle":0.2,"opacity":50,"updated":1}]),
    );
    for format in FORMATS {
        let mut font = row(
            "font.liberation-sans-substituted",
            json!(["visual"]),
            json!("malformed"),
            "/elements/0",
            "substituted",
            Value::Null,
        );
        font["severity"] = json!("info");
        let actual = checked(
            &source,
            &request(format),
            json!([malformed(json!("malformed"), 0, true, false), font]),
        );
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(actual.artifact().bytes()).unwrap();
                assert!(svg.contains("width=\"240\" height=\"60\" fill=\"#f1f3f5\""));
                assert!(svg.contains("M0 0 L240 60 M240 0 L0 60"));
                assert!(svg.contains("Malformed element"));
                assert!(svg.contains("opacity=\"0.5\""));
            }
            OutputFormat::Pdf => {
                let pdf = crate::common::pdf_inspect::pdf_text(actual.artifact().bytes());
                assert!(pdf.contains("0 0 240 60 re"));
                assert!(pdf.contains("[6 4] 0 d"));
                assert!(
                    crate::common::pdf_inspect::pdf_actual_text(&pdf).contains("Malformed element")
                );
            }
            OutputFormat::Png => {
                assert!(crate::common::raster::decode_png(actual.artifact().bytes())
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[..3] != [255, 255, 255]))
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn malformed_transform_opacity_and_clip_match_authored_geometry() {
    // A quarter turn maps the source box to x=35..45,y=5..45. The inner
    // frame clips it to y=10..40, wholly inside its straight top/bottom edges.
    // Ten-pixel source height cannot fit a label. This oracle spells out §11.4
    // geometry directly, without routing another element through recovery.
    let source = scene(json!([
        {"id":"outer","type":"frame","width":80,"height":60,"updated":1},
        {"id":"inner","type":"frame","x":10,"y":10,"width":60,"height":30,"frameId":"outer","updated":1},
        {"id":"malformed","type":null,"x":20,"y":20,"width":40,"height":10,"angle":std::f64::consts::FRAC_PI_2,"opacity":50,"frameId":"inner","updated":1}
    ]));
    let oracle = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="60">
      <defs><clipPath id="frame"><rect x="10" y="10" width="60" height="30"/></clipPath>
      <clipPath id="source"><rect width="40" height="10"/></clipPath></defs>
      <rect width="80" height="60" fill="#ffffff"/>
      <g clip-path="url(#frame)"><g transform="matrix(0 1 -1 0 45 5)" opacity="0.5">
      <g clip-path="url(#source)"><rect width="40" height="10" fill="#f1f3f5"/>
      <rect width="40" height="10" fill="none" stroke="#868e96" stroke-width="2" stroke-dasharray="6 4"/>
      <path d="M0 0 L40 10 M40 0 L0 10" fill="none" stroke="#868e96" stroke-width="2"/>
      </g></g></g></svg>"##;
    let rasterize = |svg: &str| {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
        let mut pixels = tiny_skia::Pixmap::new(80, 60).unwrap();
        resvg::render(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        pixels
    };
    let expected = rasterize(oracle);
    let document = parse(&source);
    for format in FORMATS {
        let actual = Renderer::new()
            .render(
                &document,
                &RenderRequest::builder(format)
                    .padding(0.0)
                    .frame_options(FrameOptions::new(true, true, false, false))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        report(
            actual.report(),
            format,
            json!([malformed(json!("malformed"), 2, true, false)]),
        );
        let pixels = match format {
            OutputFormat::Svg => rasterize(std::str::from_utf8(actual.artifact().bytes()).unwrap())
                .data()
                .to_vec(),
            OutputFormat::Png => {
                let decoded = crate::common::raster::decode_png(actual.artifact().bytes());
                assert_eq!((decoded.width, decoded.height), (80, 60));
                decoded.rgba
            }
            OutputFormat::Pdf => {
                let pdf = crate::common::pdf_inspect::pdf_text(actual.artifact().bytes());
                let transform = pdf
                    .lines()
                    .find(|line| {
                        line.strip_suffix(" cm").is_some_and(|values| {
                            let values: Vec<f64> = values
                                .split_whitespace()
                                .map(|n| n.parse().unwrap())
                                .collect();
                            values.len() == 6
                                && values
                                    .iter()
                                    .zip([0.0, 1.0, -1.0, 0.0, 45.0, 5.0])
                                    .all(|(a, b)| (a - b).abs() < 1e-12)
                        })
                    })
                    .expect("quarter-turn source transform");
                let before_transform = pdf.split(transform).next().unwrap();
                let frame_clip = before_transform.rsplit("q\n").next().unwrap();
                assert!(frame_clip.starts_with("18 10 m\n62 10 l\n"));
                assert!(frame_clip.contains("18 40 l\n"));
                assert!(frame_clip.contains("10 18 l\n"));
                assert!(frame_clip.ends_with("W\nn\n"));
                assert!(pdf.contains("/GS1 gs\n/FB1 Do"));
                assert!(pdf.contains("0 0 40 10 re\nW\nn"), "{pdf}");
                assert!(pdf.contains("/ca 0.5"), "{pdf}");
                assert!(pdf.contains("[6 4] 0 d"));
                assert!(pdf.contains("0 0 m\n40 10 l\n40 0 m\n0 10 l"));
                assert!(!pdf.contains("/Subtype /Image"));
                continue;
            }
            _ => unreachable!(),
        };
        assert_eq!(pixels.len(), expected.data().len());
        // The direct painter and resvg quantize antialiased coverage separately.
        // Compare every channel within 3/255; unpainted regions and the flat
        // half-opacity fill additionally have exact independently calculated probes.
        for (i, (&actual, &expected)) in pixels.iter().zip(expected.data()).enumerate() {
            assert!(
                actual.abs_diff(expected) <= 3,
                "{format:?} channel {i}: {actual} != {expected}"
            );
        }
        for (x, y, rgba) in [
            (40, 12, [248, 249, 250, 255]),
            (40, 8, [255; 4]),
            (40, 42, [255; 4]),
            (33, 20, [255; 4]),
            (46, 20, [255; 4]),
        ] {
            let offset = (y * 80 + x) * 4;
            assert_eq!(&pixels[offset..offset + 4], &rgba, "{format:?} ({x},{y})");
        }
    }
}

#[test]
fn unsupported_value_contract_covers_every_current_field_family() {
    property_disposition_inventory_is_closed();
    mixed_property_recovery_order_and_suppression();
    // Each component compares full authored reports, clean-scene artifacts,
    // source preservation, scope and silent/inapplicable branches. Running this
    // exact selector therefore executes the evidence, not just source anchors.
    super::admission_identity_contract::unsupported_value_contract();
    super::geometry_style_contract::unsupported_value_contract();
    super::text_field_contract::unsupported_value_contract();
    super::image_field_contract::unsupported_value_contract();
    super::point_stroke_contract::unsupported_value_contract();
    super::relationship_field_contract::unsupported_value_contract();
    super::frame_field_contract::unsupported_value_contract();
    super::surface_metadata_contract::unsupported_value_contract();
    invalid_property_link_values();
    known_fallback_is_suppressed_by_exact_narrower_causes();
    field_omission_keeps_required_point_placeholder();
    payload_recovery_matches_exact_escaped_authority();
}

#[test]
fn payload_recovery_matches_exact_escaped_authority() {
    for format in FORMATS {
        for (id, pointer) in [
            ("a", "/files/a/dataURL"),
            ("a/b", "/files/a~1b/dataURL"),
            ("a~1b", "/files/a~01b/dataURL"),
            ("~/é/dataURL", "/files/~0~1é~1dataURL/dataURL"),
        ] {
            let mut source = scene(
                json!([{"id":"image","type":"image","fileId":id,"width":20,"height":10,"updated":1}]),
            );
            source["files"][id] = json!({"id":id,"dataURL":null,"created":1});
            let actual = checked(
                &source,
                &request(format),
                json!([row(
                    "compatibility.unsupported-property-value",
                    json!(["visual"]),
                    Value::Null,
                    pointer,
                    if format == OutputFormat::Png {
                        "placeholder"
                    } else {
                        "omitted"
                    },
                    Value::Null
                )]),
            );
            // A missing payload selects the same prescribed image recovery,
            // with its own different primary diagnostic rather than a generic box.
            source["files"] = json!({});
            let expected = Renderer::new()
                .render(&parse(&source), &request(format))
                .unwrap();
            assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        }
    }
}

#[test]
fn field_omission_keeps_required_point_placeholder() {
    for format in FORMATS {
        for kind in ["line", "arrow"] {
            for (field, binding) in [
                ("elementId", json!({"elementId":false,"gap":0})),
                ("gap", json!({"elementId":"target","gap":false})),
                ("focus", json!({"elementId":"target","gap":0,"focus":true})),
            ] {
                let source = scene(
                    json!([{"id":"subject","type":kind,"width":20,"height":10,"points":[[0,0],[null,10]],"startBinding":binding,"seed":1,"roughness":0,"updated":1}]),
                );
                let actual = checked(
                    &source,
                    &request(format),
                    json!([
                        row(
                            "compatibility.unsupported-property-value",
                            json!(["visual"]),
                            json!("subject"),
                            "/elements/0/points/1/0",
                            "placeholder",
                            json!("renderer.known-element-unavailable.v1")
                        ),
                        row(
                            "compatibility.unsupported-property-value",
                            json!(["input_repair", "visual"]),
                            json!("subject"),
                            &format!("/elements/0/startBinding/{field}"),
                            "omitted",
                            Value::Null
                        )
                    ]),
                );
                let oracle = scene(
                    json!([{"id":"subject","type":"future","width":20,"height":10,"updated":1}]),
                );
                let expected = Renderer::new()
                    .render(&parse(&oracle), &request(format))
                    .unwrap();
                assert_eq!(
                    actual.artifact().bytes(),
                    expected.artifact().bytes(),
                    "{format:?} {kind} {field}"
                );
            }
        }
    }
}

#[test]
fn known_fallback_is_suppressed_by_exact_narrower_causes() {
    for format in FORMATS {
        for kind in ["line", "arrow", "draw", "freedraw"] {
            for bounded in [false, true] {
                let source = scene(
                    json!([{"id":"subject","type":kind,"width":if bounded {20}else{0},"height":10,"points":[[0,0],[null,10]],"seed":1,"roughness":0,"updated":1}]),
                );
                let diagnostic = row(
                    "compatibility.unsupported-property-value",
                    json!(["visual"]),
                    json!("subject"),
                    "/elements/0/points/1/0",
                    if bounded { "placeholder" } else { "omitted" },
                    if bounded {
                        json!("renderer.known-element-unavailable.v1")
                    } else {
                        Value::Null
                    },
                );
                let actual = checked(&source, &request(format), json!([diagnostic]));
                let oracle = if bounded {
                    let mut oracle = source.clone();
                    oracle["elements"][0]["type"] = json!("future");
                    oracle
                } else {
                    scene(json!([]))
                };
                let expected = Renderer::new()
                    .render(&parse(&oracle), &request(format))
                    .unwrap();
                assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
            }
        }
    }
}

#[test]
fn known_kind_dispatch_and_image_failures_have_no_generic_fallback() {
    use base64::Engine;
    use std::collections::BTreeSet;
    const SUPPORTED: &[&str] = &[
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
    ];
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../tests/oracle/scene-surfaces/revision-1.json"
    ))
    .unwrap();
    let kinds: BTreeSet<_> = manifest["elementKinds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|kind| kind["status"] == "supported")
        .map(|kind| kind["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        SUPPORTED.iter().copied().collect(),
        "new supported kinds require a fresh reachability audit"
    );
    let png = crate::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]);
    for format in FORMATS {
        for kind in SUPPORTED.iter().copied().chain(["draw", "selection"]) {
            let mut element = json!({"id":"subject","type":kind,"width":80,"height":40,"seed":1,"roughness":0,"updated":1});
            if matches!(kind, "line" | "arrow" | "draw" | "freedraw") {
                element["points"] = json!([[0, 0], [80, 40]]);
            }
            if kind == "text" {
                element["text"] = json!("A");
                element["fontFamily"] = json!(5);
                element["fontSize"] = json!(10);
                element["lineHeight"] = json!(1.25);
            }
            let mut source = scene(json!([element]));
            if kind == "image" {
                source["elements"][0]["fileId"] = json!("asset");
                source["files"]["asset"] = json!({"id":"asset","created":1,"dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(&png))});
            }
            checked(&source, &request(format), json!([]));
        }
        for (payload, code, validated) in [
            (None, "image.missing-file", Value::Null),
            (
                Some("data:image/png;base64,bm90IGEgcGljdHVyZQ=="),
                "image.unsupported-format",
                Value::Null,
            ),
            (
                Some("data:image/svg+xml;base64,PHN2Zw=="),
                "image.decode-failed",
                json!("image/svg+xml"),
            ),
        ] {
            let mut source = scene(
                json!([{"id":"subject","type":"image","width":10,"height":10,"fileId":"asset","updated":1}]),
            );
            if let Some(payload) = payload {
                source["files"]["asset"] = json!({"id":"asset","created":1,"dataURL":payload});
            }
            let expected = json!({"code":code,"severity":"warning","effects":["visual"],"element_id":"subject","json_pointer":if payload.is_none(){"/elements/0/fileId"}else{"/files/asset/dataURL"},"details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":validated,"recovery":if format==OutputFormat::Png{"placeholder"}else{"omitted"},"recipe_id":if format==OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")}else{Value::Null}}});
            checked(&source, &request(format), json!([expected]));
        }
    }
}

fn invalid_property_link_values() {
    for format in FORMATS {
        for value in [json!(false), json!(17), json!([]), json!({})] {
            let source = scene(
                json!([{"id":"subject","type":"rectangle","width":20,"height":10,"seed":1,"updated":1,"link":value}]),
            );
            let rows = if format == OutputFormat::Png {
                json!([])
            } else {
                json!([row(
                    "compatibility.unsupported-property-value",
                    json!(["semantic"]),
                    json!("subject"),
                    "/elements/0/link",
                    "omitted",
                    Value::Null
                )])
            };
            let actual = checked(&source, &request(format), rows);
            let mut clean = source;
            clean["elements"][0]["link"] = Value::Null;
            let expected = Renderer::new()
                .render(&parse(&clean), &request(format))
                .unwrap();
            assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
        }
    }
}

#[test]
fn property_disposition_inventory_is_closed() {
    use std::collections::BTreeSet;
    // This table is the reviewed field inventory, not an expected-report
    // generator. Reports remain authored in the component tests above.
    let invalid = [
        (
            "app_state",
            &[
                "exportBackground",
                "exportEmbedScene",
                "exportScale",
                "exportWithDarkMode",
                "frameRendering",
            ][..],
        ),
        (
            "frame_rendering",
            &["enabled", "clip", "name", "outline"][..],
        ),
        ("file_record", &["id", "dataURL"][..]),
        (
            "element.common",
            &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "angle",
                "fillStyle",
                "strokeWidth",
                "strokeStyle",
                "roundness",
                "roughness",
                "opacity",
                "seed",
                "isDeleted",
                "groupIds",
                "frameId",
                "boundElements",
                "link",
                "strokeSharpness",
                "boundElementIds",
            ][..],
        ),
        (
            "element.text",
            &[
                "fontSize",
                "fontFamily",
                "text",
                "textAlign",
                "verticalAlign",
                "containerId",
                "originalText",
                "lineHeight",
                "font",
            ][..],
        ),
        (
            "element.linear",
            &["points", "startBinding", "endBinding"][..],
        ),
        ("element.line", &["startArrowhead", "endArrowhead"][..]),
        (
            "element.arrow",
            &["startArrowhead", "endArrowhead", "elbowed"][..],
        ),
        (
            "element.freedraw",
            &["points", "pressures", "simulatePressure"][..],
        ),
        ("element.image", &["fileId", "status", "scale", "crop"][..]),
        ("element.frame", &["name"][..]),
        ("element.magicframe", &["name"][..]),
        ("nested.roundness", &["type", "value"][..]),
        ("nested.bound_element", &["id", "type"][..]),
        ("nested.point", &["0", "1"][..]),
        ("nested.point_binding", &["elementId", "focus", "gap"][..]),
        (
            "nested.fixed_point_binding",
            &["elementId", "focus", "gap", "fixedPoint"][..],
        ),
        (
            "nested.image_crop",
            &["x", "y", "width", "height", "naturalWidth", "naturalHeight"][..],
        ),
    ];
    let missing = [
        ("file_record", &["id"][..]),
        ("nested.bound_element", &["id", "type"][..]),
        ("nested.point", &["0", "1"][..]),
        ("nested.point_binding", &["elementId", "gap"][..]),
        ("nested.fixed_point_binding", &["elementId", "gap"][..]),
        (
            "nested.image_crop",
            &["x", "y", "width", "height", "naturalWidth", "naturalHeight"][..],
        ),
    ];
    let mut expected = BTreeSet::new();
    for (branch, records) in [
        ("invalid", invalid.as_slice()),
        ("missing", missing.as_slice()),
    ] {
        for (record, fields) in records {
            for field in *fields {
                expected.insert(format!("{record}:/{field}:{branch}"));
            }
        }
    }
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../tests/oracle/scene-surfaces/revision-1.json"
    ))
    .unwrap();
    let mut actual = BTreeSet::new();
    let mut recoveries = BTreeSet::new();
    for field in manifest["fieldDispositions"].as_array().unwrap() {
        for branch in ["invalid", "missing"] {
            let disposition = &field[branch];
            if disposition["code"] != "compatibility.unsupported-property-value" {
                continue;
            }
            actual.insert(format!(
                "{}:{}:{branch}",
                field["recordKind"].as_str().unwrap(),
                field["path"].as_str().unwrap()
            ));
            if let Some(recovery) = disposition["recovery"].as_str() {
                recoveries.insert(recovery.to_owned());
            }
            if let Some(formats) = disposition["formatRules"].as_object() {
                for rule in formats.values() {
                    recoveries.insert(rule["recovery"].as_str().unwrap().to_owned());
                }
            }
        }
        for rule in field["valueRules"].as_array().unwrap() {
            assert!(
                !rule
                    .to_string()
                    .contains("compatibility.unsupported-property-value"),
                "new value-rule branch needs ownership evidence: {field}"
            );
        }
    }
    assert_eq!(
        actual, expected,
        "update the executable field evidence before extending its ownership"
    );
    assert_eq!(
        recoveries,
        ["omitted", "placeholder", "repaired", "substituted"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "ignored is vocabulary but has no current field binding for this code"
    );
}

#[test]
fn mixed_property_recovery_order_and_suppression() {
    let source=SceneDocument::parse(br#"{"type":"excalidraw","version":2,"elements":[
        {"id":"z","type":"rectangle","x":"bad","strokeStyle":"bad","width":20,"height":10,"roughness":0,"seed":1,"updated":1},
        {"id":"a","type":"line","points":[[0,0],[false,10]],"link":"javascript:never()","width":20,"height":10,"roughness":0,"seed":1,"updated":1},
        {"id":"malformed","type":null,"x":false,"strokeStyle":false,"width":20,"height":10,"updated":1}
    ],"appState":{"exportEmbedScene":17},"files":{}}"#).unwrap();
    for format in FORMATS {
        let mut rows = vec![
            row(
                "compatibility.unsupported-property-value",
                json!(["input_repair", "visual"]),
                json!("z"),
                "/elements/0/x",
                "substituted",
                Value::Null,
            ),
            row(
                "compatibility.unsupported-property-value",
                json!(["input_repair", "visual"]),
                json!("z"),
                "/elements/0/strokeStyle",
                "substituted",
                Value::Null,
            ),
            row(
                "compatibility.unsupported-property-value",
                json!(["visual"]),
                json!("a"),
                "/elements/1/points/1/0",
                "placeholder",
                json!("renderer.known-element-unavailable.v1"),
            ),
        ];
        if format != OutputFormat::Png {
            rows.push(row(
                "link.unsafe-omitted",
                json!(["semantic"]),
                json!("a"),
                "/elements/1/link",
                "omitted",
                Value::Null,
            ));
        }
        rows.push(malformed(json!("malformed"), 2, false, false));
        rows.push(row(
            "compatibility.unsupported-property-value",
            json!(["input_repair", "metadata"]),
            Value::Null,
            "/appState/exportEmbedScene",
            "substituted",
            Value::Null,
        ));
        let request = request(format);
        let first = Renderer::new().render(&source, &request).unwrap();
        let second = Renderer::new().render(&source, &request).unwrap();
        report(first.report(), format, json!(rows.clone()));
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        let limit = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_diagnostics(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new().render(&source, &limit).unwrap_err();
        assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
        // Surface classification precedes ordinary field normalization. Its
        // committed prefix is not a prefix of the final source-sorted array.
        let first = if format == OutputFormat::Png {
            malformed(json!("malformed"), 2, false, false)
        } else {
            row(
                "link.unsafe-omitted",
                json!(["semantic"]),
                json!("a"),
                "/elements/1/link",
                "omitted",
                Value::Null,
            )
        };
        report(error.report(), format, json!([first]));
    }
}
