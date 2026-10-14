//! The sfnt facts xtask checks, read the way the renderer reads a face: that
//! bytes parse, which characters the Unicode character maps map, and a face's
//! family name.

use std::collections::BTreeSet;

use write_fonts::read::tables::cmap::{CmapSubtable, PlatformId};
use write_fonts::read::tables::name::NameRecord;
use write_fonts::read::tables::os2::SelectionFlags;
use write_fonts::read::types::Fixed;
use write_fonts::read::{FontRef, Offset, TableProvider};

pub(crate) struct Sfnt<'a>(FontRef<'a>);

impl<'a> Sfnt<'a> {
    /// Parses an sfnt whose `head`, `hhea` and `maxp` read.
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        let font = FontRef::new(bytes).ok()?;
        font.head().ok()?;
        font.hhea().ok()?;
        font.maxp().ok()?;
        Some(Self(font))
    }

    pub(crate) fn units_per_em(&self) -> u16 {
        self.0.head().map_or(0, |head| head.units_per_em())
    }

    /// OS/2's weight class, 400 without an OS/2.
    pub(crate) fn weight(&self) -> u16 {
        self.0.os2().map_or(400, |os2| os2.us_weight_class())
    }

    /// OS/2's width class from 1 to 9, 5 (normal) without one in range.
    pub(crate) fn width_class(&self) -> u16 {
        let class = self.0.os2().map_or(0, |os2| os2.us_width_class());
        if (1..=9).contains(&class) {
            class
        } else {
            5
        }
    }

    /// Italic by OS/2's selection flags or a nonzero `post` italic angle.
    pub(crate) fn is_italic(&self) -> bool {
        let flagged = self
            .0
            .os2()
            .is_ok_and(|os2| os2.fs_selection().contains(SelectionFlags::ITALIC));
        let slanted = self
            .0
            .post()
            .is_ok_and(|post| post.italic_angle() != Fixed::ZERO);
        flagged || slanted
    }

    /// Whether a Unicode subtable maps `c` to a glyph other than `.notdef`.
    pub(crate) fn maps(&self, c: char) -> bool {
        self.unicode_subtables().iter().any(|subtable| {
            subtable
                .map_codepoint(c)
                .is_some_and(|glyph| glyph.to_u32() != 0)
        })
    }

    /// Every code point a Unicode subtable maps to a glyph other than
    /// `.notdef`.
    pub(crate) fn mapped_codepoints(&self) -> BTreeSet<u32> {
        let mut codepoints = BTreeSet::new();
        for subtable in self.unicode_subtables() {
            for (codepoint, _) in subtable.iter() {
                if subtable
                    .map_codepoint(codepoint)
                    .is_some_and(|glyph| glyph.to_u32() != 0)
                {
                    codepoints.insert(codepoint);
                }
            }
        }
        codepoints
    }

    /// The Unicode platform's subtables, Windows BMP, and Windows full
    /// repertoire in format 12 or 13, in record order.
    fn unicode_subtables(&self) -> Vec<CmapSubtable<'a>> {
        let Ok(cmap) = self.0.cmap() else {
            return Vec::new();
        };
        let data = cmap.offset_data();
        cmap.encoding_records()
            .iter()
            .filter_map(|record| {
                let subtable = record.subtable(data).ok()?;
                let unicode = match record.platform_id() {
                    PlatformId::Unicode => true,
                    PlatformId::Windows => {
                        record.encoding_id() == 1
                            || (record.encoding_id() == 10 && matches!(subtable.format(), 12 | 13))
                    }
                    _ => false,
                };
                unicode.then_some(subtable)
            })
            .collect()
    }

    /// The typographic family name (ID 16), else the family name (ID 1), from
    /// the first Unicode-encoded record in any language.
    pub(crate) fn family(&self) -> Option<String> {
        let name = self.0.name().ok()?;
        let data = name.string_data().as_bytes();
        [16, 1].into_iter().find_map(|id| {
            name.name_record()
                .iter()
                .filter(|record| record.name_id().to_u16() == id)
                .find_map(|record| utf16_string(record, data))
        })
    }
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
