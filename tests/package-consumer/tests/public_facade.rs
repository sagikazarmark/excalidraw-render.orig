use base64::Engine as _;
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
const TEXT_SCENE: &[u8] =
    include_bytes!("../../conformance/cases/focused/excalifont-text/scene.excalidraw");
const SOURCE_IMAGE_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=","created":1}}}"#;
const NATIVE_NESTED_SVG_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGxpbmVhckdyYWRpZW50IGlkPSJnIj48c3RvcCBzdG9wLWNvbG9yPSIjZjAwIi8+PHN0b3Agb2Zmc2V0PSIxIiBzdG9wLWNvbG9yPSIjMDBmIi8+PC9saW5lYXJHcmFkaWVudD48Y2xpcFBhdGggaWQ9ImMiPjxjaXJjbGUgY3g9IjEwIiBjeT0iNSIgcj0iNSIvPjwvY2xpcFBhdGg+PC9kZWZzPjxyZWN0IHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCIgZmlsbD0idXJsKCNnKSIgY2xpcC1wYXRoPSJ1cmwoI2MpIi8+PC9zdmc+","created":1}}}"##;
const RASTER_FALLBACK_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGZpbHRlciBpZD0iYmx1ciIgeD0iMCIgeT0iMCIgd2lkdGg9IjEwMCUiIGhlaWdodD0iMTAwJSI+PGZlR2F1c3NpYW5CbHVyIHN0ZERldmlhdGlvbj0iMSIvPjwvZmlsdGVyPjwvZGVmcz48cmVjdCB3aWR0aD0iMjAiIGhlaWdodD0iMTAiIGZpbGw9IiMwMGZmMDAiIGZpbHRlcj0idXJsKCNibHVyKSIvPjwvc3ZnPg==","created":1}}}"##;
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
        "255af458e3d0f21dff334dfac5329702f3c9c6b2f8c2d1f804b8dbebca2f272b",
        "737130c9efaa38327d3fe31ecdeddc35ad72a01ea8dc7c11e3a57942c4867247",
    ),
];
const PDF_RISK_SET: &[(&str, &[u8], &str, &str)] = &[
    (
        "selectable-text",
        TEXT_SCENE,
        "f91353d32aa938ca0255660d8078e5067c7f0c6ac83f59c21001e0acfb50e642",
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b",
    ),
    (
        "source-image",
        SOURCE_IMAGE_SCENE,
        "720658af87ff8f784d304cc462784a439ffcda57d7c4f909747877843caf5788",
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b",
    ),
    (
        "native-nested-svg",
        NATIVE_NESTED_SVG_SCENE,
        "52119410fe97ab23d16471edd2bdcc0fc5d08e39a04c4d69f07c3365475d9a36",
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b",
    ),
    (
        "isolated-raster-fallback",
        RASTER_FALLBACK_SCENE,
        "a65af342a74440b56a3a22abd06c99e2948a10df5b7cbb51b410143a10e9b90f",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
];
const PDF_FALLBACK_FAMILIES: &[(&str, &str, &str, &str)] = &[
    (
        "blur",
        r#"<feGaussianBlur stdDeviation="1"/>"#,
        "a65af342a74440b56a3a22abd06c99e2948a10df5b7cbb51b410143a10e9b90f",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "convolution",
        r#"<feConvolveMatrix order="3" kernelMatrix="0 -1 0 -1 5 -1 0 -1 0"/>"#,
        "2ce05131423f4b568c5fd20865662b9f3baddad3bcf7da71a2e1109c86a47339",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "morphology",
        r#"<feMorphology operator="dilate" radius="1"/>"#,
        "2ce05131423f4b568c5fd20865662b9f3baddad3bcf7da71a2e1109c86a47339",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "displacement",
        r#"<feTurbulence baseFrequency="0.1" numOctaves="1" result="noise"/><feDisplacementMap in="SourceGraphic" in2="noise" scale="2"/>"#,
        "9d210c1760b455276fe1230b1688a712c967424a5ba5b2d752bb100c0ce98745",
        "a39664c294a12af60d1c206a0772a78ee73fccc76833b995d02cfdec127433c7",
    ),
    (
        "turbulence",
        r#"<feTurbulence baseFrequency="0.1" numOctaves="1"/>"#,
        "55fa1734c32a1728575c916556d7ca5b307cbd4fff896f789ee86ee4dd7cc871",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "lighting",
        r#"<feDiffuseLighting surfaceScale="2"><feDistantLight azimuth="45" elevation="60"/></feDiffuseLighting>"#,
        "517123d65c95f1d4b3fd989d11df1fb5564a1e6c66a6301ec763937adbd0e108",
        "130660f6f7c8091d99e2976c06043b1fa657c21159e98104dd50c5cd2128c14b",
    ),
    (
        "post-composite-color",
        r#"<feColorMatrix type="saturate" values="0.5"/>"#,
        "b7fbce430f1393a8e3f8510498a831bcf1d19bee374ecbcddff5f8b5b3901f20",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "filter-blend",
        r#"<feBlend in="SourceGraphic" in2="SourceGraphic" mode="overlay"/>"#,
        "2ce05131423f4b568c5fd20865662b9f3baddad3bcf7da71a2e1109c86a47339",
        "7867f55c810c54ffd7d671366bad5db3de847cf44fecae17fd93323df6b56c31",
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_preserves_pdf_risk_set_bytes_across_targets() {
    for (name, scene_bytes, artifact_sha256, report_sha256) in PDF_RISK_SET {
        let scene = SceneDocument::parse(scene_bytes).expect("the PDF risk-set scene is admitted");
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .build()
            .expect("the PDF request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("the PDF risk-set scene renders");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("the PDF risk-set scene renders repeatedly");
        let report = serde_json::to_vec(first.report()).expect("the report serializes");
        let actual_artifact_sha256 = format!("{:x}", Sha256::digest(first.artifact().bytes()));
        let actual_report_sha256 = format!("{:x}", Sha256::digest(&report));
        assert_eq!(first.artifact().bytes(), second.artifact().bytes(), "{name}");
        assert_eq!(first.report(), second.report(), "{name}");
        assert_eq!(
            actual_artifact_sha256,
            *artifact_sha256,
            "{name} PDF artifact changed"
        );
        assert_eq!(
            actual_report_sha256,
            *report_sha256,
            "{name} PDF report changed"
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_preserves_every_pdf_fallback_family_across_targets() {
    for (name, primitives, artifact_sha256, report_sha256) in PDF_FALLBACK_FAMILIES {
        let nested = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%">{primitives}</filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#effect)"/></svg>"##
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(nested);
        let source = format!(
            r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}}],"appState":{{}},"files":{{"asset":{{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}}}}}"#
        );
        let scene = SceneDocument::parse(source.as_bytes()).expect("fallback-family scene is admitted");
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .build()
            .expect("fallback-family request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("fallback-family scene renders");
        let report = serde_json::to_vec(outcome.report()).expect("fallback report serializes");
        assert_eq!(
            format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
            *artifact_sha256,
            "{name} PDF artifact changed"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&report)),
            *report_sha256,
            "{name} PDF report changed"
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
