use std::fs;

use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderOutcome, RenderRequest, RenderRequestBuilder, Renderer,
    ResourceLimits, SceneDocument,
};

use crate::common::approved_scenes::rectangle_outline_scene;
use crate::common::raster::decode_png;
use crate::common::render::{render, render_without_background, trailing_work_charges, WorkCharge};
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
fn png_vector_coverage_is_charged_to_total_work_not_the_rectangle_element_limit() {
    // The approved rectangle's semantic work fits 640 units; its PNG coverage
    // samples do not, and still never omit it: they are charged to total work
    // only, so the render is the unconstrained one.
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
        .expect("the element work limit leaves the rectangle's raster samples alone");
    let unconstrained = render_approved_png();
    assert_eq!(outcome.report(), unconstrained.report());
    assert_eq!(outcome.artifact().bytes(), unconstrained.artifact().bytes());

    // A roughness-0 80x60 rectangle covers its 82x62 stroked box at 16
    // samples per pixel. The charge is the rectangle's own total-work event,
    // after which only the canvas-wide encoding events follow, and crossing
    // it is the fatal total-work failure attributed to the rectangle.
    let scene = rectangles_scene(&[("large", 0, 80, 60)]);
    let charges = trailing_work_charges(&scene, png_request, 3);
    assert!(charges[..2]
        .iter()
        .all(|charge| charge.element_id.is_none() && charge.json_pointer.is_empty()));
    assert_eq!(
        charges[2],
        element_charge("large", "/elements/0", 16 * 82 * 62)
    );
    assert!(charges[2].units > 640);
}

#[test]
fn png_vector_coverage_is_charged_per_rectangle_without_local_omission() {
    let scene = rectangles_scene(&[("small", 0, 10, 10), ("large", 30, 80, 60)]);
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
        .expect("PNG coverage never crosses the element limit");
    let unconstrained = render(&scene, OutputFormat::Png);
    assert!(outcome.report().is_empty());
    assert_eq!(outcome.artifact().bytes(), unconstrained.artifact().bytes());

    // Each rectangle's coverage is its own total-work event, owned by it:
    // the large one's 82x62 box exceeds the 5,000-unit element limit above.
    let charges = trailing_work_charges(&scene, png_request, 4);
    assert_eq!(
        charges[2],
        element_charge("large", "/elements/1", 16 * 82 * 62)
    );
    assert_eq!(
        charges[3],
        element_charge("small", "/elements/0", 16 * 12 * 12)
    );
}

/// A rectangle larger than the default element work limit can price in PNG
/// coverage samples (16 x 3002 x 2002 > 67,108,864) paints in full: the
/// canvas the raster surface limit admitted bounds its samples.
#[test]
fn png_paints_a_large_solid_rectangle_under_default_limits() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"large","type":"rectangle","x":0,"y":0,"width":3000,"height":2000,"strokeColor":"#1e66f5","backgroundColor":"#1e66f5","fillStyle":"solid","roughness":0,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("large rectangle scene is admitted");
    assert!(16 * 3002 * 2002 > ResourceLimits::default().max_element_work_units());
    let outcome = render(&scene, OutputFormat::Png);

    assert!(
        !outcome
            .report()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED),
        "the large rectangle is not omitted: {:?}",
        outcome.report().diagnostics()
    );
    assert!(outcome.report().is_empty());
    let decoded = decode_png(outcome.artifact().bytes());
    assert_eq!((decoded.width, decoded.height), (3020, 2020));
    for (x, y) in [
        (1510, 1010),
        (500, 500),
        (2500, 1500),
        (20, 20),
        (2999, 2009),
    ] {
        let offset = 4 * (y * decoded.width as usize + x);
        assert_eq!(
            decoded.rgba[offset..offset + 4],
            [0x1e, 0x66, 0xf5, 0xff],
            "pixel ({x}, {y}) is the fill"
        );
    }
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

/// A scene of roughness-0 rectangles, `(id, x, width, height)` each at `y = 0`.
fn rectangles_scene(rectangles: &[(&str, u32, u32, u32)]) -> SceneDocument {
    let elements = rectangles
        .iter()
        .map(|(id, x, width, height)| {
            format!(
                r#"{{"id":"{id}","type":"rectangle","x":{x},"y":0,"width":{width},"height":{height},"roughness":0,"seed":1,"updated":1}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{elements}],"appState":{{}},"files":{{}}}}"#
    );
    SceneDocument::parse(source.as_bytes()).expect("rectangle scene is admitted")
}

fn png_request() -> RenderRequestBuilder {
    RenderRequest::builder(OutputFormat::Png)
}

fn element_charge(element_id: &str, json_pointer: &str, units: u64) -> WorkCharge {
    WorkCharge {
        units,
        element_id: Some(element_id.to_owned()),
        json_pointer: json_pointer.to_owned(),
    }
}
