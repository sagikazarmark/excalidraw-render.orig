use std::fs;

use excalidraw_render::{
    OutputFormat, RenderOutcome, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};

use crate::common::approved_scenes::rectangle_outline_scene;
use crate::common::raster::decode_png;
use crate::common::render::{render, render_without_background};
use crate::common::reports::assert_report_matches;
use crate::common::repository_root;
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};

#[test]
fn approved_rectangle_renders_as_a_canvas_equivalent_png() {
    let case = repository_root().join("tests/conformance/cases/focused/rectangle-outline");
    let first = render_approved_png();
    assert_eq!(first.artifact().format(), OutputFormat::Png);
    assert_eq!(first.artifact().media_type(), "image/png");
    assert_report_matches(
        first.report(),
        &case.join("expected-report-png.json"),
        "approved rectangle PNG",
    );

    let candidate = decode_png(first.artifact().bytes()).into_canonical();
    assert_eq!((candidate.width(), candidate.height()), (200, 120));
    assert_eq!(
        png_chunk_types(first.artifact().bytes()),
        [*b"IHDR", *b"IDAT", *b"IEND"]
    );
    assert!(!first
        .artifact()
        .bytes()
        .windows(4)
        .any(|bytes| bytes == b"<svg"));

    let reference = CanonicalRgba8::new(
        200,
        120,
        fs::read(case.join("evidence/official-png.rgba")).unwrap(),
    )
    .expect("approved direct-canvas evidence is canonical RGBA8");
    let report = compare(
        &reference,
        &candidate,
        VisualClass::GeometryImage,
        &[],
        ComparisonMode::Canonical,
    )
    .expect("candidate and reference dimensions agree");
    assert!(
        report.passed(),
        "direct PNG comparison failed: changed_pixels={}, ssim={:?}, deltas={:?}",
        report.whole.changed_pixels,
        report.whole.ssim,
        pixel_delta_summary(reference.rgba(), candidate.rgba())
    );
}

#[test]
fn transparent_rectangle_png_uses_canonical_straight_alpha() {
    let outcome = render_without_background(&rectangle_outline_scene(), OutputFormat::Png);
    let decoded = decode_png(outcome.artifact().bytes());
    let rgba = &decoded.rgba;

    assert_eq!((decoded.width, decoded.height), (200, 120));
    assert!(rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 0));
    assert!(rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| (1..=254).contains(&pixel[3])));
    assert!(rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] == 0)
        .all(|pixel| pixel[..3] == [0, 0, 0]));
    assert!(rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| (64..=192).contains(&pixel[3])
            && pixel[..3].iter().all(|channel| *channel >= 28)));
}

#[test]
fn png_vector_coverage_obeys_the_rectangle_element_work_limit() {
    let limits = ResourceLimits::builder()
        .max_element_work_units(640)
        .build()
        .expect("element work limit is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("resource-limited PNG request is valid");
    let outcome = Renderer::new()
        .render(&rectangle_outline_scene(), &request)
        .expect("local PNG coverage exhaustion retains a meaningful background");
    let decoded = decode_png(outcome.artifact().bytes());

    assert!(decoded
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| *pixel == [255, 255, 255, 255]));
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("local coverage crossing retains exactly one diagnostic");
    };
    assert_eq!(diagnostic.code().as_str(), "resource.element-work-exceeded");
    let descriptor = serde_json::to_value(diagnostic).expect("diagnostic serializes");
    assert_eq!(descriptor["details"]["field"], "max_element_work_units");
    assert_eq!(descriptor["details"]["recovery"], "omitted");
}

#[test]
fn png_vector_coverage_is_charged_and_recovered_per_rectangle() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"small","type":"rectangle","x":0,"width":10,"height":10,"seed":1,"updated":1},{"id":"large","type":"rectangle","x":30,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("multi-rectangle scene is admitted");
    let limits = ResourceLimits::builder()
        .max_element_work_units(5_000)
        .build()
        .expect("element work limit is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("resource-limited PNG request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("one local PNG recovery retains the other rectangle");
    let decoded = decode_png(outcome.artifact().bytes());

    assert!(decoded
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| *pixel != [255, 255, 255, 255]));
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("only the large rectangle exceeds its local work limit");
    };
    assert_eq!(diagnostic.element_id(), Some("large"));
    assert_eq!(diagnostic.json_pointer(), "/elements/1");
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

fn render_approved_png() -> RenderOutcome {
    render(&rectangle_outline_scene(), OutputFormat::Png)
}

fn png_chunk_types(bytes: &[u8]) -> Vec<[u8; 4]> {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let mut chunks = Vec::new();
    let mut offset = 8;
    while offset < bytes.len() {
        let length = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("PNG chunk has a length"),
        ) as usize;
        chunks.push(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("PNG chunk has a type"),
        );
        offset += 12 + length;
    }
    assert_eq!(offset, bytes.len());
    chunks
}
