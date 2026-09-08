use excalidraw_render::{
    DiagnosticCode, DiagnosticEffect, ErrorCode, FrameOptions, OutputFormat, RenderRequest,
    Renderer, ResourceLimits, SceneDocument, SupplementalAssets,
};

use crate::common::render::render_source;

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
fn legacy_stroke_sharpness_renders_as_the_official_roundness_migration_in_every_format() {
    // Every kind whose roundness the revision-1 manifest reads, saved before
    // `roundness` existed. The official restore gives the adaptive-radius
    // kinds (rectangle, embeddable, iframe, image) `{ type: 1 }` and the rest
    // `{ type: 2 }`; "sharp" stays `null`. The image kind is covered by the
    // normalization unit tests because it needs a payload to render silently.
    let legacy = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"rect","type":"rectangle","x":0,"y":0,"width":120,"height":80,"seed":1,"updated":1,"strokeSharpness":"round"},
        {"id":"embed","type":"embeddable","x":160,"y":0,"width":120,"height":80,"seed":2,"updated":1,"strokeSharpness":"round"},
        {"id":"frame","type":"iframe","x":320,"y":0,"width":120,"height":80,"seed":3,"updated":1,"strokeSharpness":"round"},
        {"id":"diamond","type":"diamond","x":0,"y":120,"width":120,"height":80,"seed":4,"updated":1,"strokeSharpness":"round"},
        {"id":"line","type":"line","x":160,"y":120,"width":120,"height":80,"seed":5,"updated":1,"points":[[0,0],[60,80],[120,0]],"strokeSharpness":"round"},
        {"id":"arrow","type":"arrow","x":320,"y":120,"width":120,"height":80,"seed":6,"updated":1,"points":[[0,0],[60,80],[120,0]],"strokeSharpness":"round"},
        {"id":"sharp","type":"rectangle","x":0,"y":240,"width":120,"height":80,"seed":7,"updated":1,"strokeSharpness":"sharp"}
    ],"appState":{},"files":{}}"##;
    let explicit = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"rect","type":"rectangle","x":0,"y":0,"width":120,"height":80,"seed":1,"updated":1,"roundness":{"type":1}},
        {"id":"embed","type":"embeddable","x":160,"y":0,"width":120,"height":80,"seed":2,"updated":1,"roundness":{"type":1}},
        {"id":"frame","type":"iframe","x":320,"y":0,"width":120,"height":80,"seed":3,"updated":1,"roundness":{"type":1}},
        {"id":"diamond","type":"diamond","x":0,"y":120,"width":120,"height":80,"seed":4,"updated":1,"roundness":{"type":2}},
        {"id":"line","type":"line","x":160,"y":120,"width":120,"height":80,"seed":5,"updated":1,"points":[[0,0],[60,80],[120,0]],"roundness":{"type":2}},
        {"id":"arrow","type":"arrow","x":320,"y":120,"width":120,"height":80,"seed":6,"updated":1,"points":[[0,0],[60,80],[120,0]],"roundness":{"type":2}},
        {"id":"sharp","type":"rectangle","x":0,"y":240,"width":120,"height":80,"seed":7,"updated":1,"roundness":null}
    ],"appState":{},"files":{}}"##;
    let unmigrated = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"rect","type":"rectangle","x":0,"y":0,"width":120,"height":80,"seed":1,"updated":1},
        {"id":"embed","type":"embeddable","x":160,"y":0,"width":120,"height":80,"seed":2,"updated":1},
        {"id":"frame","type":"iframe","x":320,"y":0,"width":120,"height":80,"seed":3,"updated":1},
        {"id":"diamond","type":"diamond","x":0,"y":120,"width":120,"height":80,"seed":4,"updated":1},
        {"id":"line","type":"line","x":160,"y":120,"width":120,"height":80,"seed":5,"updated":1,"points":[[0,0],[60,80],[120,0]]},
        {"id":"arrow","type":"arrow","x":320,"y":120,"width":120,"height":80,"seed":6,"updated":1,"points":[[0,0],[60,80],[120,0]]},
        {"id":"sharp","type":"rectangle","x":0,"y":240,"width":120,"height":80,"seed":7,"updated":1}
    ],"appState":{},"files":{}}"##;

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let legacy = render_source(legacy, format);
        let explicit = render_source(explicit, format);
        let unmigrated = render_source(unmigrated, format);

        assert_eq!(
            legacy.artifact().bytes(),
            explicit.artifact().bytes(),
            "{format:?}"
        );
        assert_ne!(
            legacy.artifact().bytes(),
            unmigrated.artifact().bytes(),
            "{format:?}: the migration must change the rendering"
        );
        assert!(
            legacy.report().is_empty(),
            "{format:?}: the migration is silent"
        );
    }
}

#[test]
fn legacy_font_strings_render_as_the_official_font_size_and_family_migration_in_every_format() {
    // Text saved before `fontSize`/`fontFamily` existed carries a CSS-like
    // `font` string. The official restore splits it into parseFloat(size) and
    // FONT_FAMILY[name] (Excalifont for an unknown name) and lets it override
    // both modern fields whenever the key is present. The restored line height
    // still derives from the element's own fontSize, so the "both" element's
    // explicit twin spells that line height out (50 / 1 line / 40 = 1.25).
    let legacy = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"virgil","type":"text","x":0,"y":0,"width":200,"height":35,"seed":1,"updated":1,"text":"Virgil","font":"28px Virgil"},
        {"id":"helvetica","type":"text","x":0,"y":60,"width":200,"height":20,"seed":2,"updated":1,"text":"Helvetica","font":"16px Helvetica"},
        {"id":"cascadia","type":"text","x":0,"y":120,"width":200,"height":25,"seed":3,"updated":1,"text":"Cascadia","font":"20px Cascadia"},
        {"id":"nunito","type":"text","x":0,"y":180,"width":200,"height":30,"seed":4,"updated":1,"text":"Nunito","font":"24px Nunito"},
        {"id":"unknown","type":"text","x":0,"y":240,"width":200,"height":25,"seed":5,"updated":1,"text":"Arial","font":"20px Arial"},
        {"id":"both","type":"text","x":0,"y":300,"width":200,"height":50,"seed":6,"updated":1,"text":"Both","font":"28px Virgil","fontSize":40,"fontFamily":2}
    ],"appState":{},"files":{}}"##;
    let explicit = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"virgil","type":"text","x":0,"y":0,"width":200,"height":35,"seed":1,"updated":1,"text":"Virgil","fontSize":28,"fontFamily":1},
        {"id":"helvetica","type":"text","x":0,"y":60,"width":200,"height":20,"seed":2,"updated":1,"text":"Helvetica","fontSize":16,"fontFamily":2},
        {"id":"cascadia","type":"text","x":0,"y":120,"width":200,"height":25,"seed":3,"updated":1,"text":"Cascadia","fontSize":20,"fontFamily":3},
        {"id":"nunito","type":"text","x":0,"y":180,"width":200,"height":30,"seed":4,"updated":1,"text":"Nunito","fontSize":24,"fontFamily":6},
        {"id":"unknown","type":"text","x":0,"y":240,"width":200,"height":25,"seed":5,"updated":1,"text":"Arial","fontSize":20,"fontFamily":5},
        {"id":"both","type":"text","x":0,"y":300,"width":200,"height":50,"seed":6,"updated":1,"text":"Both","fontSize":28,"fontFamily":1,"lineHeight":1.25}
    ],"appState":{},"files":{}}"##;
    let unmigrated = br##"{"type":"excalidraw","version":2,"elements":[
        {"id":"virgil","type":"text","x":0,"y":0,"width":200,"height":35,"seed":1,"updated":1,"text":"Virgil"},
        {"id":"helvetica","type":"text","x":0,"y":60,"width":200,"height":20,"seed":2,"updated":1,"text":"Helvetica"},
        {"id":"cascadia","type":"text","x":0,"y":120,"width":200,"height":25,"seed":3,"updated":1,"text":"Cascadia"},
        {"id":"nunito","type":"text","x":0,"y":180,"width":200,"height":30,"seed":4,"updated":1,"text":"Nunito"},
        {"id":"unknown","type":"text","x":0,"y":240,"width":200,"height":25,"seed":5,"updated":1,"text":"Arial"},
        {"id":"both","type":"text","x":0,"y":300,"width":200,"height":50,"seed":6,"updated":1,"text":"Both","fontSize":40,"fontFamily":2}
    ],"appState":{},"files":{}}"##;

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let legacy = render_source(legacy, format);
        let explicit = render_source(explicit, format);
        let unmigrated = render_source(unmigrated, format);

        assert_eq!(
            legacy.artifact().bytes(),
            explicit.artifact().bytes(),
            "{format:?}"
        );
        assert_ne!(
            legacy.artifact().bytes(),
            unmigrated.artifact().bytes(),
            "{format:?}: the migration must change the rendering"
        );
        // Helvetica text reports its Liberation Sans substitution either way;
        // the migration itself adds nothing.
        assert_eq!(
            legacy.report(),
            explicit.report(),
            "{format:?}: the migration is silent"
        );
        assert!(legacy.report().diagnostics().iter().all(
            |diagnostic| diagnostic.code() == DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED
        ));
    }
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
fn fractional_index_repair_preserves_source_paint_order_silently() {
    let ordered = br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","index":"a0","type":"rectangle","x":0,"y":0,"width":80,"height":60,"seed":1,"updated":1},{"id":"second","index":"a1","type":"rectangle","x":20,"y":20,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let descending = br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","index":"a1","type":"rectangle","x":0,"y":0,"width":80,"height":60,"seed":1,"updated":1},{"id":"second","index":"a0","type":"rectangle","x":20,"y":20,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"##;

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let ordered = render_source(ordered, format);
        let repaired = render_source(descending, format);
        assert_eq!(ordered.artifact().bytes(), repaired.artifact().bytes());
        assert!(repaired.report().is_empty());
    }
}

#[test]
fn invalid_frame_rendering_controls_are_repaired_for_empty_scenes() {
    let outcome = render_svg(
        br##"{"type":"excalidraw","version":2,"elements":[],"appState":{"frameRendering":{"enabled":false,"clip":"yes"}},"files":{}}"##,
    );

    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("the invalid frame control produces one diagnostic");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE
    );
    assert_eq!(diagnostic.json_pointer(), "/appState/frameRendering/clip");
    assert_eq!(
        diagnostic.effects(),
        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual]
    );
}

#[test]
fn assigned_element_repairs_are_reported_before_later_rendering_slices() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","width":10,"height":10,"points":"invalid","seed":1,"updated":1},{"id":"image","type":"image","width":10,"height":10,"status":"future","seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("assigned normalization vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default render request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable elements retain a compatibility artifact");

    assert_eq!(
        non_replacement_diagnostics(&outcome)
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        [
            "/elements/0/points",
            "/elements/1/status",
            "/elements/1/fileId",
        ]
    );
}

#[test]
fn supplemental_authority_participates_in_image_crop_normalization() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","crop":{"x":0,"y":0,"width":10,"height":10,"naturalWidth":10},"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("supplemental crop vector is admitted");
    let assets = SupplementalAssets::builder()
        .asset("asset", [0])
        .build()
        .expect("supplemental assets are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("render request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image retains a compatibility artifact");

    assert_eq!(non_replacement_diagnostics(&outcome).len(), 2);
    assert_eq!(
        non_replacement_diagnostics(&outcome)[0].json_pointer(),
        "/elements/0/crop/naturalHeight"
    );
    assert_eq!(
        non_replacement_diagnostics(&outcome)[1].code(),
        DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT
    );
}

#[test]
fn disabled_frame_names_suppress_name_value_diagnostics() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"frame","type":"frame","width":10,"height":10,"name":3,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("disabled frame-name vector is admitted");
    for frame_options in [
        FrameOptions::new(true, true, false, true),
        FrameOptions::new(false, true, true, true),
    ] {
        let request = RenderRequest::builder(OutputFormat::Svg)
            .frame_options(frame_options)
            .build()
            .expect("render request is valid");

        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("known unavailable frame retains a compatibility artifact");

        assert!(non_replacement_diagnostics(&outcome).is_empty());
    }
}

#[test]
fn request_frame_names_override_disabled_scene_frame_names() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"frame","type":"frame","width":10,"height":10,"name":3,"seed":1,"updated":1}],"appState":{"frameRendering":{"name":false}},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("frame-name override vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("render request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable frame retains a compatibility artifact");

    assert_eq!(non_replacement_diagnostics(&outcome).len(), 1);
    assert_eq!(
        non_replacement_diagnostics(&outcome)[0].json_pointer(),
        "/elements/0/name"
    );
}

#[test]
fn normalization_reports_follow_preserved_root_owner_order() {
    let source = br##"{"type":"excalidraw","version":2,"files":{"asset":{}},"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","status":"future","seed":1,"updated":1}],"appState":{}}"##;
    let scene = SceneDocument::parse(source).expect("root-order vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("render request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image retains a compatibility artifact");

    assert_eq!(
        non_replacement_diagnostics(&outcome)
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        [
            "/files/asset/id",
            "/files/asset/dataURL",
            "/files/asset/created",
            "/elements/0/status",
        ]
    );
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
fn app_state_diagnostics_follow_source_member_order_across_surface_and_normalization() {
    // The unmatched members come from the scene-surface classification and
    // the frame-rendering control from compatibility normalization; spec §11.3
    // orders them by the `appState` member they belong to regardless of which
    // layer reported them. Found by the scene fuzz.
    let source = br##"{"type":"excalidraw","version":2,"elements":[],"appState":{"frameRendering":{"clip":"yes"},"alpha":1,"beta":2},"files":{}}"##;

    let outcome = render_svg(source);
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        vec![
            "/appState/frameRendering/clip",
            "/appState/alpha",
            "/appState/beta",
        ]
    );
}

#[test]
fn a_coordinate_range_crossing_follows_its_source_member_position() {
    // The coordinate-range crossing is reported by the render layer and the
    // `y` repair by normalization; `width` is the later source member, so its
    // crossing sorts after the repair (spec §11.3). Found by the scene fuzz.
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"r","type":"rectangle","x":0,"y":"wrong","width":500,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("normalization vector is admitted");
    let limits = ResourceLimits::builder()
        .max_coordinate_magnitude(400.0)
        .build()
        .expect("local coordinate limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("resource-limited request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("the crossing recovers locally");
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        vec![
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/elements/0/y",
            ),
            (
                DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED,
                "/elements/0/width",
            ),
        ]
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
    let render_with_working_bytes = |max_working_bytes: u64| {
        let limits = ResourceLimits::builder()
            .max_working_bytes(max_working_bytes)
            .build()
            .expect("working-byte boundary is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("resource-limited request is valid");
        Renderer::new().render(&scene, &request)
    };
    let attempted_working_bytes = |error: &excalidraw_render::RenderError| {
        assert_eq!(error.code(), ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED);
        let descriptor = serde_json::to_value(error.descriptor()).expect("error serializes");
        descriptor["details"]["measurement"]["value"]
            .as_u64()
            .expect("working-byte attempt is an integer")
    };

    // Walk the working-byte charges from one byte upward, adopting each refused
    // attempt as the next ceiling, until the crossing lands on the element's
    // normalized record: the first charge at /elements/0. Its size is read off
    // the accounting rather than pinned as a literal; the walk is bounded by
    // the few charges that precede the record.
    let mut ceiling = 1;
    let mut record_crossing = None;
    for _ in 0..8 {
        let error = render_with_working_bytes(ceiling)
            .expect_err("a working-byte ceiling below the element record is fatal");
        let attempted = attempted_working_bytes(&error);
        assert!(
            attempted > ceiling,
            "a crossing reports the attempt it refused"
        );
        if error.json_pointer() == "/elements/0" {
            record_crossing = Some((error, attempted));
            break;
        }
        ceiling = attempted;
    }
    let (error, record_bytes) =
        record_crossing.expect("the normalized record is charged within eight ceilings");

    assert_eq!(error.element_id(), None);
    assert!(
        error.report().is_empty(),
        "the record is rejected before identity generation and repair diagnostics"
    );
    // At exactly its own size the record is admitted: whatever crosses next,
    // if anything does, attempts more.
    if let Err(later) = render_with_working_bytes(record_bytes) {
        assert!(
            attempted_working_bytes(&later) > record_bytes,
            "the record is admitted at equality"
        );
    }
}

#[test]
fn normalization_repairs_are_format_stable() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":30,"height":20,"seed":0}],"appState":{},"files":{}}"##;

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let first = render_source(source, format);
        let second = render_source(source, format);
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
fn diagnostic_member_order_is_stable_across_normalization_and_semantics() {
    let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"same","type":"rectangle","x":100,"width":40,"height":40,"seed":1}],"appState":{},"files":{}}"##;
    let scene = SceneDocument::parse(source).expect("cross-phase ordering vector is admitted");
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
        ["/elements/0/x", "/elements/0/updated"]
    );
}

#[test]
fn multiple_pdf_rectangles_keep_independent_opacity_resources() {
    let outcome = render_source(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","type":"rectangle","x":0,"width":40,"height":40,"opacity":50,"seed":1,"updated":1},{"id":"second","type":"rectangle","x":50,"width":40,"height":40,"opacity":75,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        OutputFormat::Pdf,
    );
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/ExtGState << /GS1 7 0 R /GS2 8 0 R >>"));
    assert_eq!(pdf.matches("/GS1 gs").count(), 1);
    assert_eq!(pdf.matches("/GS2 gs").count(), 1);
}

fn non_replacement_diagnostics(
    outcome: &excalidraw_render::RenderOutcome,
) -> Vec<&excalidraw_render::CompatibilityDiagnostic> {
    outcome
        .report()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() != DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
        })
        .collect()
}

fn render_svg(source: &[u8]) -> excalidraw_render::RenderOutcome {
    render_source(source, OutputFormat::Svg)
}
