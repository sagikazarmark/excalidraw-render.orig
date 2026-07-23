use excalidraw_render::{
    DiagnosticCode, DiagnosticEffect, ErrorCode, OutputFormat, Recovery, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};

#[test]
fn revision_1_global_policy_is_exact_and_suppresses_unknown_descendants() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "scrollToContent":{"future":{"deep":true}},
        "libraryItems":{"future":{"deep":true}},
        "unknownRoot":{"first":1,"nested":{"second":2}},
        "elements":[],
        "appState":{
            "activeTool":{"future":{"deep":true}},
            "gridModeEnabled":"preserved but irrelevant",
            "unknownAppState":{"first":1,"nested":{"second":2}}
        },
        "files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("ignored properties retain a meaningful empty artifact");

    let diagnostics = outcome.report().diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/unknownRoot",
            ),
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/appState/unknownAppState",
            ),
        ]
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.effects()
            == [
                DiagnosticEffect::Visual,
                DiagnosticEffect::Semantic,
                DiagnosticEffect::Metadata,
            ]
    }));
}

#[test]
fn revision_1_element_policy_uses_exact_kind_and_record_boundaries() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[
            {"type":"future","isDeleted":true,"unknown":{"deep":true}},
            {"type":"selection","unknown":{"deep":true}},
            {"id":"unknown","type":"future","x":0,"y":0,"width":20,"height":10,"unknown":{"deep":true}},
            {"id":"malformed","type":"","x":30,"y":0,"width":20,"height":10,"unknown":{"deep":true}},
            null
        ],
        "appState":{},
        "files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("bounded fallback and diagnosed omission retain an artifact");

    let diagnostics = outcome.report().diagnostics();
    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE,
                "/elements/2",
            ),
            (
                DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT,
                "/elements/3",
            ),
            (
                DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT,
                "/elements/4",
            ),
        ]
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| match diagnostic.details() {
                Some(excalidraw_render::ErrorDetails::Compatibility { recovery, .. }) => *recovery,
                details => panic!("unexpected compatibility details: {details:?}"),
            })
            .collect::<Vec<_>>(),
        [
            Recovery::Placeholder,
            Recovery::Placeholder,
            Recovery::Omitted
        ]
    );
}

#[test]
fn revision_1_element_policy_applies_kind_specific_inherited_participation() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"image",
            "type":"image",
            "width":20,
            "height":10,
            "fileId":"asset",
            "strokeColor":3,
            "roughness":"future",
            "seed":0,
            "updated":1,
            "customData":{"future":{"deep":true}},
            "unknown":{"future":{"deep":true}}
        }],
        "appState":{},
        "files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let assets = SupplementalAssets::builder()
        .asset("asset", [0])
        .build()
        .expect("supplemental asset is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image uses its compatibility fallback");

    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic, unsupported_asset] = diagnostics.as_slice() else {
        panic!("the unmatched property and sniffed supplemental payload participate");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY
    );
    assert_eq!(diagnostic.json_pointer(), "/elements/0/unknown");
    assert_eq!(
        unsupported_asset.code(),
        DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT
    );
    assert_eq!(unsupported_asset.json_pointer(), "");
}

#[test]
fn revision_1_file_participation_requires_a_manifested_image_reference() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"rectangle",
            "type":"rectangle",
            "width":20,
            "height":10,
            "fileId":"asset",
            "seed":1,
            "updated":1
        }],
        "appState":{},
        "files":{
            "asset":{"id":3,"created":"invalid","unknown":{"deep":true}},
            "unreferenced":false
        }
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("an unmatched rectangle property does not activate a file record");

    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("only the unmatched rectangle property participates");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY
    );
    assert_eq!(diagnostic.json_pointer(), "/elements/0/fileId");
}

#[test]
fn revision_1_referenced_file_classifies_known_and_unknown_children() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"image",
            "type":"image",
            "width":20,
            "height":10,
            "fileId":"a/b~c",
            "updated":1
        }],
        "appState":{},
        "files":{
            "a/b~c":{"not":"used"},
            "unreferenced":false
        }
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image uses its compatibility fallback");

    let diagnostics = non_replacement_diagnostics(&outcome);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/files/a~1b~0c/id",
            ),
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/files/a~1b~0c/not",
            ),
            (
                DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
                "/files/a~1b~0c/dataURL",
            ),
            (
                DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                "/files/a~1b~0c/created",
            ),
        ]
    );
}

#[test]
fn revision_1_wholly_malformed_referenced_file_suppresses_descendants() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"asset","updated":1}],
        "appState":{},"files":{"asset":false,"unreferenced":{"unknown":{"deep":true}}}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("diagnosed malformed asset retains an artifact");

    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("the malformed authoritative record is one occurrence");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::ASSET_MALFORMED_FILE_RECORD
    );
    assert_eq!(diagnostic.json_pointer(), "/files/asset");
    assert!(matches!(
        diagnostic.details(),
        Some(excalidraw_render::ErrorDetails::Asset {
            asset_id,
            recovery: Recovery::Omitted,
            ..
        }) if asset_id == "asset"
    ));
}

#[test]
fn revision_1_empty_referenced_file_key_is_malformed() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"","updated":1}],
        "appState":{},"files":{"":{"dataURL":"data:image/png;base64,AA=="}}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("diagnosed malformed asset retains an artifact");
    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("the empty authoritative key is one malformed record");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::ASSET_MALFORMED_FILE_RECORD
    );
    assert_eq!(diagnostic.json_pointer(), "/files/");
}

#[test]
fn revision_1_malformed_error_image_selects_the_error_recipe_in_png() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"asset","status":"error","updated":1}],
        "appState":{},"files":{"asset":false}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("default request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("diagnosed malformed image retains an artifact");
    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("the malformed authoritative record is one occurrence");
    };
    assert!(matches!(
        diagnostic.details(),
        Some(excalidraw_render::ErrorDetails::Asset {
            recovery: Recovery::Placeholder,
            recipe_id: Some(recipe),
            ..
        }) if recipe.starts_with("official.image-error@")
    ));
}

#[test]
fn revision_1_scene_file_remains_authoritative_over_supplement() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"asset","status":"saved","updated":1}],
        "appState":{},
        "files":{"asset":{"id":"asset","created":1,"dataURL":"data:image/png;base64,AA=="}}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let assets = SupplementalAssets::builder()
        .asset("asset", [1])
        .build()
        .expect("supplemental asset is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("default request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image retains an artifact");
    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic, unsupported_asset] = diagnostics.as_slice() else {
        panic!("the conflict and authoritative payload sniff are retained");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::ASSET_SUPPLEMENT_CONFLICT);
    assert_eq!(diagnostic.json_pointer(), "/files/asset");
    assert_eq!(
        unsupported_asset.code(),
        DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT
    );
    assert_eq!(unsupported_asset.json_pointer(), "/files/asset/dataURL");
}

#[test]
fn revision_1_file_values_use_asset_specific_manifest_recoveries() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"asset","status":"saved","updated":1}],
        "appState":{},
        "files":{"asset":{"id":"asset","created":1,"mimeType":"image/future","dataURL":"https://example.com/image.png"}}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image retains an artifact");
    assert_eq!(
        non_replacement_diagnostics(&outcome)
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH,
                "/files/asset/mimeType",
            ),
            (
                DiagnosticCode::IMAGE_FORBIDDEN_RESOURCE,
                "/files/asset/dataURL",
            ),
        ]
    );
    assert!(non_replacement_diagnostics(&outcome)
        .iter()
        .all(|diagnostic| {
            matches!(
                diagnostic.details(),
                Some(excalidraw_render::ErrorDetails::Asset { asset_id, .. }) if asset_id == "asset"
            )
        }));
}

#[test]
fn revision_1_recognized_nested_record_keeps_unknown_children_separate() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[{
            "id":"image",
            "type":"image",
            "width":20,
            "height":10,
            "fileId":"asset",
            "crop":{
                "x":0,
                "y":0,
                "width":10,
                "height":10,
                "naturalWidth":10,
                "naturalHeight":10,
                "firstUnknown":{"deep":true},
                "secondUnknown":{"deep":true}
            },
            "updated":1
        }],
        "appState":{},
        "files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let assets = SupplementalAssets::builder()
        .asset("asset", [0])
        .build()
        .expect("supplemental asset is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable image retains an artifact");

    assert_eq!(
        non_replacement_diagnostics(&outcome)
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/elements/0/crop/firstUnknown",
            ),
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/elements/0/crop/secondUnknown",
            ),
            (DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT, ""),
        ]
    );
}

#[test]
fn revision_1_frame_rendering_keeps_unknown_children_separate() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[],
        "appState":{"frameRendering":{
            "enabled":true,
            "firstUnknown":{"deep":true},
            "secondUnknown":{"deep":true}
        }},
        "files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("ignored frame controls retain a meaningful empty artifact");

    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        [
            "/appState/frameRendering/firstUnknown",
            "/appState/frameRendering/secondUnknown",
        ]
    );
}

#[test]
fn revision_1_frame_children_precede_later_app_state_members() {
    let source = br#"{
        "type":"excalidraw","version":2,"elements":[],
        "appState":{
            "frameRendering":{"unknown":{"deep":true}},
            "laterUnknown":{"deep":true}
        },
        "files":{}
    }"#;
    let outcome = render_svg(source);
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.json_pointer())
            .collect::<Vec<_>>(),
        ["/appState/frameRendering/unknown", "/appState/laterUnknown",]
    );
}

#[test]
fn revision_1_linear_nested_records_classify_the_narrowest_unknown_child() {
    let source = br#"{
        "type":"excalidraw",
        "version":2,
        "elements":[
            {
                "id":"line",
                "type":"line",
                "width":10,
                "height":10,
                "points":[[0,0,99],[10,10]],
                "startBinding":{"elementId":"target","focus":0,"gap":0,"unknown":{"deep":true}},
                "seed":1,
                "updated":1
            },
            {"id":"target","type":"rectangle","width":10,"height":10,"seed":1,"updated":1}
        ],
        "appState":{},
        "files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable line retains an artifact");

    assert_eq!(
        non_replacement_diagnostics(&outcome)
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/elements/0/points/0/2",
            ),
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                "/elements/0/startBinding/unknown",
            ),
        ]
    );
}

#[test]
fn revision_1_participating_bound_record_classifies_unknown_children() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"owner","type":"rectangle","width":20,"height":10,"boundElements":[{"id":"target","type":"text","unknown":{"deep":true}}],"seed":1,"updated":1},
            {"id":"target","type":"text","updated":1}
        ],
        "appState":{},"files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable bound text retains an artifact");
    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("the bound record has one unmatched child");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY
    );
    assert_eq!(
        diagnostic.json_pointer(),
        "/elements/0/boundElements/0/unknown"
    );
}

#[test]
fn revision_1_supported_value_uses_its_manifested_recovery() {
    let invalid = br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"rectangle","type":"rectangle","width":20,"height":10,"strokeWidth":0,"seed":1,"updated":1}],
        "appState":{},"files":{}
    }"##;
    let defaulted = br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"rectangle","type":"rectangle","width":20,"height":10,"strokeWidth":2,"seed":1,"updated":1}],
        "appState":{},"files":{}
    }"##;

    let invalid = render_svg(invalid);
    let defaulted = render_svg(defaulted);
    assert_eq!(invalid.artifact().bytes(), defaulted.artifact().bytes());
    let [diagnostic] = invalid.report().diagnostics() else {
        panic!("invalid supported value has one manifested recovery");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE
    );
    assert_eq!(diagnostic.json_pointer(), "/elements/0/strokeWidth");
    assert_eq!(
        diagnostic.effects(),
        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual]
    );
}

#[test]
fn revision_1_invalid_css_color_uses_the_manifested_color_recovery() {
    let invalid = br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"rectangle","type":"rectangle","width":20,"height":10,"strokeColor":"not-a-color","seed":1,"updated":1}],
        "appState":{},"files":{}
    }"##;
    let defaulted = br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"rectangle","type":"rectangle","width":20,"height":10,"strokeColor":"#1e1e1e","seed":1,"updated":1}],
        "appState":{},"files":{}
    }"##;
    let invalid = render_svg(invalid);
    let defaulted = render_svg(defaulted);
    assert_eq!(invalid.artifact().bytes(), defaulted.artifact().bytes());
    let [diagnostic] = invalid.report().diagnostics() else {
        panic!("invalid color has one manifested recovery");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::COLOR_INVALID);
    assert_eq!(diagnostic.json_pointer(), "/elements/0/strokeColor");
}

#[test]
fn revision_1_known_unavailable_kind_still_applies_exact_field_policy() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"text","type":"text","fontSize":0,"updated":1}],
        "appState":{},"files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("known unavailable text retains an artifact");
    let diagnostics = non_replacement_diagnostics(&outcome);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("the malformed text value has one field recovery");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE
    );
    assert_eq!(diagnostic.json_pointer(), "/elements/0/fontSize");
}

#[test]
fn revision_1_value_rules_are_format_and_kind_aware() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"diamond","type":"diamond","width":20,"height":10,"seed":0,"link":"javascript:alert(1)","updated":1}],
        "appState":{},"files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    for (format, expected_codes) in [
        (
            OutputFormat::Svg,
            vec![
                DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                DiagnosticCode::LINK_UNSAFE_OMITTED,
            ],
        ),
        (
            OutputFormat::Png,
            vec![DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED],
        ),
    ] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("default request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("known unavailable diamond retains an artifact");
        assert_eq!(
            non_replacement_diagnostics(&outcome)
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            expected_codes
        );
    }
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
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("default request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("rectangle policy vector renders")
}

#[test]
fn revision_1_unknown_subtree_is_one_diagnostic_resource_occurrence() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "firstUnknown":{"many":[{"deep":1},{"deep":2}]},
        "secondUnknown":{"many":[{"deep":3},{"deep":4}]},
        "elements":[],"appState":{},"files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let limits = ResourceLimits::builder()
        .max_diagnostics(1)
        .build()
        .expect("diagnostic limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second property occurrence crosses the diagnostic limit");

    assert_eq!(error.code(), ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED);
    assert_eq!(error.json_pointer(), "/secondUnknown");
    let [retained] = error.report().diagnostics() else {
        panic!("the first whole subtree is one retained occurrence");
    };
    assert_eq!(retained.json_pointer(), "/firstUnknown");
}

#[test]
fn revision_1_nested_diagnostic_flood_is_preflight_bounded() {
    let source = br#"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"line","type":"line","points":[[0,0,2,3,4],[10,10]],"seed":1,"updated":1}],
        "appState":{},"files":{}
    }"#;
    let scene = SceneDocument::parse(source).expect("policy vector is admitted");
    let limits = ResourceLimits::builder()
        .max_diagnostics(1)
        .build()
        .expect("diagnostic limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second nested occurrence crosses the diagnostic limit");
    assert_eq!(error.code(), ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED);
    assert_eq!(error.json_pointer(), "/elements/0/points/0/3");
    let [retained] = error.report().diagnostics() else {
        panic!("the first nested occurrence is retained");
    };
    assert_eq!(retained.json_pointer(), "/elements/0/points/0/2");
}
