use base64::Engine as _;
use excalidraw_render::{
    license_bundle, DiagnosticCode, FrameRenderRequest, OutputFormat, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const SCENE: &[u8] =
    include_bytes!("../../conformance/cases/focused/rectangle-outline/scene.excalidraw");
const EXPECTED_REPORT: &[u8] =
    include_bytes!("../../conformance/cases/focused/rectangle-outline/expected-report-svg.json");
const LICENSE_BUNDLE_SHA256: &str =
    "cadd243b2228954efb8f61f62d492bcde65a23f8e7e96a2db5a6cc42ce78a9f2";
const FALLBACK_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"future","type":"future","x":0,"y":0,"width":200,"height":60,"angle":0.2,"opacity":40,"updated":1},null],"appState":{},"files":{}}"#;
const TEXT_SCENE: &[u8] =
    include_bytes!("../../conformance/cases/focused/excalifont-text/scene.excalidraw");
const SOURCE_IMAGE_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=","created":1}}}"#;
const NATIVE_NESTED_SVG_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGxpbmVhckdyYWRpZW50IGlkPSJnIj48c3RvcCBzdG9wLWNvbG9yPSIjZjAwIi8+PHN0b3Agb2Zmc2V0PSIxIiBzdG9wLWNvbG9yPSIjMDBmIi8+PC9saW5lYXJHcmFkaWVudD48Y2xpcFBhdGggaWQ9ImMiPjxjaXJjbGUgY3g9IjEwIiBjeT0iNSIgcj0iNSIvPjwvY2xpcFBhdGg+PC9kZWZzPjxyZWN0IHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCIgZmlsbD0idXJsKCNnKSIgY2xpcC1wYXRoPSJ1cmwoI2MpIi8+PC9zdmc+","created":1}}}"##;
const RASTER_FALLBACK_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGZpbHRlciBpZD0iYmx1ciIgeD0iMCIgeT0iMCIgd2lkdGg9IjEwMCUiIGhlaWdodD0iMTAwJSI+PGZlR2F1c3NpYW5CbHVyIHN0ZERldmlhdGlvbj0iMSIvPjwvZmlsdGVyPjwvZGVmcz48cmVjdCB3aWR0aD0iMjAiIGhlaWdodD0iMTAiIGZpbGw9IiMwMGZmMDAiIGZpbHRlcj0idXJsKCNibHVyKSIvPjwvc3ZnPg==","created":1}}}"##;
const NOISY_RASTER_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":128,"height":64,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAICAYAAADwdn+XAAAB+ElEQVR42g3FbUyMAQDA8Wfo5Vg4t2jR8AFX0vIynXa5LqMdOeQ0Sa/bSRvhWmuVlA+ksjIXadMdcyvt6BxRpq22W63RqkdsbDHr3G5ys+JW6Lo/vy8/Iembipz3K7md/IUeWQzVvh/Uzvj5uKAZ8ckI6tR5jvjC2WO9ynRoHtV5WjQR+wlQnsQcXIDQY//DtjYrGR4Ta6O8tA0PMeUNxhAbiE5ZwcMdLxitjaPccZbOiGcEfTAxscHJ8iV2BrYfQGgsWo3S3oTamsmNmlRkb6WcPiFlIkjAJVpRdm1BlxyLIFRSIWlGEW6jyjqH8fAwqusNCO53azga8wDHpnEUNisF/g66XMX/v4dUW0eT0UyZXoZ5tJ6QsRwq5c9ZdbOGi6EeXO2lCJJHLQwmTZOufsl8VCtprV7k4yKJpp28yVzPZW0k3SW5pHUnUhY3w1BJJXPxOZx6/ZMVG/8inFeMEfF9M04xC3vAZ6ptEvbOxLMwWsX9r14+5e8iMDeeAQlkxxipkbswGrJZOnsGcVk6QrvGh33qKetSfqFOuENL33GqUvOJzDiEu/gVd+t+c0tTj6XBw2KdgWtbdyPRW+h1Gwg55kHYVzTCBVEk7NJBJqMLCSv04dI7OedfxGSPlI5+B+VXUohMGOJxVinexkFmazvpz7bQl9DLP6p9QFNSPs87AAAAAElFTkSuQmCC","created":1}}}"#;
const FRAME_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"first-frame","type":"frame","name":"Public One","x":10,"y":20,"width":80,"height":60,"updated":1},{"id":"first-child","type":"rectangle","x":20,"y":30,"width":20,"height":20,"frameId":"first-frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},{"id":"second-frame","type":"magicframe","name":"Public Two","x":110,"y":20,"width":50,"height":40,"updated":1},{"id":"second-child","type":"ellipse","x":120,"y":30,"width":20,"height":15,"frameId":"second-frame","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1}],"appState":{},"files":{}}"##;
const FALLBACK_HASHES: &[(OutputFormat, &str, &str)] = &[
    (
        // The SVG artifact moved with #150/#151: the placeholder label's
        // `font-family` carries the consumer fallback list after
        // `ExcalidrawRender-9`; then with #151 item 1, when `<metadata>`
        // began carrying the Liberation Sans attribution and the OFL body as
        // two notices instead of the full OFL notice under the family's
        // license identity. Then with #172: the placeholder is rotated 0.2
        // rad, and its box corners now rotate in the reference's
        // `pointRotateRads` order, which lands the scene's bottom edge and
        // the layout origin one ulp away (`viewBox` height
        // 118.53786082948673, `translate(... 29.268930414743366)`); the PNG
        // truncates to the same rows and is unchanged. Then with the subset
        // profile token naming wuff 0.2.9, skera 0.8.0 and ttf2woff2 0.14.1,
        // which moves the embedded subset's identity and notice. The report
        // is unchanged. Again with skera 0.9.0, which also serializes the
        // subset's GSUB and GPOS lists in declaration order.
        OutputFormat::Svg,
        "ca6b8973eb9b53a928b1be8a29cefb0c54d9d46e0f56ffa90a365996e51f63c0",
        "579d8c97406c5bbea1c25cd3638e2de5f96508adf89141ea4f17be5e9a7340f3",
    ),
    (
        OutputFormat::Png,
        "997f544675c9330b82daf013d021a46b189b84ff82f2c0bacfe955523c9cc0a3",
        "2557fb95fd4972c5ceaea2b0b1b8a3a73f157c2fe147bd5d348be0f639186922",
    ),
    (
        // The PDF artifact moved with #133/#150 (ADR-0008): every stream is
        // Flate, the Liberation Sans program carries its subset tag and the
        // trailer its content-derived /ID; then with #151 item 1, when the
        // XMP manifest began carrying the attribution and license body
        // notices; then with #172, the same one-ulp page height and origin
        // as the SVG above; then with ADR-0021, when Krilla became the PDF
        // writer. The report is unchanged.
        OutputFormat::Pdf,
        "c6db8497bc8550d8514e45dc1be3f708f698f859fea3658bb6d6cc003a0e00b6",
        "737130c9efaa38327d3fe31ecdeddc35ad72a01ea8dc7c11e3a57942c4867247",
    ),
];
// Every PDF artifact digest here and in PDF_FALLBACK_FAMILIES moved with
// #133/#150 (ADR-0008: Flate streams, subset-tagged chunk programs, the
// trailer /ID); the one font-bearing PDF here moved again with #151 item 1
// (the XMP manifest's attribution and license body notices). The nested-SVG
// PDFs moved again with #209 (ADR-0021): after the usvg 0.47 -> 0.45
// bridge each fallback image sits in an identity `q 1 0 0 1 0 0 cm Q` group,
// and a `8.9e-16` residue in the native nested SVG prints as `0`. Every PDF
// digest here and `expected/rectangle.pdf` moved again when Krilla became the
// PDF writer (ADR-0021). No report digest moved.
const PDF_RISK_SET: &[(&str, &[u8], &str, &str)] = &[
    (
        "selectable-text",
        TEXT_SCENE,
        "32f075d0cca826163720973c5ef06560dc169a8b572a678f58da30925d2c19b7",
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b",
    ),
    (
        "source-image",
        SOURCE_IMAGE_SCENE,
        // c9fc43d corrected image orientation, explicit interpolation, and
        // direct ICCBased image color spaces. The report is unchanged.
        "91cd0d8b71788df8989fd7107e946c3571891d372405a37f10730e5276bf83dd",
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b",
    ),
    (
        "native-nested-svg",
        NATIVE_NESTED_SVG_SCENE,
        "44d782533d4273324f33d5b7d91e5bfc21e849b5827b58081d1536aaf58925e2",
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b",
    ),
    (
        "isolated-raster-fallback",
        RASTER_FALLBACK_SCENE,
        "767770926e5f9f8f0e5eafb9a00e4ae7073a6ca401af589f6d0944a6746e8a23",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
];
const PDF_FALLBACK_FAMILIES: &[(&str, &str, &str, &str)] = &[
    (
        "blur",
        r#"<feGaussianBlur stdDeviation="1"/>"#,
        "767770926e5f9f8f0e5eafb9a00e4ae7073a6ca401af589f6d0944a6746e8a23",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "convolution",
        r#"<feConvolveMatrix order="3" kernelMatrix="0 -1 0 -1 5 -1 0 -1 0"/>"#,
        "8be83985eb902e84cd7a76521ead3692ff7d2880912d6bb9ef6ba299aee45086",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "morphology",
        r#"<feMorphology operator="dilate" radius="1"/>"#,
        "8be83985eb902e84cd7a76521ead3692ff7d2880912d6bb9ef6ba299aee45086",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "displacement",
        r#"<feTurbulence baseFrequency="0.1" numOctaves="1" result="noise"/><feDisplacementMap in="SourceGraphic" in2="noise" scale="2"/>"#,
        "a7aa9e03c47092f8c7be054a4947aa8215223936100b88bcff751ed303e43344",
        "a39664c294a12af60d1c206a0772a78ee73fccc76833b995d02cfdec127433c7",
    ),
    (
        "turbulence",
        r#"<feTurbulence baseFrequency="0.1" numOctaves="1"/>"#,
        // The old pin predated the shared raster-closure pipeline (9712833);
        // the current one was reconfirmed at c5fec02.
        "174f12465f67aa2146e1b97ddf988640d5f3822b319f7f1198f004005c9611d9",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "lighting",
        r#"<feDiffuseLighting surfaceScale="2"><feDistantLight azimuth="45" elevation="60"/></feDiffuseLighting>"#,
        "fbda47cc3b3f5f04e4ff9bdadbfcb6f74b658fac99eaea3429f1d0ff6bf4548d",
        "130660f6f7c8091d99e2976c06043b1fa657c21159e98104dd50c5cd2128c14b",
    ),
    (
        "post-composite-color",
        r#"<feColorMatrix type="saturate" values="0.5"/>"#,
        "841a1685f9ebd96ace4918aa34694574706f82e6c0454b5202649146da14db69",
        "38c2893242335a84eb4573a767700fe3b0357fb6bbe237726e146c9950319c9e",
    ),
    (
        "filter-blend",
        r#"<feBlend in="SourceGraphic" in2="SourceGraphic" mode="overlay"/>"#,
        "8be83985eb902e84cd7a76521ead3692ff7d2880912d6bb9ef6ba299aee45086",
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
fn packaged_svg_font_subsets_are_cross_target_stable() {
    let scene = SceneDocument::parse(TEXT_SCENE).expect("the typography scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("the SVG request is valid");
    let first = Renderer::new()
        .render(&scene, &request)
        .expect("the typography SVG renders");
    let second = Renderer::new()
        .render(&scene, &request)
        .expect("the typography SVG renders repeatedly");
    let svg = std::str::from_utf8(first.artifact().bytes()).expect("SVG is UTF-8");

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    // Moved with 5173fae, which stopped separating a multi-line text's tspans
    // with a raw newline (a rendered character under xml:space="preserve");
    // the in-crate Excalifont digests were repinned then, this one was not
    // (#116). Moved again with #150/#151, when `font-family` became the list
    // `Excalifont, Xiaolai, Segoe UI Emoji, sans-serif`, and with #151 item 1,
    // when `<metadata>` began carrying the Excalifont attribution and the OFL
    // body as two notices. ADR-0003's identity refresh changes the embedded
    // subset's name table and its profile-derived identity/notice, as does
    // the token naming wuff 0.2.9, skera 0.8.0 and ttf2woff2 0.14.1, and
    // again with skera 0.9.0, which also reorders the subset's GSUB and GPOS.
    assert_eq!(
        lower_hex(&Sha256::digest(first.artifact().bytes())),
        "abae5fa96e2fe6449bfd5307b5682e2b11ebbce0ec3013c9ba2ab98b09ad7fac"
    );
    assert!(svg.contains("\"asset_identity\":\"license.ofl-1.1\""));
    assert_eq!(svg.matches("<style class=\"style-fonts\">").count(), 1);
    assert!(svg.contains("svg-font-subset@sha256:"));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_renders_discovered_frames_directly_and_lazily() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("the frame scene is admitted");
    let frames = scene.frames().collect::<Vec<_>>();
    let renderer = Renderer::new();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = FrameRenderRequest::builder(format)
            .build()
            .expect("the frame request is valid");
        let direct = frames
            .iter()
            .copied()
            .map(|frame| {
                renderer
                    .render_frame(frame, &request)
                    .expect("the frame renders directly")
            })
            .collect::<Vec<_>>();
        let mut cursor = renderer
            .render_frames(&scene, frames.iter().rev().copied(), &request)
            .expect("the lazy frame selection is valid");
        for (expected_frame, direct) in frames.iter().zip(&direct) {
            let entry = cursor.next().expect("the lazy result exists");
            let lazy = entry.result().as_ref().expect("the frame renders lazily");

            assert_eq!(entry.frame().ordinal(), expected_frame.ordinal());
            assert_eq!(direct.artifact().bytes(), lazy.artifact().bytes());
            assert_eq!(direct.report(), lazy.report());
            assert_eq!(direct.profile_identity().reference_profile_digest(), lazy.profile_identity().reference_profile_digest());
        }
        assert!(cursor.next().is_none());
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_preserves_direct_and_lazy_frame_errors() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("the frame scene is admitted");
    let frames = scene.frames().collect::<Vec<_>>();
    let limits = ResourceLimits::builder()
        .max_elements(1)
        .build()
        .expect("the low element limit is valid");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("the frame request is valid");
    let renderer = Renderer::new();

    let direct = frames
        .iter()
        .copied()
        .map(|frame| {
            renderer
                .render_frame(frame, &request)
                .expect_err("the direct render crosses the element limit")
        })
        .collect::<Vec<_>>();
    let mut cursor = renderer
        .render_frames(&scene, frames.iter().rev().copied(), &request)
        .expect("semantic limits do not prevent cursor construction");
    for (expected_frame, direct) in frames.iter().zip(direct) {
        let entry = cursor.next().expect("the lazy result exists");
        let actual_ordinal = entry.frame().ordinal();
        let lazy = entry
            .into_result()
            .expect_err("the lazy render crosses the element limit");

        assert_eq!(actual_ordinal, expected_frame.ordinal());
        assert_eq!(direct.descriptor(), lazy.descriptor());
        assert_eq!(direct.report(), lazy.report());
    }
    assert!(cursor.next().is_none());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_exposes_the_exact_license_bundle() {
    let bundle: &'static str = license_bundle();

    assert_eq!(
        lower_hex(&Sha256::digest(bundle.as_bytes())),
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
            lower_hex(&Sha256::digest(outcome.artifact().bytes())),
            *artifact_sha256,
            "{} fallback artifact changed",
            format.as_str()
        );
        assert_eq!(
            lower_hex(&Sha256::digest(&report)),
            *report_sha256,
            "{} fallback report changed",
            format.as_str()
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_preserves_png_scanline_filter_bytes_across_targets() {
    // A noisy raster upscaled by the renderer makes the PNG encoder take its
    // predictive scanline filters and exercises the deflate compressor on
    // dense data, while the padding rows stay unfiltered. Together with the
    // flat rectangle expectation this pins both filter paths and the
    // compressor on every supported target (#113). The digest moved with
    // #165: the 16x8 image at 128x64 is now resampled once per destination
    // pixel centre, as the canvas resamples it, instead of at every subpixel
    // centre of the supersample, so its interior pixels changed.
    // c9fc43d subsequently matched the canvas's four-bit bilinear weights.
    // That pre-existing change was missing from this companion snapshot.
    let scene =
        SceneDocument::parse(NOISY_RASTER_SCENE).expect("the noisy raster scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("the PNG request is valid");
    let first = Renderer::new()
        .render(&scene, &request)
        .expect("the noisy raster scene renders");
    let second = Renderer::new()
        .render(&scene, &request)
        .expect("the noisy raster scene renders repeatedly");
    let report = serde_json::to_vec(first.report()).expect("the report serializes");

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    // 148x84 RGBA scanlines occupy 49,812 bytes when stored uncompressed.
    assert!(
        first.artifact().bytes().len() * 2 < 148 * (84 * 4 + 1),
        "the PNG is not deflate-compressed"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(first.artifact().bytes())),
        "46501d857058f88ab033e3e5b2fbf1d1b3e1bdef71510420d907b2b40a8dcea7"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(&report)),
        "1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b"
    );
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
        let actual_artifact_sha256 = lower_hex(&Sha256::digest(first.artifact().bytes()));
        let actual_report_sha256 = lower_hex(&Sha256::digest(&report));
        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{name}"
        );
        assert_eq!(first.report(), second.report(), "{name}");
        assert_eq!(
            actual_artifact_sha256, *artifact_sha256,
            "{name} PDF artifact changed"
        );
        assert_eq!(
            actual_report_sha256, *report_sha256,
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
        let scene =
            SceneDocument::parse(source.as_bytes()).expect("fallback-family scene is admitted");
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .build()
            .expect("fallback-family request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("fallback-family scene renders");
        let report = serde_json::to_vec(outcome.report()).expect("fallback report serializes");
        assert_eq!(
            lower_hex(&Sha256::digest(outcome.artifact().bytes())),
            *artifact_sha256,
            "{name} PDF artifact changed"
        );
        assert_eq!(
            lower_hex(&Sha256::digest(&report)),
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

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
