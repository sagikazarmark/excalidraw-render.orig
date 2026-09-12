use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::json;

#[test]
fn soft_hyphen_renders_without_diagnostics() {
    for text in ["\u{00ad}", "A\u{00ad}B"] {
        let scene = SceneDocument::parse(
            &serde_json::to_vec(&json!({
                "type":"excalidraw","version":2,"elements":[{
                    "id":"text","type":"text","text":text,"fontFamily":9,
                    "fontSize":20,"width":500,"height":40,"updated":1
                }],"appState":{},"files":{}
            }))
            .unwrap(),
        )
        .unwrap();
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format).build().unwrap();
            let first = Renderer::new().render(&scene, &request).unwrap();
            let second = Renderer::new().render(&scene, &request).unwrap();
            assert!(first.report().diagnostics().is_empty());
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
        }
    }
}
