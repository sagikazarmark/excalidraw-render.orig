//! Local resource diagnostics: independently authored complete reports and
//! boundary evidence, through the public scene/frame facade.
use base64::Engine;
use excalidraw_render::{
    FrameRenderRequest, MetadataOptions, OutputFormat, RenderOutcome, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};
use serde_json::{json, Value};

use crate::common::render::{trailing_work_charges, WorkCharge};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

#[derive(Clone, Copy)]
enum RasterLimit {
    Dimension,
    Pixels,
}

impl RasterLimit {
    fn contract(self) -> (&'static str, &'static str, u64, &'static str) {
        match self {
            Self::Dimension => (
                "resource.image-dimensions-exceeded",
                "max_raster_dimension",
                64,
                "dimension",
            ),
            Self::Pixels => (
                "resource.image-pixels-exceeded",
                "max_decoded_pixels_per_asset",
                128,
                "pixels",
            ),
        }
    }

    fn limits(self, configured: u64) -> ResourceLimits {
        match self {
            Self::Dimension => ResourceLimits::builder()
                .max_raster_dimension(configured as u32)
                .build()
                .unwrap(),
            Self::Pixels => ResourceLimits::builder()
                .max_decoded_pixels_per_asset(configured)
                .build()
                .unwrap(),
        }
    }
}

#[test]
fn image_dimension_contract_covers_codecs_recovery_and_scope() {
    raster_limit_suppression_and_recovery_envelope(RasterLimit::Dimension);
    raster_side_precedence_and_mixed_reference_recovery();
    raster_limit_contract(RasterLimit::Dimension);
    raster_limit_scope(RasterLimit::Dimension);
    nested_placeholder_artifact_oracle_for(false, Some(RasterLimit::Dimension), None);
    nested_raster_limits_preserve_exact_paths_and_local_recovery();
}

#[test]
fn image_pixel_contract_covers_codecs_recovery_and_scope() {
    raster_limit_suppression_and_recovery_envelope(RasterLimit::Pixels);
    raster_side_precedence_and_mixed_reference_recovery();
    raster_limit_contract(RasterLimit::Pixels);
    raster_limit_scope(RasterLimit::Pixels);
    nested_placeholder_artifact_oracle_for(false, Some(RasterLimit::Pixels), None);
    nested_raster_limits_preserve_exact_paths_and_local_recovery();
}

fn raster_limit_suppression_and_recovery_envelope(limit: RasterLimit) {
    let bytes = crate::common::codec_fixtures::solid_png(64, 2, [255, 0, 0, 255]);
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image width="20" height="20" href="{url}"/></svg>"#
    );
    let (code, field, n, kind) = limit.contract();
    for format in FORMATS {
        for work in [8191, 8192, 8193] {
            let limits = match limit {
                RasterLimit::Dimension => ResourceLimits::builder()
                    .max_raster_dimension(63)
                    .max_asset_work_units(work)
                    .build()
                    .unwrap(),
                RasterLimit::Pixels => ResourceLimits::builder()
                    .max_decoded_pixels_per_asset(127)
                    .max_asset_work_units(work)
                    .build()
                    .unwrap(),
            };
            let source = image_scene(parent.as_bytes(), "image/svg+xml");
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .resource_limits(limits)
                .build()
                .unwrap();
            let rows = if format == OutputFormat::Svg {
                json!([])
            } else {
                json!([{
                    "code":code,"severity":"warning","effects":["visual"],"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                    "details":{"kind":"nested_resource_limit","field":field,"configured":n-1,"value_kind":kind,"measurement":{"kind":"attempted","value":n},"asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":if work<8192 {"omitted"} else {"placeholder"},"recipe_id":if work<8192 {Value::Null} else {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")}}
                }])
            };
            let result = assert_repeat(&source, &request, rows);
            if work == 8191 && format != OutputFormat::Svg {
                let omitted = parent.replace(&format!(" href=\"{url}\""), "");
                let oracle = image_scene(omitted.as_bytes(), "image/svg+xml");
                let expected = Renderer::new()
                    .render(
                        &parse(&oracle),
                        &RenderRequest::builder(format).padding(0.0).build().unwrap(),
                    )
                    .unwrap();
                assert_eq!(result.artifact().bytes(), expected.artifact().bytes());
            }
        }
        for nested in [false, true] {
            let mut source = image_scene(
                if nested { parent.as_bytes() } else { &bytes },
                if nested { "image/svg+xml" } else { "image/png" },
            );
            source["elements"][0]["isDeleted"] = json!(true);
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .resource_limits(limit.limits(n - 1))
                .build()
                .unwrap();
            assert_repeat(&source, &request, json!([]));
        }
        let clean = crate::common::codec_fixtures::solid_png(1, 1, [0, 0, 255, 255]);
        let source = image_scene(&clean, "image/png");
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .resource_limits(limit.limits(n - 1))
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset("asset/~", bytes.clone())
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
fn raster_side_precedence_and_mixed_reference_recovery() {
    for format in FORMATS {
        for (width, height, attempted) in [(2, 64, 64), (64, 128, 64)] {
            let bytes = crate::common::codec_fixtures::solid_png(width, height, [255, 0, 0, 255]);
            for nested in [false, true] {
                let url = format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                );
                let svg = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image width="20" height="20" href="{url}"/></svg>"#
                );
                let mut source = image_scene(
                    if nested { svg.as_bytes() } else { &bytes },
                    if nested { "image/svg+xml" } else { "image/png" },
                );
                let mut second = source["elements"][0].clone();
                source["elements"][0]["status"] = json!("saved");
                second["id"] = json!("error");
                second["x"] = json!(30);
                second["status"] = json!("error");
                source["elements"].as_array_mut().unwrap().push(second);
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_raster_dimension(63)
                            .max_decoded_pixels_per_asset(127)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let rows = if format == OutputFormat::Svg {
                    json!([])
                } else if nested {
                    json!([{
                        "code":"resource.image-dimensions-exceeded","severity":"warning","effects":["visual"],"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                        "details":{"kind":"nested_resource_limit","field":"max_raster_dimension","configured":63,"value_kind":"dimension","measurement":{"kind":"attempted","value":attempted},"asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}
                    }])
                } else {
                    json!([expected(
                        "resource.image-dimensions-exceeded",
                        "max_raster_dimension",
                        json!(63),
                        json!(attempted),
                        "dimension",
                        "/files/asset~1~0/dataURL",
                        if format == OutputFormat::Png {
                            "placeholder"
                        } else {
                            "omitted"
                        }
                    )])
                };
                let actual = assert_repeat(&source, &request, rows);
                if !nested && format != OutputFormat::Svg {
                    source["files"] = json!({});
                    let missing = Renderer::new()
                        .render(
                            &parse(&source),
                            &RenderRequest::builder(format).padding(0.0).build().unwrap(),
                        )
                        .unwrap();
                    assert_eq!(
                        actual.artifact().bytes(),
                        missing.artifact().bytes(),
                        "mixed statuses retain independent recovery"
                    );
                }
            }
        }
    }
}

fn raster_limit_scope(limit: RasterLimit) {
    let bytes = crate::common::codec_fixtures::solid_png(64, 2, [255, 0, 0, 255]);
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image width="20" height="20" href="{url}"/></svg>"#
    );
    let (code, field, n, kind) = limit.contract();
    for format in FORMATS {
        for nested in [false, true] {
            for supplemental in [false, true] {
                let payload = if nested {
                    parent.as_bytes()
                } else {
                    bytes.as_slice()
                };
                let mut source = json!({"type":"excalidraw","version":2,"elements":[
                    {"id":"image-z","type":"image","width":20,"height":20,"fileId":"z/~","updated":0},
                    {"id":"image-a","type":"image","x":25,"width":20,"height":20,"fileId":"a","updated":0},
                    {"id":"repeat","type":"image","width":20,"height":20,"fileId":"z/~","updated":0}
                ],"appState":{},"files":{}});
                let assets = if supplemental {
                    SupplementalAssets::builder()
                        .asset("a", payload)
                        .asset("z/~", payload)
                        .asset("unused", payload)
                        .build()
                        .unwrap()
                } else {
                    let media = if nested { "image/svg+xml" } else { "image/png" };
                    for id in ["a", "z/~", "unused"] {
                        source["files"][id] = json!({"id":id,"created":0,"dataURL":format!("data:{media};base64,{}",base64::engine::general_purpose::STANDARD.encode(payload))});
                    }
                    SupplementalAssets::default()
                };
                let rows: Vec<_> = if format == OutputFormat::Svg {
                    vec![]
                } else {
                    [("a","image-a","/files/a/dataURL"),("z/~","image-z","/files/z~1~0/dataURL")].into_iter().map(|(id,element,pointer)| {
                        if nested {json!({"code":code,"severity":"warning","effects":["visual"],"element_id":null,"json_pointer":if supplemental {""} else {pointer},"details":{"kind":"nested_resource_limit","field":field,"configured":n-1,"value_kind":kind,"measurement":{"kind":"attempted","value":n},"asset_chain":[{"asset_id":id,"occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}})} else {
                            let mut row=expected(code,field,json!(n-1),json!(n),kind,if supplemental {""} else {pointer},if format==OutputFormat::Png {"placeholder"} else {"omitted"});
                            let _ = element; row["element_id"]=Value::Null; row
                        }
                    }).collect()
                };
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .resource_limits(limit.limits(n - 1))
                    .supplemental_assets(assets.clone())
                    .build()
                    .unwrap();
                assert_repeat(&source, &request, json!(rows));
                source["elements"].as_array_mut().unwrap().extend([
                    json!({"id":"frame","type":"frame","width":50,"height":30,"updated":0}),
                    json!({"id":"other","type":"frame","x":100,"width":50,"height":30,"updated":0}),
                ]);
                let document = parse(&source);
                let frames: Vec<_> = document.frames().collect();
                let renderer = Renderer::new();
                let frame_request = FrameRenderRequest::builder(format)
                    .resource_limits(limit.limits(n - 1))
                    .supplemental_assets(assets)
                    .build()
                    .unwrap();
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
            }
        }
    }
}

fn raster_limit_contract(limit: RasterLimit) {
    use crate::common::codec_fixtures as codec;
    let png = codec::solid_png(64, 2, [255, 0, 0, 255]);
    let codecs = [
        ("image/png", png.clone()),
        ("image/jpeg", codec::jpeg_rgb(64, 2, &[[255, 0, 0]; 128])),
        (
            "image/gif",
            codec::gif(&[codec::gif_frame(
                &[0; 128],
                64,
                2,
                &codec::RED_BLUE_PALETTE,
                None,
            )]),
        ),
        (
            "image/webp",
            codec::webp_rgba(64, 2, &[[255, 0, 0, 255]; 128]),
        ),
        ("image/bmp", codec::bmp24(64, 2, &[[255, 0, 0]; 128])),
        (
            "image/x-icon",
            codec::ico(&[
                (1, 1, 32, codec::solid_png(1, 1, [0, 0, 255, 255])),
                (64, 2, 32, png),
            ]),
        ),
    ];
    let (code, field, n, kind) = limit.contract();
    for format in FORMATS {
        for (mime, bytes) in &codecs {
            for configured in [n - 1, n, n + 1] {
                for status in ["saved", "error"] {
                    for supplemental in [false, true] {
                        let mut source = image_scene(bytes, mime);
                        source["elements"][0]["status"] = json!(status);
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
                            .resource_limits(limit.limits(configured))
                            .build()
                            .unwrap();
                        let rejected = configured < n && format != OutputFormat::Svg;
                        let rows = if rejected {
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
                        let result = assert_repeat(&source, &request, rows);
                        if rejected {
                            // Ordinary recovery must use the independently tested
                            // missing-file status recipe, including both PNG icons.
                            source["files"] = json!({});
                            let missing = Renderer::new()
                                .render(
                                    &parse(&source),
                                    &RenderRequest::builder(format).padding(0.0).build().unwrap(),
                                )
                                .unwrap();
                            assert_eq!(
                                result.artifact().bytes(),
                                missing.artifact().bytes(),
                                "{mime} {status}"
                            );
                        }
                    }
                }
            }
        }
    }
}
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn image_scene(bytes: &[u8], mime: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[
    {"id":"image","type":"image","x":5,"y":5,"width":20,"height":20,"fileId":"asset/~","updated":0}],
    "appState":{},"files":{"asset/~":{"id":"asset/~","created":0,"dataURL":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}}})
}
fn report(outcome: &RenderOutcome, format: OutputFormat, mut rows: Value) {
    exact_report(outcome.report(), format, &mut rows);
}

fn exact_report(
    report: &excalidraw_render::CompatibilityReport,
    format: OutputFormat,
    rows: &mut Value,
) {
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
fn expected(
    code: &str,
    field: &str,
    configured: Value,
    attempted: Value,
    kind: &str,
    pointer: &str,
    recovery: &str,
) -> Value {
    json!({"code":code,"severity":"warning","effects":["visual"],"element_id":if matches!(code,"resource.image-dimensions-exceeded"|"resource.image-pixels-exceeded") {Value::Null} else {json!("image")},"json_pointer":pointer,
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

// Independent visual oracles use an empty authored text box to retain the
// canvas footprint, or the specified generic placeholder through an unknown
// type. Neither invokes the resource rejection being tested.
fn box_oracle(
    format: OutputFormat,
    width: u64,
    height: u64,
    padding: f64,
    placeholder: bool,
) -> RenderOutcome {
    let mut source = text_scene("", width);
    source["elements"][0]["height"] = json!(height);
    if placeholder {
        source["elements"][0]["type"] = json!("future");
    }
    Renderer::new()
        .render(
            &parse(&source),
            &RenderRequest::builder(format)
                .padding(padding)
                .build()
                .unwrap(),
        )
        .unwrap()
}

fn canonical_nested_svg(text: &str) -> Value {
    fn node_value(node: roxmltree::Node<'_, '_>) -> Value {
        let attrs: serde_json::Map<String, Value> = node
            .attributes()
            .map(|attribute| {
                let value = if let Some(encoded) =
                    attribute.value().strip_prefix("data:image/svg+xml;base64,")
                {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .unwrap();
                    canonical_nested_svg(std::str::from_utf8(&bytes).unwrap())
                } else {
                    json!(attribute.value())
                };
                (attribute.name().to_owned(), value)
            })
            .collect();
        json!({"name":node.tag_name().name(),"attributes":attrs,"children":node.children().filter(|node|node.is_element()).map(node_value).collect::<Vec<_>>()})
    }
    node_value(roxmltree::Document::parse(text).unwrap().root_element())
}

fn embedded_svg_tree(bytes: &[u8]) -> Value {
    let document = roxmltree::Document::parse(std::str::from_utf8(bytes).unwrap()).unwrap();
    let url = document
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap()
        .attribute("href")
        .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(url.strip_prefix("data:image/svg+xml;base64,").unwrap())
        .unwrap();
    canonical_nested_svg(std::str::from_utf8(&bytes).unwrap())
}

fn pdf_drawing_streams(bytes: &[u8]) -> Vec<String> {
    crate::common::pdf_inspect::pdf_text(bytes)
        .split("\nstream\n")
        .skip(1)
        .filter_map(|part| {
            part.split_once("\nendstream")
                .map(|(body, _)| body.to_owned())
        })
        .filter(|body| body.contains(" cm\n") || body.contains(" rg\n"))
        .collect()
}

fn assert_nested_artifact_eq(actual: &RenderOutcome, expected: &RenderOutcome) {
    match actual.artifact().format() {
        OutputFormat::Svg => assert_eq!(
            embedded_svg_tree(actual.artifact().bytes()),
            embedded_svg_tree(expected.artifact().bytes())
        ),
        OutputFormat::Png => {
            let a = crate::common::raster::decode_png(actual.artifact().bytes());
            let b = crate::common::raster::decode_png(expected.artifact().bytes());
            assert_eq!((a.width, a.height), (b.width, b.height));
            assert_eq!(a.rgba, b.rgba);
        }
        OutputFormat::Pdf => assert_eq!(
            pdf_drawing_streams(actual.artifact().bytes()),
            pdf_drawing_streams(expected.artifact().bytes())
        ),
        _ => unreachable!("revision-1 formats"),
    }
}

#[derive(Clone, Copy)]
enum AssetLimit {
    Bytes,
    Work,
}

#[derive(Clone, Copy)]
enum ExtraNestedOracle {
    Work,
    Coordinate,
}

impl AssetLimit {
    fn contract(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Bytes => ("resource.asset-bytes-exceeded", "max_asset_bytes", "bytes"),
            Self::Work => (
                "resource.asset-work-exceeded",
                "max_asset_work_units",
                "work_units",
            ),
        }
    }
    fn limits(self, n: u64) -> ResourceLimits {
        match self {
            Self::Bytes => ResourceLimits::builder().max_asset_bytes(n),
            Self::Work => ResourceLimits::builder().max_asset_work_units(n),
        }
        .build()
        .unwrap()
    }
}

#[test]
fn asset_byte_contract_covers_admission_authority_recovery_and_frames() {
    top_level_asset_resource_boundaries_have_exact_reports_and_authority();
    resource_reports_are_scoped_and_replayed_for_independent_frames();
    asset_reference_scope_and_prefix(AssetLimit::Bytes);
    asset_byte_parent_admission_bounds_every_nested_payload();
    let bytes = crate::common::codec_fixtures::solid_png(2, 2, [255, 0, 0, 255]);
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_asset_bytes(bytes.len() as u64 - 1)
                    .max_total_asset_bytes(bytes.len() as u64 - 1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&image_scene(&bytes, "image/png")), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.total-asset-bytes-exceeded");
        exact_report(error.report(), format, &mut json!([]));
    }
}

#[test]
fn asset_work_contract_covers_intrinsics_nested_recovery_and_frames() {
    top_level_svg_work_exhaustion_uses_ordinary_image_recovery();
    nested_raster_work_rejection_has_exact_measurements_and_recovery_budget();
    asset_reference_scope_and_prefix(AssetLimit::Work);
    asset_intrinsic_work_boundaries();
    nested_svg_work_boundaries();
    nested_placeholder_artifact_oracle_for(false, None, Some(ExtraNestedOracle::Work));
    let bytes = crate::common::codec_fixtures::solid_png(2, 2, [255, 0, 0, 255]);
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_asset_work_units(1)
                    .max_element_work_units(1)
                    .max_total_work_units(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&image_scene(&bytes, "image/png")), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.total-work-exceeded");
        exact_report(error.report(), format, &mut json!([]));
    }
}

fn asset_reference_scope_and_prefix(limit: AssetLimit) {
    let bytes = crate::common::codec_fixtures::solid_png(2, 2, [255, 0, 0, 255]);
    let n = bytes.len() as u64;
    let (code, field, kind) = limit.contract();
    for format in FORMATS {
        for supplemental in [false, true] {
            for bounded in [false, true] {
                let mut source = json!({"type":"excalidraw","version":2,"elements":[
                    {"id":"zeta","type":"image","width":if bounded {20}else{0},"height":20,"fileId":"z/~","updated":0},
                    {"id":"alpha","type":"image","x":30,"width":20,"height":20,"fileId":"a","status":"error","updated":0},
                    {"id":"repeat","type":"image","x":60,"width":20,"height":20,"fileId":"z/~","updated":0},
                    {"id":"frame","type":"frame","width":80,"height":40,"updated":0},
                    {"id":"other","type":"frame","x":200,"width":80,"height":40,"updated":0}
                ],"appState":{},"files":{}});
                let url = format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                );
                let mut assets = SupplementalAssets::builder();
                for id in ["a", "unused", "z/~"] {
                    if supplemental {
                        assets = assets.asset(id, bytes.clone());
                    } else {
                        source["files"][id] = json!({"id":id,"created":0,"dataURL":url});
                    }
                }
                let assets = assets.build().unwrap();
                let rows: Vec<_> = [
                    ("alpha", "/files/a/dataURL"),
                    ("zeta", "/files/z~1~0/dataURL"),
                    ("repeat", "/files/z~1~0/dataURL"),
                ]
                .into_iter()
                .map(|(id, pointer)| {
                    let mut row = expected(
                        code,
                        field,
                        json!(n - 1),
                        json!(n),
                        kind,
                        if supplemental { "" } else { pointer },
                        if format == OutputFormat::Png {
                            "placeholder"
                        } else {
                            "omitted"
                        },
                    );
                    row["element_id"] = json!(id);
                    row
                })
                .collect();
                let request = RenderRequest::builder(format)
                    .supplemental_assets(assets.clone())
                    .resource_limits(limit.limits(n - 1))
                    .frame_options(
                        excalidraw_render::FrameOptions::default()
                            .with_enabled(false)
                            .with_clip(false)
                            .with_show_name(false)
                            .with_outline(false),
                    )
                    .build()
                    .unwrap();
                let actual = assert_repeat(&source, &request, json!(rows));
                let mut missing = source.clone();
                missing["files"] = json!({});
                let expected = Renderer::new()
                    .render(
                        &parse(&missing),
                        &RenderRequest::builder(format)
                            .frame_options(
                                excalidraw_render::FrameOptions::default()
                                    .with_enabled(false)
                                    .with_clip(false)
                                    .with_show_name(false)
                                    .with_outline(false),
                            )
                            .build()
                            .unwrap(),
                    )
                    .unwrap();
                assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
                let document = parse(&source);
                let frame_request = FrameRenderRequest::builder(format)
                    .supplemental_assets(assets)
                    .resource_limits(limit.limits(n - 1))
                    .build()
                    .unwrap();
                let frames: Vec<_> = document.frames().collect();
                let renderer = Renderer::new();
                let mut batch = renderer
                    .render_frames(&document, frames.iter().copied(), &frame_request)
                    .unwrap();
                for (index, frame) in frames.into_iter().enumerate() {
                    let direct = renderer.render_frame(frame, &frame_request).unwrap();
                    let prepared = batch.next().unwrap().into_result().unwrap();
                    report(
                        &direct,
                        format,
                        if index == 0 { json!(rows) } else { json!([]) },
                    );
                    assert_eq!(direct.report(), prepared.report());
                    assert_eq!(direct.artifact().bytes(), prepared.artifact().bytes());
                }
                for index in 0..3 {
                    source["elements"][index]["isDeleted"] = json!(true);
                }
                assert_repeat(&source, &request, json!([]));
            }
        }
        // A later required diagnostic cannot replace the already retained
        // first reference's resource warning with an empty report.
        let mut source = image_scene(&bytes, "image/png");
        source["elements"].as_array_mut().unwrap().push(json!({"id":"second","type":"image","width":20,"height":20,"fileId":"asset/~","updated":0}));
        let builder = match limit {
            AssetLimit::Bytes => ResourceLimits::builder().max_asset_bytes(n - 1),
            AssetLimit::Work => ResourceLimits::builder().max_asset_work_units(n - 1),
        };
        let request = RenderRequest::builder(format)
            .resource_limits(builder.max_diagnostics(1).build().unwrap())
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&source), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
        exact_report(
            error.report(),
            format,
            &mut json!([expected(
                code,
                field,
                json!(n - 1),
                json!(n),
                kind,
                "/files/asset~1~0/dataURL",
                if format == OutputFormat::Png {
                    "placeholder"
                } else {
                    "omitted"
                }
            )]),
        );
    }
}

fn asset_byte_parent_admission_bounds_every_nested_payload() {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    for format in FORMATS {
        for bounded in [false, true] {
            let parent = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="{}" height="2" href="{}"/></svg>"#,
                u8::from(bounded),
                svg_url(leaf)
            );
            let n = parent.len() as u64;
            // An accepted data child is strictly smaller than its enclosing
            // parent's payload (base64 expands it). The same local byte cap
            // therefore rejects the parent first, never this nested slot.
            assert!(parent.len() > leaf.len() + 1);
            for configured in [
                leaf.len() as u64 - 1,
                leaf.len() as u64,
                leaf.len() as u64 + 1,
                n - 1,
                n,
                n + 1,
            ] {
                let request = RenderRequest::builder(format)
                    .resource_limits(AssetLimit::Bytes.limits(configured))
                    .build()
                    .unwrap();
                let source = image_scene(parent.as_bytes(), "image/svg+xml");
                let actual = assert_repeat(
                    &source,
                    &request,
                    if configured < n {
                        json!([expected(
                            "resource.asset-bytes-exceeded",
                            "max_asset_bytes",
                            json!(configured),
                            json!(n),
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
                if configured < n {
                    let mut missing = source.clone();
                    missing["files"] = json!({});
                    let expected = Renderer::new()
                        .render(
                            &parse(&missing),
                            &RenderRequest::builder(format).build().unwrap(),
                        )
                        .unwrap();
                    assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
                }
            }
        }
    }
}

fn asset_intrinsic_work_boundaries() {
    let bytes = crate::common::codec_fixtures::solid_png(2, 2, [255, 0, 0, 255]);
    let validation = bytes.len() as u64;
    // Direct SVG validates the original raster; PNG/PDF select the five
    // subsequent payload passes and four units per decoded source pixel.
    for format in FORMATS {
        let n = if format == OutputFormat::Svg {
            validation
        } else {
            validation * 5 + 4 * 4
        };
        for configured in [n - 1, n, n + 1] {
            let source = image_scene(&bytes, "image/png");
            let request = RenderRequest::builder(format)
                .resource_limits(AssetLimit::Work.limits(configured))
                .build()
                .unwrap();
            assert_repeat(
                &source,
                &request,
                if configured < n {
                    json!([expected(
                        "resource.asset-work-exceeded",
                        "max_asset_work_units",
                        json!(configured),
                        json!(n),
                        "work_units",
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

fn nested_svg_work_boundaries() {
    // 1002 element nodes and five attributes; XML token/node/attribute
    // multiplicity is (1002+5)+1002+5 = 2014 records, at 16 units each.
    let child = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2">{}<rect width="2" height="2" fill="red"/></svg>"#,
        "<g/>".repeat(1000)
    );
    let n = child.len() as u64 + 2014 * 16;
    for format in FORMATS {
        for nested in [false, true] {
            for bounded in [false, true] {
                let parent = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="{}" height="2" href="{}"/></svg>"#,
                    u8::from(bounded),
                    svg_url(&child)
                );
                let mut source = image_scene(
                    if nested {
                        parent.as_bytes()
                    } else {
                        child.as_bytes()
                    },
                    "image/svg+xml",
                );
                if !nested && !bounded {
                    source["elements"][0]["width"] = json!(0);
                }
                for configured in [8191, 8192, n - 1, n, n + 1] {
                    let request = RenderRequest::builder(format)
                        .resource_limits(AssetLimit::Work.limits(configured))
                        .build()
                        .unwrap();
                    let rows = if configured < n {
                        json!([if nested {
                            nested_resource_row(
                                format,
                                "resource.asset-work-exceeded",
                                "max_asset_work_units",
                                json!(configured),
                                json!(n),
                                "work_units",
                                bounded && configured >= 8192,
                            )
                        } else {
                            expected(
                                "resource.asset-work-exceeded",
                                "max_asset_work_units",
                                json!(configured),
                                json!(n),
                                "work_units",
                                "/files/asset~1~0/dataURL",
                                if format == OutputFormat::Png {
                                    "placeholder"
                                } else {
                                    "omitted"
                                },
                            )
                        }])
                    } else {
                        json!([])
                    };
                    let actual = assert_repeat(&source, &request, rows);
                    if configured < n {
                        let mut oracle = source.clone();
                        if nested && (!bounded || configured < 8192) {
                            let omitted =
                                parent.replace(&format!(" href=\"{}\"", svg_url(&child)), "");
                            oracle["files"]["asset/~"]["dataURL"] = json!(svg_url(&omitted));
                            let expected = Renderer::new()
                                .render(
                                    &parse(&oracle),
                                    &RenderRequest::builder(format).build().unwrap(),
                                )
                                .unwrap();
                            assert_nested_artifact_eq(&actual, &expected);
                        } else if !nested {
                            oracle["files"] = json!({});
                            let expected = Renderer::new()
                                .render(
                                    &parse(&oracle),
                                    &RenderRequest::builder(format).build().unwrap(),
                                )
                                .unwrap();
                            assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
                        }
                    }
                }
            }
        }
    }
}

fn element_scene(kind: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":kind,"width":10,"height":10,"roughness":0,"seed":1,"updated":0}],"appState":{},"files":{}})
}

#[derive(Clone, Copy)]
enum ElementLimit {
    Geometry,
    Work,
}
impl ElementLimit {
    fn contract(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Geometry => (
                "resource.element-complexity-replaced",
                "max_geometry_segments_per_element",
                "count",
            ),
            Self::Work => (
                "resource.element-work-exceeded",
                "max_element_work_units",
                "work_units",
            ),
        }
    }
    fn limits(self, n: u64) -> ResourceLimits {
        match self {
            Self::Geometry => ResourceLimits::builder().max_geometry_segments_per_element(n as u32),
            Self::Work => ResourceLimits::builder().max_element_work_units(n),
        }
        .build()
        .unwrap()
    }
    fn full_rectangle(self) -> u64 {
        match self {
            Self::Geometry => 16,
            Self::Work => 640,
        }
    }
}

#[test]
fn element_complexity_contract_covers_geometry_text_recovery_and_frames() {
    element_outline_boundaries(ElementLimit::Geometry);
    element_fallback_boundaries(ElementLimit::Geometry);
    element_reference_order_and_frames(ElementLimit::Geometry);
    text_scalar_and_glyph_boundaries();
    nonreciprocal_bound_text_recovery_does_not_claim_lost_text();
}

#[test]
fn element_work_contract_covers_generation_shaping_projection_and_frames() {
    element_outline_boundaries(ElementLimit::Work);
    element_fallback_boundaries(ElementLimit::Work);
    element_reference_order_and_frames(ElementLimit::Work);
    empty_text_work_recovery_does_not_claim_lost_source_semantics();
    text_shaping_work_boundaries();
    png_projection_work_boundaries();
}

fn element_outline_boundaries(limit: ElementLimit) {
    let (code, field, kind) = limit.contract();
    let n = limit.full_rectangle();
    for format in FORMATS {
        // PNG also prices the stroked 12x12 coverage box at 16 samples per
        // pixel, but against total work only: the boundary at `n` is the
        // same in every format and the rectangle (or its placeholder) paints
        // although its raster samples exceed the element limit.
        for configured in [1, n - 1, n, n + 1] {
            let source = element_scene("rectangle");
            let request = RenderRequest::builder(format)
                .resource_limits(limit.limits(configured))
                .build()
                .unwrap();
            let attempted = if configured == 1 {
                match limit {
                    ElementLimit::Geometry => 2,
                    ElementLimit::Work => 128,
                }
            } else {
                n
            };
            // Four sides, two strokes, move+cubic = 16 commands. Work is
            // 14 normalized fields + index + plan at 8, plus 16*32 = 640.
            let mut row = expected(
                code,
                field,
                json!(configured),
                json!(attempted),
                kind,
                "/elements/0",
                if configured == 1 {
                    "omitted"
                } else {
                    "placeholder"
                },
            );
            if format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            let rows = if configured < n { vec![row] } else { vec![] };
            let outcome = assert_repeat(&source, &request, json!(rows));
            if configured < n {
                assert_eq!(
                    outcome.artifact().bytes(),
                    box_oracle(format, 10, 10, 10.0, configured != 1)
                        .artifact()
                        .bytes()
                );
            }
            if configured >= n {
                let baseline = Renderer::new()
                    .render(
                        &parse(&source),
                        &RenderRequest::builder(format).build().unwrap(),
                    )
                    .unwrap();
                assert_eq!(outcome.artifact().bytes(), baseline.artifact().bytes());
            }
        }
        if matches!(limit, ElementLimit::Geometry) {
            // A vertical line has four operations and no positive-width box
            // for a compatibility placeholder.
            let mut source = element_scene("line");
            source["elements"][0]["points"] = json!([[0, 0], [0, 10]]);
            for configured in [3, 4, 5] {
                let request = RenderRequest::builder(format)
                    .resource_limits(limit.limits(configured))
                    .build()
                    .unwrap();
                let mut row = expected(
                    code,
                    field,
                    json!(configured),
                    json!(4),
                    kind,
                    "/elements/0",
                    "omitted",
                );
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                let outcome = assert_repeat(
                    &source,
                    &request,
                    if configured < 4 {
                        json!([row])
                    } else {
                        json!([])
                    },
                );
                if configured < 4 {
                    assert_eq!(
                        outcome.artifact().bytes(),
                        box_oracle(format, 0, 10, 10.0, false).artifact().bytes()
                    );
                }
            }
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_geometry_segments_per_element(15)
                        .max_total_geometry_segments(15)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let error = Renderer::new()
                .render(&parse(&element_scene("rectangle")), &request)
                .unwrap_err();
            assert_eq!(error.code().as_str(), "resource.total-geometry-exceeded");
            exact_report(error.report(), format, &mut json!([]));
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_geometry_segments_per_element(15)
                        .max_element_work_units(639)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let mut row = expected(
                code,
                field,
                json!(15),
                json!(16),
                kind,
                "/elements/0",
                "placeholder",
            );
            if format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            // The placeholder's PNG raster samples go to total work only, so
            // the geometry replacement is the element's one diagnostic.
            let outcome = assert_repeat(&element_scene("rectangle"), &request, json!([row]));
            assert_eq!(
                outcome.artifact().bytes(),
                box_oracle(format, 10, 10, 10.0, true).artifact().bytes()
            );
        } else {
            let mut empty = element_scene("rectangle");
            empty["elements"][0]["width"] = json!(0);
            let request = RenderRequest::builder(format)
                .resource_limits(limit.limits(1))
                .build()
                .unwrap();
            assert_repeat(
                &empty,
                &request,
                json!([expected(
                    code,
                    field,
                    json!(1),
                    json!(128),
                    kind,
                    "/elements/0",
                    "omitted"
                )]),
            );
        }
    }
}

fn element_fallback_boundaries(limit: ElementLimit) {
    let (code, field, kind) = limit.contract();
    let n = match limit {
        ElementLimit::Geometry => 9,
        ElementLimit::Work => 9 * 32,
    };
    let source = element_scene("future");
    for format in FORMATS {
        // The placeholder's PNG raster samples (its 10x10 box at 16 per
        // pixel) go to total work only, so every format shares the boundary.
        for configured in [n - 1, n, n + 1] {
            let request = RenderRequest::builder(format)
                .resource_limits(limit.limits(configured))
                .build()
                .unwrap();
            let mut rows = vec![
                json!({"code":"compatibility.unsupported-element-type","severity":"warning","effects":["visual","semantic"],"element_id":"image","json_pointer":"/elements/0","details":{"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"}}),
            ];
            if configured < n {
                let mut row = expected(
                    code,
                    field,
                    json!(configured),
                    json!(n),
                    kind,
                    "/elements/0",
                    "omitted",
                );
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                rows.push(row);
            }
            let actual = assert_repeat(&source, &request, json!(rows));
            assert_eq!(
                actual.artifact().bytes(),
                box_oracle(format, 10, 10, 10.0, configured >= n)
                    .artifact()
                    .bytes()
            );
        }
    }
}

fn element_reference_order_and_frames(limit: ElementLimit) {
    let (code, field, kind) = limit.contract();
    let n = limit.full_rectangle();
    for format in FORMATS {
        let mut source = element_scene("rectangle");
        source["elements"][0]["id"] = json!("z");
        let mut second = source["elements"][0].clone();
        second["id"] = json!("a");
        second["x"] = json!(20);
        source["elements"].as_array_mut().unwrap().push(second);
        let rows: Vec<_> = ["z", "a"]
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                let mut row = expected(
                    code,
                    field,
                    json!(n - 1),
                    json!(n),
                    kind,
                    &format!("/elements/{index}"),
                    "placeholder",
                );
                row["element_id"] = json!(id);
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                row
            })
            .collect();
        let request = RenderRequest::builder(format)
            .resource_limits(limit.limits(n - 1))
            .build()
            .unwrap();
        assert_repeat(&source, &request, json!(rows));
        let limits = match limit {
            ElementLimit::Geometry => {
                ResourceLimits::builder().max_geometry_segments_per_element((n - 1) as u32)
            }
            ElementLimit::Work => ResourceLimits::builder().max_element_work_units(n - 1),
        }
        .max_diagnostics(1)
        .build()
        .unwrap();
        let request = RenderRequest::builder(format)
            .resource_limits(limits)
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&source), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
        exact_report(error.report(), format, &mut json!([rows[0]]));
        source["elements"].as_array_mut().unwrap().extend([
            json!({"id":"frame","type":"frame","width":40,"height":40,"updated":0}),
            json!({"id":"other","type":"frame","x":100,"width":40,"height":40,"updated":0}),
        ]);
        let document = parse(&source);
        let frames: Vec<_> = document.frames().collect();
        let renderer = Renderer::new();
        let request = FrameRenderRequest::builder(format)
            .resource_limits(limit.limits(n - 1))
            .build()
            .unwrap();
        let mut batch = renderer
            .render_frames(&document, frames.iter().copied(), &request)
            .unwrap();
        for (index, frame) in frames.into_iter().enumerate() {
            let direct = renderer.render_frame(frame, &request).unwrap();
            let prepared = batch.next().unwrap().into_result().unwrap();
            report(
                &direct,
                format,
                if index == 0 { json!(rows) } else { json!([]) },
            );
            assert_eq!(direct.report(), prepared.report());
            assert_eq!(direct.artifact().bytes(), prepared.artifact().bytes());
        }
        source["elements"][0]["isDeleted"] = json!(true);
        source["elements"][1]["isDeleted"] = json!(true);
        let request = RenderRequest::builder(format)
            .resource_limits(limit.limits(n - 1))
            .frame_options(
                excalidraw_render::FrameOptions::default()
                    .with_enabled(false)
                    .with_clip(false)
                    .with_show_name(false)
                    .with_outline(false),
            )
            .build()
            .unwrap();
        assert_repeat(&source, &request, json!([]));
    }
}

fn text_scene(text: &str, width: u64) -> Value {
    let mut source = element_scene("text");
    let element = &mut source["elements"][0];
    element["text"] = json!(text);
    element["originalText"] = json!(text);
    element["fontFamily"] = json!(5);
    element["fontSize"] = json!(10);
    element["lineHeight"] = json!(1.25);
    element["width"] = json!(width);
    source
}

// Count the pinned face's A directly through read-fonts' path conversion,
// independently of renderer shaping/accounting and without learning the count
// from a report. The renderer emits ttf-parser 0.25's command stream, which
// closes a contour that starts and ends on-curve with an explicit line back to
// its start; read-fonts closes it without one, so each such contour adds one.
fn official_a_geometry() -> (u64, u64) {
    use write_fonts::read::model::glyph::outline::OutlinePen;
    use write_fonts::read::tables::glyf::outline::{outline_to_path, PathContourStart};
    use write_fonts::read::tables::glyf::{Glyph, PointFlags};
    use write_fonts::read::types::Point;
    use write_fonts::read::{FontRef, TableProvider};

    struct Count {
        commands: u64,
        bounds: [f64; 4],
    }
    impl Count {
        fn point(&mut self, x: f32, y: f32) {
            // Excalifont's pinned metadata: 1000 UPEM, ascender 886,
            // descender -374; size10 in a 12.5-unit line box.
            let x = f64::from(x) * 0.01;
            let y = 12.5 / 2.0 + 512.0 * 0.01 / 2.0 - f64::from(y) * 0.01;
            self.bounds[0] = self.bounds[0].min(x);
            self.bounds[1] = self.bounds[1].min(y);
            self.bounds[2] = self.bounds[2].max(x);
            self.bounds[3] = self.bounds[3].max(y);
        }
    }
    impl OutlinePen for Count {
        fn move_to(&mut self, x: f32, y: f32) {
            self.commands += 1;
            self.point(x, y);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.commands += 1;
            self.point(x, y);
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            self.commands += 1;
            self.point(x1, y1);
            self.point(x, y);
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            self.commands += 1;
            self.point(x1, y1);
            self.point(x2, y2);
            self.point(x, y);
        }
        fn close(&mut self) {
            self.commands += 1;
        }
    }
    let asset = excalidraw_fonts::load_asset(
        "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
    )
    .unwrap()
    .unwrap();
    let sfnt = wuff::decompress_woff2(asset.bytes()).unwrap();
    let font = FontRef::new(&sfnt).unwrap();
    assert_eq!(font.head().unwrap().units_per_em(), 1000);
    let glyph = font.cmap().unwrap().map_codepoint('A').unwrap();
    let Some(Glyph::Simple(a)) = font
        .loca(None)
        .unwrap()
        .get_glyf(glyph, &font.glyf().unwrap())
        .unwrap()
    else {
        panic!("Excalifont's A is a simple glyph");
    };
    let (points, flags): (Vec<_>, Vec<_>) = a
        .points()
        .map(|point| {
            let flag = if point.on_curve {
                PointFlags::on_curve()
            } else {
                PointFlags::off_curve_quad()
            };
            (Point::new(f32::from(point.x), f32::from(point.y)), flag)
        })
        .unzip();
    let ends: Vec<u16> = a
        .end_pts_of_contours()
        .iter()
        .map(|end| end.get())
        .collect();
    let mut count = Count {
        commands: 0,
        bounds: [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
    };
    assert!(outline_to_path(
        &points,
        &flags,
        &ends,
        PathContourStart::ScanForward,
        &mut count
    ));
    let mut start = 0;
    for end in ends {
        let end = usize::from(end);
        count.commands += u64::from(flags[start].is_on_curve() && flags[end].is_on_curve());
        start = end + 1;
    }
    let area = (count.bounds[2].ceil() - count.bounds[0].floor())
        * (count.bounds[3].ceil() - count.bounds[1].floor());
    (count.commands, area as u64)
}

fn text_scalar_and_glyph_boundaries() {
    let (glyph_commands, _) = official_a_geometry();
    for format in FORMATS {
        for width in [0, 10] {
            for configured in [1, 2, 3] {
                let source = text_scene("Aé", width);
                let request = RenderRequest::builder(format)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_text_scalars_per_element(configured)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let mut row = expected(
                    "resource.element-complexity-replaced",
                    "max_text_scalars_per_element",
                    json!(configured),
                    json!(2),
                    "count",
                    "/elements/0",
                    if width == 0 { "omitted" } else { "placeholder" },
                );
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                let mut rows = if configured < 2 { vec![row] } else { vec![] };
                if configured < 2 && width > 0 {
                    let mut row = expected(
                        "resource.element-complexity-replaced",
                        "max_text_scalars_per_element",
                        json!(configured),
                        json!("Unavailable: text".chars().count()),
                        "count",
                        "/elements/0",
                        "omitted",
                    );
                    if format != OutputFormat::Png {
                        row["effects"] = json!(["visual", "semantic"]);
                    }
                    rows.push(row);
                }
                let actual = assert_repeat(&source, &request, json!(rows));
                if configured < 2 {
                    let expected = if width == 0 {
                        let mut empty = source.clone();
                        empty["elements"] = json!([]);
                        Renderer::new()
                            .render(
                                &parse(&empty),
                                &RenderRequest::builder(format).build().unwrap(),
                            )
                            .unwrap()
                    } else {
                        box_oracle(format, 10, 10, 10.0, true)
                    };
                    assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
                }
            }
        }
        for configured in [glyph_commands - 1, glyph_commands, glyph_commands + 1] {
            let request = RenderRequest::builder(format)
                .resource_limits(ElementLimit::Geometry.limits(configured))
                .build()
                .unwrap();
            let mut row = expected(
                "resource.element-complexity-replaced",
                "max_geometry_segments_per_element",
                json!(configured),
                json!(glyph_commands),
                "count",
                "/elements/0",
                "omitted",
            );
            if format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            let actual = assert_repeat(
                &text_scene("A", 10),
                &request,
                if configured < glyph_commands {
                    json!([row])
                } else {
                    json!([])
                },
            );
            if configured < glyph_commands {
                assert_eq!(
                    actual.artifact().bytes(),
                    box_oracle(format, 10, 10, 10.0, false).artifact().bytes()
                );
            }
        }
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_text_scalars_per_element(1)
                    .max_total_text_scalars(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&text_scene("AB", 10)), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.total-text-exceeded");
        exact_report(error.report(), format, &mut json!([]));
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_text_scalars_per_element(1)
                    .max_element_work_units(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut row = expected(
            "resource.element-complexity-replaced",
            "max_text_scalars_per_element",
            json!(1),
            json!(2),
            "count",
            "/elements/0",
            "omitted",
        );
        if format != OutputFormat::Png {
            row["effects"] = json!(["visual", "semantic"]);
        }
        assert_repeat(&text_scene("AB", 10), &request, json!([row]));
    }
}

fn text_shaping_work_boundaries() {
    // One scalar, glyph and line, followed by the pinned face's commands.
    let (commands, pixel_area) = official_a_geometry();
    let n = (3 + commands) * 32;
    // PNG's glyph coverage (16 samples per pixel of the glyph box) is the
    // text's own total-work event, never part of its element work.
    let charges = trailing_work_charges(
        &parse(&text_scene("A", 10)),
        || RenderRequest::builder(OutputFormat::Png),
        3,
    );
    assert_eq!(
        charges[2],
        WorkCharge {
            units: pixel_area * 16,
            element_id: Some("image".to_owned()),
            json_pointer: "/elements/0".to_owned(),
        }
    );
    for format in FORMATS {
        for configured in [95, n - 1, n, n + 1] {
            let request = RenderRequest::builder(format)
                .resource_limits(ElementLimit::Work.limits(configured))
                .build()
                .unwrap();
            let mut row = expected(
                "resource.element-work-exceeded",
                "max_element_work_units",
                json!(configured),
                json!(if configured == 95 { 96 } else { n }),
                "work_units",
                "/elements/0",
                "omitted",
            );
            if format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            let actual = assert_repeat(
                &text_scene("A", 10),
                &request,
                if configured < n {
                    json!([row])
                } else {
                    json!([])
                },
            );
            if configured < n {
                let expected = if configured == 95 {
                    let mut empty = text_scene("", 10);
                    empty["elements"] = json!([]);
                    Renderer::new()
                        .render(
                            &parse(&empty),
                            &RenderRequest::builder(format).build().unwrap(),
                        )
                        .unwrap()
                } else {
                    box_oracle(format, 10, 10, 10.0, false)
                };
                assert_eq!(actual.artifact().bytes(), expected.artifact().bytes());
            }
        }
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_geometry_segments_per_element((commands - 1) as u32)
                    .max_total_geometry_segments(commands - 1)
                    .max_element_work_units(n - 1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&text_scene("A", 10)), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.total-geometry-exceeded");
        exact_report(error.report(), format, &mut json!([]));
    }
}

fn png_projection_work_boundaries() {
    let bytes = crate::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]);
    let mut source = image_scene(&bytes, "image/png");
    source["elements"][0]["width"] = json!(1);
    source["elements"][0]["height"] = json!(1);
    for format in FORMATS {
        let unconstrained = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).build().unwrap(),
            )
            .unwrap();
        // The one-pixel occurrence's PNG projection, composite and mask
        // samples (24) never count against the element limit, however low.
        for configured in [1, 23, 24, 25] {
            let request = RenderRequest::builder(format)
                .resource_limits(ElementLimit::Work.limits(configured))
                .build()
                .unwrap();
            let actual = assert_repeat(&source, &request, json!([]));
            assert_eq!(actual.artifact().bytes(), unconstrained.artifact().bytes());
        }
    }
    // They are the occurrence's own total-work event instead: 12 bilinear
    // projection, 4 composite and 8 paint-mask samples per occurrence pixel,
    // followed only by the canvas-wide encoding events, and crossing them is
    // the fatal total-work failure attributed to the image.
    let charges = trailing_work_charges(
        &parse(&source),
        || RenderRequest::builder(OutputFormat::Png),
        3,
    );
    assert_eq!(
        charges[2],
        WorkCharge {
            units: 12 + 4 + 8,
            element_id: Some("image".to_owned()),
            json_pointer: "/elements/0".to_owned(),
        }
    );
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

fn nested_resource_row(
    format: OutputFormat,
    code: &str,
    field: &str,
    configured: Value,
    attempted: Value,
    kind: &str,
    placeholder: bool,
) -> Value {
    json!({"code":code,"severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
        "details":{"kind":"nested_resource_limit","field":field,"configured":configured,"value_kind":kind,"measurement":{"kind":"attempted","value":attempted},"asset_chain":[{"asset_id":"asset/~","occurrence":0}],
        "paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],
        "recovery":if placeholder {"placeholder"} else {"omitted"},"recipe_id":if placeholder {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")} else {Value::Null}}})
}

fn coordinate_derived_reports_and_boundaries() {
    for format in FORMATS {
        for configured in [124, 125, 126] {
            let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"rectangle","x":75,"width":50,"height":10,"roughness":0,"seed":1,"updated":0}],"appState":{},"files":{}});
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_coordinate_magnitude(f64::from(configured))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let mut row = expected(
                "resource.coordinate-range-exceeded",
                "max_coordinate_magnitude",
                json!(configured),
                json!(125),
                "coordinate",
                "/elements/0",
                "omitted",
            );
            if format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            let actual = assert_repeat(
                &source,
                &request,
                if configured < 125 {
                    json!([row])
                } else {
                    json!([])
                },
            );
            if configured < 125 {
                let mut empty = source.clone();
                empty["elements"] = json!([]);
                assert_eq!(
                    actual.artifact().bytes(),
                    Renderer::new()
                        .render(&parse(&empty), &request)
                        .unwrap()
                        .artifact()
                        .bytes()
                );
            }
        }
        // The Catmull-Rom control is 100 + (100 - 0)/6, although
        // normalized endpoints and dimensions are all at most 100.
        let curve = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"line","points":[[0,0],[100,0],[100,10]],"roundness":{"type":2},"roughness":0,"seed":1,"updated":0}],"appState":{},"files":{}});
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut row = expected(
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            json!(100),
            json!(100.0 + 100.0 / 6.0),
            "coordinate",
            "/elements/0",
            "placeholder",
        );
        if format != OutputFormat::Png {
            row["effects"] = json!(["visual", "semantic"]);
        }
        let actual = assert_repeat(&curve, &request, json!([row]));
        assert_eq!(
            actual.artifact().bytes(),
            box_oracle(format, 100, 10, 0.0, true).artifact().bytes()
        );

        let bytes = crate::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]);
        for crop_width in [1.0, 1e-320] {
            let mut source = image_scene(&bytes, "image/png");
            source["elements"][0]["x"] = json!(0);
            source["elements"][0]["y"] = json!(0);
            source["elements"][0]["width"] = json!(10);
            source["elements"][0]["height"] = json!(10);
            source["elements"][0]["crop"] = json!({"x":0,"y":0,"width":crop_width,"height":100,"naturalWidth":100,"naturalHeight":100});
            let mut row = expected(
                "resource.coordinate-range-exceeded",
                "max_coordinate_magnitude",
                json!(100),
                json!(1000),
                "coordinate",
                "/elements/0",
                "omitted",
            );
            if crop_width != 1.0 {
                row["details"]["measurement"] =
                    json!({"kind":"at_least","value":100.00000000000001});
            }
            let actual = assert_repeat(&source, &request, json!([row]));
            let mut empty = source.clone();
            empty["elements"] = json!([]);
            assert_eq!(
                actual.artifact().bytes(),
                Renderer::new()
                    .render(&parse(&empty), &request)
                    .unwrap()
                    .artifact()
                    .bytes()
            );
        }
    }
}

fn coordinate_nested_reports_recover_and_order_per_slot() {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0L2000 0L2000 10Z"/></svg>"#;
    for format in FORMATS {
        let overflow = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><path transform="scale(1e308)" d="M2 0L3 0L3 1Z"/></svg>"#;
        let parent = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
            svg_url(overflow)
        );
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut row = nested_resource_row(
            format,
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            json!(100),
            Value::Null,
            "coordinate",
            false,
        );
        row["details"]["measurement"] = json!({"kind":"at_least","value":100.00000000000001});
        assert_repeat(
            &image_scene(parent.as_bytes(), "image/svg+xml"),
            &request,
            json!([row]),
        );
        for bounded in [false, true] {
            let parent = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image width="{}" height="10" href="{}"/><image width="{}" height="10" href="{}"/></svg>"#,
                u8::from(bounded),
                svg_url(leaf),
                u8::from(bounded),
                svg_url(leaf)
            );
            let source = image_scene(parent.as_bytes(), "image/svg+xml");
            for configured in [100, 667, 668, 1999, 2000, 2001] {
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_coordinate_magnitude(f64::from(configured))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let rows: Vec<_> = if configured < 2000 {
                    [1, 2]
                        .into_iter()
                        .map(|ordinal| {
                            let mut row = nested_resource_row(
                                format,
                                "resource.coordinate-range-exceeded",
                                "max_coordinate_magnitude",
                                json!(configured),
                                json!(2000),
                                "coordinate",
                                bounded && configured >= 668,
                            );
                            row["details"]["paths"][0]["segments"][0]["ordinal"] = json!(ordinal);
                            row
                        })
                        .collect()
                } else {
                    vec![]
                };
                let actual = assert_repeat(&source, &request, json!(rows));
                if configured < 2000 && (!bounded || configured < 668) {
                    let omitted = parent.replace(&format!(" href=\"{}\"", svg_url(leaf)), "");
                    let expected = Renderer::new()
                        .render(
                            &parse(&image_scene(omitted.as_bytes(), "image/svg+xml")),
                            &RenderRequest::builder(format).padding(0.0).build().unwrap(),
                        )
                        .unwrap();
                    assert_nested_artifact_eq(&actual, &expected);
                }
            }
        }
        // Top-level authoritative SVG failure uses ordinary format-specific
        // image recovery and retains the precise file/supplement location.
        for supplemental in [false, true] {
            for status in ["saved", "error"] {
                let mut source = image_scene(leaf.as_bytes(), "image/svg+xml");
                source["elements"][0]["status"] = json!(status);
                let assets = if supplemental {
                    source["files"] = json!({});
                    SupplementalAssets::builder()
                        .asset("asset/~", leaf.as_bytes().to_vec())
                        .build()
                        .unwrap()
                } else {
                    SupplementalAssets::default()
                };
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .supplemental_assets(assets)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_coordinate_magnitude(100.0)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let mut row = expected(
                    "resource.coordinate-range-exceeded",
                    "max_coordinate_magnitude",
                    json!(100),
                    json!(2000),
                    "coordinate",
                    if supplemental {
                        ""
                    } else {
                        "/files/asset~1~0/dataURL"
                    },
                    if format == OutputFormat::Png {
                        "placeholder"
                    } else {
                        "omitted"
                    },
                );
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                assert_repeat(&source, &request, json!([row]));
                source["elements"][0]["isDeleted"] = json!(true);
                assert_repeat(&source, &request, json!([]));
            }
        }
    }
}

fn coordinate_report_prefix_and_frame_replay() {
    for format in FORMATS {
        let source = json!({"type":"excalidraw","version":2,"elements":[
            {"id":"z","type":"rectangle","x":101,"width":10,"height":10,"roughness":0,"seed":1,"updated":0},
            {"id":"a","type":"rectangle","x":102,"width":10,"height":10,"roughness":0,"seed":1,"updated":0}],"appState":{},"files":{}});
        let rows: Vec<_> = [("z", 101), ("a", 102)]
            .into_iter()
            .enumerate()
            .map(|(index, (id, value))| {
                let mut row = expected(
                    "resource.coordinate-range-exceeded",
                    "max_coordinate_magnitude",
                    json!(100),
                    json!(value),
                    "coordinate",
                    &format!("/elements/{index}/x"),
                    "omitted",
                );
                row["element_id"] = json!(id);
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                row
            })
            .collect();
        let limits = ResourceLimits::builder()
            .max_coordinate_magnitude(100.0)
            .build()
            .unwrap();
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .unwrap();
        assert_repeat(&source, &request, json!(rows));
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(100.0)
                    .max_diagnostics(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&source), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
        exact_report(error.report(), format, &mut json!([rows[0]]));

        let source = json!({"type":"excalidraw","version":2,"elements":[
            {"id":"bad","type":"frame","x":75,"width":50,"height":10,"updated":0},
            {"id":"good","type":"frame","width":10,"height":10,"updated":0}],"appState":{},"files":{}});
        let scene = parse(&source);
        let request = FrameRenderRequest::builder(format)
            .resource_limits(limits)
            .build()
            .unwrap();
        let renderer = Renderer::new();
        let frames: Vec<_> = scene.frames().collect();
        let mut batch = renderer
            .render_frames(&scene, frames.iter().copied(), &request)
            .unwrap();
        let direct = renderer.render_frame(frames[0], &request).unwrap_err();
        let prepared = batch.next().unwrap().into_result().unwrap_err();
        assert_eq!(direct.code().as_str(), "resource.coordinate-range-exceeded");
        assert_eq!(
            direct.category(),
            excalidraw_render::RenderErrorCategory::ResourceLimit
        );
        assert_eq!(direct.element_id(), Some("bad"));
        assert_eq!(direct.json_pointer(), "/elements/0");
        assert_eq!(
            serde_json::to_value(direct.details()).unwrap(),
            json!({"kind":"resource_limit","field":"max_coordinate_magnitude","configured":100,"value_kind":"coordinate","measurement":{"kind":"attempted","value":125},"recovery":null})
        );
        exact_report(direct.report(), format, &mut json!([]));
        assert_eq!(direct.descriptor(), prepared.descriptor());
        assert_eq!(direct.report(), prepared.report());
        let direct = renderer.render_frame(frames[1], &request).unwrap();
        let prepared = batch.next().unwrap().into_result().unwrap();
        report(&direct, format, json!([]));
        assert_eq!(direct.report(), prepared.report());
        assert_eq!(direct.artifact().bytes(), prepared.artifact().bytes());
    }
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
pub(super) fn nested_depth_contract_covers_owners_paths_frames_and_silence() {
    nested_resource_limits_report_the_rejected_source_slot();
    rejected_nested_payload_effects_follow_signatures_not_media_claims();
    nested_depth_recovery_depends_on_current_edge_not_cached_parent();
    nested_depth_recovery_envelope_and_owner_order_are_exact();
    nested_depth_placeholder_matches_manifested_geometry_and_local_inheritance();
    nested_depth_stops_discovery_and_ignores_conflicting_payload();
    nested_limit_owner_scope(false);
}

fn nested_limit_owner_scope(nodes: bool) {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/><rect width="1" height="1"/><rect width="1" height="1"/></svg>"#;
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
            let limits = if nodes {
                ResourceLimits::builder()
                    .max_nested_svg_nodes(3)
                    .build()
                    .unwrap()
            } else {
                ResourceLimits::builder()
                    .max_nested_asset_depth(1)
                    .build()
                    .unwrap()
            };
            let rows: Vec<_> = [1,2].into_iter().map(|ordinal|json!({
                "code":if nodes {"resource.nested-svg-complexity-exceeded"} else {"resource.nested-asset-depth-exceeded"},"severity":"warning","effects":if format == OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":if supplemental {""} else {"/files/asset~1~0/dataURL"},
                "details":{"kind":"nested_resource_limit","field":if nodes {"max_nested_svg_nodes"} else {"max_nested_asset_depth"},"configured":if nodes {3} else {1},"value_kind":if nodes {"count"} else {"depth"},"measurement":{"kind":"attempted","value":if nodes {4} else {2}},
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
    nested_placeholder_artifact_oracle(false);
}

fn nested_placeholder_artifact_oracle(nodes: bool) {
    nested_placeholder_artifact_oracle_for(nodes, None, None);
}

fn nested_placeholder_artifact_oracle_for(
    nodes: bool,
    raster: Option<RasterLimit>,
    extra: Option<ExtraNestedOracle>,
) {
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
    let leaf = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20">{}</svg>"#,
        r#"<rect width="30" height="20" fill="red"/>"#.repeat(8)
    );
    let leaf=match extra {
        Some(ExtraNestedOracle::Work)=>format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20">{}<rect width="30" height="20" fill="red"/></svg>"#,"<g/>".repeat(1000)),
        Some(ExtraNestedOracle::Coordinate)=>r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><path d="M0 0L2000 0L2000 20Z" fill="red"/></svg>"#.to_owned(),
        None=>leaf,
    };
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
        if raster.is_some() && format == OutputFormat::Svg {
            continue;
        }
        let mut source = image_scene(parent(&leaf).as_bytes(), "image/svg+xml");
        if raster.is_some() {
            let bytes = crate::common::codec_fixtures::solid_png(64, 2, [255, 0, 0, 255]);
            let raster_url = format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            );
            let raster_child = child(&leaf).replace(&svg_url(&leaf), &raster_url);
            let raster_parent = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><image width="60" height="40" href="{}"/></svg>"#,
                svg_url(&raster_child)
            );
            source["files"]["asset/~"]["dataURL"] = json!(svg_url(&raster_parent));
        }
        source["elements"][0]["width"] = json!(60);
        source["elements"][0]["height"] = json!(40);
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .resource_limits(if let Some(extra) = extra {
                match extra {
                    ExtraNestedOracle::Work => {
                        ResourceLimits::builder().max_asset_work_units(16384)
                    }
                    ExtraNestedOracle::Coordinate => {
                        ResourceLimits::builder().max_coordinate_magnitude(1000.0)
                    }
                }
                .build()
                .unwrap()
            } else if let Some(limit) = raster {
                limit.limits(limit.contract().2 - 1)
            } else if nodes {
                ResourceLimits::builder()
                    .max_nested_svg_nodes(7)
                    .build()
                    .unwrap()
            } else {
                ResourceLimits::builder()
                    .max_nested_asset_depth(1)
                    .build()
                    .unwrap()
            })
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
        assert_nested_artifact_eq(&actual, &expected);
        match format {
            OutputFormat::Png => {
                let a = crate::common::raster::decode_png(actual.artifact().bytes());
                assert_eq!(a.pixel(10, 8), [243, 243, 243, 255]);
                assert_eq!(a.pixel(30, 8), [255, 255, 255, 255]);
                assert_eq!(a.pixel(45, 8), [0, 0, 255, 255]);
            }
            OutputFormat::Svg => {
                let text = std::str::from_utf8(actual.artifact().bytes()).unwrap();
                assert!(text.contains("font-awesome-free-5.15.4/solid/image@"));
                assert!(text.contains("font-awesome-free-5.15.4/solid/ban@"));
            }
            OutputFormat::Pdf => {
                let a = crate::common::pdf_inspect::pdf_text(actual.artifact().bytes());
                // Compare every drawing stream, including form streams, while
                // retaining separate assertions for mandatory notice metadata.
                assert!(!pdf_drawing_streams(actual.artifact().bytes()).is_empty());
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
    nested_limit_envelope_and_owner_order(false);
}

fn nested_limit_envelope_and_owner_order(nodes: bool) {
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/><rect width="1" height="1"/></svg>"#;
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
                        .max_nested_asset_depth(if nodes { 3 } else { 1 })
                        .max_nested_svg_nodes(if nodes { 2 } else { 100_000 })
                        .max_asset_work_units(work)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let rows:Vec<_>=[("a","/files/a/dataURL"),("z/~","/files/z~1~0/dataURL")].into_iter().map(|(id,pointer)|json!({
                "code":if nodes {"resource.nested-svg-complexity-exceeded"} else {"resource.nested-asset-depth-exceeded"},"severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":pointer,
                "details":{"kind":"nested_resource_limit","field":if nodes {"max_nested_svg_nodes"} else {"max_nested_asset_depth"},"configured":if nodes {2} else {1},"value_kind":if nodes {"count"} else {"depth"},"measurement":{"kind":"attempted","value":if nodes {3} else {2}},"asset_chain":[{"asset_id":id,"occurrence":0}],"paths":[{"segments":[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"},{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}]}],"recovery":if work<8192 {"omitted"} else {"placeholder"},"recipe_id":if work<8192 {Value::Null} else {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")}}
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
        let metadata = MetadataOptions::default()
            .with_embedded_scene(true)
            .with_provenance(true)
            .with_diagnostic_summary(true);
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
fn optional_metadata_components_have_exact_output_boundaries_and_priority() {
    let source = json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}});
    for format in FORMATS {
        let render = |metadata: MetadataOptions| {
            Renderer::new()
                .render(
                    &parse(&source),
                    &RenderRequest::builder(format)
                        .metadata_options(metadata)
                        .build()
                        .unwrap(),
                )
                .unwrap()
        };
        let baseline = render(MetadataOptions::default());
        for options in [
            MetadataOptions::default().with_embedded_scene(true),
            MetadataOptions::default().with_provenance(true),
            MetadataOptions::default().with_diagnostic_summary(true),
        ] {
            let full = render(options.clone());
            let n = full.artifact().bytes().len() as u64;
            assert!(n > baseline.artifact().bytes().len() as u64);
            for configured in [n - 1, n, n + 1] {
                let request = RenderRequest::builder(format)
                    .metadata_options(options.clone())
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_output_bytes(configured)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let rows = if configured < n {
                    json!([metadata_omission_row("max_output_bytes", configured, n)])
                } else {
                    json!([])
                };
                let actual = assert_repeat(&source, &request, rows);
                assert_eq!(
                    actual.artifact().bytes(),
                    if configured < n {
                        baseline.artifact().bytes()
                    } else {
                        full.artifact().bytes()
                    }
                );
            }
        }
        let provenance = render(MetadataOptions::default().with_provenance(true));
        let summary = render(MetadataOptions::default().with_diagnostic_summary(true));
        let limit = summary.artifact().bytes().len() as u64;
        assert!(provenance.artifact().bytes().len() as u64 > limit);
        let request = RenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let result = assert_repeat(
            &source,
            &request,
            json!([metadata_omission_row(
                "max_output_bytes",
                limit,
                provenance.artifact().bytes().len() as u64
            )]),
        );
        // Exact equality also proves the frozen summary excludes its own
        // later omission warning and the refused provenance is absent.
        assert_eq!(result.artifact().bytes(), summary.artifact().bytes());
    }
}

#[test]
fn metadata_omissions_order_and_required_report_failure_are_exact() {
    let source = json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}});
    for format in FORMATS {
        let render = |options: MetadataOptions| {
            Renderer::new()
                .render(
                    &parse(&source),
                    &RenderRequest::builder(format)
                        .metadata_options(options)
                        .build()
                        .unwrap(),
                )
                .unwrap()
        };
        let baseline = render(MetadataOptions::default());
        let limit = baseline.artifact().bytes().len() as u64;
        let rows: Vec<_> = [
            MetadataOptions::default().with_provenance(true),
            MetadataOptions::default().with_diagnostic_summary(true),
            MetadataOptions::default().with_embedded_scene(true),
        ]
        .into_iter()
        .map(|options| {
            metadata_omission_row(
                "max_output_bytes",
                limit,
                render(options).artifact().bytes().len() as u64,
            )
        })
        .collect();
        let request = RenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let outcome = assert_repeat(&source, &request, json!(rows));
        assert_eq!(outcome.artifact().bytes(), baseline.artifact().bytes());
        let request = RenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .max_diagnostics(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&source), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
        let mut actual = serde_json::to_value(error.report()).unwrap();
        for row in actual["diagnostics"].as_array_mut().unwrap() {
            assert!(!row["message"].as_str().unwrap().is_empty());
            row.as_object_mut().unwrap().remove("message");
        }
        let mut first = rows[0].clone();
        first["format"] = json!(format.as_str());
        assert_eq!(actual, json!({"schema_version":1,"diagnostics":[first]}));
        let request = RenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit - 1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let error = Renderer::new()
            .render(&parse(&source), &request)
            .unwrap_err();
        assert_eq!(error.code().as_str(), "resource.output-bytes-exceeded");
        assert!(error.report().is_empty());
    }
}

fn metadata_omission_row(field: &str, configured: u64, attempted: u64) -> Value {
    let mut row = expected(
        "resource.optional-metadata-omitted",
        field,
        json!(configured),
        json!(attempted),
        if field == "max_total_work_units" {
            "work_units"
        } else {
            "bytes"
        },
        "",
        "metadata_omitted",
    );
    row["element_id"] = Value::Null;
    row["effects"] = json!(["metadata"]);
    row
}

#[test]
fn optional_metadata_keeps_notices_and_nonempty_frozen_summary() {
    let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":20,"height":20,"fileId":"missing","status":"error","updated":0}],"appState":{},"files":{}});
    for format in FORMATS {
        let render = |options: MetadataOptions| {
            Renderer::new()
                .render(
                    &parse(&source),
                    &RenderRequest::builder(format)
                        .metadata_options(options)
                        .build()
                        .unwrap(),
                )
                .unwrap()
        };
        let baseline = render(MetadataOptions::default());
        let provenance = render(MetadataOptions::default().with_provenance(true));
        let summary = render(MetadataOptions::default().with_diagnostic_summary(true));
        let limit = summary.artifact().bytes().len() as u64;
        assert!(provenance.artifact().bytes().len() as u64 > limit);
        let missing = json!({"code":"image.missing-file","severity":"warning","effects":["visual"],"element_id":"image","json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"missing","claimed_media_type":null,"validated_media_type":null,"recovery":if format==OutputFormat::Png {"placeholder"} else {"omitted"},"recipe_id":if format==OutputFormat::Png {json!("official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9")} else {Value::Null}}});
        let request = RenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let result = assert_repeat(
            &source,
            &request,
            json!([
                missing,
                metadata_omission_row(
                    "max_output_bytes",
                    limit,
                    provenance.artifact().bytes().len() as u64
                )
            ]),
        );
        assert_eq!(result.artifact().bytes(), summary.artifact().bytes());
        // The PNG ordinary error recipe requires notices even without optional
        // metadata. Exact baseline bytes preserve them when all options fail.
        let request = RenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(baseline.artifact().bytes().len() as u64)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let result = Renderer::new().render(&parse(&source), &request).unwrap();
        assert_eq!(result.artifact().bytes(), baseline.artifact().bytes());
        assert_eq!(result.report().diagnostics().len(), 4);
    }
}

#[test]
fn optional_metadata_frame_limits_are_independent_and_replayed() {
    let source = json!({"type":"excalidraw","version":2,"elements":[
        {"id":"short","type":"frame","name":"A","width":20,"height":20,"updated":0},
        {"id":"long","type":"frame","name":"B".repeat(500),"x":100,"width":20,"height":20,"updated":0}
    ],"appState":{},"files":{}});
    let scene = parse(&source);
    let frames: Vec<_> = scene.frames().collect();
    for format in FORMATS {
        let renderer = Renderer::new();
        let full_request = FrameRenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .build()
            .unwrap();
        let full: Vec<_> = frames
            .iter()
            .map(|frame| renderer.render_frame(*frame, &full_request).unwrap())
            .collect();
        let limit = full[0].artifact().bytes().len() as u64;
        assert!(full[1].artifact().bytes().len() as u64 > limit);
        let request = FrameRenderRequest::builder(format)
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut batch = renderer
            .render_frames(&scene, frames.iter().copied(), &request)
            .unwrap();
        for (i, frame) in frames.iter().enumerate() {
            let actual = renderer.render_frame(*frame, &request).unwrap();
            let replay = batch.next().unwrap().into_result().unwrap();
            report(
                &actual,
                format,
                if i == 0 {
                    json!([])
                } else {
                    json!([metadata_omission_row(
                        "max_output_bytes",
                        limit,
                        full[i].artifact().bytes().len() as u64
                    )])
                },
            );
            assert_eq!(actual.report(), replay.report());
            assert_eq!(actual.artifact().bytes(), replay.artifact().bytes());
            let accepted = renderer
                .render_frame(
                    *frame,
                    &FrameRenderRequest::builder(format)
                        .metadata_options(
                            MetadataOptions::default()
                                .with_embedded_scene(i == 0)
                                .with_provenance(true)
                                .with_diagnostic_summary(true),
                        )
                        .build()
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(actual.artifact().bytes(), accepted.artifact().bytes());
            let silent = renderer
                .render_frame(
                    *frame,
                    &FrameRenderRequest::builder(format)
                        .resource_limits(
                            ResourceLimits::builder()
                                .max_output_bytes(limit)
                                .build()
                                .unwrap(),
                        )
                        .build()
                        .unwrap(),
                )
                .unwrap();
            assert!(silent.report().is_empty());
        }
    }
}

#[test]
fn optional_metadata_contract_covers_components_limits_and_frames() {
    optional_metadata_output_boundary_retains_lower_priority_contract();
    optional_metadata_components_have_exact_output_boundaries_and_priority();
    embedded_scene_scratch_rejection_is_exact_and_keeps_visual_output();
    optional_metadata_work_crossings_preserve_committed_measurement();
    embedded_scene_work_rejects_at_each_stage_with_exact_retained_cost();
    metadata_omissions_order_and_required_report_failure_are_exact();
    optional_metadata_keeps_notices_and_nonempty_frozen_summary();
    optional_metadata_frame_limits_are_independent_and_replayed();
}

#[test]
fn embedded_scene_scratch_rejection_is_exact_and_keeps_visual_output() {
    // Enough opaque source to make embedded-scene scratch dominate the
    // already-admitted raster/PDF workspaces without adding rendered content.
    let source = json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{},"source":"x".repeat(1_000_000)});
    let scene_bytes = serde_json::to_vec(&source).unwrap().len() as u64;
    // Stored zlib: 2-byte header, 5-byte header per 65535-byte block, Adler32.
    let compressed = scene_bytes + 6 + 5 * scene_bytes.div_ceil(65_535);
    let align = |n: u64| n.div_ceil(64) * 64;
    let full_svg = Renderer::new()
        .render(
            &parse(&source),
            &RenderRequest::builder(OutputFormat::Svg)
                .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                .build()
                .unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(full_svg.artifact().bytes()).unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();
    let manifest_len = document
        .descendants()
        .find(|n| n.tag_name().name() == "manifest")
        .unwrap()
        .text()
        .unwrap()
        .len() as u64;
    let scratch = 512 + align(scene_bytes) + align(compressed) + align(manifest_len) + 1_048_576;
    for format in FORMATS {
        let working = 4096
            + 1024
            + 128
            + match format {
                OutputFormat::Svg => 262144,
                OutputFormat::Png => 256 + 1600 + 2 * 1_048_576 + 16 * 20,
                OutputFormat::Pdf => 256 + 1_048_576,
                _ => unreachable!(),
            };
        let n = working + scratch;
        let baseline = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).build().unwrap(),
            )
            .unwrap();
        let full = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format)
                    .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        let retained = n + 512 + align(full.artifact().bytes().len() as u64);
        for configured in [n - 1, n, n + 1, retained - 1, retained, retained + 1] {
            let request = RenderRequest::builder(format)
                .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                .resource_limits(
                    ResourceLimits::builder()
                        .max_working_bytes(configured)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let result = assert_repeat(
                &source,
                &request,
                if configured < retained {
                    json!([metadata_omission_row(
                        "max_working_bytes",
                        configured,
                        if configured < n { n } else { retained }
                    )])
                } else {
                    json!([])
                },
            );
            assert_eq!(
                result.artifact().bytes(),
                if configured < retained {
                    baseline.artifact().bytes()
                } else {
                    full.artifact().bytes()
                }
            );
        }
    }
}

#[test]
fn optional_metadata_work_crossings_preserve_committed_measurement() {
    let source = json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}});
    for options in [
        MetadataOptions::default().with_provenance(true),
        MetadataOptions::default().with_diagnostic_summary(true),
    ] {
        let full_svg = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(OutputFormat::Svg)
                    .metadata_options(options.clone())
                    .build()
                    .unwrap(),
            )
            .unwrap();
        let document =
            roxmltree::Document::parse(std::str::from_utf8(full_svg.artifact().bytes()).unwrap())
                .unwrap();
        let generation = document
            .descendants()
            .find(|n| n.tag_name().name() == "manifest")
            .unwrap()
            .text()
            .unwrap()
            .len() as u64;
        for format in FORMATS {
            let baseline = Renderer::new()
                .render(
                    &parse(&source),
                    &RenderRequest::builder(format).build().unwrap(),
                )
                .unwrap();
            let full = Renderer::new()
                .render(
                    &parse(&source),
                    &RenderRequest::builder(format)
                        .metadata_options(options.clone())
                        .build()
                        .unwrap(),
                )
                .unwrap();
            // Empty input:72 source bytes,11 JSON events,5 normalized records,
            // one plan op. PNG additionally clears/paints/measures 400 pixels.
            let before =
                72 + 11 * 8 + 5 * 8 + 8 + if format == OutputFormat::Png { 4800 } else { 0 };
            let retained = before + 2 * generation + full.artifact().bytes().len() as u64;
            for configured in [retained - 1, retained, retained + 1] {
                let request = RenderRequest::builder(format)
                    .metadata_options(options.clone())
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_element_work_units(1)
                            .max_asset_work_units(1)
                            .max_total_work_units(configured)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let actual = assert_repeat(
                    &source,
                    &request,
                    if configured < retained {
                        json!([metadata_omission_row(
                            "max_total_work_units",
                            configured,
                            retained
                        )])
                    } else {
                        json!([])
                    },
                );
                assert_eq!(
                    actual.artifact().bytes(),
                    if configured < retained {
                        baseline.artifact().bytes()
                    } else {
                        full.artifact().bytes()
                    }
                );
            }
        }
    }
}

#[test]
fn embedded_scene_work_rejects_at_each_stage_with_exact_retained_cost() {
    let source = json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{},"source":"x".repeat(100_000)});
    let canonical = serde_json::to_vec(&source).unwrap().len() as u64;
    let compressed = canonical + 6 + 5 * canonical.div_ceil(65_535);
    let full_svg = Renderer::new()
        .render(
            &parse(&source),
            &RenderRequest::builder(OutputFormat::Svg)
                .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                .build()
                .unwrap(),
        )
        .unwrap();
    let svg = std::str::from_utf8(full_svg.artifact().bytes()).unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();
    let manifest = document
        .descendants()
        .find(|n| n.tag_name().name() == "manifest")
        .unwrap()
        .text()
        .unwrap()
        .len() as u64;
    let payload = svg
        .split_once("<!-- payload-start -->")
        .unwrap()
        .1
        .split_once("<!-- payload-end -->")
        .unwrap()
        .0;
    let wrapper = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .unwrap()
        .len() as u64;
    let generation = canonical + compressed + wrapper + manifest;
    for format in FORMATS {
        let baseline = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format).build().unwrap(),
            )
            .unwrap();
        let full = Renderer::new()
            .render(
                &parse(&source),
                &RenderRequest::builder(format)
                    .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        // Extra source field adds its key and value to JSON admission; the
        // preservation-only envelope does not become a rendered record.
        let before =
            canonical + 13 * 8 + 5 * 8 + 8 + if format == OutputFormat::Png { 3200 } else { 0 };
        let stages = [
            before + generation,
            before + 2 * generation,
            before
                + 2 * generation
                + full.artifact().bytes().len() as u64
                + if format == OutputFormat::Png { 1600 } else { 0 },
        ];
        for (stage, attempted) in stages.into_iter().enumerate() {
            let configured = attempted - 1;
            let request = RenderRequest::builder(format)
                .metadata_options(MetadataOptions::default().with_embedded_scene(true))
                .resource_limits(
                    ResourceLimits::builder()
                        .max_element_work_units(1)
                        .max_asset_work_units(1)
                        .max_total_work_units(configured)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let actual = assert_repeat(
                &source,
                &request,
                json!([metadata_omission_row(
                    "max_total_work_units",
                    configured,
                    attempted
                )]),
            );
            assert_eq!(
                actual.artifact().bytes(),
                baseline.artifact().bytes(),
                "stage {stage}"
            );
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
                } else if bounded {
                    // Admitted components can still translate the far edge
                    // beyond the ceiling: x=21 + width=20 is 41.
                    let mut row = expected(
                        "resource.coordinate-range-exceeded",
                        "max_coordinate_magnitude",
                        json!(configured),
                        json!(41),
                        "coordinate",
                        "/elements/0",
                        "omitted",
                    );
                    if format != OutputFormat::Png {
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
fn coordinate_contract_covers_components_points_recovery_and_frames() {
    coordinate_point_recovery_and_component_precedence();
    local_outline_resource_recovery_reports_exact_crossings();
    rejected_empty_content_does_not_claim_lost_link_semantics();
    coordinate_limits_admit_non_outline_content_before_resolution();
    invalid_frame_coordinates_omit_dependants_and_fail_the_frame_target();
    invalid_frame_omission_uses_repaired_active_clipping_dependencies();
    coordinate_rejected_images_classify_authoritative_vector_content();
    frame_coordinate_recovery_reconciles_fallback_and_missing_image_reports();
    coordinate_derived_reports_and_boundaries();
    coordinate_nested_reports_recover_and_order_per_slot();
    coordinate_report_prefix_and_frame_replay();
    nested_placeholder_artifact_oracle_for(false, None, Some(ExtraNestedOracle::Coordinate));
    for format in FORMATS {
        for kind in [
            "rectangle",
            "ellipse",
            "diamond",
            "line",
            "arrow",
            "freedraw",
            "embeddable",
        ] {
            for field in ["x", "y", "width", "height"] {
                for sign in [1, -1] {
                    if sign < 0 && matches!(field, "width" | "height") {
                        continue;
                    }
                    let mut element = json!({"id":"image","type":kind,"x":0,"y":0,"width":10,"height":10,"seed":1,"roughness":0,"updated":0});
                    if matches!(kind, "line" | "arrow" | "freedraw") {
                        element["points"] = json!([[0, 0], [10, 10]]);
                    }
                    element[field] = json!(21 * sign);
                    let source = json!({"type":"excalidraw","version":2,"elements":[element],"appState":{},"files":{}});
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
                        &format!("/elements/0/{field}"),
                        "omitted",
                    );
                    if format != OutputFormat::Png {
                        row["effects"] = json!(["visual", "semantic"]);
                    }
                    let derived_extent =
                        matches!(kind, "line" | "arrow") && matches!(field, "width" | "height");
                    assert_repeat(
                        &source,
                        &request,
                        if derived_extent {
                            json!([])
                        } else {
                            json!([row])
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn coordinate_point_recovery_and_component_precedence() {
    for format in FORMATS {
        for kind in ["line", "arrow", "freedraw"] {
            for axis in [0, 1] {
                let mut point = json!([10, 10]);
                point[axis] = json!(21);
                let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":kind,"width":10,"height":10,"points":[[0,0],point],"seed":1,"roughness":0,"updated":0}],"appState":{},"files":{}});
                let request = RenderRequest::builder(format)
                    .resource_limits(
                        ResourceLimits::builder()
                            .max_coordinate_magnitude(20.0)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                let field = if kind == "freedraw" {
                    "points"
                } else if axis == 0 {
                    "width"
                } else {
                    "height"
                };
                let recovery = if kind == "freedraw" {
                    "placeholder"
                } else {
                    "omitted"
                };
                let mut row = expected(
                    "resource.coordinate-range-exceeded",
                    "max_coordinate_magnitude",
                    json!(20),
                    json!(21),
                    "coordinate",
                    &format!("/elements/0/{field}"),
                    recovery,
                );
                if format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                assert_repeat(&source, &request, json!([row]));
            }
        }
        let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"rectangle","x":21,"y":22,"width":23,"height":24,"seed":1,"updated":0}],"appState":{},"files":{}});
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(20.0)
                    .max_geometry_segments_per_element(1)
                    .max_element_work_units(1)
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
fn coordinate_limits_admit_non_outline_content_before_resolution() {
    let bytes = crate::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]);
    for format in FORMATS {
        for kind in ["text", "image", "frame", "magicframe"] {
            let mut element =
                json!({"id":"image","type":kind,"x":21,"width":10,"height":10,"updated":0});
            if kind == "text" {
                element["text"] = json!("A");
                element["fontFamily"] = json!(5);
                element["fontSize"] = json!(10);
            }
            if kind == "image" {
                element["fileId"] = json!("asset");
            }
            let source = json!({"type":"excalidraw","version":2,"elements":[element],"appState":{},"files":{}});
            let request = RenderRequest::builder(format)
                .supplemental_assets(
                    SupplementalAssets::builder()
                        .asset("asset", bytes.clone())
                        .build()
                        .unwrap(),
                )
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
            if kind != "image" && format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            assert_repeat(&source, &request, json!([row]));
        }
    }
}

#[test]
fn invalid_frame_coordinates_omit_dependants_and_fail_the_frame_target() {
    for format in FORMATS {
        for kind in ["frame", "magicframe"] {
            let source = json!({"type":"excalidraw","version":2,"elements":[
                {"id":"bad","type":kind,"x":21,"width":10,"height":10,"updated":0},
                {"id":"child","type":"rectangle","x":0,"width":5,"height":5,"frameId":"bad","seed":1,"updated":0},
                {"id":"nested","type":"frame","width":5,"height":5,"frameId":"bad","updated":0},
                {"id":"grandchild","type":"rectangle","width":3,"height":3,"frameId":"nested","seed":1,"updated":0}
            ],"appState":{},"files":{}});
            let scene = parse(&source);
            let limits = ResourceLimits::builder()
                .max_coordinate_magnitude(20.0)
                .build()
                .unwrap();
            let request = RenderRequest::builder(format)
                .resource_limits(limits.clone())
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
            row["element_id"] = json!("bad");
            if format != OutputFormat::Png {
                row["effects"] = json!(["visual", "semantic"]);
            }
            let outcome = assert_repeat(&source, &request, json!([row]));
            let empty =
                json!({"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}});
            let baseline = Renderer::new().render(&parse(&empty), &request).unwrap();
            assert_eq!(outcome.artifact().bytes(), baseline.artifact().bytes());
            let frame = scene.frames().next().unwrap();
            let request = FrameRenderRequest::builder(format)
                .resource_limits(limits)
                .build()
                .unwrap();
            let error = Renderer::new().render_frame(frame, &request).unwrap_err();
            assert_eq!(error.code().as_str(), "resource.coordinate-range-exceeded");
            assert_eq!(
                serde_json::to_value(error.details()).unwrap(),
                json!({"kind":"resource_limit","field":"max_coordinate_magnitude","configured":20,"value_kind":"coordinate","measurement":{"kind":"attempted","value":21},"recovery":null})
            );
        }
    }
}

#[test]
fn invalid_frame_omission_uses_repaired_active_clipping_dependencies() {
    for format in FORMATS {
        let source = json!({"type":"excalidraw","version":2,"elements":[
            {"id":"bad","type":"frame","x":21,"width":10,"height":10,"updated":0},
            {"id":"child","type":"rectangle","width":5,"height":5,"frameId":"bad","seed":1,"updated":0}
        ],"appState":{},"files":{}});
        let options = excalidraw_render::FrameOptions::default()
            .with_enabled(false)
            .with_clip(false)
            .with_show_name(false)
            .with_outline(false);
        let request = RenderRequest::builder(format)
            .frame_options(options)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(20.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut coordinate = expected(
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            json!(20),
            json!(21),
            "coordinate",
            "/elements/0/x",
            "omitted",
        );
        coordinate["element_id"] = json!("bad");
        if format != OutputFormat::Png {
            coordinate["effects"] = json!(["visual", "semantic"]);
        }
        let actual = assert_repeat(&source, &request, json!([coordinate]));
        let mut expected_source = source.clone();
        expected_source["elements"][0]["isDeleted"] = json!(true);
        let expected = Renderer::new()
            .render(&parse(&expected_source), &request)
            .unwrap();
        assert_eq!(
            actual.artifact().bytes(),
            expected.artifact().bytes(),
            "disabled clipping must keep child"
        );
        let mut cycle = source.clone();
        cycle["elements"][0]["frameId"] = json!("good");
        cycle["elements"].as_array_mut().unwrap().insert(
            1,
            json!({"id":"good","type":"frame","width":10,"height":10,"frameId":"bad","updated":0}),
        );
        cycle["elements"][2]["frameId"] = json!("good");
        let request = RenderRequest::builder(format)
            .frame_options(excalidraw_render::FrameOptions::default().with_show_name(false))
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(20.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let cycle_row = json!({"code":"normalization.relationship-cycle","severity":"warning","effects":if format==OutputFormat::Png {json!(["input_repair","visual"])}else{json!(["input_repair","visual","semantic"])},"element_id":"good","json_pointer":"/elements/1/frameId","details":{"kind":"compatibility","recovery":"removed","recipe_id":null}});
        let actual = assert_repeat(&cycle, &request, json!([coordinate, cycle_row]));
        cycle["elements"][0]["isDeleted"] = json!(true);
        cycle["elements"][1]["frameId"] = Value::Null;
        let expected = Renderer::new().render(&parse(&cycle), &request).unwrap();
        assert_eq!(
            actual.artifact().bytes(),
            expected.artifact().bytes(),
            "removed cycle edge cannot hide good frame"
        );
    }
}

#[test]
fn coordinate_rejected_images_classify_authoritative_vector_content() {
    let vector=br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#;
    let raster = crate::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]);
    for format in FORMATS {
        for (bytes, semantic) in [(vector.as_slice(), true), (raster.as_slice(), false)] {
            for supplemental in [false, true] {
                let mut source = image_scene(bytes, "image/png");
                source["elements"][0]["x"] = json!(21);
                let assets = if supplemental {
                    source["files"] = json!({});
                    SupplementalAssets::builder()
                        .asset("asset/~", bytes)
                        .build()
                        .unwrap()
                } else {
                    SupplementalAssets::default()
                };
                let request = RenderRequest::builder(format)
                    .supplemental_assets(assets)
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
                if semantic && format != OutputFormat::Png {
                    row["effects"] = json!(["visual", "semantic"]);
                }
                assert_repeat(&source, &request, json!([row]));
            }
        }
    }
}

#[test]
fn frame_coordinate_recovery_reconciles_fallback_and_missing_image_reports() {
    for format in FORMATS {
        let source = json!({"type":"excalidraw","version":2,"elements":[
            {"id":"bad","type":"frame","x":21,"width":10,"height":10,"updated":0},
            {"id":"fallback","type":"future","width":5,"height":5,"frameId":"bad","updated":0}
        ],"appState":{},"files":{}});
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(20.0)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut coordinate = expected(
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            json!(20),
            json!(21),
            "coordinate",
            "/elements/0/x",
            "omitted",
        );
        coordinate["element_id"] = json!("bad");
        if format != OutputFormat::Png {
            coordinate["effects"] = json!(["visual", "semantic"]);
        }
        let actual = assert_repeat(
            &source,
            &request,
            json!([coordinate,{"code":"compatibility.unsupported-element-type","severity":"warning","effects":["visual","semantic"],"element_id":"fallback","json_pointer":"/elements/1","details":{"kind":"compatibility","recovery":"omitted","recipe_id":null}}]),
        );
        let mut empty = source.clone();
        empty["elements"] = json!([]);
        assert_eq!(
            actual.artifact().bytes(),
            Renderer::new()
                .render(&parse(&empty), &request)
                .unwrap()
                .artifact()
                .bytes()
        );
        let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","fileId":"absent","link":"https://example.com","x":21,"width":10,"height":10,"updated":0}],"appState":{},"files":{}});
        let missing = json!({"code":"image.missing-file","severity":"warning","effects":["visual"],"element_id":"image","json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"absent","claimed_media_type":null,"validated_media_type":null,"recovery":if format==OutputFormat::Png {"placeholder"}else{"omitted"},"recipe_id":if format==OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")}else{Value::Null}}});
        let coordinate = expected(
            "resource.coordinate-range-exceeded",
            "max_coordinate_magnitude",
            json!(20),
            json!(21),
            "coordinate",
            "/elements/0/x",
            "omitted",
        );
        let actual = assert_repeat(&source, &request, json!([missing, coordinate]));
        assert_eq!(
            actual.artifact().bytes(),
            Renderer::new()
                .render(&parse(&empty), &request)
                .unwrap()
                .artifact()
                .bytes()
        );
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

#[test]
pub(super) fn nested_node_limits_cover_root_and_child_reports_and_silence() {
    nested_node_count_includes_text_and_stops_descendant_discovery();
    nested_limit_owner_scope(true);
    nested_placeholder_artifact_oracle(true);
    nested_limit_envelope_and_owner_order(true);
    nested_placeholder_uses_current_reference_bounds_and_local_omission();
    let leaf = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/><rect width="1" height="1" fill="blue"/></svg>"#;
    for format in FORMATS {
        for nested in [false, true] {
            for bounded in [false, true] {
                let parent = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="{}" height="2" href="{}"/></svg>"#,
                    u8::from(bounded),
                    svg_url(leaf)
                );
                let mut source = image_scene(
                    if nested {
                        parent.as_bytes()
                    } else {
                        leaf.as_bytes()
                    },
                    "image/svg+xml",
                );
                if !nested && !bounded {
                    source["elements"][0]["width"] = json!(0);
                }
                for configured in [2, 3, 4] {
                    let request = RenderRequest::builder(format)
                        .resource_limits(
                            ResourceLimits::builder()
                                .max_nested_svg_nodes(configured)
                                .build()
                                .unwrap(),
                        )
                        .build()
                        .unwrap();
                    let rows = if configured < 3 {
                        json!([{
                            "code":"resource.nested-svg-complexity-exceeded","severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                            "details":{"kind":"nested_resource_limit","field":"max_nested_svg_nodes","configured":configured,"value_kind":"count","measurement":{"kind":"attempted","value":3},"asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":if nested {json!([{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}])} else {json!([])}}],"recovery":if bounded {"placeholder"} else {"omitted"},"recipe_id":if bounded {json!("official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677")} else {Value::Null}}
                        }])
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
fn nested_node_count_includes_text_and_stops_descendant_discovery() {
    // root + whitespace + desc + its text + trailing whitespace = five nodes.
    // Neither the comment nor the desc's attribute is a node.
    let text_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"> <desc id="description">hello</desc> <!--comment--></svg>"#;
    let stopped = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect/><image width="2" height="2" href="data:image/png;base64,bm90IGFuIGltYWdl"/></svg>"#;
    for format in FORMATS {
        for nested in [false, true] {
            for (payload, n, discovery) in [(text_svg, 5, false), (stopped, 3, true)] {
                let parent = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image width="2" height="2" href="{}"/></svg>"#,
                    svg_url(payload)
                );
                let source = image_scene(
                    if nested {
                        parent.as_bytes()
                    } else {
                        payload.as_bytes()
                    },
                    "image/svg+xml",
                );
                for configured in [n - 1, n, n + 1] {
                    if discovery && configured >= n {
                        continue;
                    }
                    let request = RenderRequest::builder(format)
                        .resource_limits(
                            ResourceLimits::builder()
                                .max_nested_svg_nodes(configured)
                                .max_asset_entries(if nested { 2 } else { 1 })
                                .build()
                                .unwrap(),
                        )
                        .build()
                        .unwrap();
                    let rows = if configured < n {
                        json!([{
                            "code":"resource.nested-svg-complexity-exceeded","severity":"warning","effects":if format==OutputFormat::Png {json!(["visual"])} else {json!(["visual","semantic"])},"element_id":null,"json_pointer":"/files/asset~1~0/dataURL",
                            "details":{"kind":"nested_resource_limit","field":"max_nested_svg_nodes","configured":configured,"value_kind":"count","measurement":{"kind":"attempted","value":n},"asset_chain":[{"asset_id":"asset/~","occurrence":0}],"paths":[{"segments":if nested {json!([{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":2,"name":"href"}])} else {json!([])}}],"recovery":"placeholder","recipe_id":"official.nested-resource@sha256:0fb00e2eda840ab048f9e0fc0698bb4b43d5618d6f67880149ce164412387677"}
                        }])
                    } else {
                        json!([])
                    };
                    assert_repeat(&source, &request, rows);
                }
            }
        }
    }
}

/// Spec 11.1: a report number that needs an exponent spells it lowercase with
/// an explicit sign, as ECMAScript `JSON.stringify` does (#211).
#[test]
fn report_exponents_are_lowercase_and_signed() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"far","type":"rectangle","x":1e21,"width":1,"height":1,"roughness":0,"seed":1,"updated":1}],"appState":{},"files":{}}"#,
    )
    .unwrap();
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_coordinate_magnitude(1e-7)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        let report = String::from_utf8(serde_json::to_vec(outcome.report()).unwrap()).unwrap();
        assert!(
            report.contains(r#""configured":1e-7,"value_kind":"coordinate","measurement":{"kind":"attempted","value":1e+21}"#),
            "{format:?}: {report}"
        );
    }
}
