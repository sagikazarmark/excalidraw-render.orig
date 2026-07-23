use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

mod common;

use common::pdf_harness::{assert_structure, render_viewer};

use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use excalidraw_render_compare::{
    compare, compare_with_limits, CanonicalRgba8, ComparisonLimits, ComparisonMode, VisualClass,
};
use sha2::{Digest, Sha256};

#[test]
fn excalifont_text_renders_semantically_and_deterministically_in_every_format() {
    let scene = SceneDocument::parse(
        br##"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"text",
            "type":"text",
            "x":20,
            "y":30,
            "width":90,
            "height":72,
            "angle":0.125,
            "strokeColor":"#1e66f5",
            "opacity":70,
            "fontSize":20,
            "fontFamily":5,
            "text":"One two three\nFour",
            "textAlign":"center",
            "verticalAlign":"middle",
            "lineHeight":1.2,
            "frameId":null,
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"##,
    )
    .expect("normal Excalifont text is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("default request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("normal Excalifont text renders");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("the same text renders again");

        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert!(first.report().diagnostics().is_empty());
        assert_eq!(
            serde_json::to_vec(first.report()).expect("report serializes"),
            br#"{"schema_version":1,"diagnostics":[]}"#
        );

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
                assert!(svg.contains("@font-face{font-family:'Excalifont'"));
                assert!(svg.contains("data:font/woff2;base64,"));
                assert!(
                    svg.contains("font.excalifont.ofl-1.1")
                        || svg.contains("Excalifont and Virgil provenance is retained together")
                );
                assert!(svg.contains("font-family=\"Excalifont\" font-size=\"20\""));
                assert_eq!(svg.matches("<text data-source-text=").count(), 1);
                assert_eq!(svg.matches("<tspan ").count(), 2);
                let first_text = svg
                    .split("<tspan x=\"")
                    .nth(1)
                    .expect("text line is emitted");
                let shaped_x = first_text
                    .split('"')
                    .next()
                    .expect("text has shaped x positions");
                let shaped_y = first_text
                    .split(" y=\"")
                    .nth(1)
                    .and_then(|value| value.split('"').next())
                    .expect("text has shaped y positions");
                assert_eq!(shaped_x.split_whitespace().count(), 13);
                assert_eq!(shaped_y.split_whitespace().count(), 13);
                assert!(svg.contains(">One two three</tspan>\n<tspan"));
                assert!(svg.contains(">Four</tspan></text>"));
                assert_eq!(svg_source_text(svg), "One two three\nFour");
                assert!(!svg.contains("Unavailable: text"));
                assert_eq!(first.artifact().bytes().len(), 39_267);
                assert_eq!(
                    artifact_sha256(&first),
                    "be2e0b84f280141e1b50525648890fdea7a22d697149cd04120b1d8cb92d5085"
                );
            }
            OutputFormat::Png => {
                let decoder = png::Decoder::new(Cursor::new(first.artifact().bytes()));
                let mut reader = decoder.read_info().expect("PNG structure is valid");
                assert_eq!((reader.info().width, reader.info().height), (118, 102));
                let mut pixels = vec![0; reader.output_buffer_size()];
                let frame = reader.next_frame(&mut pixels).unwrap();
                pixels.truncate(frame.buffer_size());
                let mut count = 0_u64;
                let mut bbox = (u32::MAX, u32::MAX, 0, 0);
                for (index, pixel) in pixels.chunks_exact(4).enumerate() {
                    if pixel != [255, 255, 255, 255] {
                        count += 1;
                        let x = index as u32 % frame.width;
                        let y = index as u32 / frame.width;
                        bbox = (bbox.0.min(x), bbox.1.min(y), bbox.2.max(x), bbox.3.max(y));
                    }
                }
                assert_eq!(count, 942);
                assert_eq!(bbox, (0, 11, 117, 58));
                assert!(pixels.chunks_exact(4).any(|pixel| {
                    pixel[2] > pixel[0] && pixel[2] > pixel[1] && pixel[3] == 255
                }));
                assert_eq!(first.artifact().bytes().len(), 48_314);
                assert_eq!(
                    artifact_sha256(&first),
                    "c36b45c0c7dd23f822c6f81368c45bc81dd2ad9234c24d3a473844701f5e4d29"
                );
            }
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(first.artifact().bytes());
                assert!(pdf.contains("/Subtype /Type0"));
                assert!(pdf.contains("/BaseFont /Excalifont"));
                assert!(pdf.contains("/ToUnicode "));
                assert!(pdf.contains("/FontFile2 "));
                assert!(pdf.contains("/CIDToGIDMap "));
                assert!(!pdf.contains("/CIDToGIDMap /Identity"));
                assert!(pdf.contains("<0001> <004F>"));
                assert!(pdf.contains(
                    "/ActualText <FEFF004F006E0065002000740077006F002000740068007200650065000A>"
                ));
                assert!(pdf.contains("/ActualText <FEFF0046006F00750072>"));
                assert_eq!(pdf_actual_text(&pdf), "One two three\nFour");
                assert!(!pdf.contains("Unavailable: text"));
                assert!(pdf.contains("Excalifont and Virgil provenance is retained together"));
                assert_eq!(first.artifact().bytes().len(), 71_487);
                assert_eq!(
                    artifact_sha256(&first),
                    "5eac7a8a16860311629659f223c52509c5799da1e83c5e48805ee45ed0a32742"
                );
            }
            _ => panic!("the test covers the revision-1 format set"),
        }
    }
}

fn artifact_sha256(outcome: &excalidraw_render::RenderOutcome) -> String {
    format!("{:x}", Sha256::digest(outcome.artifact().bytes()))
}

#[test]
fn approved_excalifont_text_passes_the_direct_png_typography_budget() {
    let case = repository_root().join("tests/conformance/cases/focused/excalifont-text");
    let scene_bytes = fs::read(case.join("scene.excalidraw")).expect("approved scene is present");
    let scene = SceneDocument::parse(&scene_bytes).expect("approved scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("default PNG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("approved text renders directly to PNG");

    assert_eq!(
        serde_json::to_vec(outcome.report()).expect("report serializes"),
        fs::read(case.join("expected-report.json")).expect("approved report is present")
    );
    let candidate = decode_png(outcome.artifact().bytes());
    let reference = CanonicalRgba8::new(
        110,
        92,
        fs::read(case.join("evidence/official-png.rgba"))
            .expect("approved direct-canvas evidence is present"),
    )
    .expect("approved evidence is canonical RGBA8");
    let report = compare(
        &reference,
        &candidate,
        VisualClass::Typography,
        &[],
        ComparisonMode::Canonical,
    )
    .expect("candidate and reference dimensions agree");
    assert!(
        report.passed(),
        "typography budget failed: changed_pixels={}, ssim={:?}",
        report.whole.changed_pixels,
        report.whole.ssim
    );
}

#[test]
#[ignore = "requires the exact-pinned PDFium, PDF.js, and qpdf harnesses"]
fn approved_excalifont_text_passes_both_canonical_pdf_gates() {
    let root = repository_root();
    let case = root.join("tests/conformance/cases/focused/excalifont-text");
    let scene_bytes = fs::read(case.join("scene.excalidraw")).expect("approved scene is present");
    let scene = SceneDocument::parse(&scene_bytes).expect("approved scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("default PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("approved text renders directly to PDF");
    let exception_bytes =
        fs::read(case.join("pdf-visual-exception.json")).expect("PDF exception is present");
    assert_eq!(
        format!("{:x}", Sha256::digest(&exception_bytes)),
        "57906d0d050b20c68990c07417a66574e6685f1b8783fadcff0637e14500a0f3"
    );
    let exception: serde_json::Value =
        serde_json::from_slice(&exception_bytes).expect("PDF exception is valid JSON");
    assert_eq!(exception["approved_issue"], 73);
    assert_eq!(exception["viewer_profile"], "canonical-pdf.pdfium.v1");
    let reference = CanonicalRgba8::new(
        110,
        92,
        fs::read(case.join("evidence/official-svg.rgba"))
            .expect("approved official-SVG evidence is present"),
    )
    .expect("approved evidence is canonical RGBA8");
    let stem = format!("excalifont-pdf-candidate-{}", std::process::id());
    let candidate = format!("target/{stem}.pdf");
    fs::write(root.join(&candidate), outcome.artifact().bytes())
        .expect("write renderer-owned candidate PDF under target");

    for (viewer, script, mode) in [
        (
            "pdfjs",
            "tools/pdf-harness/run-pdfjs.sh",
            ComparisonMode::PdfJs,
        ),
        (
            "pdfium",
            "tools/pdf-harness/run-pdfium.sh",
            ComparisonMode::Canonical,
        ),
    ] {
        let output = format!("target/{stem}-{viewer}.rgba");
        let candidate = CanonicalRgba8::new(
            110,
            92,
            render_viewer(&root, viewer, script, &candidate, &output, 110, 92),
        )
        .expect("canonical PDF viewer output is valid");
        let report = if viewer == "pdfium" {
            let minimum_bits =
                u64::from_str_radix(exception["minimum_ssim_bits"].as_str().unwrap(), 16)
                    .expect("PDFium exception contains valid f64 bits");
            let limits = ComparisonLimits::tolerance_exception(
                exception["max_changed_pixels"].as_u64().unwrap(),
                f64::from_bits(minimum_bits),
            )
            .expect("PDFium exception limits are finite");
            compare_with_limits(&reference, &candidate, limits, &[], mode)
        } else {
            compare(&reference, &candidate, VisualClass::Typography, &[], mode)
        }
        .expect("candidate and reference dimensions agree");
        assert!(
            report.passed(),
            "{viewer} typography budget failed: changed_pixels={}, ssim={:?}",
            report.whole.changed_pixels,
            report.whole.ssim
        );
    }

    let structure_output = format!("target/{stem}-structure.json");
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(
                fs::read(case.join("expected-pdf-structure.json"))
                    .expect("PDF structure expectation is present")
            )
        ),
        "0fb4ce18336d021a5f3b5a7c1c5436f5ce8a2aef98269cf2f5d341f283fdc82e"
    );
    assert_structure(
        &root,
        &candidate,
        "tests/conformance/cases/focused/excalifont-text/expected-pdf-structure.json",
        &structure_output,
    );
}

#[test]
fn excalifont_text_limits_recover_at_exact_scalar_and_shaping_work_boundaries() {
    let scene = focused_scene();
    let accepted_limits = ResourceLimits::builder()
        .max_text_scalars_per_element(18)
        .max_total_text_scalars(18)
        .build()
        .expect("text scalar equality is valid");
    let accepted = render_svg_with_limits(&scene, accepted_limits);
    assert!(accepted.report().is_empty());
    assert!(std::str::from_utf8(accepted.artifact().bytes())
        .expect("SVG is UTF-8")
        .contains("font-family=\"Excalifont\""));

    let crossing_limits = ResourceLimits::builder()
        .max_text_scalars_per_element(17)
        .build()
        .expect("text scalar crossing profile is valid");
    let crossing = render_svg_with_limits(&scene, crossing_limits);
    let diagnostic = crossing
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
        })
        .expect("the local text crossing diagnostic is retained");
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
    );
    assert_eq!(diagnostic.element_id(), Some("text"));
    assert_eq!(diagnostic.json_pointer(), "/elements/0");
    assert_eq!(
        serde_json::to_value(diagnostic).expect("diagnostic serializes"),
        serde_json::json!({
            "code":"resource.element-complexity-replaced",
            "severity":"warning",
            "format":"svg",
            "effects":["visual"],
            "element_id":"text",
            "json_pointer":"/elements/0",
            "message":"text was omitted because it exceeds its scalar limit",
            "details":{
                "kind":"resource_limit",
                "field":"max_text_scalars_per_element",
                "configured":17,
                "measurement":{"kind":"attempted","value":18},
                "value_kind":"count",
                "recovery":"placeholder"
            }
        })
    );
    let crossing_svg = std::str::from_utf8(crossing.artifact().bytes()).expect("SVG is UTF-8");
    assert!(crossing_svg.contains("fill=\"#f1f3f5\""));
    assert!(!crossing_svg.contains("font-family=\"Excalifont\""));
    assert!(!crossing_svg.contains("@font-face{font-family:'Excalifont'"));

    let aggregate_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_text_scalars_per_element(17)
                .max_total_text_scalars(17)
                .build()
                .expect("aggregate text crossing limits are valid"),
        )
        .build()
        .expect("aggregate text crossing request is valid");
    let aggregate = Renderer::new()
        .render(&scene, &aggregate_request)
        .expect_err("aggregate text crossing is fatal before shaping");
    assert_eq!(
        aggregate.code(),
        excalidraw_render::ErrorCode::RESOURCE_TOTAL_TEXT_EXCEEDED
    );
    assert!(aggregate.report().is_empty());

    let work_equality = render_svg_with_limits(
        &scene,
        ResourceLimits::builder()
            .max_element_work_units(24_128)
            .build()
            .expect("work equality profile is valid"),
    );
    assert!(work_equality.report().is_empty());
    assert_eq!(
        artifact_sha256(&work_equality),
        "be2e0b84f280141e1b50525648890fdea7a22d697149cd04120b1d8cb92d5085"
    );
    let work_crossing = render_svg_with_limits(
        &scene,
        ResourceLimits::builder()
            .max_element_work_units(24_127)
            .build()
            .expect("work crossing profile is valid"),
    );
    let diagnostic = work_crossing
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED)
        .expect("the shaping-work diagnostic is retained");
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
    );
    let descriptor = serde_json::to_value(diagnostic).expect("work diagnostic serializes");
    assert_eq!(descriptor["details"]["configured"], 24_127);
    assert_eq!(descriptor["details"]["measurement"]["value"], 24_128);
    assert_eq!(descriptor["details"]["value_kind"], "work_units");
}

#[test]
fn unsupported_excalifont_chunk_text_is_diagnosed_and_never_host_falls_back() {
    let scene = text_scene("漢漢A", 60.0, 30.0);
    let outcome = render_svg_with_limits(&scene, ResourceLimits::default());
    assert_eq!(outcome.report().diagnostics().len(), 2);
    for diagnostic in outcome.report().diagnostics() {
        assert_eq!(diagnostic.code(), DiagnosticCode::FONT_MISSING_GLYPH);
        assert_eq!(diagnostic.element_id(), Some("text"));
        assert_eq!(diagnostic.json_pointer(), "/elements/0");
    }
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("data-source-text=\"漢漢A\""));
    assert!(svg.contains(">??A</tspan>"));
    assert!(!svg.contains(">漢漢A</tspan>"));
    assert!(svg.contains("@font-face{font-family:'Excalifont'"));

    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("default PDF request is valid");
    let pdf = Renderer::new()
        .render(&scene, &request)
        .expect("replacement glyph remains meaningful PDF text");
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("/ActualText <FEFF6F226F220041>"));
    assert!(pdf.contains("<003F>"));
}

#[test]
fn basic_latin_svg_positions_every_character_when_ligatures_would_normally_form() {
    let scene = text_scene("office", 100.0, 30.0);
    let outcome = render_svg_with_limits(&scene, ResourceLimits::default());
    assert!(outcome.report().is_empty());
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    let first_text = svg.split("<tspan x=\"").nth(1).expect("text is emitted");
    let shaped_x = first_text.split('"').next().expect("x positions terminate");
    assert_eq!(shaped_x.split_whitespace().count(), 6);
    assert!(svg.contains(">office</tspan>"));
    assert_eq!(svg_source_text(svg), "office");
}

#[test]
fn png_text_coverage_uses_the_transformed_glyph_rectangle() {
    let scene = focused_scene();
    let equality_request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(
            ResourceLimits::builder()
                .max_element_work_units(114_760)
                .build()
                .expect("PNG coverage equality limits are valid"),
        )
        .build()
        .expect("PNG coverage equality request is valid");
    let equality = Renderer::new()
        .render(&scene, &equality_request)
        .expect("PNG glyph coverage equality is accepted");
    assert!(equality.report().is_empty());
    assert_eq!(
        artifact_sha256(&equality),
        "c36b45c0c7dd23f822c6f81368c45bc81dd2ad9234c24d3a473844701f5e4d29"
    );

    let crossing_request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(
            ResourceLimits::builder()
                .max_element_work_units(114_759)
                .build()
                .expect("PNG coverage crossing limits are valid"),
        )
        .build()
        .expect("PNG coverage crossing request is valid");
    let crossing = Renderer::new()
        .render(&scene, &crossing_request)
        .expect("PNG coverage crossing recovers locally");
    let [diagnostic] = crossing.report().diagnostics() else {
        panic!("PNG glyph coverage crossing has one diagnostic");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
    );
    let descriptor = serde_json::to_value(diagnostic).expect("coverage diagnostic serializes");
    assert_eq!(descriptor["details"]["configured"], 114_759);
    let attempted = descriptor["details"]["measurement"]["value"]
        .as_u64()
        .expect("coverage attempt is an integer");
    assert_eq!(attempted, 114_760);
    assert!(
        attempted < 200_000,
        "coverage must not charge the 118x102 full surface"
    );
}

#[test]
fn pdf_maps_the_same_glyph_id_to_distinct_unicode_character_codes() {
    let scene = text_scene(" \u{a0}", 40.0, 30.0);
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("default PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("space mappings render");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
    assert!(pdf.contains("<0001> <0020>"));
    assert!(pdf.contains("<0002> <00A0>"));
    assert!(pdf.contains("<0001> Tj"));
    assert!(pdf.contains("<0002> Tj"));
    assert!(!pdf.contains("/CIDToGIDMap /Identity"));
    let cid_to_gid = pdf_stream_referenced_after(outcome.artifact().bytes(), "/CIDToGIDMap ");
    assert_eq!(&cid_to_gid[2..4], &cid_to_gid[4..6]);
    assert_ne!(&cid_to_gid[2..4], &[0, 0]);
}

fn focused_scene() -> SceneDocument {
    SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","x":20,"y":30,"width":90,"height":72,"angle":0.125,"strokeColor":"#1e66f5","opacity":70,"fontSize":20,"fontFamily":5,"text":"One two three\nFour","textAlign":"center","verticalAlign":"middle","lineHeight":1.2,"frameId":null,"updated":1}],"appState":{},"files":{}}"##,
    )
    .expect("focused text scene is admitted")
}

fn text_scene(text: &str, width: f64, height: f64) -> SceneDocument {
    let source = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"text\",\"type\":\"text\",\"width\":{width},\"height\":{height},\"fontSize\":20,\"fontFamily\":5,\"text\":{},\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}",
        serde_json::to_string(text).expect("test text serializes")
    );
    SceneDocument::parse(source.as_bytes()).expect("text scene is admitted")
}

fn render_svg_with_limits(
    scene: &SceneDocument,
    limits: ResourceLimits,
) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("text recovery retains a meaningful SVG")
}

fn decode_png(bytes: &[u8]) -> CanonicalRgba8 {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).expect("PNG pixels decode");
    pixels.truncate(frame.buffer_size());
    CanonicalRgba8::new(frame.width, frame.height, pixels).expect("PNG is canonical RGBA8")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}

fn pdf_stream_referenced_after<'a>(pdf: &'a [u8], key: &str) -> &'a [u8] {
    let text = String::from_utf8_lossy(pdf);
    let object = text
        .split(key)
        .nth(1)
        .and_then(|tail| tail.split_whitespace().next())
        .expect("PDF reference has an object number");
    let marker = format!("{object} 0 obj\n");
    let object_start = pdf
        .windows(marker.len())
        .position(|window| window == marker.as_bytes())
        .expect("referenced PDF object exists");
    let stream_marker = b"stream\n";
    let stream_start = pdf[object_start..]
        .windows(stream_marker.len())
        .position(|window| window == stream_marker)
        .map(|offset| object_start + offset + stream_marker.len())
        .expect("referenced object has a stream");
    let stream_end = pdf[stream_start..]
        .windows(b"\nendstream".len())
        .position(|window| window == b"\nendstream")
        .map(|offset| stream_start + offset)
        .expect("referenced stream terminates");
    &pdf[stream_start..stream_end]
}

fn svg_source_text(svg: &str) -> String {
    let mut source = String::new();
    for tail in svg.split("data-source-text=\"").skip(1) {
        let value = tail
            .split('"')
            .next()
            .expect("source annotation terminates");
        source.push_str(
            &value
                .replace("&#10;", "\n")
                .replace("&#13;", "\r")
                .replace("&#9;", "\t")
                .replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&"),
        );
    }
    source
}

fn pdf_actual_text(pdf: &str) -> String {
    let mut extracted = String::new();
    for tail in pdf.split("/ActualText <FEFF").skip(1) {
        let hex = tail.split('>').next().expect("ActualText hex terminates");
        let units = hex
            .as_bytes()
            .chunks_exact(4)
            .map(|digits| {
                u16::from_str_radix(std::str::from_utf8(digits).expect("hex is ASCII"), 16)
                    .expect("ActualText is hexadecimal")
            })
            .collect::<Vec<_>>();
        extracted.push_str(&String::from_utf16(&units).expect("ActualText is UTF-16"));
    }
    extracted
}
