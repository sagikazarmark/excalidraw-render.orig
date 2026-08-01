//! Pure-Rust rendering of Excalidraw scene documents.
//!
//! The compatibility-revision public API is being implemented incrementally.
//! The current proof-of-concept CLI behavior is not that compatibility contract.

#[cfg(not(target_arch = "wasm32"))]
mod backends;
mod catalog;
mod color;
mod compatibility;
mod frame;
#[cfg(not(target_arch = "wasm32"))]
mod model;
mod normalization;
mod outcome;
mod relationships;
mod render;
#[cfg(not(target_arch = "wasm32"))]
mod render_svg;
mod request;
mod resource_accounting;
mod resource_limits;
mod scene;
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
}

/// Returns the complete package-wide license and attribution bundle.
pub const fn license_bundle() -> &'static str {
    include_str!("../LICENSE-BUNDLE.txt")
}

/// Temporary integration seam for the proof-of-concept CLI.
///
/// This module is deliberately hidden from generated documentation. It will be
/// removed as the compatibility-revision public facade replaces the MVP.
#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub mod __mvp {
    pub use crate::backends::OutputFormat;
    pub use crate::model::ExcalidrawFile as SceneDocument;

    use anyhow::Result;

    /// Renders the MVP document model to one complete artifact.
    pub fn render(
        document: &SceneDocument,
        format: OutputFormat,
        scale: f32,
        padding: f64,
    ) -> Result<Vec<u8>> {
        let svg = crate::render_svg::render(document, padding)?;
        crate::backends::encode(format, &svg, scale)
    }
}
