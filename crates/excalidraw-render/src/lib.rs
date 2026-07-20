//! Pure-Rust rendering of Excalidraw scene documents.
//!
//! The compatibility-revision public API is being implemented incrementally.
//! The current proof-of-concept CLI behavior is not that compatibility contract.

mod backends;
mod compatibility;
mod model;
mod render_svg;

pub use compatibility::{
    CompatibilityRevision, ErrorCode, ErrorDescriptor, ErrorDetails, ErrorFamily,
    ReferenceProfileIdentity, Renderer, UnsupportedRevisionError, DEFAULT_COMPATIBILITY_REVISION,
};

/// Temporary integration seam for the proof-of-concept CLI.
///
/// This module is deliberately hidden from generated documentation. It will be
/// removed as the compatibility-revision public facade replaces the MVP.
#[doc(hidden)]
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
