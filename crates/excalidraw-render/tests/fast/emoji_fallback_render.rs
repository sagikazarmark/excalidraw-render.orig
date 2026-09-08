//! Emoji shape from the complete bundled Noto Emoji face and report
//! `font.noto-emoji-substituted` once per affected occurrence, while every
//! format keeps carrying the stored text beside the glyphs (spec 8.3).
//! `focused.emoji-fallback.v1` (#121) is the evidence-bearing case; this pins
//! what its PNG and SVG evidence cannot see: that the embedded face is the
//! manifested Noto Emoji 3.003 asset byte for byte in both vector formats, the
//! PDF's resource switches and logical text, the PDF report, and repeatability
//! in every format.

use std::fs;

use crate::common::repository_root;
use excalidraw_render::{OutputFormat, RenderOutcome, RenderRequest, Renderer, SceneDocument};
use serde_json::Value;

use crate::common::pdf_inspect::{
    embedded_pdf_fonts, family_name, font_resource, marked_content_line, pdf_actual_text,
    to_unicode_cmap,
};

const CASE: &str = "tests/conformance/cases/focused/emoji-fallback";

/// The stored text of the Excalifont element: two emoji in one occurrence.
const EXCALIFONT_TEXT: &str = "Latin \u{1f600}\u{1f389} text";
/// The stored text of the Liberation Sans element: one emoji.
const LIBERATION_TEXT: &str = "Latin \u{1f600} text";

/// The two stored texts as PDF `ActualText`: UTF-16BE with a byte-order mark,
/// so each emoji is a surrogate pair.
const EXCALIFONT_ACTUAL_TEXT: &str =
    "/ActualText <FEFF004C006100740069006E0020D83DDE00D83CDF8900200074006500780074>";
const LIBERATION_ACTUAL_TEXT: &str =
    "/ActualText <FEFF004C006100740069006E0020D83DDE0000200074006500780074>";

fn registered_scene() -> SceneDocument {
    SceneDocument::parse(
        &fs::read(repository_root().join(CASE).join("scene.excalidraw"))
            .expect("registered scene is present"),
    )
    .expect("registered scene is admitted")
}

fn render(scene: &SceneDocument, format: OutputFormat) -> RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("default request is valid");
    Renderer::new()
        .render(scene, &request)
        .unwrap_or_else(|error| panic!("{format:?} renders: {error:?}"))
}

/// The checked expected report for one evidence-bearing format.
fn checked_report(format: &str) -> Value {
    serde_json::from_slice(
        &fs::read(
            repository_root()
                .join(CASE)
                .join(format!("expected-report-{format}.json")),
        )
        .expect("checked report is present"),
    )
    .expect("checked report is valid JSON")
}

/// The manifested Noto Emoji 3.003 face: the bytes both vector formats must
/// embed unchanged, since a non-WOFF2 fallback is never subset (spec 8.3.1).
fn manifested_noto_emoji() -> Vec<u8> {
    excalidraw_render_assets_base::load_noto_emoji_sfnt()
        .expect("the Noto Emoji asset verifies")
        .bytes()
        .to_vec()
}

#[test]
fn every_format_reports_one_noto_emoji_substitution_per_occurrence_and_repeats_byte_for_byte() {
    let scene = registered_scene();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let first = render(&scene, format);
        let second = render(&scene, format);
        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{format:?} renders byte-identically"
        );
        assert_eq!(first.report(), second.report());

        // PNG and SVG carry evidence and a checked report each. The PDF has
        // neither, so its report is the SVG's two diagnostics under the PDF's
        // format tag, the one field a diagnostic varies by format.
        let expected = match format {
            OutputFormat::Svg => checked_report("svg"),
            OutputFormat::Png => checked_report("png"),
            OutputFormat::Pdf => {
                let mut expected = checked_report("svg");
                let tag = serde_json::to_value(format).expect("format serializes");
                for diagnostic in expected["diagnostics"]
                    .as_array_mut()
                    .expect("checked report lists diagnostics")
                {
                    diagnostic["format"] = tag.clone();
                }
                expected
            }
            _ => panic!("the test covers the revision-1 format set"),
        };
        let diagnostics = expected["diagnostics"]
            .as_array()
            .expect("checked report lists diagnostics");
        // Spec 8.3 counts this substitution per affected occurrence, unlike
        // `font.missing-glyph`, which counts per scalar: the Excalifont element
        // carries two emoji and still reports once.
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic["element_id"].as_str())
                .collect::<Vec<_>>(),
            [Some("excalifont-emoji"), Some("liberation-emoji")],
            "one diagnostic per element, in source order"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] == "font.noto-emoji-substituted"));
        assert_eq!(
            serde_json::to_value(first.report()).expect("report serializes"),
            expected,
            "{format:?} report is the registered expectation"
        );
    }
}

#[test]
fn svg_sets_the_emoji_from_the_complete_noto_emoji_face_and_keeps_the_stored_text() {
    let outcome = render(&registered_scene(), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    // The stored text travels beside the glyphs in both elements.
    assert!(svg.contains(&format!("data-source-text=\"{EXCALIFONT_TEXT}\"")));
    assert!(svg.contains(&format!("data-source-text=\"{LIBERATION_TEXT}\"")));

    // Three faces: the two families' subsets and the complete Noto Emoji
    // fallback, declared once. Nothing from the chain the scene does not reach.
    assert!(svg.contains("@font-face{font-family:'Excalifont'"));
    assert!(svg.contains("@font-face{font-family:'ExcalidrawRender-9'"));
    assert_eq!(
        svg.matches("@font-face{font-family:'Noto Emoji'").count(),
        1
    );
    assert!(
        !svg.contains("Xiaolai"),
        "no scalar is in the CJK table, so Xiaolai stays out"
    );
    // The fallback is not a WOFF2 subset like the families' payloads: it is
    // the manifested sfnt, complete and byte for byte.
    let (media_type, payload) = embedded_font_payload(svg, "Noto Emoji");
    assert_eq!(media_type, "font/ttf");
    assert_eq!(payload, manifested_noto_emoji());
    assert_eq!(embedded_font_payload(svg, "Excalifont").0, "font/woff2");

    // The Excalifont element shapes the Latin itself and the two emoji as one
    // Noto Emoji run between; the Liberation Sans element likewise with one.
    let excalifont_line = svg
        .split("font-family=\"Excalifont\" font-size=\"20\"")
        .nth(1)
        .and_then(|rest| rest.split("</text>").next())
        .expect("the Excalifont element is emitted");
    assert_eq!(
        tspans(excalifont_line),
        [
            ("Excalifont", "Latin "),
            ("Noto Emoji", "\u{1f600}\u{1f389}"),
            ("Excalifont", " text"),
        ]
    );
    let liberation_line = svg
        .split("font-family=\"ExcalidrawRender-9\" font-size=\"20\"")
        .nth(1)
        .and_then(|rest| rest.split("</text>").next())
        .expect("the Liberation Sans element is emitted");
    assert_eq!(
        tspans(liberation_line),
        [
            ("ExcalidrawRender-9", "Latin "),
            ("Noto Emoji", "\u{1f600}"),
            ("ExcalidrawRender-9", " text"),
        ]
    );
}

#[test]
fn pdf_embeds_the_complete_noto_emoji_face_for_the_emoji_runs_and_carries_the_stored_text() {
    let outcome = render(&registered_scene(), OutputFormat::Pdf);
    let bytes = outcome.artifact().bytes();
    let pdf = String::from_utf8_lossy(bytes);

    // One embedded face per family plus the fallback, and nothing from the
    // chain the scene does not reach.
    assert!(pdf.contains("/Subtype /Type0\n/BaseFont /Excalifont\n"));
    assert!(pdf.contains("/Subtype /Type0\n/BaseFont /NotoEmoji\n"));
    assert!(pdf.contains("/Subtype /Type0\n/BaseFont /LiberationSans\n"));
    assert!(!pdf.contains("/BaseFont /Xiaolai"));

    // Spec 8.6: the complete Noto Emoji fallback is embedded as its exact
    // manifested bytes, which have no decode. It is the only embedded program
    // that maps either emoji, which is what makes the substitution one.
    let fonts = embedded_pdf_fonts(bytes);
    assert_eq!(fonts.len(), 3, "one FontFile2 per family and one fallback");
    let noto_emoji = manifested_noto_emoji();
    assert!(
        fonts.contains(&noto_emoji),
        "the embedded Noto Emoji program is the manifested asset"
    );
    let noto_face = rustybuzz::Face::from_slice(&noto_emoji, 0).expect("Noto Emoji parses");
    for scalar in ['\u{1f600}', '\u{1f389}'] {
        assert!(noto_face.glyph_index(scalar).is_some());
    }
    for font in fonts.iter().filter(|font| *font != &noto_emoji) {
        let face = rustybuzz::Face::from_slice(font, 0).expect("embedded sfnt parses");
        assert!(
            face.glyph_index('L').is_some(),
            "both family faces shape Latin"
        );
        for scalar in ['\u{1f600}', '\u{1f389}'] {
            assert!(
                face.glyph_index(scalar).is_none(),
                "U+{:04X} is absent from the {} program",
                u32::from(scalar),
                family_name(&face)
            );
        }
    }
    // Noto Emoji's ToUnicode maps its two CIDs back to the emoji as surrogate
    // pairs, so extracting the glyph runs reads the emoji; neither family's
    // CMap names an emoji, since none is ever drawn from a family face.
    let noto_cmap = to_unicode_cmap(&pdf, "/CMapName /NotoEmoji-UCS def");
    assert!(noto_cmap.contains("<0001> <D83DDE00>"), "{noto_cmap}");
    assert!(noto_cmap.contains("<0002> <D83CDF89>"), "{noto_cmap}");
    for family in ["Excalifont", "LiberationSans"] {
        let cmap = to_unicode_cmap(&pdf, &format!("/CMapName /{family}-UCS def"));
        assert!(!cmap.contains("<D83D"), "{family}: {cmap}");
        assert!(!cmap.contains("<D83C"), "{family}: {cmap}");
    }

    // The logical text is the stored text, once per line, in logical order.
    assert_eq!(pdf.matches(EXCALIFONT_ACTUAL_TEXT).count(), 1);
    assert_eq!(pdf.matches(LIBERATION_ACTUAL_TEXT).count(), 1);
    assert_eq!(
        pdf_actual_text(&pdf),
        format!("{EXCALIFONT_TEXT}{LIBERATION_TEXT}"),
        "ActualText round-trips to the two elements' stored text"
    );

    // Each line switches to the Noto Emoji resource for exactly one run, two
    // CIDs on the Excalifont line and one on the Liberation Sans line, and
    // back to its own face for the Latin around it.
    let noto = font_resource(&pdf, "/BaseFont /NotoEmoji");
    let excalifont = font_resource(&pdf, "/BaseFont /Excalifont");
    let liberation = font_resource(&pdf, "/BaseFont /LiberationSans");
    let excalifont_line = marked_content_line(&pdf, EXCALIFONT_ACTUAL_TEXT);
    let liberation_line = marked_content_line(&pdf, LIBERATION_ACTUAL_TEXT);
    let emoji_runs = |line: &str| -> Vec<usize> {
        line.split(&format!("{noto} 20 Tf\n"))
            .skip(1)
            .map(|run| {
                run.split(" Tf\n")
                    .next()
                    .expect("the run ends at a font switch")
                    .matches(" Tj")
                    .count()
            })
            .collect()
    };
    assert_eq!(
        emoji_runs(excalifont_line),
        [2],
        "`😀🎉` is one Noto Emoji run"
    );
    assert_eq!(
        emoji_runs(liberation_line),
        [1],
        "`😀` is one Noto Emoji run"
    );
    assert_eq!(
        excalifont_line
            .matches(&format!("{excalifont} 20 Tf"))
            .count(),
        2
    );
    assert!(!excalifont_line.contains(&format!("{liberation} 20 Tf")));
    assert_eq!(
        liberation_line
            .matches(&format!("{liberation} 20 Tf"))
            .count(),
        2
    );
    assert!(!liberation_line.contains(&format!("{excalifont} 20 Tf")));
}

/// The `(font-family, text)` of every `<tspan>` in one `<text>` element body.
fn tspans(line: &str) -> Vec<(&str, &str)> {
    line.split("<tspan font-family=\"")
        .skip(1)
        .map(|rest| {
            let (family, rest) = rest.split_once('"').expect("tspan family closes");
            let text = rest
                .split_once('>')
                .and_then(|(_, body)| body.split("</tspan>").next())
                .expect("tspan body closes");
            (family, text)
        })
        .collect()
}

/// The media type and decoded payload of the `@font-face` declaring `family`.
fn embedded_font_payload(svg: &str, family: &str) -> (String, Vec<u8>) {
    use base64::Engine as _;
    let block = svg
        .split(&format!("@font-face{{font-family:'{family}';src:url(data:"))
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .unwrap_or_else(|| panic!("the SVG embeds {family}"));
    let (media_type, encoded) = block
        .split_once(";base64,")
        .expect("font src is a base64 data URL");
    (
        media_type.to_owned(),
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("font data URL is valid base64"),
    )
}
