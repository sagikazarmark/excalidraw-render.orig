use crate::common::approved_scenes::{rectangle_outline_scene, render_rectangle_outline_pdf};
use crate::common::pdf_inspect::{
    object_stream, page_content, pdf_attachment, pdf_object, pdf_text, reference,
    trailer_file_identifiers,
};
use crate::common::reports::assert_report_matches;
use crate::common::repository_root;
use excalidraw_render::{
    ErrorCode, OutputFormat, RenderErrorCategory, RenderOutcome, RenderRequest, Renderer,
    ResourceLimits, Sizing,
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
fn approved_rectangle_renders_as_a_vector_first_pdf() {
    let case = repository_root().join("tests/conformance/cases/focused/rectangle-outline");
    let first = render_rectangle_outline_pdf();
    assert_eq!(first.artifact().format(), OutputFormat::Pdf);
    assert_eq!(first.artifact().media_type(), "application/pdf");
    assert_report_matches(
        first.report(),
        &case.join("expected-report-svg.json"),
        "approved rectangle PDF",
    );

    let bytes = first.artifact().bytes();
    assert!(bytes.starts_with(b"%PDF-1.7\n"));
    assert!(bytes.ends_with(b"%%EOF"));
    let pdf = pdf_text(bytes);
    assert_eq!(pdf.matches("/Type /Page\n").count(), 1);
    assert!(pdf.contains("/MediaBox [0 0 150 90]"));
    assert!(pdf.contains("/CropBox [0 0 150 90]"));
    // No output intent is claimed; every color is in the ICC-based sRGB
    // space Krilla binds for vector content (ADR-0021).
    assert!(!pdf.contains("/OutputIntents"));
    let resources = pdf
        .split("/ColorSpace << /c0 ")
        .nth(1)
        .expect("the page binds its color space");
    let space = resources.split(' ').next().unwrap().parse().unwrap();
    assert!(pdf_object(&pdf, space).starts_with("[/ICCBased "));

    // The background and the outline, each an artifact under its own
    // page-space matrix: the logical-pixel mapping, then the element's 10 px
    // offset. Colors are Krilla's single-precision spelling of 30/255.
    let content = page_content(bytes);
    assert!(
        content.starts_with(concat!(
            "/Artifact BMC\nq\n0.75 0 0 -0.75 0 90 cm\n",
            "/c0 cs\n1 1 1 scn\n0 0 m\n200 0 l\n200 120 l\n0 120 l\nh\nf\nQ\nEMC\n",
            "/Artifact BMC\nq\n0.75 0 0 -0.75 7.5 82.5 cm\n",
            "/c0 CS\n0.11764706 0.11764706 0.11764706 SCN\n2 w\n1 J\n"
        )),
        "unexpected content stream: {content:?}"
    );
    assert!(content.contains(EXPECTED_ROUGH_PATH));
    assert!(content.ends_with("S\nQ\nEMC\n"));
    // Eight Rough subpaths and the background rectangle.
    assert_eq!(
        content.lines().filter(|line| line.ends_with(" m")).count(),
        9
    );
    assert_eq!(
        content.lines().filter(|line| line.ends_with(" c")).count(),
        8
    );
    assert_eq!(content.lines().filter(|line| *line == "S").count(), 1);

    let profile = object_stream(
        bytes,
        reference(pdf_object(&pdf, space), "/ICCBased ").expect("the space names its profile"),
    );
    // An ICC profile opens with its own byte length, so the embedded stream is
    // held to that field rather than to a literal length. Krilla's profile is
    // version 4.2 and carries the seven matrix/TRC tags.
    assert_eq!(
        u32::from_be_bytes(profile[..4].try_into().expect("ICC header has a size")),
        u32::try_from(profile.len()).expect("ICC profile length fits u32")
    );
    assert_eq!(&profile[8..10], b"\x04\x20");
    assert_eq!(&profile[12..24], b"mntrRGB XYZ ");
    assert_eq!(&profile[36..40], b"acsp");
    let tags = icc_tags(&profile);
    for required in ["rXYZ", "gXYZ", "bXYZ", "wtpt", "rTRC", "gTRC", "bTRC"] {
        assert!(tags.contains(&required), "ICC tag {required}");
    }

    for forbidden in [
        "/Subtype /Image",
        "/BleedBox",
        "/TrimBox",
        "/ArtBox",
        "/Rotate",
        "/UserUnit",
        "/Encrypt",
        "/Linearized",
    ] {
        assert!(
            !pdf.contains(forbidden),
            "PDF contains forbidden {forbidden}"
        );
    }
    assert!(!bytes.windows(4).any(|window| window == b"<svg"));

    // Every stream is one Flate stream: the content stream and the ICC
    // profile here, and nothing else.
    let raw = String::from_utf8_lossy(bytes);
    assert_eq!(raw.matches("/Filter/FlateDecode").count(), 2);
    assert_eq!(raw.matches("\nstream\n").count(), 2);
    assert!(pdf_attachment(bytes, "scene.excalidraw").is_none());
    assert!(
        !pdf.contains("/Filter"),
        "inflating drops every filter entry"
    );

    // The trailer identifiers are Krilla's stable hash of the file, equal
    // and a pure function of the artifact, and no document information
    // dictionary is written (ADR-0021).
    let trailer = pdf.rsplit("\ntrailer\n").next().unwrap();
    assert!(
        !trailer.contains("/Info"),
        "the trailer carries no document information dictionary"
    );
    let (first_id, second_id) = trailer_file_identifiers(bytes);
    assert_eq!(first_id, second_id);
    assert_eq!(
        trailer_file_identifiers(render_rectangle_outline_pdf().artifact().bytes()),
        (first_id.clone(), first_id)
    );
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
            .render(&rectangle_outline_scene(), &request)
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
        .render(&rectangle_outline_scene(), &request)
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
        .render(&rectangle_outline_scene(), &request)
        .expect("validation and encoding use identical sizing arithmetic");

    let request = RenderRequest::builder(OutputFormat::Pdf)
        .sizing(Sizing::Scale(f64::MAX))
        .build()
        .expect("huge finite PDF scale is a valid request");
    let error = Renderer::new()
        .render(&rectangle_outline_scene(), &request)
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
    let scaled_pdf = pdf_text(scaled.artifact().bytes());
    assert!(scaled_pdf.contains("/MediaBox [0 0 300 180]"));
    assert!(scaled_pdf.contains("/CropBox [0 0 300 180]"));
    let scaled_content = page_content(scaled.artifact().bytes());
    assert!(scaled_content.starts_with("/Artifact BMC\nq\n1.5 0 0 -1.5 0 180 cm\n"));
    assert!(scaled_content.contains("0 0 m\n200 0 l\n200 120 l\n0 120 l\nh\nf\n"));

    let downscaled = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .sizing(Sizing::MaxDimension(100.0))
            .build()
            .expect("maximum-dimension PDF request is valid"),
    );
    let downscaled_pdf = pdf_text(downscaled.artifact().bytes());
    assert!(downscaled_pdf.contains("/MediaBox [0 0 75 45]"));
    assert!(downscaled_pdf.contains("/CropBox [0 0 75 45]"));
    let downscaled_content = page_content(downscaled.artifact().bytes());
    assert!(downscaled_content.starts_with("/Artifact BMC\nq\n0.375 0 0 -0.375 0 45 cm\n"));

    let transparent = render_pdf(
        RenderRequest::builder(OutputFormat::Pdf)
            .include_background(false)
            .build()
            .expect("transparent PDF request is valid"),
    );
    let transparent_content = page_content(transparent.artifact().bytes());
    assert!(!transparent_content.contains("200 120 l\n"));
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
    let content = page_content(outcome.artifact().bytes());
    assert!(content.starts_with("/Artifact BMC\nq\n0.000000075 0 0 -0.000000075 0 0.000009 cm\n"));
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
    let content = page_content(outcome.artifact().bytes());
    assert!(content.contains("0 0 m\n200 0 l\n200 120 l\n0 120 l\nh\nf\n"));
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
        .render(&rectangle_outline_scene(), &request)
        .expect_err("the sixteenth Rough operation crosses atomically");
    assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("rectangle-outline"));
    assert_eq!(error.json_pointer(), "/elements/0");
    assert!(error.report().is_empty());
}

fn render_pdf(request: RenderRequest) -> RenderOutcome {
    Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect("approved rectangle renders directly as PDF vectors")
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
