use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

use crate::common::svg::path_data;

const ARROW_SCENE: &[u8] = br##"{
  "type":"excalidraw","version":2,
  "elements":[
    {"id":"target-a","type":"rectangle","x":0,"y":40,"width":40,"height":40,"boundElements":[{"id":"bound","type":"arrow"}],"seed":1,"updated":1},
    {"id":"target-b","type":"ellipse","x":180,"y":40,"width":40,"height":40,"boundElements":[{"id":"bound","type":"arrow"}],"seed":2,"updated":1},
    {"id":"line","type":"line","x":20,"y":230,"width":160,"height":30,"points":[[0,0],[80,30],[160,0]],"angle":0.2617993877991494,"seed":9,"updated":1},
    {"id":"bound","type":"arrow","x":40,"y":60,"width":140,"height":0,"points":[[0,0],[140,0]],"startBinding":{"elementId":"target-a","focus":0,"gap":1},"endBinding":{"elementId":"target-b","focus":0,"gap":1},"startArrowhead":"bar","endArrowhead":"arrow","seed":3,"updated":1},
    {"id":"elbow","type":"arrow","x":20,"y":120,"width":160,"height":80,"points":[[0,0],[80,0],[80,80],[160,80]],"elbowed":true,"startArrowhead":"circle_outline","endArrowhead":"triangle","seed":4,"updated":1},
    {"id":"dot","type":"arrow","x":240,"y":20,"width":80,"height":0,"points":[[0,0],[80,0]],"startArrowhead":"dot","endArrowhead":"circle","seed":5,"updated":1},
    {"id":"triangle-outline","type":"arrow","x":240,"y":50,"width":80,"height":0,"points":[[0,0],[80,0]],"startArrowhead":"triangle_outline","endArrowhead":"diamond","seed":6,"updated":1},
    {"id":"diamond-outline","type":"arrow","x":240,"y":80,"width":80,"height":0,"points":[[0,0],[80,0]],"startArrowhead":"diamond_outline","endArrowhead":"crowfoot_one","seed":7,"updated":1},
    {"id":"crowfeet","type":"arrow","x":240,"y":110,"width":80,"height":0,"points":[[0,0],[80,0]],"strokeStyle":"dashed","startArrowhead":"crowfoot_many","endArrowhead":"crowfoot_one_or_many","seed":8,"updated":1}
  ],
  "appState":{"viewBackgroundColor":"#ffffff"},"files":{}
}"##;

#[test]
pub(super) fn admitted_arrows_bindings_elbows_and_arrowheads_render_in_every_format() {
    let scene = SceneDocument::parse(ARROW_SCENE).expect("arrow scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("complete admitted arrow behavior renders");

        // One pin per artifact. The SVG's is the path-vector digest below, the
        // geometry this test holds; its whole-artifact digest moved for a
        // fractional viewBox extent that the vectors never saw. The PNG digest
        // is the raster canary that caught the `crowfeet` dashed stroke
        // rendering solid; it moved once more when IDAT switched to
        // deflate-compressed scanlines (#113) with the pixels unchanged. The
        // PDF digest moved when every stream became Flate and the trailer
        // gained its content-derived /ID (#133, #150; ADR-0008), with the
        // viewer rasters and operator sequence unchanged. All three moved
        // with #171: the `elbow` arrow's `triangle` end head follows a
        // `circle_outline` start, and the start head's 0.5 clamp was carried
        // to it where the reference clamps each head on its own kind, so the
        // triangle is now drawn at roughness 1 (paths 11 and 12 of the SVG;
        // every other path is byte-identical). The PNG and PDF moved once more
        // with #172: the `line` rotated 15 degrees at roughness 1 set the
        // scene's bottom edge with its rotated box corners, where the
        // reference rotates the curve's control points about the centre of
        // the points' box and takes their extrema, 20.7 px less
        // (`viewBox` height 259.488887394336, the official export's); the
        // paths are unchanged. The PDF moved again when Krilla became the PDF
        // writer (ADR-0021), with the same Rough paths.
        let whole_artifact_digest = match format {
            OutputFormat::Svg => None,
            OutputFormat::Png => {
                Some("94951f079f0d61fb6780f3c98324abb9a3cdf8e1d7a2caa1e1e546eff61b5315")
            }
            OutputFormat::Pdf => {
                Some("d35a2138960c31583967a5c609a6fe5a62a04da9e673ca969ff596846cb20b87")
            }
            _ => unreachable!("the test enumerates every revision-1 format"),
        };
        if let Some(expected_digest) = whole_artifact_digest {
            assert_eq!(
                format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
                expected_digest,
                "exact {format:?} arrow artifact changed"
            );
        }
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
                format!("{:x}", Sha256::digest(paths.join("\n"))),
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
