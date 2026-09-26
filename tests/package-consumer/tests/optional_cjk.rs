//! Run in this isolated workspace against archives, with and without `cjk`.
//! Workspace-wide renderer tests can otherwise unify the optional payload back in.
use excalidraw_render::{
    DiagnosticCode, ErrorCode, ErrorDetails, OutputFormat, RenderErrorCategory, RenderRequest,
    Renderer, SceneDocument,
};
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn scene(family: u32, text: &str) -> SceneDocument {
    scene_with_original(family, text, text)
}

fn scene_with_original(family: u32, text: &str, original: &str) -> SceneDocument {
    SceneDocument::parse(
        &serde_json::to_vec(&json!({
            "type": "excalidraw", "version": 2,
            "elements": [{
                "id": "text", "type": "text", "width": 120, "height": 30,
                "fontSize": 20, "fontFamily": family, "text": text,
                "originalText": original, "updated": 1
            }],
            "appState": {}, "files": {}
        }))
        .unwrap(),
    )
    .unwrap()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn trigger_without_declared_font_demand_keeps_missing_glyph_recovery() {
    // U+20000 activates Excalifont's CJK trigger but no Xiaolai chunk declares it.
    let scene = scene(5, "\u{20000}");
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        assert!(outcome
            .report()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH));
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn svg_semantic_demand_is_independent_of_glyph_font_demand() {
    // Only SVG uses originalText for embedding. Glyph layout sees the uncovered
    // trigger scalar, so PNG/PDF must remain available without Xiaolai.
    let scene = scene_with_original(5, "\u{20000}", "中");
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let result = Renderer::new().render(&scene, &request);
        if format == OutputFormat::Svg && !cfg!(feature = "cjk") {
            let error = result.expect_err("SVG semantic demand needs Xiaolai");
            assert_eq!(error.code(), ErrorCode::RENDER_FONT_UNAVAILABLE);
            assert_eq!(error.category(), RenderErrorCategory::Capability);
            assert!(
                matches!(error.details(), Some(ErrorDetails::FontUnavailable {
                resolved_family, asset_identity
            }) if resolved_family == "Xiaolai" && asset_identity.starts_with("fonts/Xiaolai/"))
            );
        } else {
            let outcome = result.expect("glyph-only formats retain missing-glyph recovery");
            assert!(outcome
                .report()
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH));
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn ascii_families_and_emoji_keep_their_bundled_behavior_in_every_format() {
    for (family, svg_family) in [
        (1, "ExcalidrawRender-1"),
        (2, "ExcalidrawRender-9"),
        (3, "ExcalidrawRender-3"),
        (5, "Excalifont"),
        (6, "Nunito"),
        (7, "ExcalidrawRender-7"),
        (8, "Comic Shanns"),
        (9, "ExcalidrawRender-9"),
    ] {
        for (text, emoji) in [("Catalog", false), ("Catalog 😀", true)] {
            let scene = scene(family, text);
            for format in FORMATS {
                let request = RenderRequest::builder(format).build().unwrap();
                let outcome = Renderer::new()
                    .render(&scene, &request)
                    .unwrap_or_else(|e| panic!("family {family}, {text:?}, {format:?}: {e}"));
                let codes = outcome
                    .report()
                    .diagnostics()
                    .iter()
                    .map(|d| d.code())
                    .collect::<Vec<_>>();
                let mut expected = Vec::new();
                if family == 2 {
                    expected.push(DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED);
                }
                if emoji {
                    expected.push(DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED);
                }
                assert_eq!(codes, expected, "family {family}, {text:?}, {format:?}");
                let bytes = outcome.artifact().bytes();
                match format {
                    OutputFormat::Svg => {
                        let svg = std::str::from_utf8(bytes).unwrap();
                        assert!(
                            svg.contains(&format!("font-family:'{svg_family}'")),
                            "family {family}, {text:?} must embed {svg_family}"
                        );
                        assert_eq!(svg.contains("font-family:'Noto Emoji'"), emoji);
                        assert!(!svg.contains("font-family:'Xiaolai'"));
                    }
                    OutputFormat::Png => assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n")),
                    OutputFormat::Pdf => assert!(bytes.starts_with(b"%PDF-")),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn demanded_xiaolai_obeys_the_packaged_feature_in_every_format() {
    let scene = scene(5, "中");
    for format in FORMATS {
        let request = RenderRequest::builder(format).build().unwrap();
        let result = Renderer::new().render(&scene, &request);
        if cfg!(feature = "cjk") {
            let outcome = result.expect("default consumer forwards CJK support");
            assert!(outcome.report().is_empty());
        } else {
            let error = result.expect_err("the isolated no-CJK dependency must lack Xiaolai");
            assert_eq!(error.code(), ErrorCode::RENDER_FONT_UNAVAILABLE);
            assert_eq!(error.code().as_str(), "render.font-unavailable");
            assert_eq!(error.category(), RenderErrorCategory::Capability);
            assert_eq!(error.format(), format);
            let Some(ErrorDetails::FontUnavailable {
                resolved_family,
                asset_identity,
            }) = error.details()
            else {
                panic!("expected structured unavailable-font details: {error:?}");
            };
            assert_eq!(resolved_family, "Xiaolai");
            assert_eq!(
                asset_identity,
                "fonts/Xiaolai/Xiaolai-Regular-145aa02cdd91946e67dc934e1acffe75.woff2@sha256:96f696dd01f2534ceca68a6c9795f927c7c1dd276f7e97069cc7d76d85c9156c"
            );
        }
    }
}
