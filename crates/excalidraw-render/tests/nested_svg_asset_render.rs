use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorCode, ErrorDetails, OutputFormat, PdfOptions, PdfRasterFallback, Recovery,
    RenderRequest, Renderer, ResourceLimits, SceneDocument,
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
fn malformed_nested_image_is_removed_without_discarding_valid_siblings() {
    let child = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000"/><image width="20" height="10" href="data:image/png;base64,AA=="/></svg>"##;
    let parent = svg_with_nested_data_image(child);
    let scene = image_scene(parent.as_bytes(), 20, 10);
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("malformed nested image has local recovery");
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("one nested image diagnostic is retained");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT);
    let Some(ErrorDetails::NestedSource {
        paths, recovery, ..
    }) = diagnostic.details()
    else {
        panic!("nested failure has nested-source details");
    };
    assert_eq!(*recovery, Recovery::Removed);
    assert_eq!(paths[0].segments().len(), 4);

    let decoder = png::Decoder::new(Cursor::new(outcome.artifact().bytes()));
    let mut reader = decoder.read_info().expect("artifact is PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    assert_eq!(pixel(&rgba, 40, 20, 15), [255, 0, 0, 255]);
}

#[test]
fn malformed_nested_payload_still_counts_as_an_asset_occurrence() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,AA=="/></svg>"##;
    let scene = image_scene(nested, 20, 10);
    let limits = ResourceLimits::builder()
        .max_asset_entries(1)
        .build()
        .expect("asset count limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("malformed nested occurrence crosses the aggregate entry limit");

    assert_eq!(error.code(), ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED);
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
fn cyclic_local_references_are_bounded_and_deterministic() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><rect width="20" height="10" fill="#00ff00"/><use href="#a"/></svg>"##;
    let scene = image_scene(nested, 20, 10);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("cycle-closing local reference is bounded");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("cycle handling is deterministic");
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
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
fn repeated_references_discover_an_authoritative_nested_chain_once() {
    let child = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let parent = svg_with_nested_data_image(child);
    let encoded = base64::engine::general_purpose::STANDARD.encode(parent);
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[
            {{"id":"one","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}},
            {{"id":"two","type":"image","x":20,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":2,"updated":1}}
        ],"appState":{{}},"files":{{
            "asset":{{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}
        }}}}"#
    );
    let scene =
        SceneDocument::parse(json.as_bytes()).expect("repeated-reference scene is admitted");
    let limits = ResourceLimits::builder()
        .max_asset_entries(2)
        .build()
        .expect("asset count limit is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("one authoritative parent discovers its nested occurrence once");

    assert!(outcome.report().diagnostics().is_empty());
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

#[test]
fn svg_projection_preserves_sanitized_semantics_and_selectable_text() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
        <defs><path id="mark" d="M0 0h10v10H0z"/></defs>
        <use href="#mark" fill="#ff0000"/>
        <text x="10" y="9" font-family="Excalifont" font-size="10">A</text>
    </svg>"##;
    let scene = image_scene(nested, 20, 10);
    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    let nested = embedded_svg(svg);

    assert!(nested.contains("<use"));
    assert!(nested.contains("<text"));
    assert!(nested.contains("<path"));
}

#[test]
fn png_projects_nested_vectors_at_final_device_scale() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 1 1"><circle cx="0.5" cy="0.5" r="0.5" fill="#ff0000"/></svg>"##;
    let scene = image_scene(nested, 100, 100);
    let png = render(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut rgba).expect("PNG frame decodes");

    assert_eq!((output.width, output.height), (120, 120));
    assert_eq!(pixel(&rgba, 120, 10, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(&rgba, 120, 60, 60), [255, 0, 0, 255]);
}

#[test]
fn pdf_projects_nested_text_as_vector_outlines() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><text x="2" y="16" font-family="Excalifont" font-size="18">ABC</text></svg>"##;
    let scene = image_scene(nested, 40, 20);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("outlined nested text renders in PDF");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(!pdf.contains("/Subtype /Image"));
    assert!(!pdf.contains("/Type /Font"));
    assert!(
        pdf.contains(" c\n"),
        "glyph outlines contain cubic path segments"
    );
    assert!(
        pdf.contains("\nf\n") || pdf.contains("\nf*\n"),
        "glyph outlines are filled"
    );
    assert!(!outcome
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
}

#[test]
fn closed_vector_native_families_remain_native_pdf_content() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40" viewBox="0 0 80 40" preserveAspectRatio="xMidYMid meet">
        <defs>
            <linearGradient id="linear"><stop stop-color="#ff0000" stop-opacity="0.5"/><stop offset="1" stop-color="#0000ff"/></linearGradient>
            <radialGradient id="radial"><stop stop-color="#ffffff"/><stop offset="1" stop-color="#000000"/></radialGradient>
            <pattern id="tiles" width="4" height="4" patternUnits="userSpaceOnUse"><path d="M0 0h2v2H0z" fill="#00ff00"/></pattern>
            <clipPath id="clip"><circle cx="20" cy="20" r="18"/></clipPath>
            <mask id="mask" mask-type="luminance"><rect width="80" height="40" fill="url(#radial)"/></mask>
            <marker id="arrow" markerWidth="4" markerHeight="4" refX="4" refY="2" orient="auto"><path d="M0 0L4 2L0 4z"/></marker>
            <symbol id="shape" viewBox="0 0 20 20"><path d="M1 10 Q5 1 10 10 C12 14 16 14 19 10 A4 4 0 0 1 15 18 Z" fill="url(#linear)"/></symbol>
        </defs>
        <g transform="translate(2 0)" opacity="0.8" clip-path="url(#clip)" mask="url(#mask)" style="mix-blend-mode:multiply;isolation:isolate">
            <use href="#shape" width="40" height="40"/>
            <rect x="40" width="40" height="40" fill="url(#tiles)"/>
            <path d="M2 36L76 36" stroke="#000000" marker-end="url(#arrow)"/>
        </g>
    </svg>"##;
    let scene = image_scene(nested, 80, 40);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("closed vector-native matrix renders");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    for structure in [
        "/PatternType 1",
        "/ShadingType 2",
        "/ShadingType 3",
        "/SMask",
        "/BM /Multiply",
    ] {
        assert!(pdf.contains(structure), "missing PDF structure {structure}");
    }
    assert!(!pdf.contains("/Subtype /Image"));
    assert!(!outcome
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
}

#[test]
fn every_native_blend_mode_remains_vector_pdf_content() {
    let blends = [
        ("normal", "/BM /Normal"),
        ("multiply", "/BM /Multiply"),
        ("screen", "/BM /Screen"),
        ("overlay", "/BM /Overlay"),
        ("darken", "/BM /Darken"),
        ("lighten", "/BM /Lighten"),
        ("color-dodge", "/BM /ColorDodge"),
        ("color-burn", "/BM /ColorBurn"),
        ("hard-light", "/BM /HardLight"),
        ("soft-light", "/BM /SoftLight"),
        ("difference", "/BM /Difference"),
        ("exclusion", "/BM /Exclusion"),
        ("hue", "/BM /Hue"),
        ("saturation", "/BM /Saturation"),
        ("color", "/BM /Color"),
        ("luminosity", "/BM /Luminosity"),
    ];

    for (blend, pdf_name) in blends {
        let nested = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000"/><g style="mix-blend-mode:{blend};isolation:isolate"><circle cx="10" cy="5" r="5" fill="#0000ff"/></g></svg>"##
        );
        let scene = image_scene(nested.as_bytes(), 20, 10);
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
            .build()
            .expect("PDF request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("native blend renders without fallback");
        let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

        assert!(pdf.contains(pdf_name), "missing PDF blend {blend}");
        assert!(!pdf.contains("/Subtype /Image"));
        assert!(!outcome
            .report()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
    }
}

#[test]
fn nested_raster_nodes_are_source_content_in_png_and_pdf() {
    let nested = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,{PNG_BASE64}"/></svg>"#
    );
    let scene = image_scene(nested.as_bytes(), 20, 10);

    let png = render(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    assert_ne!(pixel(&rgba, 40, 20, 15), [255, 255, 255, 255]);

    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("nested source raster renders in PDF");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
    assert!(pdf.contains("/Subtype /Image"));
    assert!(!outcome
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
}

#[test]
fn nested_bmp_preserves_direct_svg_bytes_and_normalizes_png_and_pdf() {
    let bmp = fixture_bmp24([24, 96, 208]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bmp);
    let child = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="10" height="10" fill="#ff0000"/><image x="10" width="10" height="10" href="data:image/bmp;base64,{encoded}"/></svg>"##
    );
    let parent = svg_with_nested_data_image(child.as_bytes());
    let scene = image_scene(parent.as_bytes(), 20, 10);

    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    assert!(embedded_svg(&embedded_svg(svg)).contains(&format!("data:image/bmp;base64,{encoded}")));

    let png = render(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    assert_eq!(pixel(&rgba, 40, 15, 15), [255, 0, 0, 255]);
    assert_eq!(pixel(&rgba, 40, 25, 15), [24, 96, 208, 255]);

    let pdf = String::from_utf8_lossy(&render(&scene, OutputFormat::Pdf)).into_owned();
    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/Subtype /Form"));
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

fn fixture_bmp24(rgb: [u8; 3]) -> Vec<u8> {
    let mut bytes = Vec::from(b"BM".as_slice());
    bytes.extend_from_slice(&58_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&24_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 0]);
    bytes
}

fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    rgba[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel exists")
}
