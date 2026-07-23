use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorCode, ErrorDetails, OutputFormat, Recovery, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};
use std::io::Cursor;

const PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

#[test]
fn basic_nested_svg_renders_in_svg_png_and_pdf() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10"><rect width="10" height="10" fill="#ff0000"/><rect x="10" width="10" height="10" fill="#0000ff"/></svg>"##;
    let scene = image_scene(nested, 20, 10);

    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    assert!(svg.contains("data:image/svg+xml;base64,"));
    assert!(!svg.contains("http://example.com"));

    let png = render(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut rgba).expect("PNG frame decodes");
    assert_eq!((output.width, output.height), (40, 30));
    assert_eq!(pixel(&rgba, 40, 15, 15), [255, 0, 0, 255]);
    assert_eq!(pixel(&rgba, 40, 25, 15), [0, 0, 255, 255]);

    let pdf = String::from_utf8_lossy(&render(&scene, OutputFormat::Pdf)).into_owned();
    assert!(pdf.starts_with("%PDF-1.7"));
    assert!(pdf.contains("/Svg0 Do"));
    assert!(pdf.contains("/Subtype /Form"));
    assert!(!pdf.contains("/Subtype /Image"));
}

#[test]
fn unsafe_nested_svg_boundaries_are_removed_before_embedding() {
    let nested =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" onload="alert(1)">
        <rect width="20" height="10" fill="#00ff00" onclick="alert(2)"/>
        <script>alert(3)</script>
        <animate attributeName="opacity" values="0;1"/>
        <foreignObject><div xmlns="http://www.w3.org/1999/xhtml">unsafe</div></foreignObject>
        <image href="https://example.com/tracker.png" width="20" height="10"/>
        <style>@import url(https://example.com/theme.css); rect { fill: red }</style>
    </svg>"##;
    let scene = image_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("unsafe nested content has local removal");
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    let sanitized = embedded_svg(artifact);

    for forbidden in [
        "onload",
        "onclick",
        "script",
        "animate",
        "foreignObject",
        "example.com",
        "@import",
    ] {
        assert!(!sanitized.contains(forbidden), "removed {forbidden}");
    }
    assert!(sanitized.contains("<rect"));
    assert_eq!(outcome.report().diagnostics().len(), 7);
    assert!(outcome.report().diagnostics().iter().all(|diagnostic| {
        diagnostic.code() == DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
            && diagnostic.element_id().is_none()
            && diagnostic.json_pointer() == "/files/asset/dataURL"
            && matches!(
                diagnostic.details(),
                Some(ErrorDetails::NestedSource {
                    recovery: Recovery::Removed,
                    ..
                })
            )
    }));
}

#[test]
fn dtds_and_declared_entities_are_removed_without_expansion() {
    let nested = br##"<?xml version="1.0"?><!DOCTYPE svg [<!ENTITY secret "EXPANDED">]><svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><text>&secret;</text><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let scene = image_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("DTD and entity boundaries are removed locally");
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    let sanitized = embedded_svg(artifact);

    assert!(!sanitized.contains("DOCTYPE"));
    assert!(!sanitized.contains("ENTITY"));
    assert!(!sanitized.contains("EXPANDED"));
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        [
            DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED,
            DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED,
        ]
    );
}

#[test]
fn nested_svg_node_ceiling_recovers_before_viewer_parsing() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let scene = image_scene(nested, 20, 10);
    let limits = ResourceLimits::builder()
        .max_nested_svg_nodes(1)
        .build()
        .expect("node limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("nested complexity has local recovery");

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_NESTED_SVG_COMPLEXITY_EXCEEDED
    );
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    assert!(!artifact.contains("data:image/svg+xml;base64,"));
}

#[test]
fn nested_data_svg_is_recursively_sanitized_before_embedding() {
    let child = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/><script>alert(1)</script></svg>"##;
    let child = base64::engine::general_purpose::STANDARD.encode(child);
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/svg+xml;base64,{child}"/></svg>"#
    );
    let scene = image_scene(parent.as_bytes(), 20, 10);
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("nested data SVG renders after recursive sanitization");
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    let parent = embedded_svg(artifact);
    let child = embedded_svg(&parent);

    assert!(child.contains("<rect"));
    assert!(!child.contains("script"));
    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
    );
}

#[test]
fn nested_data_svg_depth_ceiling_stops_recursive_resolution() {
    let leaf = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let child = svg_with_nested_data_image(leaf);
    let parent = svg_with_nested_data_image(child.as_bytes());
    let scene = image_scene(parent.as_bytes(), 20, 10);
    let limits = ResourceLimits::builder()
        .max_nested_asset_depth(1)
        .build()
        .expect("depth limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("nested depth has local recovery");

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_NESTED_ASSET_DEPTH_EXCEEDED
    );
}

#[test]
fn same_document_fragments_resolve_without_ambient_resources() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
        <defs><linearGradient id="paint"><stop stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></linearGradient><rect id="tile" width="10" height="10" fill="url(#paint)"/></defs>
        <use href="#tile"/><use href="#tile" x="10"/>
    </svg>"##;
    let scene = image_scene(nested, 20, 10);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("local fragments render");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("local fragments render deterministically");
        assert!(first.report().diagnostics().is_empty());
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    }
}

#[test]
fn malformed_nested_svg_reports_its_validated_media_family() {
    let scene = image_scene(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect>"#,
        20,
        10,
    );
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("malformed SVG has local recovery");
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("one malformed SVG diagnostic is retained");
    };

    assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_DECODE_FAILED);
    assert!(matches!(
        diagnostic.details(),
        Some(ErrorDetails::Asset {
            validated_media_type: Some(media_type),
            recovery: Recovery::Omitted,
            ..
        }) if media_type == "image/svg+xml"
    ));
}

#[test]
fn external_stylesheet_processing_instruction_is_diagnosed_and_removed() {
    let nested = br##"<?xml-stylesheet href="https://example.com/theme.css"?><svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let scene = image_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("external stylesheet instruction is removed locally");
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    let sanitized = embedded_svg(artifact);

    assert!(!sanitized.contains("xml-stylesheet"));
    assert!(!sanitized.contains("example.com"));
    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
    );
}

#[test]
fn nested_svg_sanitizer_is_bounded_by_asset_work() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let scene = image_scene(nested, 20, 10);
    let limits = ResourceLimits::builder()
        .max_asset_work_units(u64::try_from(nested.len() - 1).expect("fixture length fits u64"))
        .build()
        .expect("asset work limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("asset work crossing has local recovery");

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED
    );
}

#[test]
fn nested_data_resources_participate_in_asset_occurrence_accounting() {
    let child = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let parent = svg_with_nested_data_image(child);
    let scene = image_scene(parent.as_bytes(), 20, 10);
    let limits = ResourceLimits::builder()
        .max_asset_entries(1)
        .build()
        .expect("asset count limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("nested data resource crosses the aggregate occurrence ceiling");

    assert_eq!(error.code(), ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED);
}

#[test]
fn nested_raster_data_is_validated_and_canonicalized() {
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,{PNG_BASE64}"/></svg>"#
    );
    let scene = image_scene(parent.as_bytes(), 20, 10);
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("validated nested raster renders");
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    let sanitized = embedded_svg(artifact);

    assert!(sanitized.contains(&format!("data:image/png;base64,{PNG_BASE64}")));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn data_images_are_not_admitted_as_link_actions() {
    let nested = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><a href="data:image/png;base64,{PNG_BASE64}"><rect width="20" height="10" fill="#00ff00"/></a></svg>"##
    );
    let scene = image_scene(nested.as_bytes(), 20, 10);
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("unsafe link action is removed");
    let artifact = std::str::from_utf8(outcome.artifact().bytes()).expect("artifact is UTF-8");
    let sanitized = embedded_svg(artifact);

    assert!(!sanitized.contains("data:image/png"));
    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
    );
}

#[test]
fn identical_unsafe_svg_bytes_diagnose_each_authoritative_file_owner_once() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" onload="alert(1)"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let encoded = base64::engine::general_purpose::STANDARD.encode(nested);
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[
            {{"id":"one","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
            {{"id":"two","type":"image","x":20,"y":0,"width":20,"height":10,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
        ],"appState":{{}},"files":{{
            "one":{{"id":"one","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}},
            "two":{{"id":"two","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}
        }}}}"#
    );
    let scene = SceneDocument::parse(json.as_bytes()).expect("two-file scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("shared sanitized SVG renders");

    assert_eq!(outcome.report().diagnostics().len(), 2);
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        ["/files/one/dataURL", "/files/two/dataURL"]
    );
}

#[test]
fn filter_primitives_are_removed_instead_of_triggering_silent_pdf_rasterization() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blur"><feGaussianBlur stdDeviation="1000"/></filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#blur)"/></svg>"##;
    let scene = image_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("unsupported filter is removed before PDF lowering");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(!pdf.contains("/Subtype /Image"));
    assert!(!outcome
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
            })
            .count(),
        2
    );
}

fn image_scene(svg: &[u8], width: u32, height: u32) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(svg);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","x":0,"y":0,"width":{width},"height":{height},
                "angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],
                "seed":1,"updated":1
            }}],
            "appState":{{}},"files":{{
                "asset":{{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("nested-SVG scene is admitted")
}

fn render(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    let request = RenderRequest::builder(format)
        .build()
        .expect("request is valid");
    let outcome = Renderer::new()
        .render(scene, &request)
        .expect("nested SVG renders");
    assert!(outcome.report().diagnostics().is_empty());
    outcome.artifact().bytes().to_vec()
}

fn embedded_svg(artifact: &str) -> String {
    let prefix = "data:image/svg+xml;base64,";
    let start = artifact.find(prefix).expect("nested SVG data URL") + prefix.len();
    let end = artifact[start..].find('"').expect("data URL terminator") + start;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&artifact[start..end])
        .expect("canonical nested SVG base64");
    String::from_utf8(bytes).expect("sanitized nested SVG is UTF-8")
}

fn svg_with_nested_data_image(child: &[u8]) -> String {
    let child = base64::engine::general_purpose::STANDARD.encode(child);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/svg+xml;base64,{child}"/></svg>"#
    )
}

fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    rgba[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel exists")
}
