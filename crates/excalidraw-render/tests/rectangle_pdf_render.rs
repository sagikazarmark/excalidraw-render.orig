use std::fs;
use std::path::{Path, PathBuf};

mod common;

use common::pdf_harness::{assert_structure, render_viewer};

use excalidraw_render::{
    ErrorCode, OutputFormat, PdfOptions, PdfRasterFallback, RenderErrorCategory, RenderOutcome,
    RenderRequest, Renderer, ResourceLimits, SceneDocument, Sizing,
};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass,
};

const EXPECTED_ROUGH_PATH: &str = concat!(
    "0 0 m\n60.36 0.51 117.93 -1.29 180 0 c\n",
    "0 0 m\n46.23 0.2 91.7 -0.37 180 0 c\n",
    "180 0 m\n180.82 21.75 178.03 45.56 180 100 c\n",
    "180 0 m\n178.91 21.23 180.01 41.46 180 100 c\n",
    "180 100 m\n116.02 98.39 52.73 98.26 0 100 c\n",
    "180 100 m\n124.78 101.1 71.18 100.93 0 100 c\n",
    "0 100 m\n0.8 76.02 2.55 50.08 0 0 c\n",
    "0 100 m\n-0.56 64.79 -0.37 31.41 0 0 c\n",
);

#[test]
fn approved_rectangle_renders_as_a_deterministic_vector_first_pdf() {
    let case = repository_root().join("tests/conformance/cases/focused/rectangle-outline");
    let first = render_approved_pdf();
    let second = render_approved_pdf();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert_eq!(first.artifact().format(), OutputFormat::Pdf);
    assert_eq!(first.artifact().media_type(), "application/pdf");
    assert_eq!(
        serde_json::to_vec(first.report()).expect("report serializes"),
        fs::read(case.join("expected-report-svg.json")).unwrap()
    );

    let bytes = first.artifact().bytes();
    assert!(bytes.starts_with(b"%PDF-1.7\n"));
    assert!(bytes.ends_with(b"%%EOF"));
    let pdf = String::from_utf8_lossy(bytes);
    assert_eq!(pdf.matches("/Type /Page\n").count(), 1);
    assert!(pdf.contains("/MediaBox [0 0 150 90]"));
    assert!(pdf.contains("/CropBox [0 0 150 90]"));
    assert!(pdf.contains("/OutputIntents [6 0 R]"));
    assert!(pdf.contains("/ColorSpace << /CS1 [/ICCBased 5 0 R] >>"));
    assert!(pdf.contains(concat!(
        "/Type /OutputIntent\n/S /GTS_PDFA1\n",
        "/OutputConditionIdentifier (sRGB)\n",
        "/Info (excalidraw-render project sRGB)\n",
        "/DestOutputProfile 5 0 R\n"
    )));

    let content = stream_data(bytes, b"4 0 obj");
    let content = std::str::from_utf8(content).expect("content stream is canonical ASCII");
    assert!(
        content.starts_with(concat!(
            "q\n0.75 0 0 -0.75 0 90 cm\n",
            "/CS1 cs\n1 1 1 scn\n0 0 200 120 re\nf\n",
            "q\n1 0 0 1 10 10 cm\n2 w\n1 J\n",
            "/CS1 CS\n0.11764705882352941 0.11764705882352941 0.11764705882352941 SCN\n"
        )),
        "unexpected content stream: {content:?}"
    );
    assert!(content.contains(EXPECTED_ROUGH_PATH));
    assert!(content.ends_with("S\nQ\nQ"));
    assert_eq!(
        content.lines().filter(|line| line.ends_with(" m")).count(),
        8
    );
    assert_eq!(
        content.lines().filter(|line| line.ends_with(" c")).count(),
        8
    );
    assert_eq!(content.lines().filter(|line| *line == "S").count(), 1);

    let profile = stream_data(bytes, b"5 0 obj");
    assert_eq!(profile.len(), 820);
    assert_eq!(&profile[8..12], b"\x02\x10\0\0");
    assert_eq!(&profile[12..24], b"mntrRGB XYZ ");
    assert_eq!(&profile[36..40], b"acsp");
    assert_eq!(
        &["rXYZ", "gXYZ", "bXYZ", "wtpt", "rTRC", "gTRC", "bTRC"],
        icc_tags(profile).as_slice()
    );

    for forbidden in [
        "/Subtype /Image",
        "/BleedBox",
        "/TrimBox",
        "/ArtBox",
        "/Rotate",
        "/UserUnit",
        "/ID ",
        "/Encrypt",
        "/Linearized",
        "/Filter",
    ] {
        assert!(
            !pdf.contains(forbidden),
            "PDF contains forbidden {forbidden}"
        );
    }
    assert!(!bytes.windows(4).any(|window| window == b"<svg"));
}

#[test]
fn pdf_page_points_are_checked_at_equality_and_first_crossing() {
    for configured in [150.0, 151.0] {
        let limits = ResourceLimits::builder()
            .max_pdf_page_points(configured)
            .build()
            .expect("PDF page passing limit is valid");
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .resource_limits(limits)
            .build()
            .expect("resource-limited PDF request is valid");
        Renderer::new()
            .render(&approved_scene(), &request)
            .expect("equality and spare PDF page capacity succeed");
    }

    let crossing = ResourceLimits::builder()
        .max_pdf_page_points(149.0)
        .build()
        .expect("PDF page crossing limit is valid");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(crossing)
        .build()
        .expect("resource-limited PDF request is valid");
    let error = Renderer::new()
        .render(&approved_scene(), &request)
        .expect_err("the 150-point width crosses a 149-point limit");

    assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED
    );
    assert_eq!(error.json_pointer(), "");
    assert!(error.report().is_empty());
    let descriptor = serde_json::to_value(error.descriptor()).expect("error serializes");
    assert_eq!(descriptor["details"]["field"], "max_pdf_page_points");
    assert_eq!(descriptor["details"]["configured"], 149.0);
    assert_eq!(descriptor["details"]["measurement"]["kind"], "attempted");
    assert_eq!(descriptor["details"]["measurement"]["value"], 150.0);

    let scale = f64::from_bits(0x4057_ffff_ffff_ffff);
    let exact_page_width = 200.0 * scale * 0.75;
    let limits = ResourceLimits::builder()
        .max_pdf_page_points(exact_page_width)
        .build()
        .expect("rounded PDF page equality limit is valid");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .sizing(Sizing::Scale(scale))
        .resource_limits(limits)
        .build()
        .expect("rounded PDF page request is valid");
    Renderer::new()
        .render(&approved_scene(), &request)
        .expect("validation and encoding use identical sizing arithmetic");

    let request = RenderRequest::builder(OutputFormat::Pdf)
        .sizing(Sizing::Scale(f64::MAX))
        .build()
        .expect("huge finite PDF scale is a valid request");
    let error = Renderer::new()
        .render(&approved_scene(), &request)
        .expect_err("overflowing page dimensions cross the point limit");
    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED
    );
    let descriptor = serde_json::to_value(error.descriptor()).expect("error serializes");
    assert_eq!(descriptor["details"]["measurement"]["kind"], "at_least");
    assert_eq!(descriptor["details"]["measurement"]["value"], 14_401.0);
}

#[test]
fn pdf_sizing_and_background_follow_the_logical_pixel_mapping() {
    let scaled = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .sizing(Sizing::Scale(2.0))
            .build()
            .expect("scaled PDF request is valid"),
    );
    let scaled_pdf = String::from_utf8_lossy(scaled.artifact().bytes());
    assert!(scaled_pdf.contains("/MediaBox [0 0 300 180]"));
    assert!(scaled_pdf.contains("/CropBox [0 0 300 180]"));
    let scaled_content = std::str::from_utf8(stream_data(scaled.artifact().bytes(), b"4 0 obj"))
        .expect("scaled PDF content is ASCII");
    assert!(scaled_content.starts_with("q\n1.5 0 0 -1.5 0 180 cm\n"));
    assert!(scaled_content.contains("0 0 200 120 re\nf\n"));

    let downscaled = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .sizing(Sizing::MaxDimension(100.0))
            .build()
            .expect("maximum-dimension PDF request is valid"),
    );
    let downscaled_pdf = String::from_utf8_lossy(downscaled.artifact().bytes());
    assert!(downscaled_pdf.contains("/MediaBox [0 0 75 45]"));
    assert!(downscaled_pdf.contains("/CropBox [0 0 75 45]"));
    let downscaled_content =
        std::str::from_utf8(stream_data(downscaled.artifact().bytes(), b"4 0 obj"))
            .expect("downscaled PDF content is ASCII");
    assert!(downscaled_content.starts_with("q\n0.375 0 0 -0.375 0 45 cm\n"));

    let transparent = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .include_background(false)
            .build()
            .expect("transparent PDF request is valid"),
    );
    let transparent_content =
        std::str::from_utf8(stream_data(transparent.artifact().bytes(), b"4 0 obj"))
            .expect("transparent PDF content is ASCII");
    assert!(!transparent_content.contains(" re\nf\n"));
    assert_eq!(
        transparent_content
            .lines()
            .filter(|line| line.ends_with(" c"))
            .count(),
        8
    );
}

#[test]
fn pdf_numbers_are_non_exponential_round_trip_decimals_with_canonical_zero() {
    let outcome = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .sizing(Sizing::Scale(0.000_000_1))
            .build()
            .expect("small finite PDF scale is valid"),
    );
    let content = std::str::from_utf8(stream_data(outcome.artifact().bytes(), b"4 0 obj"))
        .expect("PDF content is ASCII");
    assert!(content.starts_with("q\n0.000000075 0 0 -0.000000075 0 0.000009 cm\n"));
    for token in content.split_ascii_whitespace() {
        if token.parse::<f64>().is_ok() {
            assert!(
                !token.contains(['e', 'E']),
                "exponential PDF number {token}"
            );
            assert_ne!(token, "-0");
        }
    }
}

#[test]
fn pdf_rectangle_resource_crossings_are_atomic_and_locally_recoverable() {
    let local = ResourceLimits::builder()
        .max_geometry_segments_per_element(15)
        .build()
        .expect("local PDF geometry limit is valid");
    let outcome = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .resource_limits(local)
            .build()
            .expect("resource-limited PDF request is valid"),
    );
    let content = std::str::from_utf8(stream_data(outcome.artifact().bytes(), b"4 0 obj"))
        .expect("PDF content is ASCII");
    assert!(content.contains("0 0 200 120 re\nf\n"));
    assert!(content.contains("[6 4] 0 d"));
    let diagnostic = outcome
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code().as_str() == "resource.element-complexity-replaced")
        .expect("local PDF crossing retains its resource diagnostic");
    assert_eq!(
        diagnostic.code().as_str(),
        "resource.element-complexity-replaced"
    );
    let descriptor = serde_json::to_value(diagnostic).expect("diagnostic serializes");
    assert_eq!(descriptor["format"], "pdf");
    assert_eq!(
        descriptor["details"]["field"],
        "max_geometry_segments_per_element"
    );
    assert_eq!(descriptor["details"]["recovery"], "placeholder");

    let aggregate = ResourceLimits::builder()
        .max_geometry_segments_per_element(15)
        .max_total_geometry_segments(15)
        .build()
        .expect("aggregate PDF geometry limit is valid");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(aggregate)
        .build()
        .expect("resource-limited PDF request is valid");
    let error = Renderer::new()
        .render(&approved_scene(), &request)
        .expect_err("the sixteenth Rough operation crosses atomically");
    assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("rectangle-outline"));
    assert_eq!(error.json_pointer(), "/elements/0");
    assert!(error.report().is_empty());
}

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harness"]
fn approved_rectangle_passes_both_canonical_pdf_visual_gates() {
    let root = repository_root();
    let case = root.join("tests/conformance/cases/focused/rectangle-outline");
    let exception: serde_json::Value = serde_json::from_slice(
        &fs::read(case.join("pdf-visual-exception.json")).expect("PDF exception is present"),
    )
    .expect("PDF exception is valid JSON");
    assert_eq!(exception["approved_issue"], 44);
    assert_eq!(exception["viewer_profile"], "canonical-pdf.pdfium.v1");
    let stem = format!("rectangle-pdf-candidate-{}", std::process::id());
    let candidate = format!("target/{stem}.pdf");
    let reference = CanonicalRgba8::new(
        200,
        120,
        fs::read(root.join(concat!(
            "tests/conformance/svg-viewer-qualification/rectangle-outline/",
            "evidence/chromium-reference.rgba"
        )))
        .unwrap(),
    )
    .expect("canonical Chromium official-SVG evidence is valid");
    fs::write(
        root.join(&candidate),
        render_approved_pdf().artifact().bytes(),
    )
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
            200,
            120,
            render_viewer(&root, viewer, script, &candidate, &output, 200, 120),
        )
        .expect("canonical PDF viewer output is valid");
        let report = compare(
            &reference,
            &candidate,
            VisualClass::GeometryImage,
            &[],
            mode,
        )
        .expect("candidate and reference dimensions agree");
        if report.passed() {
            continue;
        }
        assert_eq!(
            viewer, "pdfium",
            "only the approved PDFium exception applies"
        );
        assert_eq!(
            report.whole.changed_pixels,
            exception["max_changed_pixels"].as_u64().unwrap()
        );
        let SsimOutcome::Score(ssim) = report.whole.ssim else {
            panic!("PDFium exception requires a finite SSIM score");
        };
        let minimum_bits =
            u64::from_str_radix(exception["minimum_ssim_bits"].as_str().unwrap(), 16)
                .expect("PDFium exception contains valid f64 bits");
        assert!(
            ssim >= f64::from_bits(minimum_bits),
            "PDFium exception failed: changed_pixels={}, ssim={ssim:?}, deltas={:?}",
            report.whole.changed_pixels,
            pixel_delta_summary(reference.rgba(), candidate.rgba())
        );
    }

    let structure_output = format!("target/{stem}-structure.json");
    assert_structure(
        &root,
        &candidate,
        "tests/conformance/cases/focused/rectangle-outline/expected-pdf-structure.json",
        &structure_output,
    );
}

fn pixel_delta_summary(reference: &[u8], candidate: &[u8]) -> std::collections::BTreeMap<i16, u64> {
    let mut summary = std::collections::BTreeMap::new();
    for (reference, candidate) in reference.iter().zip(candidate) {
        if reference != candidate {
            *summary
                .entry(i16::from(*candidate) - i16::from(*reference))
                .or_insert(0) += 1;
        }
    }
    summary
}

fn render_approved_pdf() -> RenderOutcome {
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("default PDF request is valid");
    Renderer::new()
        .render(&approved_scene(), &request)
        .expect("approved rectangle renders directly as PDF vectors")
}

fn render_pdf(request: RenderRequest) -> RenderOutcome {
    Renderer::new()
        .render(&approved_scene(), &request)
        .expect("approved rectangle renders directly as PDF vectors")
}

fn approved_scene() -> SceneDocument {
    SceneDocument::parse(
        &fs::read(
            repository_root()
                .join("tests/conformance/cases/focused/rectangle-outline/scene.excalidraw"),
        )
        .unwrap(),
    )
    .expect("approved rectangle scene is admitted")
}

fn stream_data<'a>(pdf: &'a [u8], object: &[u8]) -> &'a [u8] {
    let object_start = find_bytes(pdf, object).expect("expected PDF object exists");
    let stream_marker = b"stream\n";
    let stream_start = object_start
        + find_bytes(&pdf[object_start..], stream_marker).expect("object has a stream")
        + stream_marker.len();
    let stream_end = stream_start
        + find_bytes(&pdf[stream_start..], b"\nendstream").expect("stream has an end marker");
    &pdf[stream_start..stream_end]
}

fn icc_tags(profile: &[u8]) -> Vec<&str> {
    let count = u32::from_be_bytes(profile[128..132].try_into().unwrap()) as usize;
    (0..count)
        .map(|index| {
            let start = 132 + index * 12;
            std::str::from_utf8(&profile[start..start + 4]).expect("ICC tag is ASCII")
        })
        .collect()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}
