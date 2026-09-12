use excalidraw_render::{
    DiagnosticCode, ErrorCode, FrameRenderRequest, OutputFormat, Recovery, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};

#[test]
fn canonical_cycle_closing_edge_is_removed_with_its_source_pointer() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"rectangle","width":40,"height":40,"frameId":"b","seed":1,"updated":1},{"id":"b","type":"rectangle","x":50,"width":40,"height":40,"frameId":"a","seed":1,"updated":1}],"appState":{},"files":{}}"##,
    )
    .expect("cycle vector is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("default request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("recoverable relationship cycle renders");
        let cycles: Vec<_> = outcome
            .report()
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == DiagnosticCode::NORMALIZATION_RELATIONSHIP_CYCLE
            })
            .collect();
        assert_eq!(cycles.len(), 1, "one source repair is diagnosed");
        let cycle = cycles[0];

        assert_eq!(cycle.element_id(), Some("b"));
        assert_eq!(cycle.json_pointer(), "/elements/1/frameId");
        assert!(matches!(
            cycle.details(),
            Some(excalidraw_render::ErrorDetails::Compatibility {
                recovery: Recovery::Removed,
                recipe_id: None,
            })
        ));
    }
}

#[test]
fn cycle_diagnostics_are_scoped_and_replayed_once_per_frame() {
    let scene = SceneDocument::parse(br#"{
        "type":"excalidraw","version":2,"elements":[
            {"id":"first","type":"frame","width":40,"height":40,"updated":1,"frameId":"first"},
            {"id":"second","type":"frame","x":50,"width":40,"height":40,"updated":1,"frameId":"second"}
        ],"appState":{},"files":{}
    }"#).unwrap();
    let frames: Vec<_> = scene.frames().collect();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let renderer = Renderer::new();
        let request = FrameRenderRequest::builder(format).build().unwrap();
        let mut batch = renderer
            .render_frames(&scene, frames.iter().copied(), &request)
            .unwrap();
        for (index, frame) in frames.iter().enumerate() {
            let standalone = renderer.render_frame(*frame, &request).unwrap();
            let item = batch.next().unwrap();
            let replayed = item.result().as_ref().unwrap();
            assert_eq!(standalone.artifact().bytes(), replayed.artifact().bytes());
            assert_eq!(
                serde_json::to_value(standalone.report()).unwrap(),
                serde_json::to_value(replayed.report()).unwrap()
            );
            let diagnostics = standalone.report().diagnostics();
            assert_eq!(
                diagnostics.len(),
                1,
                "only the selected frame's repair is reported"
            );
            assert_eq!(
                diagnostics[0].code(),
                DiagnosticCode::NORMALIZATION_RELATIONSHIP_CYCLE
            );
            assert_eq!(
                diagnostics[0].json_pointer(),
                format!("/elements/{index}/frameId")
            );
        }
        assert!(batch.next().is_none());
    }
}

#[test]
fn relationship_depth_accepts_the_limit_and_rejects_the_next_edge() {
    let below_limit = render_with_depth(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"rectangle","frameId":"b","seed":1,"updated":1},{"id":"b","type":"rectangle","seed":1,"updated":1}],"appState":{},"files":{}}"##,
        2,
    );
    assert!(below_limit.is_ok(), "a path below the limit is accepted");

    let accepted = render_with_depth(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"rectangle","frameId":"b","seed":1,"updated":1},{"id":"b","type":"rectangle","frameId":"c","seed":1,"updated":1},{"id":"c","type":"rectangle","seed":1,"updated":1}],"appState":{},"files":{}}"##,
        2,
    );
    assert!(accepted.is_ok(), "a path exactly at the limit is accepted");

    let error = render_with_depth(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"rectangle","frameId":"b","seed":1,"updated":1},{"id":"b","type":"rectangle","frameId":"c","seed":1,"updated":1},{"id":"c","type":"rectangle","frameId":"d","seed":1,"updated":1},{"id":"d","type":"rectangle","seed":1,"updated":1}],"appState":{},"files":{}}"##,
        2,
    )
    .expect_err("the first path deeper than the limit is fatal");

    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_RELATIONSHIP_DEPTH_EXCEEDED
    );
    assert_eq!(error.element_id(), Some("c"));
    assert_eq!(error.json_pointer(), "/elements/2/frameId");
    let descriptor = serde_json::to_value(error.descriptor()).expect("error serializes");
    assert_eq!(descriptor["details"]["field"], "max_relationship_depth");
    assert_eq!(descriptor["details"]["configured"], 2);
    assert_eq!(descriptor["details"]["value_kind"], "depth");
    assert_eq!(descriptor["details"]["measurement"]["value"], 3);
}

fn render_with_depth(
    source: &[u8],
    depth: u32,
) -> Result<excalidraw_render::RenderOutcome, excalidraw_render::RenderError> {
    let scene = SceneDocument::parse(source).expect("depth vector is admitted");
    let limits = ResourceLimits::builder()
        .max_relationship_depth(depth)
        .build()
        .expect("relationship depth is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");
    Renderer::new().render(&scene, &request)
}
