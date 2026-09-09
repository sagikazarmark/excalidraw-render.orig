use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, RenderRequest, Renderer, ResourceLimits,
    SceneDocument,
};

use crate::common::pdf_inspect::{declares_base_font, embedded_pdf_fonts, pdf_text};
use crate::common::svg::{
    font_family_attribute, CASCADIA_FAMILIES, COMIC_SHANNS_FAMILIES, EXCALIFONT_FAMILIES,
    LIBERATION_FAMILIES, LILITA_FAMILIES, NOTO_EMOJI_FAMILIES, NUNITO_FAMILIES, VIRGIL_FAMILIES,
    XIAOLAI_FAMILIES,
};

#[test]
fn admitted_latin_font_families_render_with_their_bundled_faces() {
    // Each element names the effective family it embeds first and the
    // consumer fallback chain spec 8.4 fixes for the family after it.
    for (font_family, expected_family) in [
        (1, VIRGIL_FAMILIES),
        (2, LIBERATION_FAMILIES),
        (3, CASCADIA_FAMILIES),
        (5, EXCALIFONT_FAMILIES),
        (6, NUNITO_FAMILIES),
        (7, LILITA_FAMILIES),
        (8, COMIC_SHANNS_FAMILIES),
        (9, LIBERATION_FAMILIES),
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
            svg.contains(&font_family_attribute(expected_family)),
            "font family {font_family} must render as {expected_family}"
        );
        assert!(svg.contains("data:font/woff2;base64,"));
        assert_eq!(svg.matches("<style class=\"style-fonts\">").count(), 1);
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
fn retained_latin_families_embed_their_pdf_base_font_names() {
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
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .build()
            .expect("request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("font family {font_family} renders: {error}"));
        let pdf = pdf_text(outcome.artifact().bytes());
        assert!(
            declares_base_font(&pdf, pdf_name),
            "font family {font_family} embeds {pdf_name}"
        );
        assert!(pdf.contains("/ActualText <FEFF0043006100740061006C006F0067>"));
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
                assert!(svg.contains("font-family:'ExcalidrawRender-1'"));
                assert!(svg.contains("font-family:'ExcalidrawRender-3'"));
                assert!(svg.contains(&font_family_attribute(VIRGIL_FAMILIES)));
                assert!(svg.contains(&font_family_attribute(CASCADIA_FAMILIES)));
                assert_eq!(svg.matches("<style class=\"style-fonts\">").count(), 1);
                assert!(svg.contains("svg-font-subset@sha256:"));
            }
            OutputFormat::Png => assert!(outcome.artifact().bytes().starts_with(b"\x89PNG")),
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                assert!(declares_base_font(&pdf, "Virgil"));
                assert!(declares_base_font(&pdf, "CascadiaCode"));
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

        let [diagnostic] = first.report().diagnostics() else {
            panic!("emoji substitution has one occurrence diagnostic");
        };
        assert_eq!(
            diagnostic.code(),
            DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED
        );
        assert_eq!(diagnostic.element_id(), Some("text"));
        // Messages are non-normative (spec 11.1), but this one names the face
        // the substitution used: the emoji went to Noto Emoji, not to the
        // Liberation Sans replacement the other substitution diagnostic reports.
        assert_eq!(diagnostic.message(), "emoji text used bundled Noto Emoji");

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("font-family:'Excalifont'"));
                assert!(svg.contains("font-family:'Xiaolai'"));
                assert!(svg.contains("font-family:'Noto Emoji'"));
                assert!(svg.contains(&font_family_attribute(XIAOLAI_FAMILIES)));
                assert!(svg.contains(&font_family_attribute(NOTO_EMOJI_FAMILIES)));
            }
            OutputFormat::Png => assert!(first.artifact().bytes().starts_with(b"\x89PNG")),
            OutputFormat::Pdf => {
                let pdf = pdf_text(first.artifact().bytes());
                assert!(declares_base_font(&pdf, "Excalifont"));
                assert!(declares_base_font(&pdf, "Xiaolai"));
                assert!(declares_base_font(&pdf, "NotoEmoji"));
                assert!(pdf.contains("/ActualText <FEFF00414E2DD83DDE00>"));
            }
            _ => unreachable!("this test covers revision 1 formats"),
        }
    }
}

#[test]
fn xiaolai_stays_declared_when_local_recovery_omits_the_text_that_fired_the_trigger() {
    // The trigger is evaluated once, before per-element local recovery, and
    // carried with the resolved scene. When the only CJK text is then omitted
    // for exceeding its work limit, the surviving won sign still renders with
    // Xiaolai in every format and the SVG still declares the face it uses,
    // rather than emitting a run in an undeclared family.
    let han = "\u{4e2d}".repeat(400);
    let scene = SceneDocument::parse(
        format!(
            r#"{{"type":"excalidraw","version":2,"elements":[
                {{"id":"won","type":"text","width":160,"height":40,"fontSize":20,"fontFamily":5,"text":"\u20a9","updated":1}},
                {{"id":"han","type":"text","x":180,"width":8000,"height":40,"fontSize":20,"fontFamily":5,"text":"{han}","updated":2}}
            ],"appState":{{}},"files":{{}}}}"#
        )
        .as_bytes(),
    )
    .expect("omission scene is admitted");
    let limits = ResourceLimits::builder()
        .max_element_work_units(60_000)
        .build()
        .expect("element work limit is valid");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .expect("request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("the omitted text leaves a meaningful artifact");
        let codes = outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.element_id(), diagnostic.code()))
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            [(Some("han"), DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED)],
            "only the CJK text is omitted for {format:?}"
        );
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("font-family:'Xiaolai'"));
                assert_eq!(
                    svg.matches(&format!(
                        "<tspan {}",
                        font_family_attribute(XIAOLAI_FAMILIES)
                    ))
                    .count(),
                    1
                );
                assert!(svg.contains(">\u{20a9}</tspan>"));
                assert!(!svg.contains('\u{4e2d}'));
            }
            OutputFormat::Png => assert!(outcome.artifact().bytes().starts_with(b"\x89PNG")),
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                assert!(declares_base_font(&pdf, "Xiaolai"));
                assert!(pdf.contains("/ActualText <FEFF20A9>"));
                assert!(!pdf.contains("4E2D"));
            }
            _ => unreachable!("this test covers revision 1 formats"),
        }
    }
}

#[test]
fn pdf_embeds_the_pinned_face_whose_declared_range_covers_the_text() {
    // Every Nunito chunk carries an `A` glyph, so glyph coverage alone cannot
    // choose a face and catalog order would embed the Cyrillic chunk. Face
    // selection follows declared Unicode ranges in pinned face order, so Latin
    // text embeds the Latin chunk: the one mapping U+00E9 and not U+0416.
    let scene = text_scene(6, "A");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("Nunito renders");
    let fonts = embedded_pdf_fonts(outcome.artifact().bytes());
    let [font] = fonts.as_slice() else {
        panic!("one Nunito face is embedded, found {}", fonts.len());
    };
    let face = rustybuzz::Face::from_slice(font, 0).expect("embedded font parses");

    assert!(face.glyph_index('A').is_some());
    assert!(
        face.glyph_index('\u{e9}').is_some(),
        "the Latin chunk maps é"
    );
    assert!(
        face.glyph_index('\u{416}').is_none(),
        "the Latin chunk does not map Ж"
    );

    // Text spanning two declared ranges embeds the chunk declaring each scalar,
    // as the SVG selects them, even though the Cyrillic chunk alone maps both.
    let scene = text_scene(6, "A\u{416}");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("multi-range Nunito renders");
    assert!(outcome.report().is_empty());
    let fonts = embedded_pdf_fonts(outcome.artifact().bytes());
    let faces = fonts
        .iter()
        .map(|font| rustybuzz::Face::from_slice(font, 0).expect("embedded font parses"))
        .collect::<Vec<_>>();
    assert_eq!(
        faces.len(),
        2,
        "the Latin and the Cyrillic chunk are embedded"
    );
    assert!(faces.iter().any(|face| {
        face.glyph_index('\u{e9}').is_some() && face.glyph_index('\u{416}').is_none()
    }));
    assert!(faces.iter().any(|face| {
        face.glyph_index('\u{416}').is_some() && face.glyph_index('\u{e9}').is_none()
    }));
}

/// Spec 8.6: PDF embeds each selected official font chunk's reviewed decode
/// byte for byte, the sfnt whose length and digest the revision catalog
/// records for the chunk under the #120 approval. A Latin, a multi-range, and
/// a Xiaolai selection each embed exactly their chunks' reviewed decodes and
/// nothing else.
#[test]
fn pdf_embeds_each_selected_chunks_reviewed_decode_byte_for_byte() {
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let embedded_reviewed_decodes = |font_family: i32, text: &str| {
        let outcome = Renderer::new()
            .render(&text_scene(font_family, text), &request)
            .expect("text renders");
        embedded_pdf_fonts(outcome.artifact().bytes())
            .into_iter()
            .map(|font| {
                reviewed_decode_of(&font).unwrap_or_else(|| {
                    panic!("an embedded FontFile2 is no chunk's reviewed decode")
                })
            })
            .collect::<Vec<_>>()
    };

    // Latin: the one chunk whose reviewed decode is also checked in (#58).
    let [excalifont_latin] = embedded_reviewed_decodes(5, "A")[..] else {
        panic!("Latin Excalifont text embeds one face");
    };
    assert_eq!(
        excalifont_latin.upstream_path,
        "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2"
    );
    assert_eq!(
        excalifont_latin.decoded_sha256,
        excalidraw_render_assets_base::excalifont_basic_latin_sfnt_transformation().output_sha256
    );

    // Multi-range: Nunito's Latin and Cyrillic chunks, each its own decode.
    let nunito = embedded_reviewed_decodes(6, "A\u{416}");
    assert_eq!(
        nunito.len(),
        2,
        "the Latin and the Cyrillic chunk are embedded"
    );
    assert!(nunito.iter().all(|chunk| chunk.family == "Nunito"));
    assert_ne!(nunito[0].identity, nunito[1].identity);
    for (chunk, scalar) in nunito.iter().zip(['A', '\u{416}']) {
        let declared = excalidraw_render_assets_base::font_face_unicode_ranges(
            chunk.family,
            chunk.upstream_path,
        )
        .expect("Nunito chunk declares ranges");
        assert!(
            declared
                .iter()
                .any(|(start, end)| (*start..=*end).contains(&u32::from(scalar))),
            "{} declares {scalar:?}",
            chunk.upstream_path
        );
    }

    // Xiaolai: the CJK subset trigger adds the one Xiaolai chunk declaring U+4E2D
    // beside the Excalifont Latin chunk; both are reviewed decodes.
    let cjk = embedded_reviewed_decodes(5, "A\u{4e2d}");
    assert_eq!(cjk.len(), 2, "Excalifont Latin and one Xiaolai chunk");
    assert_eq!(cjk[0].identity, excalifont_latin.identity);
    assert_eq!(cjk[1].family, "Xiaolai");
    let declared =
        excalidraw_render_assets_base::font_face_unicode_ranges("Xiaolai", cjk[1].upstream_path)
            .expect("Xiaolai chunk declares ranges");
    assert!(declared
        .iter()
        .any(|(start, end)| (*start..=*end).contains(&0x4e2d)));
}

#[test]
fn bidirectional_runs_keep_visual_direction_and_logical_pdf_text() {
    // Spec 8.3: each source line is resolved by UAX #9 into visual runs, laid
    // out left to right in visual order and split by selected face; the text
    // a format carries beside the glyphs stays logical. In an LTR paragraph
    // the number after the Hebrew joins its right-to-left sequence, so the
    // visual order is "abc ", "123", then the Hebrew run with its space.
    let scene = text_scene(9, "abc \u{5d0}\u{5d1}\u{5d2} 123");

    let svg_request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let svg_outcome = Renderer::new()
        .render(&scene, &svg_request)
        .expect("bidirectional SVG renders");
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    let missing = svg_outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.code(), DiagnosticCode::FONT_MISSING_GLYPH);
            match diagnostic.details() {
                Some(ErrorDetails::MissingGlyph { codepoint, .. }) => *codepoint,
                other => panic!("missing-glyph details, got {other:?}"),
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        missing,
        [0x5d0, 0x5d1, 0x5d2],
        "missing scalars are reported in logical order, not visual order"
    );
    assert!(svg.contains("data-source-text=\"abc \u{5d0}\u{5d1}\u{5d2} 123\""));
    let runs = svg_text_runs(svg);
    assert_eq!(
        runs.iter()
            .map(|run| (run.direction.as_str(), run.text.as_str()))
            .collect::<Vec<_>>(),
        [("ltr", "abc "), ("ltr", "123"), ("rtl", "??? ")],
        "one tspan per visual run, in visual order, with the run's logical text"
    );
    assert!(
        runs.iter()
            .all(|run| run.font_family == "ExcalidrawRender-9"),
        "every run names the one face that shaped it"
    );
    // SVG assigns the nth `x` to the nth character of the tspan's content,
    // which is logical, so a right-to-left run's list runs from its rightmost
    // glyph down to its leftmost: the trailing space of "אבג " is the run's
    // visual left edge (#126).
    for run in &runs {
        assert_eq!(
            run.x.len(),
            run.text.chars().count(),
            "one x per logical character"
        );
        if run.direction == "rtl" {
            assert!(
                run.x.windows(2).all(|pair| pair[0] > pair[1]),
                "an rtl run's x list descends in logical order: {:?}",
                run.x
            );
        } else {
            assert!(
                run.x.windows(2).all(|pair| pair[0] < pair[1]),
                "an ltr run's x list ascends in logical order: {:?}",
                run.x
            );
        }
    }
    let extent = |run: &SvgTextRun| {
        let min = run.x.iter().copied().fold(f64::INFINITY, f64::min);
        let max = run.x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (min, max)
    };
    assert!(
        runs.windows(2)
            .all(|pair| extent(&pair[0]).1 < extent(&pair[1]).0),
        "visual runs advance left to right across the line"
    );
    let [_, digits, hebrew] = &runs[..] else {
        panic!("three runs");
    };
    let space_x = *hebrew.x.last().expect("the Hebrew run has a space");
    assert!(
        space_x > digits.x[2] && space_x < hebrew.x[2],
        "the space sits between the digits and the leftmost replacement glyph: {space_x} in {:?} .. {:?}",
        digits.x,
        hebrew.x
    );

    let pdf_request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let first = Renderer::new()
        .render(&scene, &pdf_request)
        .expect("bidirectional PDF renders");
    let pdf = pdf_text(first.artifact().bytes());
    assert!(pdf.contains("/ActualText <FEFF006100620063002005D005D105D20020003100320033>"));
}

#[test]
fn face_runs_of_a_right_to_left_visual_run_follow_in_visual_order() {
    // Spec 8.3: a visual run is split further by the face that shapes its
    // scalars, and glyph runs are laid out left to right in visual order.
    // Excalifont shapes the period of "אב." and Liberation Sans the Hebrew's
    // replacement glyphs, so the one right-to-left visual run is two tspans,
    // and the period, logically last, comes first (#126).
    let scene = text_scene(5, "\u{5d0}\u{5d1}.");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("face-split RTL SVG renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"\u{5d0}\u{5d1}.\""));
    let runs = svg_text_runs(svg);
    assert_eq!(
        runs.iter()
            .map(|run| (
                run.font_family.as_str(),
                run.direction.as_str(),
                run.text.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("Excalifont", "rtl", "."),
            ("ExcalidrawRender-9", "rtl", "??")
        ]
    );
    let [period, hebrew] = &runs[..] else {
        panic!("two runs");
    };
    assert_eq!(period.x, [0.0], "the period is the line's leftmost glyph");
    assert!(
        hebrew.x.iter().all(|x| *x > 0.0),
        "the replacement glyphs follow it: {:?}",
        hebrew.x
    );
}

#[test]
fn a_paragraph_separator_inside_a_source_line_keeps_the_text_after_it() {
    // Spec 8.3: only `\r\n`, `\r`, and `\n` end a source line, so U+2029 stays
    // inside one line as content; UAX #9 splits that line into two paragraphs
    // and both are laid out (#125). Excalifont maps the Latin, nothing maps the
    // separator, so the line is three runs of two faces and the separator is
    // the line's one missing glyph.
    let scene = text_scene(5, "ab\u{2029}cd");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("paragraph-separator SVG renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    let missing = outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.code(), DiagnosticCode::FONT_MISSING_GLYPH);
            match diagnostic.details() {
                Some(ErrorDetails::MissingGlyph { codepoint, .. }) => *codepoint,
                other => panic!("missing-glyph details, got {other:?}"),
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(missing, [0x2029]);
    assert!(svg.contains("data-source-text=\"ab\u{2029}cd\""));
    let runs = svg_text_runs(svg);
    assert_eq!(
        runs.iter()
            .map(|run| (
                run.font_family.as_str(),
                run.direction.as_str(),
                run.text.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("Excalifont", "ltr", "ab"),
            ("ExcalidrawRender-9", "ltr", "?"),
            ("Excalifont", "ltr", "cd"),
        ],
        "both paragraphs of the line are laid out in order around the separator"
    );
    assert!(
        runs.windows(2)
            .all(|pair| pair[0].x.last() < pair[1].x.first()),
        "the runs advance left to right"
    );

    let pdf_request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");
    let pdf_outcome = Renderer::new()
        .render(&scene, &pdf_request)
        .expect("paragraph-separator PDF renders");
    let pdf = pdf_text(pdf_outcome.artifact().bytes());
    assert!(pdf.contains("/ActualText <FEFF00610062202900630064>"));
}

/// One `<tspan>` of a run-structured SVG text element. `font_family` is the
/// first family of the run's list, the face that shapes it; the rest of the
/// list is the consumer fallback chain spec 8.4 fixes per family.
struct SvgTextRun {
    font_family: String,
    direction: String,
    x: Vec<f64>,
    text: String,
}

/// The run tspans of the first `<text>` element in `svg`, in document order.
fn svg_text_runs(svg: &str) -> Vec<SvgTextRun> {
    let (_, text) = svg.split_once("<text ").expect("SVG has a text element");
    let (text, _) = text.split_once("</text>").expect("text element is closed");
    let attribute = |tspan: &str, name: &str| -> String {
        let (_, tail) = tspan
            .split_once(&format!(" {name}=\""))
            .unwrap_or_else(|| panic!("tspan has {name}: {tspan}"));
        tail.split_once('"')
            .expect("attribute is closed")
            .0
            .to_owned()
    };
    text.split("<tspan")
        .skip(1)
        .map(|tspan| {
            let (open, rest) = tspan.split_once('>').expect("tspan opens");
            let (content, _) = rest.split_once("</tspan>").expect("tspan closes");
            SvgTextRun {
                font_family: attribute(open, "font-family")
                    .split(',')
                    .next()
                    .expect("a run names a face")
                    .to_owned(),
                direction: attribute(open, "direction"),
                x: attribute(open, "x")
                    .split(' ')
                    .map(|value| value.parse().expect("x is numeric"))
                    .collect(),
                text: content.to_owned(),
            }
        })
        .collect()
}

#[test]
fn one_family_can_select_multiple_unicode_range_chunks() {
    let scene = SceneDocument::parse(
        "{\"type\":\"excalidraw\",\"version\":2,\"elements\":[\
            {\"id\":\"mixed\",\"type\":\"text\",\"width\":160,\"height\":40,\"fontSize\":20,\"fontFamily\":5,\"text\":\"A\",\"originalText\":\"AЖ\",\"updated\":1},\
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
    assert_eq!(svg.matches("<style class=\"style-fonts\">").count(), 1);
    assert_eq!(svg.matches("font-family:'Excalifont'").count(), 2);
    assert!(!svg.contains("font-style:"));
    assert!(!svg.contains("font-weight:"));
    assert!(svg.contains(&font_family_attribute(EXCALIFONT_FAMILIES)));
    assert!(svg.contains(">A</tspan>"));
    assert!(!svg.contains(">AЖ</tspan>"));
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
    assert!(svg.contains(&font_family_attribute(LIBERATION_FAMILIES)));
}

#[test]
fn embedded_subsets_retain_demanded_mappings_and_drop_unrelated_cmap_entries() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":160,"height":40,"fontSize":20,"fontFamily":5,"text":"A\u0416","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("multi-range scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("subsets render");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    let fonts = embedded_woff2(svg);

    assert_eq!(fonts.len(), 2);
    let faces = fonts
        .iter()
        .map(|font| wuff::decompress_woff2(font).expect("generated WOFF2 decodes"))
        .collect::<Vec<_>>();
    assert!(faces.iter().any(|font| {
        rustybuzz::Face::from_slice(font, 0).is_some_and(|face| face.glyph_index('A').is_some())
    }));
    assert!(faces.iter().any(|font| {
        rustybuzz::Face::from_slice(font, 0).is_some_and(|face| face.glyph_index('Ж').is_some())
    }));
    assert!(faces.iter().all(|font| {
        rustybuzz::Face::from_slice(font, 0).is_some_and(|face| face.glyph_index('Ω').is_none())
    }));
}

#[test]
fn reserved_font_names_are_rewritten_inside_embedded_subsets() {
    let scene = text_scene(1, "Alias");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("reserved-name subset renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    let fonts = embedded_woff2(svg);
    let [font] = fonts.as_slice() else {
        panic!("one Virgil subset is embedded");
    };
    let sfnt = wuff::decompress_woff2(font).expect("generated WOFF2 decodes");
    let face = rustybuzz::Face::from_slice(&sfnt, 0).expect("generated sfnt parses");
    let name = |id| {
        face.names()
            .into_iter()
            .find(|name| name.name_id == id && name.language_id == 0x0409)
            .and_then(|name| name.to_string())
            .unwrap_or_else(|| panic!("name ID {id} is retained"))
    };

    assert_eq!(name(1), "ExcalidrawRender-1");
    assert_eq!(name(2), "Regular");
    assert!(name(3).starts_with("excalidraw-render;ExcalidrawRender-1;"));
    assert!(name(4).starts_with("ExcalidrawRender-1 Regular "));
    assert!(name(6).starts_with("ExcalidrawRender-1-Regular-"));
}

fn text_scene(font_family: i32, text: &str) -> SceneDocument {
    let source = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"text\",\"type\":\"text\",\"width\":160,\"height\":40,\"fontSize\":20,\"fontFamily\":{font_family},\"text\":{},\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}",
        serde_json::to_string(text).expect("test text serializes")
    );
    SceneDocument::parse(source.as_bytes()).expect("text scene is admitted")
}

fn embedded_woff2(svg: &str) -> Vec<Vec<u8>> {
    const PREFIX: &str = "data:font/woff2;base64,";
    let mut fonts = Vec::new();
    let mut remaining = svg;
    while let Some((_, tail)) = remaining.split_once(PREFIX) {
        let (encoded, rest) = tail.split_once(')').expect("font data URL is terminated");
        fonts.push(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .expect("font data URL is valid base64"),
        );
        remaining = rest;
    }
    fonts
}

/// The official font chunk whose reviewed decode the embedded bytes are, by the
/// length and digest the revision catalog records; `None` when they are no
/// chunk's.
fn reviewed_decode_of(font: &[u8]) -> Option<&'static excalidraw_render_assets_base::AssetRecord> {
    use sha2::{Digest as _, Sha256};
    let digest = format!("{:x}", Sha256::digest(font));
    excalidraw_render_assets_base::revision_1()
        .assets
        .iter()
        .find(|asset| asset.decoded_length == font.len() && asset.decoded_sha256 == digest)
}
