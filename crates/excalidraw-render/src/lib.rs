//! Pure-Rust rendering of Excalidraw scene documents.
//!
//! Scene and frame exports to SVG, PNG, and vector-first PDF are produced
//! against a compatibility revision: an immutable static-export promise
//! covering artifact bytes, compatibility diagnostics, and failure behavior.
//! The compatibility-revision public API is being implemented incrementally.

mod catalog;
mod color;
mod compatibility;
mod diagnostic_registry;
mod font_metadata;
mod frame;
mod normalization;
mod outcome;
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
