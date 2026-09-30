//! The shaping boundary (#202): ttf-parser parses and measures a face, and
//! HarfRust shapes it. [`Face`] dereferences to the parsed face, so metric,
//! outline and table access go straight to ttf-parser.

use std::ops::Deref;

pub(crate) use harfrust::{Direction, Feature, GlyphBuffer, Tag, UnicodeBuffer};

pub(crate) struct Face<'a> {
    parsed: ttf_parser::Face<'a>,
    font: harfrust::FontRef<'a>,
    shaper_data: harfrust::ShaperData,
}

impl<'a> Face<'a> {
    /// Parses face `index` of an SFNT. Both parsers must accept it.
    pub(crate) fn from_slice(data: &'a [u8], index: u32) -> Option<Self> {
        let parsed = ttf_parser::Face::parse(data, index).ok()?;
        let font = harfrust::FontRef::from_index(data, index).ok()?;
        let shaper_data = harfrust::ShaperData::new(&font);
        Some(Self {
            parsed,
            font,
            shaper_data,
        })
    }
}

impl<'a> Deref for Face<'a> {
    type Target = ttf_parser::Face<'a>;

    fn deref(&self) -> &Self::Target {
        &self.parsed
    }
}

/// Shapes `buffer` in font units (no scale) with `features`.
pub(crate) fn shape(face: &Face<'_>, features: &[Feature], buffer: UnicodeBuffer) -> GlyphBuffer {
    face.shaper_data
        .shaper(&face.font)
        .build()
        .shape(buffer, harfrust::ShapeOptions::new().features(features))
}
