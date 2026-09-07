use excalidraw_render::{
    DiagnosticCode, DiagnosticEffect, ErrorCode, ErrorDetails, OutputFormat, Recovery,
    RenderRequest, Renderer, ResourceLimits, SceneDocument,
};

use std::io::Cursor;

#[test]
fn unknown_element_renders_the_exact_transformed_svg_placeholder() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"unknown",
            "type":"future<&",
            "x":20,
            "y":30,
            "width":240,
            "height":100,
            "angle":0.25,
            "opacity":40,
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"##,
    )
    .expect("unknown element remains admitted scene content");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default SVG request is valid");

    let first = Renderer::new()
        .render(&scene, &request)
        .expect("bounded unknown content produces a meaningful placeholder");
    let second = Renderer::new()
        .render(&scene, &request)
        .expect("the same placeholder renders again");

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let diagnostics = boundary_diagnostics(&first);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("the unknown element produces one boundary diagnostic");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE
    );
    assert_eq!(
        diagnostic.effects(),
        [DiagnosticEffect::Visual, DiagnosticEffect::Semantic]
    );
    assert_eq!(diagnostic.element_id(), Some("unknown"));
    assert_eq!(diagnostic.json_pointer(), "/elements/0");
    assert!(matches!(
        diagnostic.details(),
        Some(ErrorDetails::Compatibility {
            recovery: Recovery::Placeholder,
            recipe_id: Some(recipe),
        }) if recipe == "renderer.unknown-element.v1"
    ));

    let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.starts_with(concat!(
        "<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" ",
        "viewBox=\"0 0 277.27937713600704 176.26819239215\""
    )));
    assert!(svg.contains(concat!(
        "<g transform=\"translate(18.639688568003514 38.13409619607499)",
        " rotate(14.32394487827058 120 50)\" opacity=\"0.4\">"
    )));
    assert!(svg.contains(concat!(
        "<clipPath id=\"fallback-0\"><rect x=\"0\" y=\"0\" width=\"240\" ",
        "height=\"100\"/></clipPath>"
    )));
    assert!(svg.contains(concat!(
        "<g clip-path=\"url(#fallback-0)\">",
        "<rect x=\"0\" y=\"0\" width=\"240\" height=\"100\" fill=\"#f1f3f5\"/>",
        "<rect x=\"0\" y=\"0\" width=\"240\" height=\"100\" fill=\"none\" ",
        "stroke=\"#868e96\" stroke-width=\"2\" stroke-dasharray=\"6 4\" stroke-dashoffset=\"0\"/>",
        "<path d=\"M0 0 L240 100 M240 0 L0 100\" fill=\"none\" stroke=\"#868e96\" ",
        "stroke-width=\"2\"/>"
    )));
    assert!(svg.contains(concat!(
        "<text x=\"120\" y=\"50\" fill=\"#495057\" font-family=\"ExcalidrawRender-9\" ",
        "font-size=\"14\" text-anchor=\"middle\" dominant-baseline=\"central\">",
        "Unsupported: future&lt;&amp;</text>"
    )));
}

#[test]
fn bounded_fallback_and_unbounded_omission_succeed_in_every_format() {
    let scene = SceneDocument::parse(
        br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[
            {"id":"unknown","type":"future","x":0,"y":0,"width":200,"height":80,"updated":1},
            null
        ],
        "appState":{},
        "files":{}
    }"#,
    )
    .expect("fallback and omission occurrences are admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("default request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("diagnosed local loss retains a meaningful artifact");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("the same diagnosed local loss renders again");

        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert_eq!(first.artifact().format(), format);
        assert_eq!(
            boundary_diagnostics(&first)
                .iter()
                .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
                .collect::<Vec<_>>(),
            [
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE,
                    "/elements/0"
                ),
                (
                    DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT,
                    "/elements/1"
                ),
            ]
        );
        assert!(matches!(
            boundary_diagnostics(&first)[1].details(),
            Some(ErrorDetails::Compatibility {
                recovery: Recovery::Omitted,
                recipe_id: None,
            })
        ));

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("Unsupported: future</text>"));
            }
            OutputFormat::Png => {
                let decoder = png::Decoder::new(Cursor::new(first.artifact().bytes()));
                let reader = decoder.read_info().expect("PNG structure is valid");
                assert_eq!((reader.info().width, reader.info().height), (220, 100));
            }
            OutputFormat::Pdf => {
                let pdf = first.artifact().bytes();
                assert!(pdf.starts_with(b"%PDF-1.7"));
                assert!(pdf.windows(10).any(|bytes| bytes == b"[6 4] 0 d\n"));
            }
            _ => panic!("the test covers the revision-1 format set"),
        }
    }
}

#[test]
fn compatibility_labels_use_full_ellipsis_or_omitted_fit_behavior() {
    let scene = SceneDocument::parse(
        br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[
            {"id":"wide","type":"future","x":0,"y":0,"width":200,"height":60,"updated":1},
            {"id":"narrow","type":"","x":220,"y":0,"width":20,"height":30,"updated":1},
            {"id":"tiny","type":"future","x":260,"y":0,"width":8,"height":8,"updated":1}
        ],
        "appState":{},
        "files":{}
    }"#,
    )
    .expect("fit vectors are admitted");

    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(svg.matches("<text ").count(), 2);
    assert!(svg.contains(">Unsupported: future</text>"));
    assert!(svg.contains(">…</text>"));
    assert!(svg.contains("<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">"));
    assert!(svg.contains("Modified Version under font.liberation-sans.ofl-1.1."));

    let png = render(&scene, OutputFormat::Png);
    let (_, _, rgba) = decode_png(png.artifact().bytes());
    assert!(rgba
        .chunks_exact(4)
        .any(|pixel| pixel == [0x49, 0x50, 0x57, 0xff]));
    assert!(!png
        .artifact()
        .bytes()
        .windows("liberation-sans.ofl-full".len())
        .any(|window| window == b"liberation-sans.ofl-full"));

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/ActualText <FEFF0055006E0073007500700070006F0072007400650064003A0020006600750074007500720065>"));
    assert!(pdf.contains("/ActualText <FEFF2026>"));
    assert!(pdf.contains("/Subtype /Type0"));
    assert!(pdf.contains("/ToUnicode "));
    assert!(pdf.contains("BT\n/F1 14 Tf\n"));
    assert!(pdf.contains("/Type /Metadata\n/Subtype /XML"));
    assert!(pdf.contains("<er:manifest>{\"schema_version\":1,"));
    assert!(pdf.contains("Digitized data copyright (c) 2010 Google Corporation\\n"));
}

#[test]
fn arrows_and_freedraw_render_without_known_element_fallbacks() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[
            {
                "id":"arrow",
                "type":"arrow",
                "x":0,
                "y":0,
                "width":180,
                "height":60,
                "points":[[0,0],[180,60]],
                "strokeColor":"#1e1e1e",
                "backgroundColor":"transparent",
                "fillStyle":"solid",
                "strokeWidth":2,
                "strokeStyle":"solid",
                "roughness":1,
                "opacity":100,
                "seed":1,
                "updated":1
            },
            {"id":"freedraw","type":"freedraw","x":210,"y":0,"width":180,"height":60,
                "points":[[0,0],[180,60]],"pressures":[],"simulatePressure":true,"updated":1}
        ],
        "appState":{},
        "files":{}
    }"##,
    )
    .expect("known element vectors are admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);
        assert!(boundary_diagnostics(&outcome).is_empty());
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
            assert!(!svg.contains(">Unavailable: arrow</text>"));
            assert!(!svg.contains(">Unavailable: freedraw</text>"));
        }
    }
}

#[test]
fn compatibility_placeholder_preserves_safe_links_only_in_semantic_formats() {
    let link = "https://example.com/path?x=1&y=2";
    let source = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"linked","type":"future","x":0,"y":0,"width":200,"height":60,"angle":0.2,"link":"{link}","updated":1}}],"appState":{{}},"files":{{}}}}"#
    );
    let scene = SceneDocument::parse(source.as_bytes()).expect("safe linked fallback is admitted");

    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("<a href=\"https://example.com/path?x=1&amp;y=2\">"));
    assert!(svg.contains("</g></a>"));

    let png = render(&scene, OutputFormat::Png);
    assert!(!png
        .artifact()
        .bytes()
        .windows(link.len())
        .any(|window| window == link.as_bytes()));

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/Subtype /Link"));
    assert!(pdf.contains("/S /URI\n/URI (https://example.com/path?x=1&y=2)"));
    assert!(pdf.contains("/QuadPoints ["));
    let page = pdf
        .split_once("/Type /Page\n")
        .map(|(_, page)| page)
        .expect("PDF has one page dictionary");
    assert!(page.contains("/Resources <<\n/ColorSpace"));
    assert!(page.contains(">>\n/Annots ["));
}

#[test]
fn pinned_link_sanitizer_preserves_its_safe_canonical_value_and_omits_active_actions() {
    for (link, expected) in [
        (
            "  https://example.com/a\\tb  ",
            Some("https://example.com/ab"),
        ),
        ("../relative/path", Some("../relative/path")),
        ("ftp://example.com/file", Some("ftp://example.com/file")),
        ("ééééé:https", Some("ééééé:https")),
        ("java&#115;cript:alert(1)", None),
        ("java&#4294967411;cript:alert(1)", None),
        ("java&#0000000000000000000000115;cript:alert(1)", None),
        ("data:text/html,unsafe", None),
        ("vbscript:unsafe", None),
    ] {
        let source = format!(
            r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"linked","type":"future","x":0,"y":0,"width":200,"height":60,"link":"{link}","updated":1}}],"appState":{{}},"files":{{}}}}"#
        );
        let scene = SceneDocument::parse(source.as_bytes()).expect("link case is admitted");
        let outcome = render(&scene, OutputFormat::Svg);
        let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

        match expected {
            Some(expected) => {
                assert!(svg.contains(&format!("<a href=\"{expected}\">")), "{link}");
                assert!(!outcome
                    .report()
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code() == DiagnosticCode::LINK_UNSAFE_OMITTED));
            }
            None => {
                assert!(!svg.contains("<a href="), "{link}");
                assert!(
                    outcome
                        .report()
                        .diagnostics()
                        .iter()
                        .any(|diagnostic| diagnostic.code() == DiagnosticCode::LINK_UNSAFE_OMITTED),
                    "{link}"
                );
            }
        }
    }
}

#[test]
fn supported_linked_elements_keep_navigation_in_semantic_formats() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"rectangle","type":"rectangle","x":0,"y":0,"width":120,"height":60,"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid","roundness":null,"roughness":1,"opacity":100,"seed":1,"isDeleted":false,"groupIds":[],"frameId":null,"boundElements":[],"link":"/docs","version":1,"versionNonce":0,"updated":1,"locked":false}],"appState":{},"files":{}}"##,
    )
    .expect("linked rectangle is admitted");

    let svg = render(&scene, OutputFormat::Svg);
    assert!(!svg.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
    }));
    let svg_bytes = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg_bytes.contains("<a href=\"/docs\">"));

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/Subtype /Link"));
    assert!(pdf.contains("/URI (http://reference.invalid/docs)"));
}

#[test]
fn pdf_link_annotations_are_ordered_from_topmost_to_bottommost() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"bottom","type":"rectangle","x":0,"y":0,"width":120,"height":60,"strokeColor":"#1e1e1e","link":"https://example.com/bottom","seed":1,"updated":1},{"id":"top","type":"rectangle","x":10,"y":10,"width":120,"height":60,"strokeColor":"#1e1e1e","link":"https://example.com/top","seed":2,"updated":2}],"appState":{},"files":{}}"##,
    )
    .expect("overlapping links are admitted");

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    let top = pdf.find("/URI (https://example.com/top)").unwrap();
    let bottom = pdf.find("/URI (https://example.com/bottom)").unwrap();
    assert!(top < bottom);
}

#[test]
fn iframe_and_embeddable_render_the_official_offline_shell_in_the_final_pass() {
    let scene = SceneDocument::parse(
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"iframe","type":"iframe","x":0,"y":0,"width":240,"height":100,"strokeColor":"transparent","backgroundColor":"transparent","seed":1,"updated":1},
                {"id":"after","type":"rectangle","x":300,"y":0,"width":40,"height":40,"strokeColor":"#ff0000","seed":2,"updated":2},
                {"id":"embed","type":"embeddable","x":0,"y":140,"width":240,"height":100,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","link":"https://example.com/embed","seed":3,"updated":3}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("offline embeddables are admitted");

    let outcome = render(&scene, OutputFormat::Svg);
    assert!(!outcome.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
    }));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"IFrame element\""));
    assert!(svg.contains("data-source-text=\"https://example.com/embed\""));
    assert!(svg.contains("fill=\"#d3d3d3\""));
    assert!(svg.contains("fill=\"#a5d8ff\""));
    assert!(
        svg.rfind("data-source-text=\"IFrame element\"").unwrap() > svg.find("#ff0000").unwrap()
    );
    assert!(svg.contains("<a href=\"https://example.com/embed\">"));
    assert!(svg.contains("ExcalidrawRender-9"));

    let png = render(&scene, OutputFormat::Png);
    let (_, _, rgba) = decode_png(png.artifact().bytes());
    assert!(rgba
        .chunks_exact(4)
        .any(|pixel| pixel[..3] == [0xd3, 0xd3, 0xd3]));
    assert!(!png
        .artifact()
        .bytes()
        .windows(8)
        .any(|bytes| bytes == b"https://"));

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/Subtype /Link"));
    assert!(pdf.contains("/URI (https://example.com/embed)"));
    assert!(pdf.contains("/ActualText <FEFF"));
}

#[test]
fn unsupported_embeddable_values_use_field_recovery_without_replacing_the_shell() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"embed","type":"embeddable","x":0,"y":0,"width":200,"height":80,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","strokeWidth":0,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
    )
    .expect("embeddable with a recoverable field value is admitted");

    let outcome = render(&scene, OutputFormat::Svg);
    assert!(outcome.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE
            && diagnostic.json_pointer() == "/elements/0/strokeWidth"
    }));
    assert!(!outcome.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
    }));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"Empty Web-Embed\""));
    assert!(svg.contains("fill=\"#a5d8ff\""));
}

/// The official export derives the placeholder label from the element's link
/// after its restore path has sanitized it, so an unsafe link labels its
/// placeholder `about:blank` and is never shown as text. The navigation itself
/// is still omitted and diagnosed.
#[test]
fn unsafe_embeddable_navigation_is_omitted_and_its_label_is_the_restored_link() {
    let link = "javascript:alert(1)";
    let source = format!(
        r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"embed","type":"embeddable","x":0,"y":0,"width":200,"height":80,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","link":"{link}","seed":1,"updated":1}}],"appState":{{}},"files":{{}}}}"##
    );
    let scene = SceneDocument::parse(source.as_bytes()).expect("unsafe embeddable is admitted");

    let outcome = render(&scene, OutputFormat::Svg);
    assert!(outcome
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::LINK_UNSAFE_OMITTED));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"about:blank\""));
    assert!(!svg.contains("javascript:alert(1)"));
    assert!(!svg.contains("<a href="));
}

/// A safe link labels its placeholder with the sanitized link text, which for
/// an ordinary URL is the link itself; surrounding whitespace is trimmed the
/// way the restore path trims it.
#[test]
fn safe_embeddable_label_is_the_sanitized_link() {
    let source = r##"{"type":"excalidraw","version":2,"elements":[{"id":"embed","type":"embeddable","x":0,"y":0,"width":200,"height":80,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","link":"  https://example.com/embed  ","seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source.as_bytes()).expect("embeddable is admitted");
    let outcome = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"https://example.com/embed\""));
}

#[test]
fn long_embeddable_links_wrap_within_the_offline_label_width() {
    let link = "https://example.com/this-is-a-deliberately-long-embeddable-link";
    let source = format!(
        r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"embed","type":"embeddable","x":0,"y":0,"width":120,"height":100,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","link":"{link}","seed":1,"updated":1}}],"appState":{{}},"files":{{}}}}"##
    );
    let scene =
        SceneDocument::parse(source.as_bytes()).expect("long linked embeddable is admitted");

    let outcome = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.matches("<tspan ").count() > 1);
}

#[test]
fn compatibility_placeholder_omits_components_that_do_not_fit_source_bounds() {
    let scene = SceneDocument::parse(
        br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"tiny","type":"future","x":0,"y":0,"width":1,"height":1,"updated":1}],
        "appState":{},"files":{}
    }"#,
    )
    .expect("tiny positive bounds are recoverable");

    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("width=\"1\" height=\"1\" fill=\"#f1f3f5\""));
    assert!(!svg.contains("stroke=\"#868e96\""));
    assert!(!svg.contains("<path "));
    assert!(!svg.contains("<text "));

    let png = render(&scene, OutputFormat::Png);
    let (_, _, rgba) = decode_png(png.artifact().bytes());
    assert!(!rgba
        .chunks_exact(4)
        .any(|pixel| pixel[..3] == [0x86, 0x8e, 0x96]));

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(!pdf.contains("[6 4] 0 d"));
    assert!(!pdf.contains("/ActualText"));
}

#[test]
fn fallback_geometry_is_admitted_as_its_own_semantic_transaction() {
    let scene = SceneDocument::parse(
        br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"tiny","type":"future","x":0,"y":0,"width":1,"height":1,"updated":1}],
        "appState":{},"files":{}
    }"#,
    )
    .expect("tiny fallback is admitted");

    let accepted = ResourceLimits::builder()
        .max_geometry_segments_per_element(5)
        .max_total_geometry_segments(5)
        .build()
        .expect("fallback geometry equality limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(accepted)
        .build()
        .expect("resource-limited SVG request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("the exact five-command fallback fits");

    let rejected = ResourceLimits::builder()
        .max_geometry_segments_per_element(4)
        .max_total_geometry_segments(4)
        .build()
        .expect("fallback geometry crossing limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(rejected)
        .build()
        .expect("resource-limited SVG request is valid");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the fifth fallback command crosses atomically");

    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("tiny"));
    assert_eq!(error.json_pointer(), "/elements/0");
}

#[test]
fn compatibility_placeholder_preserves_frame_clipping_in_every_format() {
    let scene = SceneDocument::parse(br#"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"frame","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
            {"id":"child","type":"future","x":80,"y":10,"width":80,"height":40,"frameId":"frame","updated":1}
        ],
        "appState":{},"files":{}
    }"#)
    .expect("framed fallback is admitted");

    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(svg.matches("<clipPath id=\"frame-clip-0\">").count(), 1);
    assert!(svg.contains("<g clip-path=\"url(#frame-clip-0)\">"));

    let png = render(&scene, OutputFormat::Png);
    let (width, _, rgba) = decode_png(png.artifact().bytes());
    let pixel = |x: u32, y: u32| {
        let offset = usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4)
            .expect("fixture pixel offset fits usize");
        &rgba[offset..offset + 4]
    };
    assert_ne!(pixel(95, 64), [255, 255, 255, 255]);
    assert_eq!(pixel(140, 64), [255, 255, 255, 255]);

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.matches("W\nn\n").count() >= 2);
    assert!(!pdf.contains("/Subtype /Image"));
}

#[test]
fn fallback_label_text_is_preflighted_at_aggregate_and_local_boundaries() {
    let scene = SceneDocument::parse(br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"unknown","type":"future","x":0,"y":0,"width":200,"height":60,"updated":1}],
        "appState":{},"files":{}
    }"#)
    .expect("fallback text vector is admitted");

    let accepted = ResourceLimits::builder()
        .max_text_scalars_per_element(19)
        .max_total_text_scalars(19)
        .build()
        .expect("fallback text equality limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(accepted)
        .build()
        .expect("resource-limited SVG request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("all nineteen generated scalars fit at equality");

    let aggregate = ResourceLimits::builder()
        .max_text_scalars_per_element(18)
        .max_total_text_scalars(18)
        .build()
        .expect("aggregate text crossing limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(aggregate)
        .build()
        .expect("resource-limited SVG request is valid");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the nineteenth generated scalar crosses atomically");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_TEXT_EXCEEDED);
    assert_eq!(error.element_id(), Some("unknown"));

    let competing = ResourceLimits::builder()
        .max_geometry_segments_per_element(4)
        .max_total_geometry_segments(4)
        .max_text_scalars_per_element(18)
        .max_total_text_scalars(18)
        .build()
        .expect("competing fallback limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(competing)
        .build()
        .expect("resource-limited SVG request is valid");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("geometry wins at a shared fallback source point");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);

    let local = ResourceLimits::builder()
        .max_text_scalars_per_element(18)
        .max_total_text_scalars(19)
        .build()
        .expect("local text crossing limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(local)
        .build()
        .expect("resource-limited SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("local fallback exhaustion retains source-sized artifact bounds");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("viewBox=\"0 0 220 80\""));
    assert!(svg.contains("fill=\"#f1f3f5\""));
    assert!(!svg.contains("Unsupported: future"));
    assert!(outcome.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
    }));
}

#[test]
fn png_fallback_surface_is_charged_after_the_required_main_raster() {
    let scene = SceneDocument::parse(br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"unknown","type":"future","x":0,"y":0,"width":200,"height":60,"updated":1}],
        "appState":{},"files":{}
    }"#)
    .expect("fallback raster vector is admitted");

    let aggregate = ResourceLimits::builder()
        .max_raster_surface_pixels(17_600)
        .max_total_rasterized_pixels(29_599)
        .build()
        .expect("aggregate fallback raster limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(aggregate)
        .build()
        .expect("resource-limited PNG request is valid");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("aggregate fallback pixels beat local recovery");
    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED
    );

    let equality = ResourceLimits::builder()
        .max_raster_surface_pixels(17_600)
        .max_total_rasterized_pixels(29_600)
        .build()
        .expect("fallback raster equality limits are valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(equality)
        .build()
        .expect("resource-limited PNG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("the main and fallback surfaces fit at exact aggregate equality");
    let (_, _, rgba) = decode_png(outcome.artifact().bytes());
    assert!(rgba
        .chunks_exact(4)
        .any(|pixel| pixel != [255, 255, 255, 255]));
}

#[test]
fn compatibility_placeholder_opacity_is_applied_to_the_composited_group() {
    let scene = SceneDocument::parse(br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"unknown","type":"future","x":0,"y":0,"width":200,"height":60,"opacity":40,"updated":1}],
        "appState":{},"files":{}
    }"#)
    .expect("translucent fallback is admitted");

    let png = render(&scene, OutputFormat::Png);
    let (width, _, rgba) = decode_png(png.artifact().bytes());
    let pixel = |x: u32, y: u32| {
        let offset = usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4)
            .expect("fixture pixel offset fits usize");
        &rgba[offset..offset + 4]
    };
    assert_eq!(pixel(30, 20), [249, 250, 251, 255]);

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/Group << /S /Transparency /I true /CS /DeviceRGB >>"));
    assert!(pdf.contains("/FB1 Do"));
}

#[test]
fn generated_labels_report_controlled_font_substitution_and_missing_glyphs() {
    let scene = SceneDocument::parse(r#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"unknown","type":"future😀","x":0,"y":0,"width":240,"height":60,"updated":1}],
        "appState":{},"files":{}
    }"#.as_bytes())
    .expect("Unicode fallback label is admitted");

    let outcome = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(!svg.contains("future😀"));
    assert!(svg.contains("future?"));
    assert!(outcome.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED
    }));
    let missing = outcome
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH)
        .expect("unsupported emoji uses the controlled replacement glyph");
    assert!(matches!(
        missing.details(),
        Some(ErrorDetails::MissingGlyph {
            codepoint: 0x1f600,
            recovery: Recovery::Substituted,
        })
    ));
}

#[test]
fn embeddable_labels_use_noto_for_emoji_occurrences() {
    let scene = SceneDocument::parse(
        r##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"embed","type":"embeddable","x":0,"y":0,"width":260,"height":80,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","link":"https://example.com/😀","seed":1,"updated":1}],
        "appState":{},"files":{}
    }"##
        .as_bytes(),
    )
    .expect("emoji embeddable label is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);
        assert!(outcome.report().diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED
        }));
        assert!(!outcome
            .report()
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH }));
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
            assert!(svg.contains("font-family=\"Noto Emoji\""));
        }
    }
}

#[test]
fn resource_specific_omission_never_uses_the_generic_known_element_box() {
    let scene = SceneDocument::parse(br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","x":0,"y":0,"width":200,"height":60,"fileId":"missing","status":"saved","updated":1}],
        "appState":{},"files":{}
    }"#)
    .expect("missing image vector is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Pdf] {
        let outcome = render(&scene, format);
        assert!(outcome.report().diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == DiagnosticCode::IMAGE_MISSING_FILE
                && matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Omitted,
                        ..
                    })
                )
        }));
        assert!(!outcome.report().diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
        }));
        assert!(!outcome
            .artifact()
            .bytes()
            .windows("Unavailable: image".len())
            .any(|window| window == b"Unavailable: image"));
    }
}

#[test]
fn scene_text_reports_missing_glyphs_and_emoji_substitution_in_every_format() {
    // The neighbouring tests cover generated compatibility labels and embeddable
    // labels. This covers the same two font paths reached from an ordinary scene
    // text element, where the renderer diverges from the official export on
    // purpose: official draws whatever its environment supplies, revision 1
    // draws the deterministic basic-Latin replacement or the bundled Noto Emoji
    // face and reports it. Both divergences are specified and each is evidenced
    // as a parity case with a recorded exception (`focused.missing-glyph.v1`,
    // #124; `focused.emoji-fallback.v1`, #121); this pins the two paths side by
    // side in one scene.
    let scene = SceneDocument::parse(
        r##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"unmapped","type":"text","x":20,"y":20,"width":200,"height":40,"text":"𐀀𐀁","originalText":"𐀀𐀁","fontSize":28,"fontFamily":5,"textAlign":"left","verticalAlign":"top","lineHeight":1.25,"containerId":null,"seed":31,"updated":1},
            {"id":"emoji","type":"text","x":20,"y":80,"width":200,"height":40,"text":"😀","originalText":"😀","fontSize":28,"fontFamily":5,"textAlign":"left","verticalAlign":"top","lineHeight":1.25,"containerId":null,"seed":32,"updated":1}
        ],
        "appState":{},"files":{}
    }"##
        .as_bytes(),
    )
    .expect("unmapped scene text is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);

        // Linear B U+10000 and U+10001 are absent from every bundled face.
        let missing: Vec<_> = outcome
            .report()
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH)
            .collect();
        assert_eq!(
            missing.len(),
            2,
            "each unmapped scene codepoint reports once for {format:?}"
        );
        for (diagnostic, codepoint) in missing.iter().zip([0x10000_u32, 0x10001]) {
            assert!(
                matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::MissingGlyph {
                        codepoint: reported,
                        recovery: Recovery::Substituted,
                    }) if *reported == codepoint
                ),
                "unmapped scene codepoint {codepoint:#x} is substituted for {format:?}"
            );
        }

        // Emoji in scene text resolves through the bundled Noto Emoji face and is
        // therefore never reported as a missing glyph.
        assert!(
            outcome
                .report()
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED),
            "scene emoji uses the bundled Noto Emoji face for {format:?}"
        );
    }

    // The replacement is deterministic and reaches the SVG text content.
    let svg = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
    assert!(
        svg.contains("??"),
        "unmapped text uses the replacement glyph"
    );
}

fn render(scene: &SceneDocument, format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("default request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("compatibility content renders")
}

fn boundary_diagnostics(
    outcome: &excalidraw_render::RenderOutcome,
) -> Vec<&excalidraw_render::CompatibilityDiagnostic> {
    outcome
        .report()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code(),
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE
                    | DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT
                    | DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
            )
        })
        .collect()
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
    rgba.truncate(frame.buffer_size());
    (frame.width, frame.height, rgba)
}
