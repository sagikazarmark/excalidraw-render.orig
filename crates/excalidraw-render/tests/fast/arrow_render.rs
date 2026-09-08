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
        assert!(
            outcome.report().is_empty(),
            "arrows must not use compatibility recovery for {format:?}: {:?}",
            outcome.report().diagnostics()
        );
        assert!(!outcome.artifact().bytes().is_empty());
        if format == OutputFormat::Svg {
            let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
            assert!(!svg.contains("Unavailable:"));
            let paths = svg_path_data(svg);
            assert!(paths.len() > 8, "arrowheads are separate vectors");
            // The arrow geometry is pinned as its path vectors rather than as
            // whole artifacts: the whole-artifact digests moved for a
            // fractional viewBox extent and for PNG IDAT compression while the
            // vectors, which are what this test holds, did not.
            assert_eq!(
                format!("{:x}", Sha256::digest(paths.join("\n"))),
                "5e25ba2ed774c760ceb3cb5389467fb12d6ef4c86ac1e453feb3951475b26c16",
                "exact arrow vectors changed"
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

/// Every `<path d>` value in document order.
fn svg_path_data(svg: &str) -> Vec<&str> {
    svg.split("<path d=\"")
        .skip(1)
        .map(|tail| tail.split_once('"').expect("path data terminates").0)
        .collect()
}
