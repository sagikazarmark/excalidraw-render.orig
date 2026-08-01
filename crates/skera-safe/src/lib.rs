//! Narrow safety adapter for the renderer's pinned `skera` dependency.

pub use skera_upstream::{Plan, SubsetFlags};
use write_fonts::read::{FontRef, TableProvider};
use write_fonts::types::Tag;

/// Subsets a font after rejecting a malformed name-record array that panics in
/// `skera 0.5.1` rather than returning its declared table error.
pub fn subset_font(
    font: &FontRef<'_>,
    plan: &Plan,
) -> Result<Vec<u8>, skera_upstream::SubsetError> {
    let name = font
        .name()
        .map_err(|_| skera_upstream::SubsetError::SubsetTableError(Tag::new(b"name")))?;
    if name
        .offset_data()
        .as_bytes()
        .get(name.name_record_byte_range())
        .is_none()
    {
        return Err(skera_upstream::SubsetError::SubsetTableError(Tag::new(
            b"name",
        )));
    }
    skera_upstream::subset_font(font, plan)
}
