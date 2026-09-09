//! The revision-1 scene-surface policy where it meets another stage: a
//! supplemental or authoritative payload the asset stage sniffs after the
//! surface classified the record, the element-record boundaries the render
//! stage's placeholders and omissions follow, and the preflight diagnostic
//! limits. The policy's own classification of unknown and malformed members is
//! tested through the normalization observer in `scene_surface.rs`.

use excalidraw_render::{
    DiagnosticCode, ErrorCode, OutputFormat, Recovery, RenderRequest, Renderer, ResourceLimits,
    SceneDocument, SupplementalAssets,
};

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
