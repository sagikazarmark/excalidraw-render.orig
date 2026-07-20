use excalidraw_render::{
    DiagnosticCode, DiagnosticEffect, ErrorCode, OutputFormat, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};

#[test]
fn revision_1_compatibility_defaults_match_explicit_rectangle_values() {
    let defaulted = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"defaults","type":"rectangle","width":180,"height":100,"updated":1}],"appState":{},"files":{}}"##,
    );
    let explicit = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"defaults","type":"rectangle","x":0,"y":0,"width":180,"height":100,"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid","roundness":null,"roughness":1,"opacity":100,"seed":1,"isDeleted":false,"groupIds":[],"frameId":null,"boundElements":[],"link":null,"version":1,"versionNonce":0,"updated":1,"locked":false}],"appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##,
    );

    assert_eq!(defaulted.artifact().bytes(), explicit.artifact().bytes());
    assert!(defaulted.report().is_empty());
    assert!(explicit.report().is_empty());
}

#[test]
fn zero_dimension_defaults_remain_a_meaningful_nonpainting_scene() {
    let defaulted = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"zero","type":"rectangle","seed":1,"updated":1}],"appState":{},"files":{}}"##,
    );
    let empty = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"##,
    );

    assert_eq!(defaulted.artifact().bytes(), empty.artifact().bytes());
    assert!(defaulted.report().is_empty());
}

#[test]
fn revision_1_negative_dimensions_shift_the_origin_without_a_diagnostic() {
    let negative = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"negative","type":"rectangle","x":20,"y":30,"width":-180,"height":-100,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
    );
    let normalized = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"negative","type":"rectangle","x":-160,"y":-70,"width":180,"height":100,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
    );

    assert_eq!(negative.artifact().bytes(), normalized.artifact().bytes());
    assert!(negative.report().is_empty());

    let negative_zero = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"zero","type":"rectangle","width":-0,"height":100,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
    );
    let canonical_zero = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"zero","type":"rectangle","width":0,"height":100,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
    );
    assert_eq!(
        negative_zero.artifact().bytes(),
        canonical_zero.artifact().bytes()
    );
}

#[test]
fn revision_1_identity_timestamp_and_seed_repairs_are_stable_and_source_located() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"##;

    let first = render_svg(source);
    let second = render_svg(source);

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let diagnostics = first.report().diagnostics();
    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        vec![
            DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
            DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
            DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
        ]
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        vec!["/elements/0/id", "/elements/0/seed", "/elements/0/updated",]
    );
    assert_eq!(diagnostics[0].effects(), &[DiagnosticEffect::InputRepair]);
    assert_eq!(
        diagnostics[1].effects(),
        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual]
    );
    assert_eq!(diagnostics[2].effects(), &[DiagnosticEffect::InputRepair]);
    let normalized_id = diagnostics[0]
        .element_id()
        .expect("the missing identity repair exposes the normalized identity");
    assert_eq!(
        normalized_id,
        "generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.element_id() == Some(normalized_id)));
    for diagnostic in diagnostics {
        let value = serde_json::to_value(diagnostic).expect("diagnostic serializes");
        assert_eq!(
            value["details"],
            serde_json::json!({
                "kind": "compatibility",
                "recovery": "repaired",
                "recipe_id": null,
            })
        );
    }

    let already_normalized = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792","type":"rectangle","width":180,"height":100,"seed":642336616,"updated":0}],"appState":{},"files":{}}"##,
    );
    assert_eq!(
        first.artifact().bytes(),
        already_normalized.artifact().bytes()
    );
    assert!(already_normalized.report().is_empty());
}

#[test]
fn revision_1_rekeys_each_later_duplicate_identity_in_source_order() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"duplicate","type":"rectangle","x":0,"y":0,"width":80,"height":60,"seed":1,"updated":1},{"id":"duplicate","type":"rectangle","x":100,"y":0,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"##;

    let first = render_svg(source);
    let second = render_svg(source);

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let [diagnostic] = first.report().diagnostics() else {
        panic!("the later duplicate produces exactly one repair diagnostic");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID
    );
    assert_eq!(diagnostic.json_pointer(), "/elements/1/id");
    assert_eq!(
        diagnostic.element_id(),
        Some("generated-3180035c77cb940ec4c79c4729441c64bad5072d494bd981721e7e1b9c85c3a3")
    );
    assert_eq!(diagnostic.effects(), &[DiagnosticEffect::InputRepair]);

    let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(svg.matches("<path ").count(), 2);
}

#[test]
fn generated_identity_collision_is_rekeyed_without_displacing_the_first_owner() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"generated-d02dc903e43157ace063528d673d5657c6e6433aa467e643d1b7a2cb9296386a","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"type":"rectangle","x":100,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"##;

    let outcome = render_svg(source);
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("the missing identity produces one repair diagnostic");
    };

    assert_eq!(
        diagnostic.element_id(),
        Some("generated-604f36b7127cb8ae0e242c4cf18cbb0f437f585790ef41c6f0edaa07a7676e63")
    );
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID
    );
}

#[test]
fn generated_values_ignore_discarded_json_lexical_spelling() {
    let compact = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"##;
    let alternate = br##" {
        "\u0074ype" : "excalidraw",
        "version" : 2,
        "elements" : [ { "\u0074ype" : "rectangle", "width" : 180, "height" : 100, "seed" : 0 } ],
        "appState" : { },
        "files" : { }
    } "##;

    let compact = render_svg(compact);
    let alternate = render_svg(alternate);

    assert_eq!(compact.artifact().bytes(), alternate.artifact().bytes());
    assert_eq!(compact.report(), alternate.report());
}

#[test]
fn normalization_diagnostics_follow_total_source_and_field_order() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","x":0,"width":40,"height":40,"seed":0},{"id":"same","type":"rectangle","x":50,"width":40,"height":40,"seed":0},{"id":"same","type":"rectangle","x":100,"width":40,"height":40,"seed":0}],"appState":{},"files":{}}"##;

    let outcome = render_svg(source);
    let actual = outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
                "/elements/0/id",
            ),
            (
                DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                "/elements/0/seed",
            ),
            (
                DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                "/elements/0/updated",
            ),
            (
                DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                "/elements/1/seed",
            ),
            (
                DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                "/elements/1/updated",
            ),
            (
                DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID,
                "/elements/2/id",
            ),
            (
                DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                "/elements/2/seed",
            ),
            (
                DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                "/elements/2/updated",
            ),
        ]
    );
}

#[test]
fn normalization_order_respects_source_member_position() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"same","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"seed":0,"id":"same","type":"rectangle","x":50,"width":40,"height":40,"updated":1}],"appState":{},"files":{}}"##;

    let outcome = render_svg(source);
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        vec!["/elements/1/seed", "/elements/1/id"]
    );
}

#[test]
fn diagnostic_limit_crossing_preserves_earlier_normalization_repairs() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("normalization vector is admitted");
    let limits = ResourceLimits::builder()
        .max_diagnostics(2)
        .build()
        .expect("diagnostic boundary is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the third ordered repair crosses the diagnostic limit");

    assert_eq!(error.code(), ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED);
    assert_eq!(error.json_pointer(), "/elements/0/updated");
    assert_eq!(
        error
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        vec![
            DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
            DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
        ]
    );
}

#[test]
fn normalized_record_limit_precedes_identity_generation_and_diagnostics() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("normalization vector is admitted");
    let limits = ResourceLimits::builder()
        .max_working_bytes(5_823)
        .build()
        .expect("working-byte boundary is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the normalized record is rejected before repair work");

    assert_eq!(error.code(), ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED);
    assert_eq!(error.element_id(), None);
    assert_eq!(error.json_pointer(), "/elements/0");
    assert!(error.report().is_empty());
}

#[test]
fn normalization_repairs_are_format_stable() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":30,"height":20,"seed":0}],"appState":{},"files":{}}"##;

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let first = render(source, format);
        let second = render(source, format);
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert!(first
            .report()
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.format() == format));
    }
}

#[test]
fn deleted_rectangles_remain_silent_and_produce_a_meaningful_empty_artifact() {
    let deleted = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[{"type":"future","isDeleted":true,"seed":0},{"type":"selection"}],"appState":{},"files":{}}"##,
    );
    let empty = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"##,
    );

    assert_eq!(deleted.artifact().bytes(), empty.artifact().bytes());
    assert!(deleted.report().is_empty());
}

#[test]
fn later_rectangle_resource_diagnostics_keep_the_later_source_location() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"id":"second","type":"rectangle","x":100,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("resource vector is admitted");
    let limits = ResourceLimits::builder()
        .max_coordinate_magnitude(99.0)
        .build()
        .expect("coordinate boundary is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("later local loss retains a meaningful artifact");
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("one later coordinate is diagnosed");
    };
    assert_eq!(diagnostic.json_pointer(), "/elements/1/x");
    assert_eq!(diagnostic.element_id(), Some("second"));
}

#[test]
fn final_diagnostic_order_is_by_source_owner_not_execution_phase() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","type":"rectangle","x":100,"width":40,"height":40,"seed":1,"updated":1},{"id":"second","type":"rectangle","x":0,"width":40,"height":40,"seed":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("ordering vector is admitted");
    let limits = ResourceLimits::builder()
        .max_coordinate_magnitude(99.0)
        .build()
        .expect("coordinate boundary is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("ordered local losses retain a meaningful artifact");
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        vec!["/elements/0/x", "/elements/1/updated"]
    );
}

#[test]
fn multiple_pdf_rectangles_keep_independent_opacity_resources() {
    let outcome = render(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","type":"rectangle","x":0,"width":40,"height":40,"opacity":50,"seed":1,"updated":1},{"id":"second","type":"rectangle","x":50,"width":40,"height":40,"opacity":75,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        OutputFormat::Pdf,
    );
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/ExtGState << /GS1 7 0 R /GS2 8 0 R >>"));
    assert_eq!(pdf.matches("/GS1 gs").count(), 1);
    assert_eq!(pdf.matches("/GS2 gs").count(), 1);
}

fn render_svg(source: &[u8]) -> excalidraw_render::RenderOutcome {
    render(source, OutputFormat::Svg)
}

fn render(source: &[u8], format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let scene = SceneDocument::parse(source).expect("normalization vector is admitted");
    let request = RenderRequest::builder(format)
        .build()
        .expect("default render request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("normalization vector renders")
}
