use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, Renderer, SceneDocument};

#[test]
fn admitted_latin_font_families_render_with_their_bundled_faces() {
    for (font_family, expected_family) in [
        (1, "Virgil"),
        (2, "Liberation Sans"),
        (3, "Cascadia Code"),
        (5, "Excalifont"),
        (6, "Nunito"),
        (7, "Lilita One"),
        (8, "Comic Shanns"),
        (9, "Liberation Sans"),
    ] {
        let scene = text_scene(font_family, "Catalog");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("SVG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("font family {font_family} renders: {error}"));
        let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

        assert!(
            svg.contains(&format!("font-family=\"{expected_family}\"")),
            "font family {font_family} must render as {expected_family}"
        );
        assert!(svg.contains("data:font/woff2;base64,"));
        if font_family == 2 {
            let [diagnostic] = outcome.report().diagnostics() else {
                panic!("Helvetica substitution has one diagnostic");
            };
            assert_eq!(
                diagnostic.code(),
                DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED
            );
            assert_eq!(diagnostic.element_id(), Some("text"));
            assert_eq!(diagnostic.json_pointer(), "/elements/0");
        } else {
            assert!(
                outcome.report().is_empty(),
                "font family {font_family} is bundled directly"
            );
        }
    }
}

#[test]
fn retained_latin_families_render_deterministically_in_png_and_pdf() {
    for (font_family, pdf_name) in [
        (1, "Virgil"),
        (2, "LiberationSans"),
        (3, "CascadiaCode"),
        (5, "Excalifont"),
        (6, "Nunito"),
        (7, "LilitaOne"),
        (8, "ComicShanns"),
        (9, "LiberationSans"),
    ] {
        let scene = text_scene(font_family, "Catalog");
        for format in [OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format)
                .build()
                .expect("request is valid");
            let first = Renderer::new()
                .render(&scene, &request)
                .unwrap_or_else(|error| panic!("font family {font_family} renders: {error}"));
            let second = Renderer::new()
                .render(&scene, &request)
                .expect("repeated rendering succeeds");

            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(first.report(), second.report());
            match format {
                OutputFormat::Png => assert!(first.artifact().bytes().starts_with(b"\x89PNG")),
                OutputFormat::Pdf => {
                    let pdf = String::from_utf8_lossy(first.artifact().bytes());
                    assert!(pdf.contains(&format!("/BaseFont /{pdf_name}")));
                    assert!(pdf.contains("/ActualText <FEFF0043006100740061006C006F0067>"));
                }
                _ => unreachable!("this test selects PNG and PDF"),
            }
        }
    }
}

#[test]
fn helvetica_substitution_is_reported_once_per_text_occurrence() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[
            {"id":"first","type":"text","width":100,"height":30,"fontSize":20,"fontFamily":2,"text":"First","updated":1},
            {"id":"direct","type":"text","x":120,"width":100,"height":30,"fontSize":20,"fontFamily":9,"text":"Direct","updated":2},
            {"id":"second","type":"text","x":240,"width":100,"height":30,"fontSize":20,"fontFamily":2,"text":"Second","updated":3}
        ],"appState":{},"files":{}}"#,
    )
    .expect("substitution scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("substituted text renders");

    let diagnostics = outcome.report().diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].element_id(), Some("first"));
    assert_eq!(diagnostics[1].element_id(), Some("second"));
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code() == DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED
    }));
}

#[test]
fn mixed_official_families_keep_independent_font_resources() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[
            {"id":"virgil","type":"text","width":100,"height":30,"fontSize":20,"fontFamily":1,"text":"Virgil","updated":1},
            {"id":"cascadia","type":"text","x":120,"width":100,"height":30,"fontSize":20,"fontFamily":3,"text":"Code","updated":2}
        ],"appState":{},"files":{}}"#,
    )
    .expect("mixed-family scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("mixed official families render");
        assert!(outcome.report().is_empty());
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("font-family:'Virgil'"));
                assert!(svg.contains("font-family:'Cascadia Code'"));
                assert!(svg.contains("font-family=\"Virgil\""));
                assert!(svg.contains("font-family=\"Cascadia Code\""));
                assert!(
                    svg.find("{\"asset_identity\":\"fonts/Cascadia").unwrap()
                        < svg.find("{\"asset_identity\":\"fonts/Virgil").unwrap(),
                    "mandatory notices are sorted by canonical asset identity"
                );
            }
            OutputFormat::Png => assert!(outcome.artifact().bytes().starts_with(b"\x89PNG")),
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
                assert!(pdf.contains("/BaseFont /Virgil"));
                assert!(pdf.contains("/BaseFont /CascadiaCode"));
                assert!(pdf.contains("/F2 20 Tf"));
                assert!(pdf.contains("/F3 20 Tf"));
            }
            _ => unreachable!("this test covers revision 1 formats"),
        }
    }
}

#[test]
fn empty_helvetica_text_does_not_report_a_substitution() {
    let scene = text_scene(2, "");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("empty text remains meaningful");
    assert!(outcome.report().is_empty());
}

#[test]
fn comic_shanns_carries_its_mit_notice_without_ofl_terms() {
    let scene = text_scene(8, "Comic");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("Comic Shanns renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("MIT License"));
    assert!(!svg.contains("SIL OPEN FONT LICENSE"));
}

#[test]
fn excalifont_uses_ordered_cjk_and_emoji_fallbacks_in_every_format() {
    let scene = text_scene(5, "A\u{4e2d}\u{1f600}");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("mixed-range text renders");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("repeated mixed-range rendering succeeds");

        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        let [diagnostic] = first.report().diagnostics() else {
            panic!("emoji substitution has one occurrence diagnostic");
        };
        assert_eq!(
            diagnostic.code(),
            DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED
        );
        assert_eq!(diagnostic.element_id(), Some("text"));

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("font-family:'Excalifont'"));
                assert!(svg.contains("font-family:'Xiaolai'"));
                assert!(svg.contains("font-family:'Noto Emoji'"));
                assert!(svg.contains("font-family=\"Xiaolai\""));
                assert!(svg.contains("font-family=\"Noto Emoji\""));
            }
            OutputFormat::Png => assert!(first.artifact().bytes().starts_with(b"\x89PNG")),
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(first.artifact().bytes());
                assert!(pdf.contains("/BaseFont /Excalifont"));
                assert!(pdf.contains("/BaseFont /Xiaolai"));
                assert!(pdf.contains("/BaseFont /NotoEmoji"));
                assert!(pdf.contains("/ActualText <FEFF00414E2DD83DDE00>"));
            }
            _ => unreachable!("this test covers revision 1 formats"),
        }
    }
}

#[test]
fn bidirectional_runs_keep_visual_direction_and_logical_pdf_text() {
    let scene = text_scene(9, "abc \u{5d0}\u{5d1}\u{5d2} 123");

    let svg_request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let svg_outcome = Renderer::new()
        .render(&scene, &svg_request)
        .expect("bidirectional SVG renders");
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg_outcome
        .report()
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH));
    assert!(svg.contains("direction=\"rtl\""));
    assert!(svg.contains("direction=\"ltr\""));

    let pdf_request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let first = Renderer::new()
        .render(&scene, &pdf_request)
        .expect("bidirectional PDF renders");
    let second = Renderer::new()
        .render(&scene, &pdf_request)
        .expect("repeated bidirectional PDF renders");
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    let pdf = String::from_utf8_lossy(first.artifact().bytes());
    assert!(pdf.contains("/ActualText <FEFF006100620063002005D005D105D20020003100320033>"));
}

#[test]
fn one_family_can_select_multiple_unicode_range_chunks() {
    let scene = SceneDocument::parse(
        "{\"type\":\"excalidraw\",\"version\":2,\"elements\":[\
            {\"id\":\"mixed\",\"type\":\"text\",\"width\":160,\"height\":40,\"fontSize\":20,\"fontFamily\":5,\"text\":\"AЖ\",\"updated\":1},\
            {\"id\":\"latin\",\"type\":\"text\",\"x\":180,\"width\":160,\"height\":40,\"fontSize\":20,\"fontFamily\":5,\"text\":\"Latin\",\"updated\":2}\
        ],\"appState\":{},\"files\":{}}"
            .as_bytes(),
    )
    .expect("multi-range scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("multiple chunks from one family render");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().is_empty());
    assert!(svg.matches("font-family:'Excalifont'").count() >= 2);
    assert!(svg.contains("font-family=\"Excalifont\""));
    assert!(svg.contains("AЖ"));
}

#[test]
fn codepoints_missing_from_the_complete_fallback_chain_use_replacement_glyphs() {
    let scene = text_scene(5, "A\u{10ffff}");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("missing glyph remains meaningful");
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("one missing scalar has one diagnostic");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::FONT_MISSING_GLYPH);
    assert_eq!(diagnostic.element_id(), Some("text"));
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"A\u{10ffff}\""));
    assert!(svg.contains(">A</tspan>"));
    assert!(svg.contains(">?</tspan>"));
    assert!(svg.contains("font-family=\"Liberation Sans\""));
}

fn text_scene(font_family: i32, text: &str) -> SceneDocument {
    let source = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"text\",\"type\":\"text\",\"width\":160,\"height\":40,\"fontSize\":20,\"fontFamily\":{font_family},\"text\":{},\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}",
        serde_json::to_string(text).expect("test text serializes")
    );
    SceneDocument::parse(source.as_bytes()).expect("text scene is admitted")
}
