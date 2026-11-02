use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

use crate::common::lower_hex;
use crate::common::svg::path_data;

/// Every arrow family: bound, elbowed, rotated and dashed, with each arrowhead.
/// The golden record holds its bytes in every format (`facade.arrows.v1`).
const ARROW_SCENE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/golden/facade-scenes/arrows.excalidraw"
));

pub(super) fn admitted_arrows_bindings_elbows_and_arrowheads_render_in_every_format() {
    let scene = SceneDocument::parse(ARROW_SCENE).expect("arrow scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("complete admitted arrow behavior renders");

        // The golden record holds every format's bytes (`facade.arrows.v1`).
        // The SVG's path-vector digest below is the geometry this test holds,
        // so a path change names itself here rather than as a golden byte
        // difference.
        assert!(
            outcome.report().is_empty(),
            "arrows must not use compatibility recovery for {format:?}: {:?}",
            outcome.report().diagnostics()
        );
        assert!(!outcome.artifact().bytes().is_empty());
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
            assert!(!svg.contains("Unavailable:"));
            let paths = path_data(svg);
            assert!(paths.len() > 8, "arrowheads are separate vectors");
            assert_eq!(
                lower_hex(&Sha256::digest(paths.join("\n"))),
                "dd4a3a37eed922fa3ead257755202c3d3fe0e2872fc008e4cc95ef526d28cb87",
                "exact arrow vectors changed"
            );
        } else if format == OutputFormat::Pdf {
            // Krilla draws each path in its own graphics state (ADR-0021), so
            // a dash set for a shaft ends with it: every dashed state strokes
            // one path, and the arrowheads after it are solid.
            let content = crate::common::pdf_inspect::page_content(outcome.artifact().bytes());
            let dashed: Vec<&str> = content
                .split("\nq\n")
                .map(|block| block.split("\nQ\n").next().unwrap())
                .filter(|body| body.contains("] 0 d\n"))
                .collect();
            assert!(!dashed.is_empty(), "a dashed shaft is drawn");
            for body in dashed {
                assert_eq!(
                    body.matches("\nS\n").count(),
                    1,
                    "dashed arrowheads reset the shaft dash"
                );
            }
        }
    }
}

#[test]
fn arrowhead_geometry_limit_recovers_locally_and_deterministically() {
    let scene = SceneDocument::parse(ARROW_SCENE).unwrap();
    let limits = ResourceLimits::builder()
        .max_geometry_segments_per_element(8)
        .build()
        .unwrap();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .resource_limits(limits.clone())
            .build()
            .unwrap();
        let first = Renderer::new().render(&scene, &request).unwrap();
        let second = Renderer::new().render(&scene, &request).unwrap();

        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert!(first.report().diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
                && matches!(diagnostic.element_id(), Some("bound" | "elbow" | "dot"))
        }));
    }
}
