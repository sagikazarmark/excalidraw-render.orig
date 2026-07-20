use excalidraw_render::{license_bundle, OutputFormat, RenderRequest, Renderer, SceneDocument};
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
