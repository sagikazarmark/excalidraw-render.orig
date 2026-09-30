//! Scalars absent from the complete fallback chain shape as the deterministic
//! basic-Latin `?` from Liberation Sans and report `font.missing-glyph`, while
//! every format keeps carrying the stored scalars beside the glyphs (spec 8.3).
//! `focused.missing-glyph.v1` (#124) is the evidence-bearing case; this pins
//! what its PNG and SVG evidence cannot see: the PDF's embedded faces and
//! logical text, the PDF report, and repeatability in every format.

use std::fs;

use crate::common::render::render;
use crate::common::repository_root;
use crate::common::svg::{font_family_attribute, EXCALIFONT_FAMILIES, LIBERATION_FAMILIES};
use excalidraw_render::{OutputFormat, SceneDocument};
use serde_json::Value;

use crate::common::lower_hex;
use crate::common::pdf_inspect::{
    declares_base_font, embedded_pdf_fonts, family_name, font_resource, pdf_actual_text, pdf_text,
    to_unicode_cmap,
};

const CASE: &str = "tests/conformance/cases/focused/missing-glyph";

/// The stored text of both elements: Latin, Linear B U+10000 and U+10001, the
/// noncharacter U+10FFFF, Latin.
const SOURCE_TEXT: &str = "Latin \u{10000}\u{10001} \u{10ffff} text";

/// `SOURCE_TEXT` as PDF `ActualText`: UTF-16BE with a byte-order mark, so the
/// two supplementary-plane scalars are surrogate pairs.
const ACTUAL_TEXT: &str =
    "/ActualText <FEFF004C006100740069006E0020D800DC00D800DC010020DBFFDFFF00200074006500780074>";

fn registered_scene() -> SceneDocument {
    SceneDocument::parse(
        &fs::read(repository_root().join(CASE).join("scene.excalidraw"))
            .expect("registered scene is present"),
    )
    .expect("registered scene is admitted")
}

/// The checked expected report for one evidence-bearing format.
fn checked_report(format: &str) -> Value {
    crate::common::reports::checked_report(
        &repository_root()
            .join(CASE)
            .join(format!("expected-report-{format}.json")),
    )
}

#[test]
fn every_format_reports_the_registered_missing_glyph_diagnostics() {
    let scene = registered_scene();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);

        // PNG and SVG carry evidence and a checked report each. The PDF has
        // neither, so its report is the SVG's six diagnostics under the PDF's
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
        assert_eq!(
            diagnostics.len(),
            6,
            "three scalars on each of two elements"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] == "font.missing-glyph"));
        assert_eq!(
            serde_json::to_value(outcome.report()).expect("report serializes"),
            expected,
            "{format:?} report is the registered expectation"
        );
    }
}

#[test]
fn svg_sets_the_replacement_from_liberation_sans_and_keeps_the_stored_scalars() {
    let outcome = render(&registered_scene(), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    // The stored text travels beside the glyphs in both elements.
    assert_eq!(
        svg.matches(&format!("data-source-text=\"{SOURCE_TEXT}\""))
            .count(),
        2
    );
    // Excalifont shapes the Latin; the three replacements are Liberation Sans
    // runs inside the Excalifont element, `??` then `?` around the space.
    assert!(svg.contains("@font-face{font-family:'Excalifont'"));
    assert!(svg.contains("@font-face{font-family:'ExcalidrawRender-9'"));
    assert!(
        !svg.contains("font-family:'Xiaolai'"),
        "no scalar is in the CJK table, so Xiaolai stays out"
    );
    let excalifont_line = svg
        .split(&format!(
            "{} font-size=\"20\"",
            font_family_attribute(EXCALIFONT_FAMILIES)
        ))
        .nth(1)
        .and_then(|rest| rest.split("</text>").next())
        .expect("the Excalifont element is emitted");
    assert_eq!(
        excalifont_line
            .matches(&format!(
                "<tspan {}",
                font_family_attribute(LIBERATION_FAMILIES)
            ))
            .count(),
        2
    );
    assert!(excalifont_line.contains(">??</tspan>"));
    assert!(excalifont_line.contains(">?</tspan>"));
    assert!(excalifont_line.contains(">Latin </tspan>"));
    assert!(excalifont_line.contains("> text</tspan>"));
    // The Liberation Sans element is one run of its own face.
    let liberation_line = svg
        .split(&format!(
            "{} font-size=\"20\"",
            font_family_attribute(LIBERATION_FAMILIES)
        ))
        .nth(1)
        .and_then(|rest| rest.split("</text>").next())
        .expect("the Liberation Sans element is emitted");
    assert_eq!(liberation_line.matches("<tspan ").count(), 1);
    assert!(liberation_line.contains(">Latin ?? ? text</tspan>"));
}

#[test]
fn pdf_embeds_liberation_sans_for_the_replacement_runs_and_carries_the_stored_scalars() {
    let outcome = render(&registered_scene(), OutputFormat::Pdf);
    let bytes = outcome.artifact().bytes();
    let pdf = pdf_text(bytes);

    // One embedded face per family, and nothing from the fallback chain the
    // scene does not reach.
    assert!(declares_base_font(&pdf, "Excalifont"));
    assert!(declares_base_font(&pdf, "LiberationSans"));
    assert!(!declares_base_font(&pdf, "Xiaolai"));
    assert!(!declares_base_font(&pdf, "NotoEmoji"));

    // The PDF embeds each family's program as a renamed subset (ADR-0021),
    // so what the replacement pins is which program the `?` runs are drawn
    // from: one derived from the manifested Liberation Sans asset, which maps
    // `?`; and no embedded program maps a missing scalar, which is what makes
    // the replacement a replacement.
    let fonts = embedded_pdf_fonts(bytes);
    assert_eq!(fonts.len(), 2, "one FontFile2 per family");
    let liberation_asset =
        excalidraw_fonts::load_asset("fonts/Liberation/LiberationSans-Regular.woff2")
            .expect("the Liberation Sans asset verifies")
            .expect("the Liberation Sans asset is manifested");
    let liberation_program =
        wuff::decompress_woff2(liberation_asset.bytes()).expect("manifested WOFF2 decodes");
    let liberation_digest = {
        use sha2::Digest as _;
        lower_hex(&sha2::Sha256::digest(&liberation_program))
    };
    assert!(
        crate::common::pdf_inspect::pdf_font_program_sources(bytes)
            .iter()
            .any(
                |(identity, digest)| identity.starts_with("fonts/Liberation/")
                    && *digest == liberation_digest
            ),
        "an embedded program is derived from the manifested Liberation Sans asset"
    );
    let faces: Vec<_> = fonts
        .iter()
        .map(|font| ttf_parser::Face::parse(font, 0).expect("embedded sfnt parses"))
        .collect();
    let liberation_face = ttf_parser::Face::parse(&liberation_program, 0)
        .expect("the Liberation Sans program parses");
    assert!(liberation_face.glyph_index('?').is_some());
    assert!(faces.iter().any(|face| family_name(face) == "Excalifont"));
    assert!(faces
        .iter()
        .any(|face| family_name(face) == "ExcalidrawRender-9"));
    // A subset carries no cmap; the ToUnicode CMaps say what each maps.
    for family in ["Excalifont", "LiberationSans"] {
        let cmap = to_unicode_cmap(&pdf, family);
        assert!(cmap.contains("<004C>\n"), "{family} shapes Latin: {cmap}");
        for missing in ["<D800DC00>", "<D800DC01>", "<DBFFDFFF>"] {
            assert!(!cmap.contains(missing), "{family} maps {missing}: {cmap}");
        }
    }
    // Liberation Sans's ToUnicode maps the replacement's CID back to `?`, so
    // extracting the glyph run itself reads `?`; Excalifont's maps nothing to
    // `?`, since the replacement is never drawn from the primary face.
    let liberation_cmap = to_unicode_cmap(&pdf, "/CMapName /LiberationSans-UCS def");
    assert!(
        liberation_cmap.contains("<0001> <003F>"),
        "{liberation_cmap}"
    );
    let excalifont_cmap = to_unicode_cmap(&pdf, "/CMapName /Excalifont-UCS def");
    assert!(!excalifont_cmap.contains("<003F>"), "{excalifont_cmap}");

    // The logical text is the stored text, once per line, in logical order.
    assert_eq!(pdf.matches(ACTUAL_TEXT).count(), 2);
    assert_eq!(
        pdf_actual_text(&pdf),
        format!("{SOURCE_TEXT}{SOURCE_TEXT}"),
        "ActualText round-trips to the two elements' stored text"
    );

    // The Excalifont line switches to the Liberation Sans resource for the
    // `??` and `?` runs, each drawn as the `?` CID, and back to Excalifont
    // for the Latin around them; the Liberation Sans line never leaves its
    // own face.
    let liberation = font_resource(&pdf, "LiberationSans");
    let excalifont = font_resource(&pdf, "Excalifont");
    let lines: Vec<&str> = pdf
        .split(ACTUAL_TEXT)
        .skip(1)
        .map(|rest| rest.split("EMC").next().expect("marked content closes"))
        .collect();
    let [excalifont_line, liberation_line] = lines.as_slice() else {
        panic!("two marked-content lines carry the stored text");
    };
    let replacement_runs: Vec<&str> = excalifont_line
        .split(&format!("{liberation} 20 Tf\n"))
        .skip(1)
        .map(|run| {
            run.split(" Tf\n")
                .next()
                .expect("the run ends at a font switch")
        })
        .collect();
    assert_eq!(replacement_runs.len(), 2, "`??` and `?`");
    assert_eq!(
        crate::common::pdf_inspect::shown_codes(replacement_runs[0]),
        [1, 1]
    );
    assert_eq!(
        crate::common::pdf_inspect::shown_codes(replacement_runs[1]),
        [1]
    );
    assert_eq!(
        excalifont_line
            .matches(&format!("{excalifont} 20 Tf"))
            .count(),
        3
    );
    assert_eq!(
        liberation_line
            .matches(&format!("{liberation} 20 Tf"))
            .count(),
        1
    );
    assert!(!liberation_line.contains(&format!("{excalifont} 20 Tf")));
}
