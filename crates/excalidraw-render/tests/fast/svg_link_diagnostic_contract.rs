//! Complete report and artifact contracts for nested SVG and scene-link recovery.
use base64::Engine;
use excalidraw_render::{
    FrameRenderRequest, OutputFormat, RenderOutcome, RenderRequest, Renderer, ResourceLimits,
    SceneDocument, SupplementalAssets,
};
use serde_json::{json, Value};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const FORBIDDEN_RECIPE:&str="official.forbidden-resource@sha256:4452f99158233e3703e46dbbb83f6900a894fbceacd3e98b141332d552e22827";

fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn scene(elements: Value, files: Value) -> Value {
    json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":files})
}
fn image(id: &str, file: &str) -> Value {
    json!({"id":id,"type":"image","width":30,"height":20,"fileId":file,"updated":1})
}
fn svg_url(svg: &str) -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    )
}
fn file(url: &str) -> Value {
    json!({"id":"outer","created":1,"dataURL":url})
}
fn row(code: &str, effects: Value, id: Value, pointer: &str, details: Value) -> Value {
    json!({"code":code,"severity":"warning","effects":effects,"element_id":id,"json_pointer":pointer,"details":details})
}
fn forbidden(id: &str, pointer: &str, paint: bool) -> Value {
    row(
        "image.forbidden-resource",
        json!(["visual", "semantic"]),
        Value::Null,
        pointer,
        json!({"kind":"asset","asset_id":id,"claimed_media_type":null,"validated_media_type":null,"recovery":if paint {"placeholder"}else{"omitted"},"recipe_id":if paint {json!(FORBIDDEN_RECIPE)}else{Value::Null}}),
    )
}
fn nested_row(code: &str, effects: Value, segments: Value, recovery: &str) -> Value {
    row(
        code,
        effects,
        Value::Null,
        "/files/outer/dataURL",
        json!({"kind":"nested_source","asset_chain":[{"asset_id":"outer","occurrence":0}],"paths":[{"segments":segments}],"recovery":recovery}),
    )
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
        "source {source}"
    );
    report(first.report(), request.format(), rows);
    assert_eq!(first.report(), second.report());
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    first
}
fn canonical_svg(text: &str) -> Value {
    fn node(n: roxmltree::Node<'_, '_>) -> Value {
        let attributes: serde_json::Map<String, Value> = n
            .attributes()
            .map(|a| {
                let value =
                    if let Some(encoded) = a.value().strip_prefix("data:image/svg+xml;base64,") {
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
        json!({"tag":n.tag_name().name(),"attributes":attributes,"text":n.children().filter(|n|n.is_text()).filter_map(|n|n.text()).collect::<String>(),"children":n.children().filter(|n|n.is_element()).map(node).collect::<Vec<_>>()})
    }
    node(roxmltree::Document::parse(text).unwrap().root_element())
}
fn embedded_svg(bytes: &[u8]) -> Value {
    let doc = roxmltree::Document::parse(std::str::from_utf8(bytes).unwrap()).unwrap();
    let href = doc
        .descendants()
        .find(|n| n.has_tag_name("image"))
        .expect("surviving nested SVG")
        .attribute("href")
        .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(href.strip_prefix("data:image/svg+xml;base64,").unwrap())
        .unwrap();
    canonical_svg(std::str::from_utf8(&bytes).unwrap())
}
fn svg_drawing(bytes: &[u8]) -> Value {
    let mut svg = canonical_svg(std::str::from_utf8(bytes).unwrap());
    svg["children"]
        .as_array_mut()
        .unwrap()
        .retain(|node| node["tag"] != "metadata");
    svg
}
fn drawing_streams(bytes: &[u8]) -> Vec<String> {
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
fn same_image(actual: &RenderOutcome, expected: &RenderOutcome) {
    match actual.artifact().format() {
        OutputFormat::Svg => {
            assert_eq!(
                embedded_svg(actual.artifact().bytes()),
                embedded_svg(expected.artifact().bytes())
            );
            assert_eq!(
                svg_drawing(actual.artifact().bytes()),
                svg_drawing(expected.artifact().bytes())
            );
        }
        OutputFormat::Png => {
            let a = crate::common::raster::decode_png(actual.artifact().bytes());
            let b = crate::common::raster::decode_png(expected.artifact().bytes());
            assert_eq!((a.width, a.height), (b.width, b.height));
            assert_eq!(a.rgba, b.rgba);
        }
        OutputFormat::Pdf => assert_eq!(
            drawing_streams(actual.artifact().bytes()),
            drawing_streams(expected.artifact().bytes())
        ),
        _ => unreachable!(),
    }
}
fn error_recipe() -> String {
    // Frozen image-error paths, not the renderer's recovery constructor.
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
    let paths = recipe["glyph"]["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| {
            let matrix = path["transform"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                r#"<path fill="{}" transform="matrix({matrix})" d="{}"/>"#,
                path["fill"].as_str().unwrap(),
                path["d"].as_str().unwrap()
            )
        })
        .collect::<String>();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20" viewBox="0 0 30 20"><rect width="30" height="20" fill="#E7E7E7"/><svg x="11" y="6" width="8" height="8" viewBox="0 0 668 668">{paths}</svg></svg>"##
    )
}

#[test]
fn forbidden_resource_contract_in_every_format() {
    super::diagnostic_contract::forbidden_resource_reports_bounds_selected_recovery();
    forbidden_scope_and_recovery_envelope();
    nested_forbidden_occurrences_keep_their_own_bounds();
    href_precedence_keeps_recovery_local();
    for format in FORMATS {
        let mut transformed = image("image", "outer");
        transformed["x"] = json!(25);
        transformed["y"] = json!(10);
        transformed["angle"] = json!(std::f64::consts::FRAC_PI_2);
        transformed["scale"] = json!([-1, 1]);
        transformed["opacity"] = json!(50);
        transformed["frameId"] = json!("frame");
        transformed["roundness"] = json!({"type":3,"value":4});
        transformed["crop"] =
            json!({"x":30,"y":0,"width":30,"height":20,"naturalWidth":60,"naturalHeight":20});
        let elements =
            json!([{"id":"frame","type":"frame","width":40,"height":40,"updated":1},transformed]);
        let source = scene(
            elements.clone(),
            json!({"outer":file("https://example.invalid/image.png")}),
        );
        let request = RenderRequest::builder(format)
            .frame_options(excalidraw_render::FrameOptions::new(
                true, true, false, true,
            ))
            .metadata_options(excalidraw_render::MetadataOptions::new(true, false, false))
            .build()
            .unwrap();
        let actual = checked(
            &source,
            &request,
            json!([forbidden("outer", "/files/outer/dataURL", true)]),
        );
        let mut oracle = scene(elements, json!({"outer":file(&svg_url(&error_recipe()))}));
        oracle["elements"][1]
            .as_object_mut()
            .unwrap()
            .remove("crop");
        let reference = Renderer::new().render(&parse(&oracle), &request).unwrap();
        same_image(&actual, &reference);
        assert_eq!(
            super::artifact_metadata::extract(format, actual.artifact().bytes()).1,
            serde_json::to_vec(&source).unwrap()
        );
        for status in ["saved", "error"] {
            let mut element = image("image", "outer");
            element["status"] = json!(status);
            let source = scene(
                json!([element]),
                json!({"outer":file("https://example.invalid/image.png")}),
            );
            let request = RenderRequest::builder(format)
                .metadata_options(excalidraw_render::MetadataOptions::new(true, false, false))
                .build()
                .unwrap();
            let actual = checked(
                &source,
                &request,
                json!([forbidden("outer", "/files/outer/dataURL", true)]),
            );
            let oracle = scene(
                json!([element]),
                json!({"outer":file(&svg_url(&error_recipe()))}),
            );
            let expected = Renderer::new().render(&parse(&oracle), &request).unwrap();
            same_image(&actual, &expected);
            let (notices, original) =
                super::artifact_metadata::extract(format, actual.artifact().bytes());
            assert_eq!(original, serde_json::to_vec(&source).unwrap());
            assert!(notices.contains("font-awesome-free-5.15.4/solid/image@"));
            assert!(notices.contains("font-awesome-free-5.15.4/solid/ban@"));
        }
    }
}

fn forbidden_scope_and_recovery_envelope() {
    for format in FORMATS {
        for url in [
            "https://example.invalid/a.png",
            "file:///unreadable/a.png",
            "relative/a.png",
            "//example.invalid/a.png",
            "",
        ] {
            let source = scene(json!([image("image", "outer")]), json!({"outer":file(url)}));
            let request = RenderRequest::builder(format).build().unwrap();
            checked(
                &source,
                &request,
                json!([forbidden("outer", "/files/outer/dataURL", true)]),
            );
        }
        let mut zero = image("zero", "outer");
        zero["width"] = json!(0);
        zero["height"] = json!(0);
        let mut bounded = image("bounded", "outer");
        bounded["x"] = json!(110);
        let mut deleted = image("deleted", "outer");
        deleted["isDeleted"] = json!(true);
        let source = scene(
            json!([zero,bounded,deleted,
            {"id":"zero-frame","type":"frame","width":80,"height":60,"updated":1},
            {"id":"bounded-frame","type":"frame","x":100,"width":80,"height":60,"updated":1}]),
            json!({"outer":file("https://example.invalid/image.png")}),
        );
        let document = parse(&source);
        let frames: Vec<_> = document.frames().collect();
        let renderer = Renderer::new();
        let request = FrameRenderRequest::builder(format).build().unwrap();
        let mut batch = renderer
            .render_frames(&document, frames.iter().copied(), &request)
            .unwrap();
        for (i, frame) in frames.into_iter().enumerate() {
            let direct = renderer.render_frame(frame, &request).unwrap();
            let replay = batch.next().unwrap().into_result().unwrap();
            report(
                direct.report(),
                format,
                json!([forbidden("outer", "/files/outer/dataURL", i == 1)]),
            );
            assert_eq!(direct.report(), replay.report());
            assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
        }
        let source = scene(
            json!([zero, deleted]),
            json!({"outer":file("https://example.invalid/image.png")}),
        );
        checked(
            &source,
            &RenderRequest::builder(format).build().unwrap(),
            json!([forbidden("outer", "/files/outer/dataURL", false)]),
        );
        checked(
            &scene(
                json!([deleted]),
                json!({"outer":file("https://example.invalid/image.png")}),
            ),
            &RenderRequest::builder(format).build().unwrap(),
            json!([]),
        );
        for (work, coordinate, paint) in [
            (8191, 1e9, false),
            (8192, 1e9, true),
            (8193, 1e9, true),
            (8192, 667.0, false),
            (8192, 668.0, true),
        ] {
            let source = scene(
                json!([image("image", "outer")]),
                json!({"outer":file("https://example.invalid/image.png")}),
            );
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_asset_work_units(work)
                        .max_coordinate_magnitude(coordinate)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let actual = checked(
                &source,
                &request,
                json!([forbidden("outer", "/files/outer/dataURL", paint)]),
            );
            let replacement = if paint {
                error_recipe()
            } else {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#.to_owned()
            };
            let oracle = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&replacement))}),
            );
            let reference = Renderer::new()
                .render(
                    &parse(&oracle),
                    &RenderRequest::builder(format).build().unwrap(),
                )
                .unwrap();
            same_image(&actual, &reference);
            assert_eq!(
                actual
                    .artifact()
                    .bytes()
                    .windows(b"font-awesome".len())
                    .any(|bytes| bytes == b"font-awesome"),
                paint
            );
        }
        let source = scene(
            json!([image("z", "z/~"), image("a", "a"), image("again", "z/~")]),
            json!({
            "z/~":{"id":"z/~","created":1,"dataURL":"https://example.invalid/z"},"a":{"id":"a","created":1,"dataURL":"file:///a"},"unused":file("https://example.invalid/unused")}),
        );
        let request = RenderRequest::builder(format)
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset("z/~", b"never replaces scene authority")
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let conflict = row(
            "asset.supplement-conflict",
            json!(["input_repair"]),
            Value::Null,
            "/files/z~1~0",
            json!({"kind":"asset","asset_id":"z/~","claimed_media_type":null,"validated_media_type":null,"recovery":"ignored","recipe_id":null}),
        );
        checked(
            &source,
            &request,
            json!([
                forbidden("a", "/files/a/dataURL", true),
                conflict,
                forbidden("z/~", "/files/z~1~0/dataURL", true)
            ]),
        );
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
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
        report(
            error.report(),
            format,
            json!([forbidden("a", "/files/a/dataURL", true)]),
        );
        let mut record = file("https://example.invalid/a.png");
        record["mimeType"] = json!(42);
        checked(
            &scene(json!([image("image", "outer")]), json!({"outer":record})),
            &RenderRequest::builder(format).build().unwrap(),
            json!([
                forbidden("outer", "/files/outer/dataURL", true),
                row(
                    "asset.media-type-mismatch",
                    json!(["input_repair"]),
                    Value::Null,
                    "/files/outer/mimeType",
                    json!({"kind":"asset","asset_id":"outer","claimed_media_type":null,"validated_media_type":null,"recovery":"repaired","recipe_id":null})
                )
            ]),
        );
        // Supplemental values are bytes, not fetchable URL references.
        let source = scene(json!([image("image", "outer")]), json!({}));
        let request = RenderRequest::builder(format)
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset("outer", b"https://example.invalid/not-a-payload")
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        checked(
            &source,
            &request,
            json!([row(
                "image.unsupported-format",
                json!(["visual"]),
                json!("image"),
                "",
                json!({"kind":"asset","asset_id":"outer","claimed_media_type":null,"validated_media_type":null,"recovery":if format==OutputFormat::Png {"placeholder"}else{"omitted"},"recipe_id":if format==OutputFormat::Png {json!("official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d")}else{Value::Null}})
            )]),
        );
    }
}

fn nested_forbidden_occurrences_keep_their_own_bounds() {
    let parent = |href: &str| {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><defs><clipPath id="cut"><rect width="25" height="40"/></clipPath></defs><g clip-path="url(#cut)"><image x="5" y="3" width="30" height="20" opacity="0.5" transform="translate(2 1)" href="{href}"/></g><rect x="40" y="5" width="10" height="10" fill="blue"/></svg>"#
        )
    };
    for format in FORMATS {
        let mixed = r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="20"><image href="https://example.invalid/a.png" width="30" height="20"/><rect onclick="never()" x="30" width="30" height="20"/></svg>"#;
        checked(
            &scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(mixed))}),
            ),
            &RenderRequest::builder(format).build().unwrap(),
            json!([
                nested_row(
                    "image.forbidden-resource",
                    json!(["visual", "semantic"]),
                    json!([{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":0,"name":"href"}]),
                    "placeholder"
                ),
                nested_row(
                    "image.unsafe-svg-content-removed",
                    json!(["semantic"]),
                    json!([{"kind":"node","ordinal":2,"name":null},{"kind":"attribute","ordinal":0,"name":"onclick"}]),
                    "removed"
                )
            ]),
        );
        for nested in [false, true] {
            let wrap = |s: &str| {
                if nested {
                    format!(
                        r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><image width="60" height="40" href="{}"/></svg>"#,
                        svg_url(s)
                    )
                } else {
                    s.to_owned()
                }
            };
            let mut element = image("image", "outer");
            element["width"] = json!(60);
            element["height"] = json!(40);
            let source = scene(
                json!([element]),
                json!({"outer":file(&svg_url(&wrap(&parent("https://example.invalid/a.png"))))}),
            );
            let mut path = vec![];
            if nested {
                path.extend([
                    json!({"kind":"node","ordinal":1,"name":null}),
                    json!({"kind":"attribute","ordinal":2,"name":"href"}),
                ]);
            }
            path.extend([
                json!({"kind":"node","ordinal":5,"name":null}),
                json!({"kind":"attribute","ordinal":6,"name":"href"}),
            ]);
            let request = RenderRequest::builder(format).padding(0.0).build().unwrap();
            let actual = checked(
                &source,
                &request,
                json!([nested_row(
                    "image.forbidden-resource",
                    json!(["visual", "semantic"]),
                    json!(path),
                    "placeholder"
                )]),
            );
            let oracle = scene(
                json!([element]),
                json!({"outer":file(&svg_url(&wrap(&parent(&svg_url(&error_recipe())))))}),
            );
            let expected = Renderer::new().render(&parse(&oracle), &request).unwrap();
            same_image(&actual, &expected);
            if format == OutputFormat::Png {
                let pixels = crate::common::raster::decode_png(actual.artifact().bytes());
                assert_eq!(pixels.pixel(10, 8), [243, 243, 243, 255]);
                assert_eq!(pixels.pixel(30, 8), [255, 255, 255, 255]);
                assert_eq!(pixels.pixel(45, 8), [0, 0, 255, 255]);
            }
        }
        for (width, work) in [("0", 8192), ("30", 8191)] {
            let body = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><image width="{width}" height="20" href="file:///a"/><use href="https://example.invalid/a.svg#x"/><rect width="5" height="5" fill="red"/></svg>"#
            );
            let source = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&body))}),
            );
            let rows:Vec<_>=[(1,2),(2,0)].into_iter().map(|(node,attr)|nested_row("image.forbidden-resource",json!(["visual","semantic"]),json!([{"kind":"node","ordinal":node,"name":null},{"kind":"attribute","ordinal":attr,"name":"href"}]),"omitted")).collect();
            let request = RenderRequest::builder(format)
                .resource_limits(
                    ResourceLimits::builder()
                        .max_asset_work_units(work)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            let actual = checked(&source, &request, json!(rows));
            let clean = body
                .replace(" href=\"file:///a\"", "")
                .replace(" href=\"https://example.invalid/a.svg#x\"", "");
            let oracle = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&clean))}),
            );
            let expected = Renderer::new()
                .render(
                    &parse(&oracle),
                    &RenderRequest::builder(format).build().unwrap(),
                )
                .unwrap();
            same_image(&actual, &expected);
        }
    }
}

#[test]
fn unsafe_svg_contract_in_every_format() {
    super::diagnostic_contract::unsafe_svg_categories_follow_static_effects();
    super::diagnostic_contract::unsafe_svg_attributes_and_safe_styles_are_explicit();
    super::diagnostic_contract::unsafe_nested_siblings_sort_by_full_source_path();
    super::diagnostic_contract::nested_failure_and_removal_share_one_source_order();
    super::diagnostic_contract::unsafe_svg_removals_have_exact_effects_and_silence();
    super::diagnostic_contract::nested_event_handler_removal_keeps_its_full_path_and_semantic_effect();
    unsafe_svg_artifact_oracles();
    unsafe_svg_owners_frames_and_prefixes();
    repeated_nested_unsafe_boundaries();
    entity_removals_keep_source_boundaries();
}

fn href_precedence_keeps_recovery_local() {
    for format in FORMATS {
        for reverse in [false, true] {
            let attributes = if reverse {
                r#"xlink:href="https://example.invalid/ignored" href="https://example.invalid/effective""#
            } else {
                r#"href="https://example.invalid/effective" xlink:href="https://example.invalid/ignored""#
            };
            let wrap = |attributes: &str| {
                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="60" height="20"><image width="30" height="20" {attributes}/><rect x="40" width="10" height="10" fill="blue"/></svg>"#
                )
            };
            let source = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&wrap(attributes)))}),
            );
            let request = RenderRequest::builder(format).build().unwrap();
            let actual = checked(
                &source,
                &request,
                json!([nested_row(
                    "image.forbidden-resource",
                    json!(["visual", "semantic"]),
                    json!([{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":if reverse {3}else{2},"name":"href"}]),
                    "placeholder"
                )]),
            );
            let reference = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&wrap(&format!("href=\"{}\"",svg_url(&error_recipe())))))}),
            );
            same_image(&actual, &checked(&reference, &request, json!([])));
            let safe = format!(
                "href=\"{}\" xlink:href=\"https://example.invalid/ignored\"",
                svg_url(&error_recipe())
            );
            let source = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&wrap(&safe)))}),
            );
            same_image(
                &checked(&source, &request, json!([])),
                &checked(&reference, &request, json!([])),
            );
        }
    }
}

fn entity_removals_keep_source_boundaries() {
    for format in FORMATS {
        for (body, clean, paths) in [
            (
                r#"<text y="10"><![CDATA[A]]>&e;</text><text y="20"><![CDATA[B]]>&e;</text>"#,
                r#"<text y="10">A</text><text y="20">B</text>"#,
                json!([[{"kind":"node","ordinal":2,"name":null}],[{"kind":"node","ordinal":4,"name":null}]]),
            ),
            (
                r#"<text>&e;<![CDATA[A]]></text><rect onclick="never()" width="30" height="20" fill="red"/>"#,
                r#"<text>A</text><rect width="30" height="20" fill="red"/>"#,
                json!([[{"kind":"node","ordinal":2,"name":null}],[{"kind":"node","ordinal":3,"name":null},{"kind":"attribute","ordinal":0,"name":"onclick"}]]),
            ),
            (
                r#"<text y="10">A&e;</text><text y="20">&e;B</text>"#,
                r#"<text y="10">A</text><text y="20">B</text>"#,
                json!([[{"kind":"node","ordinal":2,"name":null}],[{"kind":"node","ordinal":4,"name":null}]]),
            ),
            (
                r#"<text>&e;</text><rect onclick="never()" width="30" height="20" fill="red"/>"#,
                r#"<text></text><rect width="30" height="20" fill="red"/>"#,
                json!([[{"kind":"node","ordinal":2,"name":null}],[{"kind":"node","ordinal":3,"name":null},{"kind":"attribute","ordinal":0,"name":"onclick"}]]),
            ),
            (
                r#"<rect fill="re&e;d" width="30" height="20"/>"#,
                r#"<rect fill="red" width="30" height="20"/>"#,
                json!([[{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":0,"name":"fill"}]]),
            ),
            (
                r#"<!-- <!DOCTYPE svg>&e; --><text><![CDATA[<!DOCTYPE svg>&e;]]></text>"#,
                r#"<text>&lt;!DOCTYPE svg&gt;&amp;e;</text>"#,
                json!([]),
            ),
        ] {
            let wrap = |body: &str| {
                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20">{body}</svg>"#
                )
            };
            let bad = format!(r#"<!DOCTYPE svg [<!ENTITY e "untrusted">]>{}"#, wrap(body));
            let source = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&bad))}),
            );
            let mut rows = vec![nested_row(
                "image.unsafe-svg-content-removed",
                json!(["semantic"]),
                json!([{"kind":"node","ordinal":0,"name":null}]),
                "removed",
            )];
            for path in paths.as_array().unwrap() {
                let event = path.as_array().unwrap().last().unwrap()["name"] == "onclick";
                rows.push(nested_row(
                    "image.unsafe-svg-content-removed",
                    if event {
                        json!(["semantic"])
                    } else {
                        json!(["visual", "semantic"])
                    },
                    path.clone(),
                    "removed",
                ));
            }
            let request = RenderRequest::builder(format).build().unwrap();
            let actual = checked(&source, &request, json!(rows));
            let reference = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&wrap(clean)))}),
            );
            same_image(&actual, &checked(&reference, &request, json!([])));
        }
    }
}

fn repeated_nested_unsafe_boundaries() {
    let bad = r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><rect onclick="never()" width="30" height="20" fill="red"/></svg>"#;
    let clean = bad.replace(" onclick=\"never()\"", "");
    let parent = |child: &str| {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="20"><image width="30" height="20" href="{}"/><image x="30" width="30" height="20" href="{}"/></svg>"#,
            svg_url(child),
            svg_url(child)
        )
    };
    let mut second = image("second", "outer");
    second["x"] = json!(40);
    for format in FORMATS {
        let source = scene(
            json!([image("first", "outer"), second]),
            json!({"outer":file(&svg_url(&parent(bad)))}),
        );
        let rows:Vec<_>=[(1,2),(2,3)].into_iter().map(|(ordinal,attribute)|nested_row("image.unsafe-svg-content-removed",json!(["semantic"]),json!([
            {"kind":"node","ordinal":ordinal,"name":null},{"kind":"attribute","ordinal":attribute,"name":"href"},
            {"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":0,"name":"onclick"}]),"removed")).collect();
        let request = RenderRequest::builder(format).build().unwrap();
        let actual = checked(&source, &request, json!(rows));
        let oracle = scene(
            json!([image("first", "outer"), second]),
            json!({"outer":file(&svg_url(&parent(&clean)))}),
        );
        let reference = checked(&oracle, &request, json!([]));
        same_image(&actual, &reference);
    }
}

fn unsafe_svg_artifact_oracles() {
    let cases = [
        (
            r#"<script>never()</script><rect width="30" height="20" fill="red"/>"#,
            r#"<rect width="30" height="20" fill="red"/>"#,
            false,
            None,
        ),
        (
            r#"<foreignObject><div xmlns="http://www.w3.org/1999/xhtml">lost</div></foreignObject><rect width="30" height="20" fill="red"/>"#,
            r#"<rect width="30" height="20" fill="red"/>"#,
            true,
            None,
        ),
        (
            r#"<style>@import url(https://example.invalid/style); rect {fill:red}</style><rect width="30" height="20"/>"#,
            r#"<rect width="30" height="20"/>"#,
            true,
            None,
        ),
        (
            r#"<animate attributeName="opacity" values="0;1"/><rect width="30" height="20" fill="red"/>"#,
            r#"<rect width="30" height="20" fill="red"/>"#,
            true,
            None,
        ),
        (
            r#"<g xmlns="https://example.invalid/foreign"><rect/></g><rect width="30" height="20" fill="red"/>"#,
            r#"<rect width="30" height="20" fill="red"/>"#,
            true,
            None,
        ),
        (
            r#"<rect onclick="never()" width="30" height="20" fill="red"/>"#,
            r#"<rect width="30" height="20" fill="red"/>"#,
            false,
            Some("onclick"),
        ),
        (
            r#"<rect style="fill:url(https://example.invalid/paint)" width="30" height="20" fill="blue"/>"#,
            r#"<rect width="30" height="20" fill="blue"/>"#,
            true,
            Some("style"),
        ),
        (
            r#"<a href="https://example.invalid/page"><rect width="30" height="20" fill="red"/></a>"#,
            r#"<a><rect width="30" height="20" fill="red"/></a>"#,
            false,
            Some("href"),
        ),
    ];
    for format in FORMATS {
        for (bad, clean, visual, attribute) in cases {
            let wrap = |s: &str| {
                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20">{s}</svg>"#
                )
            };
            let mut segments = vec![json!({"kind":"node","ordinal":1,"name":null})];
            if let Some(name) = attribute {
                segments.push(json!({"kind":"attribute","ordinal":0,"name":name}));
            }
            let expected_row = nested_row(
                "image.unsafe-svg-content-removed",
                if visual {
                    json!(["visual", "semantic"])
                } else {
                    json!(["semantic"])
                },
                json!(segments),
                "removed",
            );
            let source = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&wrap(bad)))}),
            );
            let oracle = scene(
                json!([image("image", "outer")]),
                json!({"outer":file(&svg_url(&wrap(clean)))}),
            );
            let request = RenderRequest::builder(format).build().unwrap();
            let actual = checked(&source, &request, json!([expected_row]));
            let clean = checked(&oracle, &request, json!([]));
            same_image(&actual, &clean);
            let metadata = RenderRequest::builder(format)
                .metadata_options(excalidraw_render::MetadataOptions::new(true, false, false))
                .build()
                .unwrap();
            let embedded = checked(&source, &metadata, json!([expected_row]));
            assert_eq!(
                super::artifact_metadata::extract(format, embedded.artifact().bytes()).1,
                serde_json::to_vec(&source).unwrap()
            );
        }
    }
}

fn unsafe_svg_owners_frames_and_prefixes() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><rect onclick="never()" width="30" height="20" fill="red"/></svg>"#;
    let clean = svg.replace(" onclick=\"never()\"", "");
    let mut a = image("a-image", "a");
    a["x"] = json!(5);
    let mut z = image("z-image", "z/~");
    z["x"] = json!(45);
    let mut again = z.clone();
    again["id"] = json!("again");
    again["x"] = json!(85);
    for format in FORMATS {
        for supplemental in [false, true] {
            // Source file order is z then a, opposite both lexical ID order
            // and first reference order. Supplemental owners use ID byte order.
            let source_text = format!(
                r#"{{"type":"excalidraw","version":2,"elements":{},"appState":{{}},"files":{{"z/~":{{"id":"z/~","created":1,"dataURL":"{}"}},"a":{{"id":"a","created":1,"dataURL":"{}"}},"unused":{{"id":"unused","created":1,"dataURL":"{}"}}}}}}"#,
                json!([a,z,again,{"id":"frame","type":"frame","width":140,"height":80,"updated":1},{"id":"other","type":"frame","x":500,"width":140,"height":80,"updated":1}]),
                svg_url(svg),
                svg_url(svg),
                svg_url(svg)
            );
            let source = if supplemental {
                parse(&scene(
                    json!([a,z,again,{"id":"frame","type":"frame","width":140,"height":80,"updated":1},{"id":"other","type":"frame","x":500,"width":140,"height":80,"updated":1}]),
                    json!({}),
                ))
            } else {
                SceneDocument::parse(source_text.as_bytes()).unwrap()
            };
            let assets = if supplemental {
                SupplementalAssets::builder()
                    .asset("z/~", svg.as_bytes())
                    .asset("a", svg.as_bytes())
                    .asset("unused", svg.as_bytes())
                    .build()
                    .unwrap()
            } else {
                SupplementalAssets::default()
            };
            let ids = if supplemental {
                ["a", "z/~"]
            } else {
                ["z/~", "a"]
            };
            let rows:Vec<_>=ids.into_iter().map(|id|{
                let mut r=nested_row("image.unsafe-svg-content-removed",json!(["semantic"]),json!([{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":0,"name":"onclick"}]),"removed");
                r["json_pointer"]=json!(if supplemental {String::new()}else{format!("/files/{}/dataURL",id.replace('~',"~0").replace('/',"~1"))});r["details"]["asset_chain"][0]["asset_id"]=json!(id);r
            }).collect();
            let request = RenderRequest::builder(format)
                .supplemental_assets(assets.clone())
                .frame_options(excalidraw_render::FrameOptions::new(
                    true, true, false, true,
                ))
                .build()
                .unwrap();
            let first = Renderer::new().render(&source, &request).unwrap();
            let second = Renderer::new().render(&source, &request).unwrap();
            report(first.report(), format, json!(rows));
            assert_eq!(first.report(), second.report());
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            let frame_request = FrameRenderRequest::builder(format)
                .supplemental_assets(assets)
                .build()
                .unwrap();
            let frames: Vec<_> = source.frames().collect();
            let renderer = Renderer::new();
            let mut batch = renderer
                .render_frames(&source, frames.iter().copied(), &frame_request)
                .unwrap();
            for (i, frame) in frames.into_iter().enumerate() {
                let direct = renderer.render_frame(frame, &frame_request).unwrap();
                let replay = batch.next().unwrap().into_result().unwrap();
                report(
                    direct.report(),
                    format,
                    if i == 0 { json!(rows) } else { json!([]) },
                );
                assert_eq!(direct.report(), replay.report());
                assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
            }
        }
        // A scene payload wins over an unsafe supplemental payload.
        let source = scene(
            json!([image("image", "outer")]),
            json!({"outer":file(&svg_url(&clean))}),
        );
        let request = RenderRequest::builder(format)
            .supplemental_assets(
                SupplementalAssets::builder()
                    .asset("outer", svg.as_bytes())
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        checked(
            &source,
            &request,
            json!([row(
                "asset.supplement-conflict",
                json!(["input_repair"]),
                Value::Null,
                "/files/outer",
                json!({"kind":"asset","asset_id":"outer","claimed_media_type":null,"validated_media_type":null,"recovery":"ignored","recipe_id":null})
            )]),
        );
        let flood = r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><g onload="first()"><rect onclick="second()" width="30" height="20"/></g></svg>"#;
        let source = scene(
            json!([image("image", "outer")]),
            json!({"outer":file(&svg_url(flood))}),
        );
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
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
        report(
            error.report(),
            format,
            json!([nested_row(
                "image.unsafe-svg-content-removed",
                json!(["semantic"]),
                json!([{"kind":"node","ordinal":1,"name":null},{"kind":"attribute","ordinal":0,"name":"onload"}]),
                "removed"
            )]),
        );
    }
}

fn link_row(id: &str, index: usize) -> Value {
    row(
        "link.unsafe-omitted",
        json!(["semantic"]),
        json!(id),
        &format!("/elements/{index}/link"),
        json!({"kind":"compatibility","recovery":"omitted","recipe_id":null}),
    )
}
fn linked_source(kind: &str, link: Value) -> Value {
    let mut element = json!({"id":"subject","type":kind,"width":80,"height":10,"seed":1,"roughness":0,"updated":1,"link":link});
    if matches!(kind, "line" | "arrow" | "freedraw") {
        element["points"] = json!([[0, 0], [80, 10]]);
    }
    if kind == "text" {
        element["text"] = json!("A");
        element["fontFamily"] = json!(5);
        element["fontSize"] = json!(10);
        element["lineHeight"] = json!(1.25);
    }
    let mut files = json!({});
    if kind == "image" {
        element["fileId"] = json!("outer");
        let bytes = crate::common::codec_fixtures::solid_png(1, 1, [255, 0, 0, 255]);
        files["outer"] = file(&format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
    }
    scene(json!([element]), files)
}
fn link_request(format: OutputFormat) -> RenderRequest {
    RenderRequest::builder(format)
        .frame_options(excalidraw_render::FrameOptions::new(
            true, true, false, true,
        ))
        .build()
        .unwrap()
}
fn assert_no_navigation(outcome: &RenderOutcome) {
    match outcome.artifact().format() {
        OutputFormat::Svg => {
            let doc = roxmltree::Document::parse(
                std::str::from_utf8(outcome.artifact().bytes()).unwrap(),
            )
            .unwrap();
            assert!(!doc.descendants().any(|n| n.has_tag_name("a")));
        }
        OutputFormat::Pdf => assert!(!crate::common::pdf_inspect::pdf_text(
            outcome.artifact().bytes()
        )
        .contains("/Subtype /Link")),
        OutputFormat::Png => {}
        _ => unreachable!(),
    }
}
fn compare_link_paint(actual: &RenderOutcome, reference: &RenderOutcome, embeddable: bool) {
    assert_no_navigation(actual);
    if !embeddable || actual.artifact().format() == OutputFormat::Png {
        assert_eq!(actual.artifact().bytes(), reference.artifact().bytes());
        return;
    }
    match actual.artifact().format() {
        OutputFormat::Svg => {
            // Remove only the reference's about:blank anchor and its transparent
            // hit rectangle; the actual artifact independently has no navigation.
            let reference = std::str::from_utf8(reference.artifact().bytes())
                .unwrap()
                .replace("<a href=\"about:blank\">", "")
                .replace("</a>", "")
                .replace(
                    "<rect x=\"0\" y=\"0\" width=\"80\" height=\"10\" fill=\"transparent\"/>",
                    "",
                );
            assert_eq!(
                canonical_svg(std::str::from_utf8(actual.artifact().bytes()).unwrap()),
                canonical_svg(&reference)
            );
        }
        OutputFormat::Pdf => assert_eq!(
            drawing_streams(actual.artifact().bytes()),
            drawing_streams(reference.artifact().bytes())
        ),
        _ => unreachable!(),
    }
}

#[test]
fn unsafe_link_contract_in_semantic_formats() {
    super::compatibility_fallback_render::pinned_link_sanitizer_preserves_its_safe_canonical_value_and_omits_active_actions();
    super::compatibility_fallback_render::supported_linked_elements_keep_navigation_in_semantic_formats();
    super::compatibility_fallback_render::pdf_link_annotations_are_ordered_from_topmost_to_bottommost();
    link_kind_and_generated_label_contract();
    link_value_order_and_silence_contract();
    generated_link_frame_contract();
}

fn link_kind_and_generated_label_contract() {
    for format in FORMATS {
        for kind in [
            "rectangle",
            "ellipse",
            "diamond",
            "line",
            "arrow",
            "freedraw",
            "text",
            "image",
            "frame",
            "magicframe",
            "future",
            "iframe",
            "embeddable",
        ] {
            let source = linked_source(kind, json!("javascript:alert(1)"));
            let mut rows = vec![];
            if kind == "future" {
                rows.push(row("compatibility.unsupported-element-type",json!(["visual","semantic"]),json!("subject"),"/elements/0",json!({"kind":"compatibility","recovery":"placeholder","recipe_id":"renderer.unknown-element.v1"})));
            }
            if format != OutputFormat::Png {
                rows.push(link_row("subject", 0));
            }
            let request = link_request(format);
            let actual = checked(&source, &request, json!(rows));
            let mut oracle = source.clone();
            oracle["elements"][0]["link"] = if kind == "embeddable" {
                json!("about:blank")
            } else {
                Value::Null
            };
            let reference = Renderer::new().render(&parse(&oracle), &request).unwrap();
            compare_link_paint(&actual, &reference, kind == "embeddable");
            if matches!(kind, "iframe" | "embeddable") {
                let label = if kind == "iframe" {
                    "IFrame element"
                } else {
                    "about:blank"
                };
                match format {
                    OutputFormat::Svg => assert!(std::str::from_utf8(actual.artifact().bytes())
                        .unwrap()
                        .contains(&format!("data-source-text=\"{label}\""))),
                    OutputFormat::Pdf => assert!(crate::common::pdf_inspect::pdf_actual_text(
                        &crate::common::pdf_inspect::pdf_text(actual.artifact().bytes())
                    )
                    .contains(label)),
                    OutputFormat::Png => {}
                    _ => unreachable!(),
                }
            }
            let mut deleted = source.clone();
            deleted["elements"][0]["isDeleted"] = json!(true);
            checked(&deleted, &request, json!([]));
        }
    }
}

fn link_value_order_and_silence_contract() {
    for format in FORMATS {
        for link in [
            "",
            " \t\n",
            "JaVaScRiPt:alert(1)",
            "java&#115;cript:alert(1)",
            "javascript&colon;alert(1)",
            "data:text/html,bad",
            "vbscript:bad",
        ] {
            let source = linked_source("rectangle", json!(link));
            let request = link_request(format);
            let actual = checked(
                &source,
                &request,
                if format == OutputFormat::Png {
                    json!([])
                } else {
                    json!([link_row("subject", 0)])
                },
            );
            let reference = Renderer::new()
                .render(&parse(&linked_source("rectangle", Value::Null)), &request)
                .unwrap();
            compare_link_paint(&actual, &reference, false);
        }
        for value in [Value::Null, json!(false), json!(42), json!([]), json!({})] {
            let source = linked_source("rectangle", value.clone());
            let rows = if value.is_null() || format == OutputFormat::Png {
                json!([])
            } else {
                json!([row(
                    "compatibility.unsupported-property-value",
                    json!(["semantic"]),
                    json!("subject"),
                    "/elements/0/link",
                    json!({"kind":"compatibility","recovery":"omitted","recipe_id":null})
                )])
            };
            checked(&source, &link_request(format), rows);
        }
        for (link, canonical) in [
            ("  https://example.com/a\tb  ", "https://example.com/ab"),
            ("/docs", "/docs"),
            ("mailto:a@example.com", "mailto:a@example.com"),
        ] {
            let actual = checked(
                &linked_source("rectangle", json!(link)),
                &link_request(format),
                json!([]),
            );
            let reference = checked(
                &linked_source("rectangle", json!(canonical)),
                &link_request(format),
                json!([]),
            );
            assert_eq!(actual.artifact().bytes(), reference.artifact().bytes());
        }
        let mut source = linked_source("rectangle", json!("javascript:first()"));
        source["elements"][0]["id"] = json!("z");
        let mut second = source["elements"][0].clone();
        second["id"] = json!("a");
        second["link"] = json!("data:text/html,second");
        source["elements"].as_array_mut().unwrap().push(second);
        let rows = if format == OutputFormat::Png {
            json!([])
        } else {
            json!([link_row("z", 0), link_row("a", 1)])
        };
        checked(&source, &link_request(format), rows);
        let request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_diagnostics(1)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        if format == OutputFormat::Png {
            checked(&source, &request, json!([]));
        } else {
            let error = Renderer::new()
                .render(&parse(&source), &request)
                .unwrap_err();
            assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
            report(error.report(), format, json!([link_row("z", 0)]));
        }
        let request = RenderRequest::builder(format)
            .metadata_options(excalidraw_render::MetadataOptions::new(true, false, false))
            .build()
            .unwrap();
        let actual = checked(
            &source,
            &request,
            if format == OutputFormat::Png {
                json!([])
            } else {
                json!([link_row("z", 0), link_row("a", 1)])
            },
        );
        assert_eq!(
            super::artifact_metadata::extract(format, actual.artifact().bytes()).1,
            serde_json::to_vec(&source).unwrap()
        );
    }
}

fn generated_link_frame_contract() {
    for format in FORMATS {
        for kind in ["rectangle", "iframe", "embeddable"] {
            let mut source = linked_source(kind, json!("javascript:never()"));
            source["elements"].as_array_mut().unwrap().extend([
                json!({"id":"frame","type":"frame","width":120,"height":60,"updated":1}),
                json!({"id":"other","type":"frame","x":500,"width":120,"height":60,"updated":1}),
            ]);
            let document = parse(&source);
            let frames: Vec<_> = document.frames().collect();
            let renderer = Renderer::new();
            let request = FrameRenderRequest::builder(format).build().unwrap();
            let mut batch = renderer
                .render_frames(&document, frames.iter().copied(), &request)
                .unwrap();
            for (index, frame) in frames.into_iter().enumerate() {
                let direct = renderer.render_frame(frame, &request).unwrap();
                let replay = batch.next().unwrap().into_result().unwrap();
                report(
                    direct.report(),
                    format,
                    if index == 0 && format != OutputFormat::Png {
                        json!([link_row("subject", 0)])
                    } else {
                        json!([])
                    },
                );
                assert_eq!(direct.report(), replay.report());
                assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
                assert_no_navigation(&direct);
                let mut oracle = source.clone();
                oracle["elements"][0]["link"] = if kind == "embeddable" {
                    json!("about:blank")
                } else {
                    Value::Null
                };
                let reference_scene = parse(&oracle);
                let reference = renderer
                    .render_frame(reference_scene.frames().nth(index).unwrap(), &request)
                    .unwrap();
                compare_link_paint(&direct, &reference, kind == "embeddable" && index == 0);
            }
        }
    }
}
