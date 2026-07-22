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

fn text_scene(font_family: i32, text: &str) -> SceneDocument {
    let source = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"text\",\"type\":\"text\",\"width\":160,\"height\":40,\"fontSize\":20,\"fontFamily\":{font_family},\"text\":{},\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}",
        serde_json::to_string(text).expect("test text serializes")
    );
    SceneDocument::parse(source.as_bytes()).expect("text scene is admitted")
}
