//! Nested-SVG images end to end. The sanitizer's removals, refusals and
//! recursion ceilings are unit tested at `render::asset::svg`, and which PDF
//! raster closures a nested SVG's effects need on the PDF plan in
//! `render::plan_tests`; what stays here is the session's mapping of those
//! outcomes to diagnostics and errors, the accounting they participate in, and
//! the native-versus-raster projection of each format.

use crate::common::pdf_inspect::pdf_text;
use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, DiagnosticDetails, ErrorCode, OutputFormat, PdfOptions, PdfRasterFallback,
    Recovery, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};

use crate::common::image_scene::image_scene;
use crate::common::raster::decode_png;
use crate::common::render::render_artifact;

const PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

#[test]
fn basic_nested_svg_renders_in_svg_png_and_pdf() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10"><rect width="10" height="10" fill="#ff0000"/><rect x="10" width="10" height="10" fill="#0000ff"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    assert!(svg.contains("data:image/svg+xml;base64,"));
    assert!(!svg.contains("http://example.com"));

    let png = render_artifact(&scene, OutputFormat::Png);
    let decoded = decode_png(&png);
    assert_eq!((decoded.width, decoded.height), (40, 30));
    assert_eq!(decoded.pixel(15, 15), [255, 0, 0, 255]);
    assert_eq!(decoded.pixel(25, 15), [0, 0, 255, 255]);

    let pdf = pdf_text(&render_artifact(&scene, OutputFormat::Pdf));
    assert!(pdf.starts_with("%PDF-1.7"));
    // The nested tree is drawn in place as vector paths (ADR-0021): at the
    // 10 px padding on the 22.5 pt page, one SVG unit per logical pixel.
    assert!(pdf.contains(concat!(
        "0.75 0 0 -0.75 7.5 15 cm\n/c0 cs\n1 0 0 scn\n",
        "0 0 m\n10 0 l\n10 10 l\n0 10 l\nh\nf\n"
    )));
    assert!(pdf.contains("0 0 1 scn\n10 0 m\n20 0 l\n20 10 l\n10 10 l\nh\nf\n"));
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
    let scene = nested_svg_scene(nested, 20, 10);
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
        matches!(
            diagnostic.code(),
            DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
                | DiagnosticCode::IMAGE_FORBIDDEN_RESOURCE
        ) && diagnostic.element_id().is_none()
            && diagnostic.json_pointer() == "/files/asset/dataURL"
            && matches!(
                diagnostic.details(),
                Some(DiagnosticDetails::NestedSource {
                    recovery: Recovery::Removed | Recovery::Placeholder,
                    ..
                })
            )
    }));
}

#[test]
fn nested_svg_node_ceiling_recovers_before_viewer_parsing() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
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
    let recovered = embedded_svg(artifact);
    assert!(recovered.contains("#E7E7E7"));
    assert!(!recovered.contains("#00ff00"));
}

/// The default stack of a spawned `std::thread`: a library user rendering off
/// the main thread has no more than this.
const SPAWNED_THREAD_STACK: usize = 2 << 20;

fn on_spawned_thread_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(SPAWNED_THREAD_STACK)
        .spawn(test)
        .expect("test thread spawns")
        .join()
        .expect("test thread completes");
}

/// A nested SVG whose drawing sits under `depth` nested `open`/`close` pairs.
fn deeply_nested_svg(depth: usize, open: &str, close: &str, leaf: &str) -> Vec<u8> {
    let mut svg =
        String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">"#);
    svg.push_str(&open.repeat(depth));
    svg.push_str(leaf);
    svg.push_str(&close.repeat(depth));
    svg.push_str("</svg>");
    svg.into_bytes()
}

#[test]
fn excessive_nested_svg_element_depth_recovers_without_exhausting_the_stack() {
    on_spawned_thread_stack(|| {
        let groups = deeply_nested_svg(80_000, "<g>", "</g>", r#"<rect width="20" height="10"/>"#);
        let spans = deeply_nested_svg(17_000, "<tspan>", "</tspan>", "x");
        let spans = String::from_utf8(spans)
            .expect("fixture is UTF-8")
            .replacen("<tspan>", "<text y=\"10\"><tspan>", 1)
            .replacen("</svg>", "</text></svg>", 1);
        for nested in [groups, spans.into_bytes()] {
            let scene = nested_svg_scene(&nested, 20, 10);
            for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
                let request = RenderRequest::builder(format)
                    .build()
                    .expect("request is valid");
                let outcome = Renderer::new()
                    .render(&scene, &request)
                    .expect("excessive nesting has local recovery");
                let [diagnostic] = outcome.report().diagnostics() else {
                    panic!("one nesting diagnostic in {format:?}");
                };
                assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_DECODE_FAILED);
            }
        }
    });
}

#[test]
fn nested_svg_at_the_element_depth_ceiling_draws_on_a_spawned_thread_stack() {
    // 256 element levels, counted from the outermost nested root through every
    // nested data image: a root sits at level 0, and a nested image's root one
    // level below its image element.
    const LEVELS: usize = 256;
    on_spawned_thread_stack(|| {
        let images = usize::try_from(ResourceLimits::default().max_nested_asset_depth())
            .expect("depth fits usize");
        // Every document holding an image spends its groups, the image and the
        // nested root below it.
        let groups = 40;
        let leaf = r##"<rect width="20" height="10" fill="#00ff00"/>"##;
        let mut chain = deeply_nested_svg(LEVELS - images * (groups + 2) - 1, "<g>", "</g>", leaf);
        for _ in 0..images {
            let child = base64::engine::general_purpose::STANDARD.encode(&chain);
            let image = format!(
                r#"<image width="20" height="10" href="data:image/svg+xml;base64,{child}"/>"#
            );
            chain = deeply_nested_svg(groups, "<g>", "</g>", &image);
        }
        let text = |id: &str, spans: usize| {
            format!(
                r#"<text{id} y="10">{}x{}</text>"#,
                "<tspan>".repeat(spans),
                "</tspan>".repeat(spans)
            )
        };
        let spans = deeply_nested_svg(0, "", "", &text("", LEVELS - 1));
        // The viewer instantiates a `use` in place, so it walks the deep text
        // again below the deep reference.
        let reused = deeply_nested_svg(LEVELS - 1, "<g>", "</g>", r##"<use href="#t"/>"##);
        let reused = String::from_utf8(reused)
            .expect("fixture is UTF-8")
            .replacen(
                "<g>",
                &format!("<defs>{}</defs><g>", text(r#" id="t""#, LEVELS - 2)),
                1,
            );
        // PDF lowers a filtered group through its own raster walk.
        let filtered = deeply_nested_svg(
            LEVELS - 1,
            "<g>",
            "</g>",
            r##"<rect width="20" height="10" fill="#00ff00" filter="url(#f)"/>"##,
        );
        let filtered = String::from_utf8(filtered)
            .expect("fixture is UTF-8")
            .replacen(
                "<g>",
                r#"<defs><filter id="f"><feGaussianBlur stdDeviation="1"/></filter></defs><g>"#,
                1,
            );
        for (nested, effect) in [
            (chain, false),
            (spans, false),
            (reused.into_bytes(), false),
            (filtered.into_bytes(), true),
        ] {
            let scene = nested_svg_scene(&nested, 20, 10);
            for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
                let request = RenderRequest::builder(format)
                    .build()
                    .expect("request is valid");
                let outcome = Renderer::new()
                    .render(&scene, &request)
                    .expect("nesting at the ceiling renders");
                assert!(
                    outcome
                        .report()
                        .diagnostics()
                        .iter()
                        .all(|diagnostic| effect
                            && diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK),
                    "nesting at the ceiling draws in {format:?}: {:?}",
                    outcome.report().diagnostics()
                );
            }
        }
    });
}

#[test]
fn nested_data_svg_depth_ceiling_stops_recursive_resolution() {
    let leaf = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let child = svg_with_nested_data_image(leaf);
    let parent = svg_with_nested_data_image(child.as_bytes());
    let scene = nested_svg_scene(parent.as_bytes(), 20, 10);
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
    let scene = nested_svg_scene(parent.as_bytes(), 20, 10);
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
    let Some(DiagnosticDetails::NestedSource {
        paths, recovery, ..
    }) = diagnostic.details()
    else {
        panic!("nested failure has nested-source details");
    };
    assert_eq!(*recovery, Recovery::Removed);
    assert_eq!(paths[0].segments().len(), 4);

    let decoded = decode_png(outcome.artifact().bytes());
    assert_eq!(decoded.pixel(20, 15), [255, 0, 0, 255]);
}

#[test]
fn malformed_nested_payload_still_counts_as_an_asset_occurrence() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,AA=="/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
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
    let scene = nested_svg_scene(nested, 20, 10);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("local fragments render");
        assert!(first.report().diagnostics().is_empty());
    }
}

#[test]
fn cyclic_local_references_are_bounded() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><rect width="20" height="10" fill="#00ff00"/><use href="#a"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        Renderer::new()
            .render(&scene, &request)
            .expect("cycle-closing local reference is bounded");
    }
}

#[test]
fn stylesheet_reference_cycles_break_like_their_attribute_twin() {
    // A mask cycle declared only by rules once overflowed the viewer
    // library's stack; the closing rule reference is now overridden on its
    // element, exactly as the closing `mask` attribute is dropped.
    let styled = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><style>.a{mask:url(#b)} .b{mask:url(#c)} .c{mask:url(#a)}</style><defs><mask id="a"><rect class="a" width="24" height="24" fill="white"/></mask><mask id="b"><rect class="b" width="24" height="24" fill="white"/></mask><mask id="c"><rect class="c" width="24" height="24" fill="white"/></mask></defs><rect width="24" height="24" fill="red" mask="url(#a)"/></svg>"#;
    let twin = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><mask id="a"><rect mask="url(#b)" width="24" height="24" fill="white"/></mask><mask id="b"><rect mask="url(#c)" width="24" height="24" fill="white"/></mask><mask id="c"><rect mask="url(#a)" width="24" height="24" fill="white"/></mask></defs><rect width="24" height="24" fill="red" mask="url(#a)"/></svg>"#;
    let scene = nested_svg_scene(styled, 24, 24);
    let twin = nested_svg_scene(twin, 24, 24);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("the rule cycle is broken");
        assert!(first.report().diagnostics().is_empty(), "{format:?}");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("the rule cycle is broken");
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        if format == OutputFormat::Svg {
            let artifact = std::str::from_utf8(first.artifact().bytes()).expect("UTF-8");
            let sanitized = embedded_svg(artifact);
            assert!(
                sanitized.contains(r#"<rect class="c" width="24" height="24" fill="white" style="mask:url(#__excalidraw_missing);">"#),
                "{sanitized}"
            );
        }
    }

    let png = render_artifact(&scene, OutputFormat::Png);
    assert_eq!(png, render_artifact(&twin, OutputFormat::Png));
    let decoded = decode_png(&png);
    assert_eq!(decoded.pixel(30, 30), [255, 0, 0, 255]);
}

#[test]
fn malformed_nested_svg_reports_its_validated_media_family() {
    let scene = nested_svg_scene(
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
        Some(DiagnosticDetails::Asset {
            validated_media_type: Some(media_type),
            recovery: Recovery::Omitted,
            ..
        }) if media_type == "image/svg+xml"
    ));
}

#[test]
fn nested_svg_sanitizer_is_bounded_by_asset_work() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
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
fn rejected_parent_svg_never_admits_or_decodes_nested_occurrences() {
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image width="1" height="1" href="data:image/png;base64,{PNG_BASE64}"/></svg>"#
    );
    let scene = nested_svg_scene(parent.as_bytes(), 1, 1);
    let limits = ResourceLimits::builder()
        .max_asset_entries(1)
        .max_asset_work_units(u64::try_from(parent.len()).expect("fixture length fits u64"))
        .build()
        .expect("parent rejection limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("parent work recovery occurs before nested discovery");

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
    let scene = nested_svg_scene(parent.as_bytes(), 20, 10);
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
fn nested_raster_occurrences_are_admitted_before_distinct_identity_decodes() {
    let bmp = base64::engine::general_purpose::STANDARD.encode(fixture_bmp24([0, 0, 255]));
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><image width="1" height="1" href="data:image/png;base64,{PNG_BASE64}"/><image x="1" width="1" height="1" href="data:image/bmp;base64,{bmp}"/></svg>"#
    );
    let scene = nested_svg_scene(parent.as_bytes(), 2, 1);
    let limits = ResourceLimits::builder()
        .max_decoded_pixels_per_asset(1)
        .max_total_decoded_pixels(1)
        .build()
        .expect("nested pixel limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second identity is rejected before its raster decode");

    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_TOTAL_DECODED_PIXELS_EXCEEDED
    );
    assert!(error.report().diagnostics().is_empty());
}

#[test]
fn repeated_nested_occurrences_share_decode_but_not_occurrence_admission() {
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><image width="1" height="1" href="data:image/png;base64,{PNG_BASE64}"/><image x="1" width="1" height="1" href="data:image/png;base64,{PNG_BASE64}"/></svg>"#
    );
    let scene = nested_svg_scene(parent.as_bytes(), 2, 1);
    let limits = ResourceLimits::builder()
        .max_asset_entries(3)
        .max_decoded_pixels_per_asset(1)
        .max_total_decoded_pixels(1)
        .build()
        .expect("shared nested identity limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("both occurrences render from one admitted decode identity");

    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn shared_parent_identity_never_bypasses_nested_occurrence_admission() {
    let parent = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image width="1" height="1" href="data:image/png;base64,{PNG_BASE64}"/></svg>"#
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(parent);
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[
            {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
            {{"id":"two","type":"image","x":1,"y":0,"width":1,"height":1,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
        ],"appState":{{}},"files":{{
            "one":{{"id":"one","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}},
            "two":{{"id":"two","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}
        }}}}"#
    );
    let scene = SceneDocument::parse(json.as_bytes()).expect("shared-parent scene is admitted");
    let limits = ResourceLimits::builder()
        .max_asset_entries(3)
        .build()
        .expect("asset occurrence limit is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("both authoritative parents admit their nested occurrence");

    assert_eq!(error.code(), ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED);
    assert!(error.report().diagnostics().is_empty());
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
fn data_images_are_not_admitted_as_link_actions() {
    let nested = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><a href="data:image/png;base64,{PNG_BASE64}"><rect width="20" height="10" fill="#00ff00"/></a></svg>"##
    );
    let scene = nested_svg_scene(nested.as_bytes(), 20, 10);
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
fn pdf_rasterizes_an_isolated_effect_closure_with_exact_diagnostics() {
    // Which closures a nested SVG's effects need, their bounds, device pixels
    // and causes are the PDF plan's decisions and are tested on the plan in
    // `render::plan_tests`; this keeps one closure reaching the PDF bytes with
    // the diagnostic the session owes for it.
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blur" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#blur)"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("isolated filter closure is rasterized");
    let pdf = pdf_text(outcome.artifact().bytes());

    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/Width 20"));
    assert!(pdf.contains("/Height 10"));
    assert_eq!(outcome.report().diagnostics().len(), 1);
    let diagnostic = &outcome.report().diagnostics()[0];
    assert_eq!(diagnostic.code(), DiagnosticCode::PDF_RASTER_FALLBACK);
    assert_eq!(diagnostic.json_pointer(), "/files/asset/dataURL");
    let Some(DiagnosticDetails::PdfRasterFallback {
        recovery: Some(Recovery::Rasterized),
        logical_bounds: Some(bounds),
        pixel_width: Some(20),
        pixel_height: Some(10),
        causes,
        ..
    }) = diagnostic.details()
    else {
        panic!("fallback diagnostic has exact typed details");
    };
    assert_eq!(
        (bounds.x(), bounds.y(), bounds.width(), bounds.height()),
        (10.0, 10.0, 20.0, 10.0)
    );
    assert!(!causes.is_empty());
}

#[test]
fn pdf_forbid_rejects_the_first_effect_closure_without_a_fallback_warning() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blur" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#blur)"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
        .build()
        .expect("request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("forbid rejects the remaining closure");

    assert_eq!(error.code(), ErrorCode::PDF_RASTER_FALLBACK_FORBIDDEN);
    assert_eq!(error.json_pointer(), "/files/asset/dataURL");
    assert!(error.report().diagnostics().is_empty());
    assert!(matches!(
        error.details(),
        Some(DiagnosticDetails::PdfRasterFallback {
            recovery: None,
            logical_bounds: Some(_),
            pixel_width: Some(20),
            pixel_height: Some(10),
            causes,
            ..
        }) if !causes.is_empty()
    ));
}

#[test]
fn pdf_rejects_effects_that_require_external_backdrop_capture() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blend" x="0" y="0" width="100%" height="100%"><feBlend in="SourceGraphic" in2="BackgroundImage" mode="multiply"/></filter></defs><rect width="20" height="10" filter="url(#blend)"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("external backdrop cannot be isolated");

    assert_eq!(error.code(), ErrorCode::PDF_RASTER_FALLBACK_UNISOLATABLE);
    assert!(error.report().diagnostics().is_empty());
}

#[test]
fn pdf_effect_surface_limit_is_preflighted_before_any_fallback_is_emitted() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blur" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" filter="url(#blur)"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
    let accepted_limits = ResourceLimits::builder()
        .max_raster_surface_pixels(200)
        .build()
        .expect("limits are valid");
    let accepted = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(accepted_limits)
        .build()
        .expect("request is valid");
    Renderer::new()
        .render(&scene, &accepted)
        .expect("the exact surface limit is accepted");

    let rejected_limits = ResourceLimits::builder()
        .max_raster_surface_pixels(199)
        .build()
        .expect("limits are valid");
    let rejected = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(rejected_limits)
        .build()
        .expect("request is valid");
    let error = Renderer::new()
        .render(&scene, &rejected)
        .expect_err("one pixel over the surface limit is rejected");

    assert_eq!(error.code(), ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED);
    assert!(error.report().diagnostics().is_empty());

    let combined_limits = ResourceLimits::builder()
        .max_raster_surface_pixels(199)
        .max_total_rasterized_pixels(199)
        .build()
        .expect("combined fallback limits are valid");
    let combined = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(combined_limits)
        .build()
        .expect("request is valid");
    let error = Renderer::new()
        .render(&scene, &combined)
        .expect_err("fatal aggregate pixels beat the local fallback-area crossing");

    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED
    );
    assert!(error.report().diagnostics().is_empty());
}

#[test]
fn svg_projection_preserves_sanitized_semantics_and_selectable_text() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
        <defs><path id="mark" d="M0 0h10v10H0z"/></defs>
        <use href="#mark" fill="#ff0000"/>
        <text x="10" y="9" font-family="Excalifont" font-size="10">A</text>
    </svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);
    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    let nested = embedded_svg(svg);

    assert!(nested.contains("<use"));
    assert!(nested.contains("<text"));
    assert!(nested.contains("<path"));
}

#[test]
fn png_projects_nested_vectors_at_final_device_scale() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 1 1"><circle cx="0.5" cy="0.5" r="0.5" fill="#ff0000"/></svg>"##;
    let scene = nested_svg_scene(nested, 100, 100);
    let png = render_artifact(&scene, OutputFormat::Png);
    let decoded = decode_png(&png);

    assert_eq!((decoded.width, decoded.height), (120, 120));
    assert_eq!(decoded.pixel(10, 10), [255, 255, 255, 255]);
    assert_eq!(decoded.pixel(60, 60), [255, 0, 0, 255]);
}

#[test]
fn pdf_projects_nested_text_as_vector_outlines() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><text x="2" y="16" font-family="Excalifont" font-size="18">ABC</text></svg>"##;
    let scene = nested_svg_scene(nested, 40, 20);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("outlined nested text renders in PDF");
    let pdf = pdf_text(outcome.artifact().bytes());

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
    let scene = nested_svg_scene(nested, 80, 40);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("closed vector-native matrix renders");
    let pdf = pdf_text(outcome.artifact().bytes());

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
        // Normal is PDF's default blend, which Krilla leaves unwritten.
        ("normal", ""),
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
        // PDF has no additive blend. usvg before 0.49 parsed `plus-lighter`
        // as `normal`, and the PDF still draws what it drew then.
        ("plus-lighter", ""),
    ];

    let mut normal_pdf = None;
    for (blend, pdf_name) in blends {
        let nested = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000"/><g style="mix-blend-mode:{blend};isolation:isolate"><circle cx="10" cy="5" r="5" fill="#0000ff"/></g></svg>"##
        );
        let scene = nested_svg_scene(nested.as_bytes(), 20, 10);
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
            .build()
            .expect("PDF request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("native blend renders without fallback");
        let pdf = pdf_text(outcome.artifact().bytes());
        match blend {
            "normal" => normal_pdf = Some(outcome.artifact().bytes().to_vec()),
            "plus-lighter" => assert_eq!(
                Some(outcome.artifact().bytes()),
                normal_pdf.as_deref(),
                "plus-lighter draws the normal-blend PDF"
            ),
            _ => {}
        }

        if pdf_name.is_empty() {
            assert!(!pdf.contains("/BM "), "unexpected PDF blend for {blend}");
        } else {
            assert!(pdf.contains(pdf_name), "missing PDF blend {blend}");
        }
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
    let scene = nested_svg_scene(nested.as_bytes(), 20, 10);

    let png = render_artifact(&scene, OutputFormat::Png);
    let decoded = decode_png(&png);
    assert_ne!(decoded.pixel(20, 15), [255, 255, 255, 255]);

    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("nested source raster renders in PDF");
    let pdf = pdf_text(outcome.artifact().bytes());
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
    let scene = nested_svg_scene(parent.as_bytes(), 20, 10);

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    assert!(embedded_svg(&embedded_svg(svg)).contains(&format!("data:image/bmp;base64,{encoded}")));

    let png = render_artifact(&scene, OutputFormat::Png);
    let decoded = decode_png(&png);
    assert_eq!(decoded.pixel(15, 15), [255, 0, 0, 255]);
    assert_eq!(decoded.pixel(25, 15), [24, 96, 208, 255]);

    let pdf = pdf_text(&render_artifact(&scene, OutputFormat::Pdf));
    assert!(pdf.contains("/Subtype /Image"));
    assert_eq!(crate::common::pdf_inspect::color_images(&pdf).len(), 1);
    assert!(pdf.contains(" Do\n"), "the normalized BMP is painted");
}

#[test]
fn unavailable_nested_sans_text_uses_a_deterministic_fallback_in_png_and_pdf() {
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40" fill="#326ce5"/><text x="4" y="31" style="font-family:Sans;font-size:32px;fill:#ffffff">ABC</text></svg>"##;
    let scene = nested_svg_scene(nested, 80, 40);

    let png = render_artifact(&scene, OutputFormat::Png);
    let decoded = decode_png(&png);
    let white_text_pixels = (12..48)
        .flat_map(|y| (12..88).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let [red, green, blue, _] = decoded.pixel(*x, *y);
            red > 220 && green > 220 && blue > 220
        })
        .count();
    assert!(
        white_text_pixels > 100,
        "fallback glyphs must remain visible, got {white_text_pixels} white pixels"
    );

    let pdf = pdf_text(&render_artifact(&scene, OutputFormat::Pdf));
    assert!(!pdf.contains("/Type /Font"));
    assert!(
        pdf.contains(" c\n"),
        "fallback glyphs must remain as vector outlines"
    );
}

#[test]
fn safe_class_styles_preserve_nested_opacity_and_layer_order() {
    let inline = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" style="fill:#ff0000;opacity:0.5"/><rect x="10" width="10" height="10" style="fill:#0000ff;opacity:0.5"/></svg>"##;
    let inline_scene = nested_svg_scene(inline, 20, 10);
    let inline_png = render_artifact(&inline_scene, OutputFormat::Png);
    let decoded = decode_png(&inline_png);
    assert_rgb_near(decoded.pixel(15, 15), [255, 128, 128, 255]);
    assert_rgb_near(decoded.pixel(25, 15), [128, 64, 191, 255]);

    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><style>.back{fill:#ff0000;opacity:0.5}.front{fill:#0000ff;opacity:0.5}</style></defs><rect class="back" width="20" height="10"/><rect class="front" x="10" width="10" height="10"/></svg>"##;
    let scene = nested_svg_scene(nested, 20, 10);

    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("PNG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("safe class styles render");
    let decoded = decode_png(outcome.artifact().bytes());
    assert_rgb_near(decoded.pixel(15, 15), [255, 128, 128, 255]);
    assert_rgb_near(decoded.pixel(25, 15), [128, 64, 191, 255]);
    assert!(outcome.report().diagnostics().is_empty());

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("artifact SVG is UTF-8");
    assert!(embedded_svg(svg).contains("<style>"));

    let pdf = pdf_text(&render_artifact(&scene, OutputFormat::Pdf));
    assert!(pdf.contains("/ca 0.5"));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn stylesheet_token_boundaries_preserve_admission_and_removal_sources() {
    for (style, admitted) in [
        ("rect {fill:rgb(255, 0, 0);opacity:50%}", true),
        (r#"[class="a,b"] {fill:red}"#, true),
        (r#"rect {font-family:"a:b,c";fill:red}"#, true),
        // Admission historically leaves invalid property values to usvg.
        ("rect {fill:unknown(nested(1, 2));opacity:1}", true),
        ("rect {fill:rgb(1, 2, 3}", true),
        (r#"rect {font-family:"unclosed}"#, true),
        (r#"rect {font-family:"a;fill:red"}"#, true),
        ("rect {fill:red", false),
        ("rect {fill:red}}", false),
        ("rect {fill:{red}}", false),
        (r#"[class="a:b"] {fill:red}"#, false),
        (r#"rect {font-family:"a;b";fill:red}"#, false),
        (r#"rect {font-family:"a}b";fill:red}"#, false),
        ("@media all {rect {fill:red}}", false),
        (r"rect {f\69ll:red}", false),
        ("rect {/* comment */fill:red}", false),
        ("rect {unknown:red}", false),
        ("rect {fill:url(https://example.invalid/paint)}", false),
        ("rect {fill:}", false),
    ] {
        let nested = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><style>{style}</style><rect width="20" height="10"/></svg>"#
        );
        let scene = nested_svg_scene(nested.as_bytes(), 20, 10);
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format).build().unwrap();
            let outcome = Renderer::new().render(&scene, &request).unwrap();
            let diagnostics = outcome.report().diagnostics();
            if admitted {
                assert!(
                    diagnostics.is_empty(),
                    "{style:?} / {format:?}: {diagnostics:?}"
                );
            } else {
                assert_eq!(diagnostics.len(), 1, "{style:?} / {format:?}");
                let diagnostic = &diagnostics[0];
                assert_eq!(
                    diagnostic.code(),
                    DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
                );
                assert_eq!(diagnostic.json_pointer(), "/files/asset/dataURL");
                let Some(DiagnosticDetails::NestedSource {
                    paths, recovery, ..
                }) = diagnostic.details()
                else {
                    panic!("style removal has source provenance");
                };
                assert_eq!(*recovery, Recovery::Removed);
                assert_eq!(
                    serde_json::to_value(paths).unwrap(),
                    serde_json::json!([
                        {"segments":[{"kind":"node","ordinal":1,"name":null}]}
                    ])
                );
            }
            if format == OutputFormat::Svg {
                let svg = embedded_svg(std::str::from_utf8(outcome.artifact().bytes()).unwrap());
                // Removing an at-rule or pointing a non-fragment URL at a
                // missing fragment keeps the stylesheet element.
                let kept = admitted || style.starts_with('@') || style.contains("url(https:");
                assert_eq!(svg.contains("<style>"), kept, "{style:?}");
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn stylesheet_uninterpreted_depth_preserves_asset_work_admission() {
    // Deep uninterpreted values and many rules after an unclosed value retain
    // their original admission without recursive or quadratic boundary work.
    for style in [
        format!(
            "rect {{fill:{}red{}}}",
            "unknown(".repeat(1024),
            ")".repeat(1024)
        ),
        format!("rect {{fill:unknown(}}{}", "rect {opacity:1}".repeat(1024)),
    ] {
        let nested = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><style>{style}</style><rect width="20" height="10"/></svg>"#
        );
        let scene = nested_svg_scene(nested.as_bytes(), 20, 10);
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format).build().unwrap();
            let outcome = Renderer::new().render(&scene, &request).unwrap();
            assert!(outcome.report().diagnostics().is_empty());
            let limits = ResourceLimits::builder()
                .max_asset_work_units(nested.len() as u64 - 1)
                .build()
                .unwrap();
            let request = RenderRequest::builder(format)
                .resource_limits(limits)
                .build()
                .unwrap();
            let outcome = Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED
            );
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn stylesheet_token_boundaries_retain_cascade_and_exact_effect_sources() {
    use serde_json::json;
    let styled = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><style>[class="unused,a"] {font-family:"family:part,tail";fill:rgb(255,0,0)} .fx, rect.fx {fill:#00ff00;filter:url(#effect)} [id] {opacity:0}</style><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" class="fx"/></svg>"##;
    let scene = nested_svg_scene(styled.as_bytes(), 20, 10);
    let request = RenderRequest::builder(OutputFormat::Pdf).build().unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("one stylesheet effect");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::PDF_RASTER_FALLBACK);
    assert_eq!(diagnostic.json_pointer(), "/files/asset/dataURL");
    let mut details = json!({
        "kind":"pdf_raster_fallback", "recovery":"rasterized",
        "logical_bounds":{"x":10,"y":10,"width":20,"height":10},
        "pixel_width":20,"pixel_height":10,
        "causes":[
            {"segments":[{"kind":"node","ordinal":6,"name":null}]},
            {"segments":[{"kind":"node","ordinal":1,"name":null},
                {"kind":"css_rule","ordinal":1,"name":null},
                {"kind":"css_declaration","ordinal":1,"name":"filter"}]},
            {"segments":[{"kind":"node","ordinal":5,"name":null}]}
        ]
    });
    assert_eq!(serde_json::to_value(diagnostic.details()).unwrap(), details);
    assert!(pdf_text(outcome.artifact().bytes()).contains("/Subtype /Image"));
    let forbid = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    let error = Renderer::new().render(&scene, &forbid).unwrap_err();
    assert_eq!(error.code(), ErrorCode::PDF_RASTER_FALLBACK_FORBIDDEN);
    assert_eq!(error.json_pointer(), "/files/asset/dataURL");
    details["recovery"] = json!(null);
    assert_eq!(serde_json::to_value(error.details()).unwrap(), details);
    assert!(error.report().diagnostics().is_empty());

    // Independently authored presentation attributes paint the same pixels.
    let presentation = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#effect)"/></svg>"##;
    assert_eq!(
        render_artifact(&scene, OutputFormat::Png),
        render_artifact(&nested_svg_scene(presentation, 20, 10), OutputFormat::Png)
    );

    // EM SPACE belongs to the selector identifier, not CSS whitespace.
    let unmatched = styled.replace(".fx, rect.fx", "\u{2003}rect");
    let outcome = Renderer::new()
        .render(&nested_svg_scene(unmatched.as_bytes(), 20, 10), &forbid)
        .unwrap();
    assert!(outcome.report().diagnostics().is_empty());
}

/// Existing independent rendering and bounded-admission evidence, selected by
/// the final nested-resource surface owner through the facade.
pub(super) fn assert_local_resource_contract() {
    nested_bmp_preserves_direct_svg_bytes_and_normalizes_png_and_pdf();
    basic_nested_svg_renders_in_svg_png_and_pdf();
    unsafe_nested_svg_boundaries_are_removed_before_embedding();
    same_document_fragments_resolve_without_ambient_resources();
    cyclic_local_references_are_bounded();
    nested_svg_node_ceiling_recovers_before_viewer_parsing();
    nested_data_svg_depth_ceiling_stops_recursive_resolution();
    nested_svg_sanitizer_is_bounded_by_asset_work();
    malformed_nested_image_is_removed_without_discarding_valid_siblings();
    nested_data_resources_participate_in_asset_occurrence_accounting();
    repeated_nested_occurrences_share_decode_but_not_occurrence_admission();
    shared_parent_identity_never_bypasses_nested_occurrence_admission();
    data_images_are_not_admitted_as_link_actions();
    svg_projection_preserves_sanitized_semantics_and_selectable_text();
    png_projects_nested_vectors_at_final_device_scale();
    pdf_projects_nested_text_as_vector_outlines();
    pdf_rasterizes_an_isolated_effect_closure_with_exact_diagnostics();
    pdf_forbid_rejects_the_first_effect_closure_without_a_fallback_warning();
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

/// A nested SVG whose one rectangle paints through a filter of `primitives`.
fn filtered_svg(primitives: &str) -> Vec<u8> {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><filter id="f">{primitives}</filter><rect width="100" height="100" fill="#00ff00" filter="url(#f)"/></svg>"##
    )
    .into_bytes()
}

#[test]
fn hostile_filter_effects_are_refused_by_their_charged_work() {
    // Each is a small SVG whose filter would keep resvg busy for minutes or
    // hours at this size: work that grows with octaves, kernel cells, the
    // morphology neighbourhood, or a long primitive chain. The charge is
    // measured before anything is rasterized, so the render is refused at
    // once in both rasterizing formats.
    let kernel = vec!["1"; 200 * 200].join(" ");
    let effects = [
        r#"<feTurbulence baseFrequency="0.9" numOctaves="1000000"/>"#.to_owned(),
        r#"<feTurbulence baseFrequency="0.9" numOctaves="1000"/>"#.to_owned(),
        r#"<feMorphology radius="100000"/>"#.to_owned(),
        format!(r#"<feConvolveMatrix order="200" kernelMatrix="{kernel}"/>"#),
        r#"<feGaussianBlur stdDeviation="5"/>"#.repeat(5000),
    ];
    for primitives in &effects {
        let scene = nested_svg_scene(&filtered_svg(primitives), 500, 500);
        for format in [OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format)
                .build()
                .expect("request is valid");
            let error = Renderer::new()
                .render(&scene, &request)
                .expect_err("the filter's work exceeds the default limits");
            let effect = &primitives[..primitives.len().min(40)];
            assert_eq!(
                error.code(),
                ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED,
                "{format:?} {effect}"
            );
            assert_eq!(error.element_id(), Some("image"), "{format:?} {effect}");
        }
    }
}

#[test]
fn convolve_orders_past_u32_are_removed_before_the_viewer_parses_them() {
    // usvg multiplies the two orders in `u32`: with overflow checks on (this
    // tier, and any consumer's debug build) a product past `u32::MAX` would
    // panic, and without them "65536 65537" wraps to a 65536-cell kernel the
    // matrix below matches. Each such `order` is dropped as unsafe content.
    let wrapped_kernel = vec!["1"; 65_536].join(" ");
    let hostile = [
        format!(r#"<feConvolveMatrix order="65536 65537" kernelMatrix="{wrapped_kernel}"/>"#),
        r#"<feConvolveMatrix order="1e10" kernelMatrix="1"/>"#.to_owned(),
        r#"<feConvolveMatrix order="70000,70000" kernelMatrix="1"/>"#.to_owned(),
    ];
    // Orders whose product fits are kept verbatim, the largest among them.
    let kept = [
        r#"<feConvolveMatrix order="2 3" kernelMatrix="1 1 1 1 1 1"/>"#,
        r#"<feConvolveMatrix order="65535 65537" kernelMatrix="1"/>"#,
        r#"<feConvolveMatrix order="-70000 70000" kernelMatrix="1"/>"#,
    ];
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        for primitive in &hostile {
            let effect = &primitive[..primitive.len().min(48)];
            let outcome = Renderer::new()
                .render(
                    &nested_svg_scene(&filtered_svg(primitive), 20, 10),
                    &request,
                )
                .unwrap_or_else(|error| panic!("{format:?} {effect}: {error:?}"));
            let removals = outcome
                .report()
                .diagnostics()
                .iter()
                .filter(|diagnostic| {
                    diagnostic.code() == DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
                })
                .count();
            assert_eq!(removals, 1, "{format:?} {effect}");
            if format == OutputFormat::Svg {
                let artifact = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                assert!(!embedded_svg(artifact).contains("order="), "{effect}");
            }
        }
        for primitive in kept {
            let outcome = Renderer::new()
                .render(
                    &nested_svg_scene(&filtered_svg(primitive), 20, 10),
                    &request,
                )
                .unwrap_or_else(|error| panic!("{format:?} {primitive}: {error:?}"));
            assert!(
                outcome.report().diagnostics().iter().all(|diagnostic| {
                    diagnostic.code() != DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED
                }),
                "{format:?} {primitive}"
            );
            if format == OutputFormat::Svg {
                let artifact = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                let order = primitive.split('"').nth(1).unwrap();
                assert!(
                    embedded_svg(artifact).contains(&format!(r#"order="{order}""#)),
                    "{primitive}"
                );
            }
        }
    }
}

#[test]
fn filter_effect_surfaces_count_toward_the_rasterized_pixel_limit() {
    // One blur fits the ceiling in both rasterizing formats; twenty chained
    // blurs each keep a result surface of the filter region, which the same
    // ceiling refuses.
    let limits = ResourceLimits::builder()
        .max_raster_surface_pixels(4_000)
        .max_total_rasterized_pixels(4_000)
        .build()
        .expect("limits are valid");
    let blur = r#"<feGaussianBlur stdDeviation="1"/>"#;
    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .expect("request is valid");
        let single = nested_svg_scene(&filtered_svg(blur), 20, 10);
        Renderer::new()
            .render(&single, &request)
            .expect("one blur fits the rasterized pixel ceiling");
        let chained = nested_svg_scene(&filtered_svg(&blur.repeat(20)), 20, 10);
        let error = Renderer::new()
            .render(&chained, &request)
            .expect_err("twenty blur results exceed the rasterized pixel ceiling");
        assert_eq!(
            error.code(),
            ErrorCode::RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED,
            "{format:?}"
        );
        assert_eq!(error.element_id(), Some("image"), "{format:?}");
    }
}

#[test]
fn far_offset_repeating_gradients_render_to_pdf() {
    // Each gradient puts the painted rect beyond 2^24 periods from its
    // origin, where period indices are no longer exact in f32: across the
    // wide rect it spans 100 periods there, across the narrow one it
    // collapses to a single point.
    for (rect_width, x2, offset) in [("100", "1", "-20000000"), ("1", "10", "-340000000")] {
        for spread in ["repeat", "reflect"] {
            let nested = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="paint" gradientUnits="userSpaceOnUse" x1="0" x2="{x2}" spreadMethod="{spread}" gradientTransform="translate({offset} 0)"><stop stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></linearGradient></defs><rect width="{rect_width}" height="100" fill="url(#paint)"/></svg>"##
            );
            let scene = nested_svg_scene(nested.as_bytes(), 100, 100);
            let pdf = render_artifact(&scene, OutputFormat::Pdf);
            assert!(pdf_text(&pdf).starts_with("%PDF-1.7"), "{spread} {offset}");
        }
    }
}

fn assert_rgb_near(actual: [u8; 4], expected: [u8; 4]) {
    for (actual_channel, expected_channel) in actual.into_iter().zip(expected) {
        assert!(
            actual_channel.abs_diff(expected_channel) <= 1,
            "pixel {actual:?} differs from {expected:?}"
        );
    }
}

fn nested_svg_scene(svg: &[u8], width: u32, height: u32) -> SceneDocument {
    image_scene(svg, "image/svg+xml", width, height)
}
