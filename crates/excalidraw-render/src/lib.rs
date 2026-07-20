//! Pure-Rust rendering of Excalidraw scene documents.
//!
//! The compatibility-revision public API is being implemented incrementally.
//! The current proof-of-concept CLI behavior is not that compatibility contract.

#[cfg(not(target_arch = "wasm32"))]
mod backends;
mod compatibility;
#[cfg(not(target_arch = "wasm32"))]
mod model;
mod outcome;
mod render;
#[cfg(not(target_arch = "wasm32"))]
mod render_svg;
mod request;
mod resource_accounting;
mod resource_limits;
mod scene;

pub use compatibility::{
    AssetChainEntry, CompatibilityRevision, ErrorCode, ErrorDescriptor, ErrorDetails, ErrorFamily,
    FiniteValue, LogicalBounds, Measurement, MetadataComponent, MetadataOperation, NestedPath,
    NestedPathSegment, NestedSegmentKind, Recovery, ReferenceProfileIdentity, Renderer,
    ResourceMeasurement, ResourceValue, UnsupportedRevisionError, ValueKind,
    DEFAULT_COMPATIBILITY_REVISION,
};
pub use outcome::{
    Artifact, CompatibilityDiagnostic, CompatibilityReport, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, RenderError, RenderErrorCategory, RenderOutcome,
};
pub use request::{
    ColorMode, FrameOptions, MetadataOptions, OutputFormat, PdfOptions, PdfRasterFallback,
    RenderRequest, RenderRequestBuilder, RequestError, Sizing, SupplementalAsset,
    SupplementalAssets, SupplementalAssetsBuilder,
};
pub use resource_limits::{ResourceLimits, ResourceLimitsBuilder};
pub use scene::{SceneDocument, SceneParseError};

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
