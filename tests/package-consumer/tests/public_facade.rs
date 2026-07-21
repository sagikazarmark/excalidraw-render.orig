use excalidraw_render::{
    license_bundle, DiagnosticCode, OutputFormat, RenderRequest, Renderer, SceneDocument,
};
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const SCENE: &[u8] =
    include_bytes!("../../conformance/cases/focused/rectangle-outline/scene.excalidraw");
const EXPECTED_REPORT: &[u8] =
    include_bytes!("../../conformance/cases/focused/rectangle-outline/expected-report.json");
const LICENSE_BUNDLE_SHA256: &str =
    "a3236318603e4035951ad166ad827ef9fd3a98d9eeae9996e804aa6048d20077";
const FALLBACK_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"future","type":"future","x":0,"y":0,"width":200,"height":60,"angle":0.2,"opacity":40,"updated":1},null],"appState":{},"files":{}}"#;
const FALLBACK_HASHES: &[(OutputFormat, &str, &str)] = &[
    (
        OutputFormat::Svg,
        "796a5cd9d9089e8f12bf53292e928559f7dd97ea91c4cbc7dccdab09de2ffb88",
        "579d8c97406c5bbea1c25cd3638e2de5f96508adf89141ea4f17be5e9a7340f3",
    ),
    (
        OutputFormat::Png,
        "2a118b5686dd514fc2b709716d7b0661de971c63ca910cb88852e392ff662068",
        "2557fb95fd4972c5ceaea2b0b1b8a3a73f157c2fe147bd5d348be0f639186922",
    ),
    (
        OutputFormat::Pdf,
        "42a499315a51de73083be0ce4dc6d9c95009d9d501e15920c27b29211558cc52",
        "737130c9efaa38327d3fe31ecdeddc35ad72a01ea8dc7c11e3a57942c4867247",
    ),
];

struct ArtifactExpectation {
    format: OutputFormat,
    bytes: &'static [u8],
}

const ARTIFACT_EXPECTATIONS: &[ArtifactExpectation] = &[
    ArtifactExpectation {
        format: OutputFormat::Svg,
        bytes: include_bytes!("../expected/rectangle.svg"),
    },
    ArtifactExpectation {
        format: OutputFormat::Png,
        bytes: include_bytes!("../expected/rectangle.png"),
    },
    ArtifactExpectation {
        format: OutputFormat::Pdf,
        bytes: include_bytes!("../expected/rectangle.pdf"),
    },
];

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_preserves_artifact_byte_stability() {
    let scene = SceneDocument::parse(SCENE).expect("the approved rectangle scene is admitted");
    let renderer = Renderer::new();

    let forward_round = render_round(&renderer, &scene, ARTIFACT_EXPECTATIONS.iter());
    let mut reverse_round = render_round(&renderer, &scene, ARTIFACT_EXPECTATIONS.iter().rev());
    reverse_round.reverse();

    assert!(
        forward_round == reverse_round,
        "repeated render calls or format order changed their bytes"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_exposes_the_exact_license_bundle() {
    let bundle: &'static str = license_bundle();

    assert_eq!(
        format!("{:x}", Sha256::digest(bundle.as_bytes())),
        LICENSE_BUNDLE_SHA256
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_enforces_the_frozen_scene_surface() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"libraryItems":{"deep":{"future":true}},"unknown":{"deep":{"future":true}},"elements":[],"appState":{},"files":{}}"#,
    )
    .expect("the forward-compatibility scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("the default request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("an ignored property retains a meaningful empty artifact");

    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("the unknown subtree is one policy occurrence");
    };
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY
    );
    assert_eq!(diagnostic.json_pointer(), "/unknown");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_renders_fallbacks_with_cross_target_stable_bytes() {
    let scene = SceneDocument::parse(FALLBACK_SCENE).expect("the fallback scene is admitted");
    for (format, artifact_sha256, report_sha256) in FALLBACK_HASHES {
        let request = RenderRequest::builder(*format)
            .build()
            .expect("the fallback request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("fallback and omission retain a meaningful artifact");
        let report = serde_json::to_vec(outcome.report()).expect("the report serializes");

        assert_eq!(
            format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
            *artifact_sha256,
            "{} fallback artifact changed",
            format.as_str()
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&report)),
            *report_sha256,
            "{} fallback report changed",
            format.as_str()
        );
    }
}

fn render_round<'a>(
    renderer: &Renderer,
    scene: &SceneDocument,
    expectations: impl Iterator<Item = &'a ArtifactExpectation>,
) -> Vec<(Vec<u8>, Vec<u8>)> {
    expectations
        .map(|expectation| {
            let request = RenderRequest::builder(expectation.format)
                .build()
                .expect("the default format request is valid");
            let outcome = renderer
                .render(scene, &request)
                .expect("the approved rectangle renders through the public facade");
            let report = serde_json::to_vec(outcome.report()).expect("the report serializes");

            assert_eq!(outcome.artifact().format(), expectation.format);
            assert_eq!(
                outcome.artifact().media_type(),
                expectation.format.media_type()
            );
            assert!(
                outcome.artifact().bytes() == expectation.bytes,
                "{} artifact bytes differ from the native expectation",
                expectation.format.as_str()
            );
            assert_eq!(report, EXPECTED_REPORT);

            (outcome.artifact().bytes().to_vec(), report)
        })
        .collect()
}
