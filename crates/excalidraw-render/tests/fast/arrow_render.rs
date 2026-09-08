use excalidraw_render::{
    DiagnosticCode, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

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
fn admitted_arrows_bindings_elbows_and_arrowheads_render_in_every_format() {
    let scene = SceneDocument::parse(ARROW_SCENE).expect("arrow scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("complete admitted arrow behavior renders");
        let repeated = Renderer::new()
            .render(&scene, &request)
            .expect("repeated arrow render succeeds");

        assert_eq!(outcome.artifact().bytes(), repeated.artifact().bytes());
        assert_eq!(outcome.report(), repeated.report());
        // The SVG digest moved when elbow arrow bounds started deriving from the
        // declared route: the scene's `elbow` element makes the document a
        // fraction of a pixel shorter, which the SVG records in its viewBox. PNG
        // and PDF are unchanged because the difference does not cross the
        // truncation boundary for this scene, so the raster is identical. An
        // earlier PNG-only move came from fixing stroke dashing, which the
        // `crowfeet` element exercises.
        let expected_digest = match format {
            OutputFormat::Svg => "cf381468226c5326196b98fda7c940636a35b3637021ea344e3fc765daf15756",
            // Updated when PNG stroke dashing was fixed: the previous digest pinned the
            // `crowfeet` arrow's dashed stroke rendering as solid. SVG and PDF are
            // unchanged, which is what confirms the move was PNG-only. Moved again
            // when IDAT switched from stored to deflate-compressed scanlines (#113);
            // the decoded pixels are unchanged.
            OutputFormat::Png => "09e2eb955e4134f3f3a6b19b1cee567f130b42d947248764faa036e1199fae23",
            OutputFormat::Pdf => "99505f0a67ce2181670ec8fcc9992e68e6376e6e9700cb1a59fae74a8a192566",
            _ => unreachable!("the test enumerates every revision-1 format"),
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
            expected_digest,
            "exact arrow vector changed for {format:?}"
        );
        assert!(
            outcome.report().is_empty(),
            "arrows must not use compatibility recovery for {format:?}: {:?}",
            outcome.report().diagnostics()
        );
        assert!(!outcome.artifact().bytes().is_empty());
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
            assert!(!svg.contains("Unavailable:"));
            assert!(
                svg.matches("<path ").count() > 8,
                "arrowheads are separate vectors"
            );
        } else if format == OutputFormat::Pdf {
            let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
            assert!(
                pdf.contains("[] 0 d\n"),
                "dashed arrowheads reset the shaft dash"
            );
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
