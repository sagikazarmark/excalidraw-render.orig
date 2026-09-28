//! Public-facade PDF closure reports, with authored source paths and geometry.
use base64::Engine as _;
use excalidraw_render::{
    CompatibilityReport, FrameRenderRequest, OutputFormat, PdfOptions, PdfRasterFallback,
    RenderOutcome, RenderRequest, Renderer, ResourceLimits, SceneDocument, Sizing,
    SupplementalAssets,
};
use serde_json::{json, Value};

const BLUR: &str = r#"<feGaussianBlur stdDeviation="1"/>"#;
fn data(svg: &str) -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    )
}
fn svg(primitives: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%">{primitives}</filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#effect)"/></svg>"##
    )
}
fn image(id: &str, file: &str, x: f64) -> Value {
    json!({"id":id,"type":"image","x":x,"y":0,"width":20,"height":10,"fileId":file,"status":"saved","scale":[1,1],"seed":1,"updated":1})
}
fn source(svg: &str) -> Value {
    json!({"type":"excalidraw","version":2,"elements":[image("image","asset/~",0.0)],"appState":{},"files":{"asset/~":{"id":"asset/~","dataURL":data(svg),"mimeType":"image/svg+xml","created":1}}})
}
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn request() -> RenderRequest {
    RenderRequest::builder(OutputFormat::Pdf).build().unwrap()
}
fn segment(kind: &str, ordinal: u64, name: Value) -> Value {
    json!({"kind":kind,"ordinal":ordinal,"name":name})
}
fn node(ordinal: u64) -> Value {
    json!({"segments":[segment("node",ordinal,Value::Null)]})
}
fn attribute(node: u64, ordinal: u64, name: &str, kind: &str) -> Value {
    json!({"segments":[segment("node",node,Value::Null),segment(kind,ordinal,json!(name))]})
}
fn causes(usage: u64, attr: u64, primitives: &[u64]) -> Value {
    let mut paths = vec![attribute(usage, attr, "filter", "attribute")];
    paths.extend(primitives.iter().map(|&p| node(p)));
    json!(paths)
}
fn row(id: &str, pointer: &str, bounds: [f64; 4], pixels: [u64; 2], causes: Value) -> Value {
    let bounds = bounds.map(|n| {
        if n.fract() == 0.0 {
            json!(n as i64)
        } else {
            json!(n)
        }
    });
    json!({"code":"pdf.raster-fallback","severity":"warning","format":"pdf","effects":["visual","semantic"],"element_id":id,"json_pointer":pointer,
        "details":{"kind":"pdf_raster_fallback","recovery":"rasterized","logical_bounds":{"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3]},"pixel_width":pixels[0],"pixel_height":pixels[1],"causes":causes}})
}
fn report(actual: &CompatibilityReport, expected: Value) {
    let mut actual = serde_json::to_value(actual).unwrap();
    for diagnostic in actual["diagnostics"].as_array_mut().unwrap() {
        assert!(diagnostic["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty()));
        diagnostic.as_object_mut().unwrap().remove("message");
    }
    assert_eq!(actual, json!({"schema_version":1,"diagnostics":expected}));
}
fn checked(source: &Value, request: &RenderRequest, expected: Value) -> RenderOutcome {
    let document = parse(source);
    let first = Renderer::new().render(&document, request).unwrap();
    let second = Renderer::new().render(&document, request).unwrap();
    report(first.report(), expected);
    assert_eq!(
        serde_json::to_vec(first.report()).unwrap(),
        serde_json::to_vec(second.report()).unwrap()
    );
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    first
}

struct ImagePlane {
    object: usize,
    soft_mask: Option<usize>,
    width: usize,
    height: usize,
    gray: bool,
    bytes: Vec<u8>,
}
fn image_planes(pdf: &[u8]) -> Vec<ImagePlane> {
    let pdf = crate::common::pdf_inspect::inflate_streams(pdf);
    let mut planes = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = pdf[cursor..].windows(7).position(|w| w == b"stream\n") {
        let start = cursor + offset + 7;
        let header = String::from_utf8_lossy(&pdf[cursor..start]);
        let integer = |key: &str| {
            header
                .rsplit(key)
                .next()
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap()
        };
        let length = integer("/Length ");
        if header.contains("/Subtype /Image") {
            assert!(header.contains("/BitsPerComponent 8"));
            assert!(!header.contains("/DecodeParms"));
            planes.push(ImagePlane {
                object: header
                    .rsplit_once(" 0 obj\n")
                    .unwrap()
                    .0
                    .split_whitespace()
                    .last()
                    .unwrap()
                    .parse()
                    .unwrap(),
                soft_mask: header
                    .rsplit_once("/SMask ")
                    .map(|(_, tail)| tail.split_whitespace().next().unwrap().parse().unwrap()),
                width: integer("/Width "),
                height: integer("/Height "),
                gray: header.contains("/ColorSpace /DeviceGray"),
                bytes: pdf[start..start + length].to_vec(),
            });
        }
        cursor = start + length + b"\nendstream".len();
    }
    planes
}
fn isolated_pixels_match_source_effect(pdf: &[u8], svg: &str) {
    isolated_pixels_match_region(pdf, svg, [0, 0, 20, 10]);
}
fn isolated_pixels_match_region(pdf: &[u8], svg: &str, region: [usize; 4]) {
    let [left, top, width, height] = region;
    let planes = image_planes(pdf);
    let rgb: Vec<_> = planes.iter().filter(|p| !p.gray).collect();
    assert_eq!(
        rgb.len(),
        1,
        "one isolated color surface, not a page raster"
    );
    let rgb = rgb[0];
    assert_eq!((rgb.width, rgb.height), (width, height));
    assert_eq!(rgb.bytes.len(), width * height * 3);
    let masks: Vec<_> = planes.iter().filter(|p| p.gray).collect();
    assert!(masks.len() <= 1);
    if let Some(mask) = masks.first() {
        assert_eq!(
            (mask.width, mask.height, mask.bytes.len()),
            (width, height, width * height)
        );
    }
    // The authored SVG, directly rasterized over transparent pixels, is the
    // appearance oracle; no PDF plan or production diagnostic builds it.
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
    let mut expected = tiny_skia::Pixmap::new(20, 10).unwrap();
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut expected.as_mut(),
    );
    for i in 0..width * height {
        let offset = ((top + i / width) * 20 + left + i % width) * 4;
        let pixel = &expected.data()[offset..offset + 4];
        let alpha = masks.first().map_or(255, |mask| mask.bytes[i]);
        assert_eq!(alpha, pixel[3], "alpha pixel {i}");
        for (channel, &expected) in pixel[..3].iter().enumerate() {
            let actual =
                ((u16::from(rgb.bytes[i * 3 + channel]) * u16::from(alpha) + 127) / 255) as u8;
            assert!(
                actual.abs_diff(expected) <= 1,
                "pixel {i} channel {channel}: {actual} != {expected}"
            );
        }
    }
}
fn failure(source: &Value, request: &RenderRequest, code: &str, first: &Value, prefix: Value) {
    let document = parse(source);
    let error = Renderer::new().render(&document, request).unwrap_err();
    let repeated = Renderer::new().render(&document, request).unwrap_err();
    assert_eq!(error.code().as_str(), code);
    assert_eq!(
        error.category(),
        excalidraw_render::RenderErrorCategory::NoMeaningfulArtifact
    );
    assert_eq!(error.format(), OutputFormat::Pdf);
    assert_eq!(error.element_id(), first["element_id"].as_str());
    assert_eq!(
        error.json_pointer(),
        first["json_pointer"].as_str().unwrap()
    );
    assert!(!error.to_string().is_empty());
    let mut details = first["details"].clone();
    details["recovery"] = Value::Null;
    assert_eq!(serde_json::to_value(error.details()).unwrap(), details);
    report(error.report(), prefix);
    assert_eq!(error, repeated);
}

#[test]
fn raster_fallback_contract_covers_families_occurrences_geometry_and_failures() {
    partial_alpha_closure_preserves_straight_samples_and_attached_mask();
    every_effect_family_has_an_exact_report_and_isolated_artifact();
    occurrence_order_and_merged_causes_are_exact();
    noops_and_native_content_are_silent_under_forbid();
    scaled_bounds_and_failure_preflight_are_exact();
    frame_reports_are_scoped_and_replayed();
    mixed_reports_follow_original_scene_owners();
    authority_and_expansion_keep_occurrence_specific_causes();
    cropped_closure_dimensions_match_admitted_grid();
    subpixel_anisotropic_and_nested_resource_grids_are_exact();
    stylesheet_effects_and_cascade_are_admitted_and_attributed();
    resource_occurrence_transforms_and_filter_inputs_share_the_plan();
    tiny_geometry_and_transforms_survive_normalized_serialization();
    rotated_closure_uses_the_final_device_grid();
    cropped_repeating_patterns_keep_their_complete_tile();
    inline_filter_cascade_has_exact_provenance_without_a_stylesheet();
}

#[test]
fn partial_alpha_closure_preserves_straight_samples_and_attached_mask() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%" color-interpolation-filters="sRGB"><feComponentTransfer><feFuncA type="linear" slope="0.5"/></feComponentTransfer></filter></defs><rect width="20" height="10" fill="#03fcdb" fill-opacity="0.047058823529411764" filter="url(#effect)"/></svg>"##;
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        causes(5, 4, &[3]),
    );
    let outcome = checked(&source(svg), &request(), json!([expected]));
    let planes = image_planes(outcome.artifact().bytes());
    assert_eq!(planes.len(), 2);
    let rgb = planes.iter().find(|plane| !plane.gray).unwrap();
    let alpha = planes.iter().find(|plane| plane.gray).unwrap();
    assert_eq!(
        (rgb.width, rgb.height, alpha.width, alpha.height),
        (20, 10, 20, 10)
    );
    assert_eq!(rgb.soft_mask, Some(alpha.object));
    // Authored constant field: after the alpha-halving effect, premultiplied
    // [0, 6, 5, 6] converts to the reference-qualified straight sample below.
    assert_eq!(alpha.bytes, vec![6; 200]);
    assert_eq!(rgb.bytes, [0, 255, 212].repeat(200));
}

#[test]
fn every_effect_family_has_an_exact_report_and_isolated_artifact() {
    let cases = [
        ("blur", BLUR, 4, vec![node(3)]),
        (
            "convolution",
            r#"<feConvolveMatrix order="3" kernelMatrix="0 -1 0 -1 5 -1 0 -1 0"/>"#,
            4,
            vec![node(3)],
        ),
        (
            "morphology",
            r#"<feMorphology operator="dilate" radius="1"/>"#,
            4,
            vec![node(3)],
        ),
        (
            "displacement",
            r#"<feTurbulence baseFrequency="0.1" numOctaves="1" result="noise"/><feDisplacementMap in="SourceGraphic" in2="noise" scale="2"/>"#,
            5,
            vec![
                node(3),
                node(4),
                attribute(4, 0, "in", "effect_input"),
                attribute(4, 1, "in2", "effect_input"),
            ],
        ),
        (
            "turbulence",
            r#"<feTurbulence baseFrequency="0.1" numOctaves="1"/>"#,
            4,
            vec![node(3)],
        ),
        (
            "diffuse-lighting",
            r#"<feDiffuseLighting surfaceScale="2"><feDistantLight azimuth="45" elevation="60"/></feDiffuseLighting>"#,
            5,
            vec![node(3)],
        ),
        (
            "specular-lighting",
            r#"<feSpecularLighting surfaceScale="2" specularExponent="2"><fePointLight x="10" y="5" z="10"/></feSpecularLighting>"#,
            5,
            vec![node(3)],
        ),
        (
            "color-matrix",
            r#"<feColorMatrix type="saturate" values="0.5"/>"#,
            4,
            vec![node(3)],
        ),
        (
            "component-transfer",
            r#"<feComponentTransfer><feFuncG type="linear" slope="0.5"/></feComponentTransfer>"#,
            5,
            vec![node(3)],
        ),
        (
            "filter-blend",
            r#"<feBlend in="SourceGraphic" in2="SourceGraphic" mode="overlay"/>"#,
            4,
            vec![
                node(3),
                attribute(3, 0, "in", "effect_input"),
                attribute(3, 1, "in2", "effect_input"),
            ],
        ),
        (
            "drop-shadow",
            r#"<feDropShadow dx="1" dy="1" stdDeviation="1"/>"#,
            4,
            vec![node(3)],
        ),
        ("offset", r#"<feOffset dx="2" dy="1"/>"#, 4, vec![node(3)]),
        (
            "flood",
            r##"<feFlood flood-color="#ff0000" flood-opacity="0.5"/>"##,
            4,
            vec![node(3)],
        ),
        (
            "composite",
            r##"<feFlood flood-color="#ff0000" result="f"/><feComposite in="f" in2="SourceGraphic" operator="in"/>"##,
            5,
            vec![
                node(3),
                node(4),
                attribute(4, 0, "in", "effect_input"),
                attribute(4, 1, "in2", "effect_input"),
            ],
        ),
        (
            "merge",
            r#"<feMerge><feMergeNode in="SourceGraphic"/><feMergeNode in="SourceGraphic"/></feMerge>"#,
            6,
            vec![node(3)],
        ),
        (
            "tile",
            r#"<feOffset dx="0" dy="0" result="o"/><feTile in="o"/>"#,
            5,
            vec![node(3), node(4), attribute(4, 0, "in", "effect_input")],
        ),
    ];
    for (name, primitives, usage, primitive_causes) in cases {
        let svg = svg(primitives);
        let source = source(&svg);
        let mut paths = vec![attribute(usage, 3, "filter", "attribute")];
        paths.extend(primitive_causes);
        let expected = row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 20.0, 10.0],
            [20, 10],
            json!(paths),
        );
        let actual = checked(&source, &request(), json!([expected]));
        isolated_pixels_match_source_effect(actual.artifact().bytes(), &svg);
        let pdf = crate::common::pdf_inspect::pdf_text(actual.artifact().bytes());
        assert!(pdf.contains("/Subtype /Image"), "{name}");
        assert!(
            pdf.contains(" Do\n"),
            "the rasterized closure is painted: {name}"
        );
        assert!(
            pdf.contains("/Width 20") && pdf.contains("/Height 10"),
            "{name}"
        );
        assert!(
            !pdf.contains("/Width 40\n/Height 30"),
            "page must not be rasterized: {name}"
        );
        let forbid = RenderRequest::builder(OutputFormat::Pdf)
            .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
            .build()
            .unwrap();
        failure(
            &source,
            &forbid,
            "pdf.raster-fallback-forbidden",
            &expected,
            json!([]),
        );
        for format in [OutputFormat::Svg, OutputFormat::Png] {
            checked(
                &source,
                &RenderRequest::builder(format).build().unwrap(),
                json!([]),
            );
        }
    }
}

#[test]
fn occurrence_order_and_merged_causes_are_exact() {
    let nested = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="10" height="10" filter="url(#effect)"/><rect x="10" width="10" height="10" filter="url(#effect)"/></svg>"##;
    let mut source = source(nested);
    source["elements"] = json!([image("z", "asset/~", 0.0), image("a", "asset/~", 30.0)]);
    let rows = json!([
        row(
            "z",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 10.0, 10.0],
            [10, 10],
            causes(4, 2, &[3])
        ),
        row(
            "z",
            "/files/asset~1~0/dataURL",
            [20.0, 10.0, 10.0, 10.0],
            [10, 10],
            causes(5, 3, &[3])
        ),
        row(
            "a",
            "/files/asset~1~0/dataURL",
            [40.0, 10.0, 10.0, 10.0],
            [10, 10],
            causes(4, 2, &[3])
        ),
        row(
            "a",
            "/files/asset~1~0/dataURL",
            [50.0, 10.0, 10.0, 10.0],
            [10, 10],
            causes(5, 3, &[3])
        )
    ]);
    checked(&source, &request(), rows);
    let merged = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="outer" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><filter id="inner" x="0" y="0" width="100%" height="100%"><feColorMatrix type="saturate" values="0.5"/></filter></defs><g filter="url(#outer)"><rect width="20" height="10" filter="url(#inner)"/></g></svg>"##;
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        json!([
            attribute(6, 0, "filter", "attribute"),
            node(3),
            attribute(7, 2, "filter", "attribute"),
            node(5)
        ]),
    );
    let actual = checked(&self::source(merged), &request(), json!([expected]));
    assert_eq!(
        crate::common::pdf_inspect::pdf_text(actual.artifact().bytes())
            .matches("/Subtype /Image")
            .count(),
        2
    );
}

#[test]
fn noops_and_native_content_are_silent_under_forbid() {
    let forbid = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    for primitives in [
        "",
        r#"<feGaussianBlur stdDeviation="0"/>"#,
        r#"<feOffset dx="0" dy="0"/>"#,
        r#"<feColorMatrix type="saturate" values="1"/>"#,
        r#"<feColorMatrix type="hueRotate" values="0"/>"#,
        r#"<feColorMatrix type="matrix" values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0"/>"#,
        r#"<feComponentTransfer><feFuncR type="identity"/></feComponentTransfer>"#,
    ] {
        let actual = checked(&source(&svg(primitives)), &forbid, json!([]));
        let clean = source(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##,
        );
        let clean = checked(&clean, &forbid, json!([]));
        assert_eq!(
            actual.artifact().bytes(),
            clean.artifact().bytes(),
            "{primitives}"
        );
    }
    let native = crate::common::approved_scenes::render_nested_native_operations();
    report(native.report(), json!([]));
    let project = crate::common::approved_scenes::render_project_operations();
    report(project.report(), json!([]));
}

#[test]
fn scaled_bounds_and_failure_preflight_are_exact() {
    let source = source(&svg(BLUR));
    for scale in [0.5, 1.0, 2.0] {
        let expected = row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 20.0, 10.0],
            [(20.0 * scale) as u64, (10.0 * scale) as u64],
            causes(4, 3, &[3]),
        );
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .sizing(Sizing::Scale(scale))
            .build()
            .unwrap();
        let actual = checked(&source, &request, json!([expected]));
        let planes = image_planes(actual.artifact().bytes());
        assert!(!planes.is_empty());
        for plane in planes {
            assert_eq!(
                (plane.width, plane.height),
                ((20.0 * scale) as usize, (10.0 * scale) as usize),
                "scale {scale}: encoded closure must use the reported final grid"
            );
        }
    }
    let rejected = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(
            ResourceLimits::builder()
                .max_raster_surface_pixels(199)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    let error = Renderer::new()
        .render(&parse(&source), &rejected)
        .unwrap_err();
    assert_eq!(error.code().as_str(), "resource.raster-surface-exceeded");
    report(error.report(), json!([]));
    let external = svg(r#"<feBlend in="SourceGraphic" in2="BackgroundImage" mode="multiply"/>"#);
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        json!([
            attribute(4, 3, "filter", "attribute"),
            node(3),
            attribute(3, 0, "in", "effect_input"),
            attribute(3, 1, "in2", "effect_input")
        ]),
    );
    failure(
        &self::source(&external),
        &request(),
        "pdf.raster-fallback-unisolatable",
        &expected,
        json!([]),
    );
    let mut pair = source.clone();
    pair["elements"]
        .as_array_mut()
        .unwrap()
        .push(image("later", "asset/~", 30.0));
    let first = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        causes(4, 3, &[3]),
    );
    let limit = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(
            ResourceLimits::builder()
                .max_diagnostics(1)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    let error = Renderer::new().render(&parse(&pair), &limit).unwrap_err();
    assert_eq!(error.code().as_str(), "resource.diagnostic-count-exceeded");
    assert_eq!(error.element_id(), Some("later"));
    assert_eq!(error.json_pointer(), "/files/asset~1~0/dataURL");
    assert_eq!(
        serde_json::to_value(error.details()).unwrap(),
        json!({"kind":"resource_limit","field":"max_diagnostics","configured":1,"value_kind":"count","measurement":{"kind":"attempted","value":2},"recovery":null})
    );
    report(error.report(), json!([first]));
    pair["elements"][1]["fileId"] = json!("other");
    pair["files"]["other"] =
        json!({"id":"other","dataURL":data(&external),"mimeType":"image/svg+xml","created":1});
    let mut later = expected;
    later["element_id"] = json!("later");
    later["json_pointer"] = json!("/files/other/dataURL");
    later["details"]["logical_bounds"]["x"] = json!(40);
    // A later unisolatable closure rejects the complete plan before even the
    // earlier admissible occurrence gets a raster-fallback warning.
    failure(
        &pair,
        &request(),
        "pdf.raster-fallback-unisolatable",
        &later,
        json!([]),
    );
    let forbid = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    failure(
        &pair,
        &forbid,
        "pdf.raster-fallback-forbidden",
        &first,
        json!([]),
    );
}

#[test]
fn frame_reports_are_scoped_and_replayed() {
    let mut source = source(&svg(BLUR));
    source["elements"][0]["frameId"] = json!("first");
    source["elements"].as_array_mut().unwrap().extend([
        json!({"id":"first","type":"frame","width":20,"height":10,"updated":1}),
        json!({"id":"second","type":"frame","x":100,"width":20,"height":10,"updated":1}),
    ]);
    let document = parse(&source);
    let frames: Vec<_> = document.frames().collect();
    let renderer = Renderer::new();
    let request = FrameRenderRequest::builder(OutputFormat::Pdf)
        .build()
        .unwrap();
    let mut batch = renderer
        .render_frames(&document, frames.iter().copied(), &request)
        .unwrap();
    for (index, frame) in frames.into_iter().enumerate() {
        let direct = renderer.render_frame(frame, &request).unwrap();
        let prepared = batch.next().unwrap().into_result().unwrap();
        report(
            direct.report(),
            if index == 0 {
                json!([row(
                    "image",
                    "/files/asset~1~0/dataURL",
                    [0.0, 0.0, 20.0, 10.0],
                    [20, 10],
                    causes(4, 3, &[3])
                )])
            } else {
                json!([])
            },
        );
        assert_eq!(direct.report(), prepared.report());
        assert_eq!(direct.artifact().bytes(), prepared.artifact().bytes());
    }
}

#[test]
fn mixed_reports_follow_original_scene_owners() {
    let mut source = source(&svg(BLUR));
    source["elements"].as_array_mut().unwrap().insert(0,json!({"id":"before","type":"rectangle","width":1,"height":1,"link":"javascript:never()","seed":1,"roughness":0,"updated":1}));
    checked(
        &source,
        &request(),
        json!([
            {"code":"link.unsafe-omitted","severity":"warning","format":"pdf","effects":["semantic"],"element_id":"before","json_pointer":"/elements/0/link","details":{"kind":"compatibility","recovery":"omitted","recipe_id":null}},
            row("image","/files/asset~1~0/dataURL",[10.0,10.0,20.0,10.0],[20,10],causes(4,3,&[3]))
        ]),
    );
}

#[test]
fn authority_and_expansion_keep_occurrence_specific_causes() {
    let nested = svg(BLUR);
    let mut scene = source(&nested);
    let record = scene["files"]["asset/~"].clone();
    scene["files"] = json!({"a":record.clone(),"z":record});
    scene["files"]["a"]["id"] = json!("a");
    scene["files"]["z"]["id"] = json!("z");
    scene["elements"] = json!([image("first", "z", 0.0), image("second", "a", 30.0)]);
    checked(
        &scene,
        &request(),
        json!([
            row(
                "first",
                "/files/z/dataURL",
                [10.0, 10.0, 20.0, 10.0],
                [20, 10],
                causes(4, 3, &[3])
            ),
            row(
                "second",
                "/files/a/dataURL",
                [40.0, 10.0, 20.0, 10.0],
                [20, 10],
                causes(4, 3, &[3])
            )
        ]),
    );
    let mut scene = source(&nested);
    scene["files"] = json!({});
    let supplemented = RenderRequest::builder(OutputFormat::Pdf)
        .supplemental_assets(
            SupplementalAssets::builder()
                .asset("asset/~", nested.as_bytes())
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    checked(
        &scene,
        &supplemented,
        json!([row(
            "image",
            "",
            [10.0, 10.0, 20.0, 10.0],
            [20, 10],
            causes(4, 3, &[3])
        )]),
    );

    let reused = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><g id="shape" filter="url(#effect)"><rect width="10" height="10"/></g></defs><use href="#shape"/><use href="#shape" x="10"/></svg>"##;
    checked(
        &source(reused),
        &request(),
        json!([
            row(
                "image",
                "/files/asset~1~0/dataURL",
                [10.0, 10.0, 10.0, 10.0],
                [10, 10],
                causes(4, 1, &[3])
            ),
            row(
                "image",
                "/files/asset~1~0/dataURL",
                [20.0, 10.0, 10.0, 10.0],
                [10, 10],
                causes(4, 1, &[3])
            )
        ]),
    );
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="{}"/></svg>"#,
        data(&nested)
    );
    let mut paths = causes(4, 3, &[3]);
    for path in paths.as_array_mut().unwrap() {
        let segments = path["segments"].as_array_mut().unwrap();
        segments.splice(
            0..0,
            [
                segment("node", 1, Value::Null),
                segment("attribute", 2, json!("href")),
            ],
        );
    }
    checked(
        &source(&parent),
        &request(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 20.0, 10.0],
            [20, 10],
            paths
        )]),
    );
}

#[test]
fn cropped_closure_dimensions_match_admitted_grid() {
    let mut scene = source(&svg(BLUR));
    scene["elements"][0]["width"] = json!(10);
    scene["elements"][0]["crop"] =
        json!({"x":10,"y":0,"width":10,"height":10,"naturalWidth":20,"naturalHeight":10});
    let rejected = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(
            ResourceLimits::builder()
                .max_raster_surface_pixels(100)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    let error = Renderer::new()
        .render(&parse(&scene), &rejected)
        .unwrap_err();
    assert_eq!(error.code().as_str(), "resource.raster-surface-exceeded");
    report(error.report(), json!([]));
    assert_eq!(
        serde_json::to_value(error.details()).unwrap(),
        json!({"kind":"resource_limit","field":"max_raster_surface_pixels","configured":100,"value_kind":"pixels","measurement":{"kind":"attempted","value":200},"recovery":null})
    );
    // Full input preserves blur at the crop edge and must be admitted too.
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(
            ResourceLimits::builder()
                .max_raster_surface_pixels(200)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    let actual = checked(
        &scene,
        &request,
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 10.0, 10.0],
            [10, 10],
            causes(4, 3, &[3])
        )]),
    );
    let planes = image_planes(actual.artifact().bytes());
    isolated_pixels_match_region(actual.artifact().bytes(), &svg(BLUR), [10, 0, 10, 10]);
    assert!(!planes.is_empty());
    for plane in planes {
        assert_eq!(
            (plane.width, plane.height),
            (10, 10),
            "encoded closure must use the admitted/reported device grid"
        );
    }
}

#[test]
fn subpixel_anisotropic_and_nested_resource_grids_are_exact() {
    let fractional = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feColorMatrix type="saturate" values="0.5"/></filter></defs><rect x="0.25" y="0.25" width="0.5" height="0.5" filter="url(#effect)"/></svg>"##;
    let actual = checked(
        &source(fractional),
        &RenderRequest::builder(OutputFormat::Pdf)
            .sizing(Sizing::Scale(0.5))
            .build()
            .unwrap(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 2.0, 2.0],
            [1, 1],
            causes(4, 4, &[3])
        )]),
    );
    for plane in image_planes(actual.artifact().bytes()) {
        assert_eq!((plane.width, plane.height), (1, 1));
    }
    let mut stretched = source(&svg(BLUR));
    stretched["elements"][0]["width"] = json!(30);
    let actual = checked(
        &stretched,
        &request(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 30.0, 10.0],
            [30, 10],
            causes(4, 3, &[3])
        )]),
    );
    for plane in image_planes(actual.artifact().bytes()) {
        assert_eq!((plane.width, plane.height), (30, 10));
    }
    let pattern = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="4" height="4" filter="url(#effect)"/></pattern></defs><rect width="20" height="10" fill="url(#p)"/></svg>"##;
    let actual = checked(
        &source(pattern),
        &request(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 4.0, 4.0],
            [4, 4],
            causes(5, 2, &[3])
        )]),
    );
    isolated_pixels_match_region(actual.artifact().bytes(), pattern, [0, 0, 4, 4]);
    let mask = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="20" height="10" mask-type="alpha"><g filter="url(#effect)"><rect width="20" height="10" fill="white"/></g></mask></defs><rect width="20" height="10" mask="url(#m)"/></svg>"##;
    checked(
        &source(mask),
        &request(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 20.0, 10.0],
            [20, 10],
            causes(5, 0, &[3])
        )]),
    );
}

#[test]
fn stylesheet_effects_and_cascade_are_admitted_and_attributed() {
    let styled = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><style>.fx {filter:url(#effect);fill:#00ff00} [id] {opacity:0}</style><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" class="fx"/></svg>"##;
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        json!([node(6),{"segments":[segment("node",1,Value::Null),segment("css_rule",0,Value::Null),segment("css_declaration",0,json!("filter"))]},node(5)]),
    );
    let actual = checked(&source(styled), &request(), json!([expected]));
    isolated_pixels_match_source_effect(actual.artifact().bytes(), &svg(BLUR));
    let forbid = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    failure(
        &source(styled),
        &forbid,
        "pdf.raster-fallback-forbidden",
        &expected,
        json!([]),
    );
    let overridden = styled.replace(
        ".fx {filter:url(#effect);fill:#00ff00}",
        ".fx {filter:url(#effect);fill:#00ff00} rect.fx {filter:none}",
    );
    checked(&source(&overridden), &forbid, json!([]));
    let inline = styled.replace("class=\"fx\"", "class=\"fx\" style=\"filter:none\"");
    checked(&source(&inline), &forbid, json!([]));
    let mut invisible = source(&svg(BLUR));
    invisible["elements"][0]["opacity"] = json!(0);
    checked(&invisible, &forbid, json!([]));
}

#[test]
fn resource_occurrence_transforms_and_filter_inputs_share_the_plan() {
    let pattern = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="scale(2)"><rect width="4" height="4" filter="url(#effect)"/></pattern></defs><rect width="20" height="10" fill="url(#p)"/><rect width="20" height="10" fill="url(#p)" transform="translate(10 0) scale(0.5)"/></svg>"##;
    let actual = checked(
        &source(pattern),
        &request(),
        json!([
            row(
                "image",
                "/files/asset~1~0/dataURL",
                [10.0, 10.0, 8.0, 8.0],
                [8, 8],
                causes(5, 2, &[3])
            ),
            row(
                "image",
                "/files/asset~1~0/dataURL",
                [20.0, 10.0, 4.0, 4.0],
                [4, 4],
                causes(5, 2, &[3])
            )
        ]),
    );
    let mut sizes: Vec<_> = image_planes(actual.artifact().bytes())
        .into_iter()
        .filter(|p| !p.gray)
        .map(|p| (p.width, p.height))
        .collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [(4, 4), (8, 8)]);
    let mask = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="10" height="5" mask-type="alpha"><g filter="url(#effect)"><rect width="10" height="5" fill="white"/></g></mask></defs><g transform="scale(2)" mask="url(#m)"><rect width="10" height="5"/></g></svg>"##;
    let actual = checked(
        &source(mask),
        &request(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 20.0, 10.0],
            [20, 10],
            causes(5, 0, &[3])
        )]),
    );
    for plane in image_planes(actual.artifact().bytes()) {
        assert_eq!((plane.width, plane.height), (20, 10));
    }
    let dependency = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="outer" x="0" y="0" width="100%" height="100%"><feImage href="#shape"/></filter><filter id="inner" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><g id="shape" filter="url(#inner)"><rect width="20" height="10" fill="#00ff00"/></g></defs><rect width="20" height="10" filter="url(#outer)"/></svg>"##;
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        json!([
            attribute(8, 2, "filter", "attribute"),
            node(3),
            attribute(6, 1, "filter", "attribute"),
            node(5)
        ]),
    );
    let actual = checked(&source(dependency), &request(), json!([expected]));
    isolated_pixels_match_source_effect(actual.artifact().bytes(), dependency);
}

#[test]
fn tiny_geometry_and_transforms_survive_normalized_serialization() {
    let tiny = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="0" y="0" width="20" height="10"><feFlood flood-color="#00ff00"/></filter></defs><rect width="0.0000000000001" height="0.0000000000001" filter="url(#effect)"/></svg>"##;
    let actual = checked(
        &source(tiny),
        &request(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [10.0, 10.0, 20.0, 10.0],
            [20, 10],
            causes(4, 2, &[3])
        )]),
    );
    isolated_pixels_match_source_effect(actual.artifact().bytes(), tiny);
}

#[test]
fn rotated_closure_uses_the_final_device_grid() {
    let svg = svg(BLUR);
    let mut scene = source(&svg);
    let image = &mut scene["elements"][0];
    image["x"] = json!(10);
    image["y"] = json!(15);
    image["angle"] = json!(std::f64::consts::FRAC_PI_2);
    image["frameId"] = json!("frame");
    scene["elements"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"frame","type":"frame","width":40,"height":40,"updated":1}));
    let document = parse(&scene);
    let outcome = Renderer::new()
        .render_frame(
            document.frames().next().unwrap(),
            &FrameRenderRequest::builder(OutputFormat::Pdf)
                .build()
                .unwrap(),
        )
        .unwrap();
    report(
        outcome.report(),
        json!([row(
            "image",
            "/files/asset~1~0/dataURL",
            [15.0, 10.0, 10.0, 20.0],
            [10, 20],
            causes(4, 3, &[3])
        )]),
    );
    let planes = image_planes(outcome.artifact().bytes());
    for plane in &planes {
        assert_eq!((plane.width, plane.height), (10, 20));
    }
    let alpha = planes.iter().find(|p| p.gray).unwrap();
    // Evaluate the authored effect in its rotated device grid. Rotating an
    // already-filtered bitmap would assume the finite raster filter's rounding
    // is rotation-equivariant, which is not part of the reference contract.
    let rotated = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><g transform="matrix(0 1 -1 0 10 0)">{svg}</g></svg>"#
    );
    let tree = usvg::Tree::from_str(&rotated, &usvg::Options::default()).unwrap();
    let mut expected = tiny_skia::Pixmap::new(10, 20).unwrap();
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut expected.as_mut(),
    );
    for y in 0..20 {
        for x in 0..10 {
            let expected = expected.data()[(y * 10 + x) * 4 + 3];
            assert!(
                alpha.bytes[y * 10 + x].abs_diff(expected) <= 1,
                "quarter-turn alpha at ({x},{y}): {} != {expected}",
                alpha.bytes[y * 10 + x]
            );
        }
    }
}

fn assert_streams_are_compressed(bytes: &[u8]) {
    let mut cursor = 0;
    while let Some(relative) = bytes[cursor..].windows(7).position(|w| w == b"stream\n") {
        let start = cursor + relative + 7;
        let header = String::from_utf8_lossy(&bytes[cursor..start]);
        let length = header
            .rsplit("/Length ")
            .next()
            .unwrap()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<usize>()
            .unwrap();
        // Krilla writes every stream Flate-filtered, in its compact syntax
        // (ADR-0021).
        assert!(
            header.contains("/Filter/FlateDecode") || header.contains("/Filter /FlateDecode"),
            "uncompressed stream: {header}"
        );
        cursor = start + length + b"\nendstream".len();
    }
}

#[test]
fn cropped_repeating_patterns_keep_their_complete_tile() {
    let pattern = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter><pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="4" height="4" filter="url(#effect)"/></pattern></defs><rect width="20" height="10" fill="url(#p)"/></svg>"##;
    let mut scene = source(pattern);
    scene["elements"][0]["width"] = json!(10);
    scene["elements"][0]["crop"] =
        json!({"x":10,"y":0,"width":10,"height":10,"naturalWidth":20,"naturalHeight":10});
    // The base tile is outside the image crop, but its repetitions are visible.
    // Its own 4x4 viewport, rather than the scene crop, defines this dependency.
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [0.0, 10.0, 4.0, 4.0],
        [4, 4],
        causes(5, 2, &[3]),
    );
    let actual = checked(&scene, &request(), json!([expected]));
    isolated_pixels_match_region(actual.artifact().bytes(), pattern, [0, 0, 4, 4]);
    assert_streams_are_compressed(actual.artifact().bytes());
    assert!(
        crate::common::pdf_inspect::pdf_text(actual.artifact().bytes()).contains("/PatternType 1")
    );
    let forbid = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    failure(
        &scene,
        &forbid,
        "pdf.raster-fallback-forbidden",
        &expected,
        json!([]),
    );
    for origin in [8, -8] {
        let shifted = pattern.replace(
            "<pattern id=\"p\"",
            &format!("<pattern id=\"p\" x=\"{origin}\""),
        );
        let mut shifted_scene = source(&shifted);
        shifted_scene["elements"] = scene["elements"].clone();
        let expected = row(
            "image",
            "/files/asset~1~0/dataURL",
            [f64::from(origin), 10.0, 4.0, 4.0],
            [4, 4],
            causes(5, 2, &[3]),
        );
        let actual = checked(&shifted_scene, &request(), json!([expected]));
        // Offsets by whole tile periods retain the independently authored tile.
        isolated_pixels_match_region(actual.artifact().bytes(), pattern, [0, 0, 4, 4]);
        failure(
            &shifted_scene,
            &forbid,
            "pdf.raster-fallback-forbidden",
            &expected,
            json!([]),
        );
        assert_streams_are_compressed(actual.artifact().bytes());
    }
    let invisible = pattern.replace(
        "<rect width=\"20\" height=\"10\" fill=\"url(#p)\"/>",
        "<rect x=\"30\" width=\"20\" height=\"10\" fill=\"url(#p)\"/>",
    );
    let invisible = checked(&source(&invisible), &forbid, json!([]));
    assert!(image_planes(invisible.artifact().bytes()).is_empty());
}

#[test]
fn inline_filter_cascade_has_exact_provenance_without_a_stylesheet() {
    let original = svg(BLUR);
    let expected = row(
        "image",
        "/files/asset~1~0/dataURL",
        [10.0, 10.0, 20.0, 10.0],
        [20, 10],
        json!([attribute(4, 3, "style", "attribute"), node(3)]),
    );
    let inline = original.replace(
        "filter=\"url(#effect)\"",
        "style=\"filter:url(#effect) !important\"",
    );
    let actual = checked(&source(&inline), &request(), json!([expected]));
    isolated_pixels_match_source_effect(actual.artifact().bytes(), &original);
    let forbid = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    failure(
        &source(&inline),
        &forbid,
        "pdf.raster-fallback-forbidden",
        &expected,
        json!([]),
    );
    for attrs in [
        "filter=\"url(#effect)\" style=\"filter:none\"",
        "style=\"filter:none\" filter=\"url(#effect)\"",
    ] {
        let overridden = original.replace("filter=\"url(#effect)\"", attrs);
        let actual = checked(&source(&overridden), &forbid, json!([]));
        assert!(image_planes(actual.artifact().bytes()).is_empty());
    }
}
