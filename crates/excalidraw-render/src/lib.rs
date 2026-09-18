//! Pure-Rust rendering of Excalidraw scene documents.
//!
//! Scene and frame exports to SVG, PNG, and vector-first PDF are produced
//! against a compatibility revision: an immutable static-export promise
//! covering artifact bytes, compatibility diagnostics, and failure behavior.
//! The compatibility-revision public API is being implemented incrementally.

mod catalog;
mod color;
mod compatibility;
mod css_syntax;
mod diagnostic_registry;
mod font_metadata;
mod frame;
#[cfg(test)]
mod metadata_failure_contract;
mod normalization;
mod outcome;
#[cfg(all(test, target_arch = "wasm32"))]
#[path = "../tests/common/png_color_reference.rs"]
mod png_color_reference;
mod portable_math;
mod relationships;
mod render;
mod request;
mod resource_accounting;
mod resource_limits;
mod scene;
#[cfg(test)]
mod scene_fuzz;
mod scene_surface;

pub use compatibility::{
    AssetChainEntry, CompatibilityRevision, ErrorCode, ErrorDescriptor, ErrorDetails, ErrorFamily,
    FiniteValue, LogicalBounds, Measurement, MetadataComponent, MetadataOperation, NestedPath,
    NestedPathSegment, NestedSegmentKind, Recovery, ReferenceProfileIdentity, Renderer,
    ResourceMeasurement, ResourceValue, SvgFontSubsetStage, UnsupportedRevisionError, ValueKind,
    DEFAULT_COMPATIBILITY_REVISION,
};
pub use frame::{
    FrameBatchError, FrameBatchErrorKind, FrameKind, FrameRef, FrameRefs, FrameRenderCursor,
    FrameRenderEntry,
};
pub use outcome::{
    Artifact, CompatibilityDiagnostic, CompatibilityReport, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, RenderError, RenderErrorCategory, RenderOutcome,
};
pub use request::{
    ColorMode, FrameOptions, FrameRenderRequest, FrameRenderRequestBuilder, MetadataOptions,
    OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest, RenderRequestBuilder, RequestError,
    Sizing, SupplementalAsset, SupplementalAssets, SupplementalAssetsBuilder,
};
pub use resource_limits::{ResourceLimits, ResourceLimitsBuilder};
pub use scene::{SceneDocument, SceneParseError};

#[cfg(all(test, target_arch = "wasm32"))]
pub mod wasm_tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

    #[wasm_bindgen_test]
    fn png_sdr_color_reference_in_wasm() {
        let evidence = crate::png_color_reference::evidence();
        for row in evidence["cases"].as_array().unwrap() {
            let scene =
                crate::SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
            let request = crate::RenderRequest::builder(crate::OutputFormat::Png)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap();
            let first = crate::Renderer::new().render(&scene, &request).unwrap();
            let second = crate::Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
            crate::png_color_reference::report(
                row,
                "png",
                serde_json::to_value(first.report()).unwrap(),
            );
            if crate::png_color_reference::recovery(row, "png").is_some() {
                let mut control = row["scene"].clone();
                control["files"] = serde_json::json!({});
                let control =
                    crate::SceneDocument::parse(&serde_json::to_vec(&control).unwrap()).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    crate::Renderer::new()
                        .render(&control, &request)
                        .unwrap()
                        .artifact()
                        .bytes()
                );
            } else {
                crate::png_color_reference::appearance(
                    row,
                    crate::png_color_reference::png(first.artifact().bytes()),
                    false,
                );
            }
        }
    }

    #[wasm_bindgen_test]
    fn png_precision_reference_in_wasm() {
        use base64::Engine as _;
        use sha2::{Digest, Sha256};
        let bytes = include_bytes!("../../../tests/oracle/png-precision/reference.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "8400494ec5f6e744b00e175db6931aff33c2a659dc57c3b3ab62646da0cf83e5"
        );
        let evidence: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let decode_png = |bytes: &[u8]| {
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let mut rgba = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut rgba).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            assert_eq!(info.bit_depth, png::BitDepth::Eight);
            rgba.truncate(info.buffer_size());
            (info.width, info.height, rgba)
        };
        for row in evidence["cases"].as_array().unwrap() {
            let scene =
                crate::SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
            let request = crate::RenderRequest::builder(crate::OutputFormat::Png)
                .padding(0.0)
                .include_background(row["options"]["exportBackground"].as_bool().unwrap())
                .build()
                .unwrap();
            let first = crate::Renderer::new().render(&scene, &request).unwrap();
            let second = crate::Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(first.report(), second.report());
            assert_eq!(
                serde_json::to_value(first.report()).unwrap(),
                serde_json::json!({"schema_version":1,"diagnostics":[]})
            );
            let expected = base64::engine::general_purpose::STANDARD
                .decode(row["png_base64"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                decode_png(first.artifact().bytes()),
                decode_png(&expected),
                "{}",
                row["id"]
            );
        }
    }

    #[wasm_bindgen_test]
    fn complete_font_subset_qualification_corpus_in_wasm() {
        crate::render::assert_complete_font_subset_qualification_corpus();
    }

    #[wasm_bindgen_test]
    fn font_subset_fuzz_regression_corpus_in_wasm() {
        crate::render::assert_font_subset_fuzz_regression_corpus();
    }

    #[wasm_bindgen_test]
    fn font_subset_fuzz_fixed_seed_smoke_in_wasm() {
        crate::render::assert_font_subset_fuzz_smoke();
    }

    #[wasm_bindgen_test]
    fn font_subset_resource_trace_matrix_in_wasm() {
        crate::render::assert_font_subset_resource_trace_matrix();
    }

    #[wasm_bindgen_test]
    fn scene_fuzz_regression_corpus_in_wasm() {
        crate::scene_fuzz::assert_scene_fuzz_regression_corpus();
    }

    #[wasm_bindgen_test]
    fn scene_fuzz_fixed_seed_smoke_in_wasm() {
        crate::scene_fuzz::assert_scene_fuzz_smoke();
    }
}

/// Returns the complete package-wide license and attribution bundle.
pub const fn license_bundle() -> &'static str {
    include_str!("../LICENSE-BUNDLE.txt")
}
