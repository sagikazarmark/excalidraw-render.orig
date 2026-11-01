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
/// The golden record of every registered export target
/// (`tests/conformance/golden`): the packaged facade must reproduce the
/// rectangle case's recorded bytes, which anchors this consumer to the same
/// record the render tier judges and `just update-golden` rewrites.
const GOLDEN_RECORD: &str = include_str!("../../conformance/golden/native.json");
const GOLDEN_CASE: &str = "focused.rectangle-outline.v1";
const FALLBACK_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"future","type":"future","x":0,"y":0,"width":200,"height":60,"angle":0.2,"opacity":40,"updated":1},null],"appState":{},"files":{}}"#;
const TEXT_SCENE: &[u8] =
    include_bytes!("../../conformance/cases/focused/excalifont-text/scene.excalidraw");
const SOURCE_IMAGE_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=","created":1}}}"#;
const NATIVE_NESTED_SVG_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGxpbmVhckdyYWRpZW50IGlkPSJnIj48c3RvcCBzdG9wLWNvbG9yPSIjZjAwIi8+PHN0b3Agb2Zmc2V0PSIxIiBzdG9wLWNvbG9yPSIjMDBmIi8+PC9saW5lYXJHcmFkaWVudD48Y2xpcFBhdGggaWQ9ImMiPjxjaXJjbGUgY3g9IjEwIiBjeT0iNSIgcj0iNSIvPjwvY2xpcFBhdGg+PC9kZWZzPjxyZWN0IHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCIgZmlsbD0idXJsKCNnKSIgY2xpcC1wYXRoPSJ1cmwoI2MpIi8+PC9zdmc+","created":1}}}"##;
const RASTER_FALLBACK_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGZpbHRlciBpZD0iYmx1ciIgeD0iMCIgeT0iMCIgd2lkdGg9IjEwMCUiIGhlaWdodD0iMTAwJSI+PGZlR2F1c3NpYW5CbHVyIHN0ZERldmlhdGlvbj0iMSIvPjwvZmlsdGVyPjwvZGVmcz48cmVjdCB3aWR0aD0iMjAiIGhlaWdodD0iMTAiIGZpbGw9IiMwMGZmMDAiIGZpbHRlcj0idXJsKCNibHVyKSIvPjwvc3ZnPg==","created":1}}}"##;
const NOISY_RASTER_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":128,"height":64,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAICAYAAADwdn+XAAAB+ElEQVR42g3FbUyMAQDA8Wfo5Vg4t2jR8AFX0vIynXa5LqMdOeQ0Sa/bSRvhWmuVlA+ksjIXadMdcyvt6BxRpq22W63RqkdsbDHr3G5ys+JW6Lo/vy8/Iembipz3K7md/IUeWQzVvh/Uzvj5uKAZ8ckI6tR5jvjC2WO9ynRoHtV5WjQR+wlQnsQcXIDQY//DtjYrGR4Ta6O8tA0PMeUNxhAbiE5ZwcMdLxitjaPccZbOiGcEfTAxscHJ8iV2BrYfQGgsWo3S3oTamsmNmlRkb6WcPiFlIkjAJVpRdm1BlxyLIFRSIWlGEW6jyjqH8fAwqusNCO53azga8wDHpnEUNisF/g66XMX/v4dUW0eT0UyZXoZ5tJ6QsRwq5c9ZdbOGi6EeXO2lCJJHLQwmTZOufsl8VCtprV7k4yKJpp28yVzPZW0k3SW5pHUnUhY3w1BJJXPxOZx6/ZMVG/8inFeMEfF9M04xC3vAZ6ptEvbOxLMwWsX9r14+5e8iMDeeAQlkxxipkbswGrJZOnsGcVk6QrvGh33qKetSfqFOuENL33GqUvOJzDiEu/gVd+t+c0tTj6XBw2KdgWtbdyPRW+h1Gwg55kHYVzTCBVEk7NJBJqMLCSv04dI7OedfxGSPlI5+B+VXUohMGOJxVinexkFmazvpz7bQl9DLP6p9QFNSPs87AAAAAElFTkSuQmCC","created":1}}}"#;
const FRAME_SCENE: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"first-frame","type":"frame","name":"Public One","x":10,"y":20,"width":80,"height":60,"updated":1},{"id":"first-child","type":"rectangle","x":20,"y":30,"width":20,"height":20,"frameId":"first-frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},{"id":"second-frame","type":"magicframe","name":"Public Two","x":110,"y":20,"width":50,"height":40,"updated":1},{"id":"second-child","type":"ellipse","x":120,"y":30,"width":20,"height":15,"frameId":"second-frame","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1}],"appState":{},"files":{}}"##;
/// The nested-SVG filter primitives whose PDF export takes the raster
/// fallback, or an appearance-equivalent native lowering, one family each.
const PDF_FALLBACK_FAMILIES: &[(&str, &str)] = &[
    ("blur", r#"<feGaussianBlur stdDeviation="1"/>"#),
    (
        "convolution",
        r#"<feConvolveMatrix order="3" kernelMatrix="0 -1 0 -1 5 -1 0 -1 0"/>"#,
    ),
    (
        "morphology",
        r#"<feMorphology operator="dilate" radius="1"/>"#,
    ),
    (
        "displacement",
        r#"<feTurbulence baseFrequency="0.1" numOctaves="1" result="noise"/><feDisplacementMap in="SourceGraphic" in2="noise" scale="2"/>"#,
    ),
    (
        "turbulence",
        r#"<feTurbulence baseFrequency="0.1" numOctaves="1"/>"#,
    ),
    (
        "lighting",
        r#"<feDiffuseLighting surfaceScale="2"><feDistantLight azimuth="45" elevation="60"/></feDiffuseLighting>"#,
    ),
    (
        "post-composite-color",
        r#"<feColorMatrix type="saturate" values="0.5"/>"#,
    ),
    (
        "filter-blend",
        r#"<feBlend in="SourceGraphic" in2="SourceGraphic" mode="overlay"/>"#,
    ),
];
/// The PDF scenes whose writer paths differ: selectable text, a raster image,
/// natively lowered nested SVG, and an isolated raster fallback.
const PDF_RISK_SET: &[(&str, &[u8])] = &[
    ("selectable-text", TEXT_SCENE),
    ("source-image", SOURCE_IMAGE_SCENE),
    ("native-nested-svg", NATIVE_NESTED_SVG_SCENE),
    ("isolated-raster-fallback", RASTER_FALLBACK_SCENE),
];

/// The golden record's artifact digest and length for the rectangle case's
/// default variant in `format`.
fn golden_rectangle(format: OutputFormat) -> (String, usize) {
    let record: serde_json::Value =
        serde_json::from_str(GOLDEN_RECORD).expect("the golden record parses");
    let expectation = record["expectations"]
        .as_array()
        .expect("golden expectations")
        .iter()
        .find(|expectation| {
            expectation["key"]["case"] == GOLDEN_CASE
                && expectation["key"]["variant"] == "default"
                && expectation["key"]["format"] == format.as_str()
        })
        .unwrap_or_else(|| {
            panic!(
                "the golden record holds {GOLDEN_CASE} in {}",
                format.as_str()
            )
        });
    let bytes = &expectation["sample"]["outcome"]["bytes"];
    (
        bytes["sha256"].as_str().expect("digest").to_owned(),
        bytes["byte_length"].as_u64().expect("length") as usize,
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_reproduces_the_golden_rectangle() {
    let scene = SceneDocument::parse(SCENE).expect("the approved rectangle scene is admitted");
    let renderer = Renderer::new();
    let formats = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

    let forward_round = render_round(&renderer, &scene, formats.iter().copied());
    let mut reverse_round = render_round(&renderer, &scene, formats.iter().rev().copied());
    reverse_round.reverse();

    assert!(
        forward_round == reverse_round,
        "repeated render calls or format order changed their bytes"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_svg_font_subsets_are_embedded_deterministically() {
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
            assert_eq!(
                direct.profile_identity().reference_profile_digest(),
                lazy.profile_identity().reference_profile_digest()
            );
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
fn packaged_public_facade_exposes_the_license_bundle() {
    let bundle: &'static str = license_bundle();

    assert!(bundle.contains("SIL Open Font License"));
    assert!(bundle.contains("Excalifont"));
    assert!(bundle.contains("Liberation"));
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
fn packaged_public_facade_renders_fallbacks_deterministically() {
    let scene = SceneDocument::parse(FALLBACK_SCENE).expect("the fallback scene is admitted");
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("the fallback request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("fallback and omission retain a meaningful artifact");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("fallback and omission render repeatedly");

        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert!(
            !first.report().diagnostics().is_empty(),
            "{}: the unknown element and the null entry are diagnosed",
            format.as_str()
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_compresses_png_scanlines() {
    // A noisy raster upscaled by the renderer makes the PNG encoder take its
    // predictive scanline filters and exercises the deflate compressor on
    // dense data, while the padding rows stay unfiltered; the flat rectangle
    // reproduced from the golden record covers the other filter path. The
    // bytes themselves are the golden record's concern (#113).
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
    assert!(first.report().diagnostics().is_empty(), "{report:?}");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_renders_the_pdf_risk_set_deterministically() {
    for (name, scene_bytes) in PDF_RISK_SET {
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
        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{name}"
        );
        assert_eq!(first.report(), second.report(), "{name}");
        assert!(
            first.artifact().bytes().starts_with(b"%PDF-"),
            "{name} is a PDF"
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn packaged_public_facade_renders_every_pdf_fallback_family_deterministically() {
    for (name, primitives) in PDF_FALLBACK_FAMILIES {
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
        let first = Renderer::new()
            .render(&scene, &request)
            .expect("fallback-family scene renders");
        let second = Renderer::new()
            .render(&scene, &request)
            .expect("fallback-family scene renders repeatedly");
        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{name} PDF artifact is deterministic"
        );
        assert_eq!(first.report(), second.report(), "{name}");
    }
}

fn render_round(
    renderer: &Renderer,
    scene: &SceneDocument,
    formats: impl Iterator<Item = OutputFormat>,
) -> Vec<(Vec<u8>, Vec<u8>)> {
    formats
        .map(|format| {
            let request = RenderRequest::builder(format)
                .build()
                .expect("the default format request is valid");
            let outcome = renderer
                .render(scene, &request)
                .expect("the approved rectangle renders through the public facade");
            let report = serde_json::to_vec(outcome.report()).expect("the report serializes");
            let (sha256, byte_length) = golden_rectangle(format);

            assert_eq!(outcome.artifact().format(), format);
            assert_eq!(outcome.artifact().media_type(), format.media_type());
            assert_eq!(
                outcome.artifact().bytes().len(),
                byte_length,
                "{} artifact length differs from the golden record",
                format.as_str()
            );
            assert_eq!(
                lower_hex(&Sha256::digest(outcome.artifact().bytes())),
                sha256,
                "{} artifact bytes differ from the golden record",
                format.as_str()
            );
            assert_eq!(report, EXPECTED_REPORT);

            (outcome.artifact().bytes().to_vec(), report)
        })
        .collect()
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
