//! The shaping boundary (#202): ttf-parser parses and measures a face, and
//! HarfRust shapes it. [`Face`] dereferences to the parsed face, so metric,
//! outline and table access go straight to ttf-parser.

use std::ops::Deref;
use std::sync::Arc;

pub(crate) use harfrust::{Buffer, Direction, Feature, Tag};

pub(crate) struct Face<'a> {
    parsed: ttf_parser::Face<'a>,
    // HarfRust's font owns its bytes and caches its layout data on first shape.
    font: harfrust::Font,
}

impl<'a> Face<'a> {
    /// Parses face `index` of an SFNT. Both parsers must accept it. HarfRust
    /// keeps its own copy of `data`; [`Face::from_shared`] shares it instead.
    pub(crate) fn from_slice(data: &'a [u8], index: u32) -> Option<Self> {
        Self::new(data, data.to_vec().into(), index)
    }

    /// Parses face `index` of a shared SFNT without copying it.
    pub(crate) fn from_shared(data: &'a Arc<[u8]>, index: u32) -> Option<Self> {
        let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(Arc::clone(data));
        Self::new(data, shared.into(), index)
    }

    fn new(data: &'a [u8], blob: harfrust::font::Blob, index: u32) -> Option<Self> {
        let parsed = ttf_parser::Face::parse(data, index).ok()?;
        let font = harfrust::Font::new(blob, index)?;
        Some(Self { parsed, font })
    }
}

impl<'a> Deref for Face<'a> {
    type Target = ttf_parser::Face<'a>;

    fn deref(&self) -> &Self::Target {
        &self.parsed
    }
}

/// Shapes `text` in font units (no scale) with `features`, in `direction`
/// when given. The script, and the direction when none is given, are guessed
/// from the text.
pub(crate) fn shape(
    face: &Face<'_>,
    features: &[Feature],
    text: &str,
    direction: Option<Direction>,
) -> Buffer {
    let mut buffer = Buffer::new();
    buffer.push_str(text);
    if let Some(direction) = direction {
        buffer.set_direction(direction);
    }
    buffer.guess_segment_properties();
    harfrust::shape(
        &harfrust::ShaperFont::new(&face.font),
        &mut buffer,
        harfrust::ShapeOptions::new().features(features),
    )
    // A fresh buffer with guessed properties has text and a direction.
    .expect("a guessed text buffer shapes");
    buffer
}
