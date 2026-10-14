//! Reads what a test asserts about an sfnt: whether it maps a character, a
//! glyph's advance, its glyph count, and its English name records.

use write_fonts::read::tables::name::NameRecord;
use write_fonts::read::types::GlyphId;
use write_fonts::read::{FontRef, Offset, TableProvider};

pub struct Sfnt<'a>(FontRef<'a>);

impl<'a> Sfnt<'a> {
    /// Parses an sfnt that carries the tables a renderable face needs.
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        let font = FontRef::new(bytes).ok()?;
        font.head().ok()?;
        font.hhea().ok()?;
        font.maxp().ok()?;
        Some(Self(font))
    }

    /// The glyph other than `.notdef` that the font's best character map
    /// subtable, the one HarfBuzz shapes with, maps `c` to.
    pub fn glyph(&self, c: char) -> Option<GlyphId> {
        let (_, _, subtable) = self.0.cmap().ok()?.best_subtable()?;
        subtable
            .map_codepoint(c)
            .filter(|glyph| glyph.to_u32() != 0)
    }

    pub fn advance(&self, glyph: GlyphId) -> Option<u16> {
        if glyph.to_u32() >= u32::from(self.glyph_count()) {
            return None;
        }
        self.0.hmtx().ok()?.advance(glyph)
    }

    pub fn glyph_count(&self) -> u16 {
        self.0.maxp().map_or(0, |maxp| maxp.num_glyphs())
    }

    /// The English (US) name record `id`, from a Unicode-encoded record.
    pub fn name(&self, id: u16) -> Option<String> {
        let name = self.0.name().ok()?;
        name.name_record()
            .iter()
            .filter(|record| record.name_id().to_u16() == id && record.language_id() == 0x0409)
            .find_map(|record| utf16_string(record, name.string_data().as_bytes()))
    }
}

/// The family name (name ID 1) of an embedded face.
pub fn family_name(bytes: &[u8]) -> String {
    Sfnt::parse(bytes)
        .and_then(|sfnt| sfnt.name(1))
        .expect("embedded face keeps its English family name")
}

/// A Unicode platform or Windows symbol/BMP record decoded as UTF-16BE.
fn utf16_string(record: &NameRecord, data: &[u8]) -> Option<String> {
    let unicode = match record.platform_id() {
        0 => true,
        3 => matches!(record.encoding_id(), 0 | 1),
        _ => false,
    };
    if !unicode {
        return None;
    }
    let start = record.string_offset().to_usize();
    let bytes = data.get(start..start + usize::from(record.length()))?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).ok()
}
