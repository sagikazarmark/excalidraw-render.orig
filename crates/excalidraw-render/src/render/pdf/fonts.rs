//! PDF fonts: each embedded face becomes six objects — Type0 font, CIDFontType2
//! descendant, descriptor, FontFile2 stream, ToUnicode CMap and CIDToGIDMap —
//! and every shaped glyph the content stream shows is assigned a deterministic
//! Identity-H character code in first-use order, one mapping table for the
//! Liberation Sans fallback face and one per official font.

use std::fmt::Write as _;
use std::io::Write as _;

use super::super::text::{ResolvedGlyph, ResolvedLabel};
use super::super::FormatEncodeError;
use super::{begin_object, pdf_size_error, write_error, PdfElement, PdfPlan};

#[derive(Debug)]
pub(super) struct PdfGlyphMapping {
    pub(super) code: u16,
    pub(super) glyph_id: u16,
    pub(super) advance_1000: u16,
    pub(super) unicode: Vec<u16>,
}

impl PdfPlan {
    pub(super) fn write_font_objects(
        &self,
        pdf: &mut Vec<u8>,
        offsets: &mut [usize],
        font_object: usize,
        font: &[u8],
        base_font: &str,
        mappings: &[PdfGlyphMapping],
    ) -> Result<(), FormatEncodeError> {
        let cid_font_object = font_object.checked_add(1).ok_or_else(pdf_size_error)?;
        let descriptor_object = font_object.checked_add(2).ok_or_else(pdf_size_error)?;
        let font_file_object = font_object.checked_add(3).ok_or_else(pdf_size_error)?;
        let to_unicode_object = font_object.checked_add(4).ok_or_else(pdf_size_error)?;
        let cid_to_gid_object = font_object.checked_add(5).ok_or_else(pdf_size_error)?;
        let face = rustybuzz::ttf_parser::Face::parse(font, 0).map_err(|_| {
            FormatEncodeError::Encoding("PDF fallback font is not a valid sfnt".to_owned())
        })?;
        let units_per_em = i64::from(face.units_per_em());
        let metric = |value: i16| i64::from(value) * 1_000 / units_per_em;
        let bbox = face.global_bounding_box();

        begin_object(pdf, offsets, font_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Font\n/Subtype /Type0\n/BaseFont /{}\n",
                "/Encoding /Identity-H\n/DescendantFonts [{} 0 R]\n",
                "/ToUnicode {} 0 R\n>>\nendobj\n"
            ),
            base_font, cid_font_object, to_unicode_object,
        )
        .map_err(write_error)?;

        begin_object(pdf, offsets, cid_font_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Font\n/Subtype /CIDFontType2\n/BaseFont /{}\n",
                "/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>\n",
                "/FontDescriptor {} 0 R\n/CIDToGIDMap {} 0 R\n/DW 1000\n/W ["
            ),
            base_font, descriptor_object, cid_to_gid_object,
        )
        .map_err(write_error)?;
        for mapping in mappings {
            write!(pdf, " {} [{}]", mapping.code, mapping.advance_1000).map_err(write_error)?;
        }
        pdf.extend_from_slice(b" ]\n>>\nendobj\n");

        begin_object(pdf, offsets, descriptor_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /FontDescriptor\n/FontName /{}\n/Flags 32\n",
                "/FontBBox [{} {} {} {}]\n/ItalicAngle 0\n/Ascent {}\n/Descent {}\n",
                "/CapHeight {}\n/StemV 80\n/FontFile2 {} 0 R\n>>\nendobj\n"
            ),
            base_font,
            i64::from(bbox.x_min) * 1_000 / units_per_em,
            i64::from(bbox.y_min) * 1_000 / units_per_em,
            i64::from(bbox.x_max) * 1_000 / units_per_em,
            i64::from(bbox.y_max) * 1_000 / units_per_em,
            metric(face.ascender()),
            metric(face.descender()),
            metric(face.capital_height().unwrap_or(face.ascender())),
            font_file_object,
        )
        .map_err(write_error)?;

        begin_object(pdf, offsets, font_file_object)?;
        write!(
            pdf,
            "<<\n/Length {}\n/Length1 {}\n>>\nstream\n",
            font.len(),
            font.len()
        )
        .map_err(write_error)?;
        pdf.extend_from_slice(font);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        let cmap = to_unicode_cmap(mappings, base_font)?;
        begin_object(pdf, offsets, to_unicode_object)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", cmap.len()).map_err(write_error)?;
        pdf.extend_from_slice(cmap.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        let map_length = mappings.last().map_or(2_usize, |mapping| {
            usize::from(mapping.code)
                .saturating_add(1)
                .saturating_mul(2)
        });
        let mut cid_to_gid = Vec::new();
        cid_to_gid.try_reserve_exact(map_length).map_err(|_| {
            FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_cid_to_gid_map",
            }
        })?;
        cid_to_gid.resize(map_length, 0);
        for mapping in mappings {
            let offset = usize::from(mapping.code).saturating_mul(2);
            let [high, low] = mapping.glyph_id.to_be_bytes();
            cid_to_gid[offset] = high;
            cid_to_gid[offset + 1] = low;
        }
        begin_object(pdf, offsets, cid_to_gid_object)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", cid_to_gid.len()).map_err(write_error)?;
        pdf.extend_from_slice(&cid_to_gid);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        Ok(())
    }

    pub(super) fn fallback_glyph_mappings(
        &self,
    ) -> Result<Vec<PdfGlyphMapping>, FormatEncodeError> {
        glyph_mappings(
            self.elements.iter().filter_map(|element| match element {
                PdfElement::Fallback {
                    label: Some(label), ..
                } => Some(label),
                PdfElement::Frame {
                    name: Some(name), ..
                } => Some(&name.label),
                _ => None,
            }),
            "pdf_fallback_glyph_mappings",
        )
    }

    pub(super) fn official_glyph_mappings(
        &self,
    ) -> Result<Vec<Vec<PdfGlyphMapping>>, FormatEncodeError> {
        let mut all_mappings = Vec::new();
        all_mappings
            .try_reserve_exact(self.official_fonts.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_text_font_mappings",
            })?;
        for font_index in 0..self.official_fonts.len() {
            let mut mappings = Vec::new();
            for element in &self.elements {
                let text = match element {
                    PdfElement::Text { text, .. }
                    | PdfElement::RoughOutline {
                        generated_label: Some(text),
                        ..
                    } => text,
                    _ => continue,
                };
                for line in &text.lines {
                    if line.runs.is_empty() {
                        if text.font_index == font_index {
                            for glyph in &line.glyphs {
                                retain_glyph_mapping(
                                    &mut mappings,
                                    glyph,
                                    "pdf_text_glyph_mappings",
                                )?;
                            }
                        }
                    } else {
                        for run in &line.runs {
                            if run.font_index != font_index {
                                continue;
                            }
                            let glyphs = line
                                .glyphs
                                .get(run.glyph_start..run.glyph_end)
                                .ok_or_else(|| {
                                    FormatEncodeError::Encoding(
                                        "PDF text run has invalid glyph bounds".to_owned(),
                                    )
                                })?;
                            for glyph in glyphs {
                                retain_glyph_mapping(
                                    &mut mappings,
                                    glyph,
                                    "pdf_text_glyph_mappings",
                                )?;
                            }
                        }
                    }
                }
            }
            all_mappings.push(mappings);
        }
        Ok(all_mappings)
    }
}

fn glyph_mappings<'a>(
    labels: impl IntoIterator<Item = &'a ResolvedLabel>,
    subject: &'static str,
) -> Result<Vec<PdfGlyphMapping>, FormatEncodeError> {
    let mut mappings = Vec::new();
    for label in labels {
        for glyph in &label.glyphs {
            retain_glyph_mapping(&mut mappings, glyph, subject)?;
        }
    }
    Ok(mappings)
}

fn retain_glyph_mapping(
    mappings: &mut Vec<PdfGlyphMapping>,
    glyph: &ResolvedGlyph,
    subject: &'static str,
) -> Result<(), FormatEncodeError> {
    if mappings
        .iter()
        .any(|mapping| mapping.glyph_id == glyph.id && mapping.unicode == glyph.unicode)
    {
        return Ok(());
    }
    let code = u16::try_from(mappings.len().saturating_add(1)).map_err(|_| {
        FormatEncodeError::Encoding(
            "PDF font requires more deterministic character codes than Identity-H supports"
                .to_owned(),
        )
    })?;
    let mut unicode = Vec::new();
    unicode
        .try_reserve_exact(glyph.unicode.len())
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject,
        })?;
    unicode.extend_from_slice(&glyph.unicode);
    mappings
        .try_reserve(1)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject,
        })?;
    mappings.push(PdfGlyphMapping {
        code,
        glyph_id: glyph.id,
        advance_1000: glyph.advance_1000,
        unicode,
    });
    Ok(())
}

pub(super) fn glyph_mapping_code(
    mappings: &[PdfGlyphMapping],
    glyph: &ResolvedGlyph,
) -> Result<u16, FormatEncodeError> {
    mappings
        .iter()
        .find(|mapping| mapping.glyph_id == glyph.id && mapping.unicode == glyph.unicode)
        .map(|mapping| mapping.code)
        .ok_or_else(|| {
            FormatEncodeError::Encoding(
                "PDF shaped glyph has no deterministic character-code mapping".to_owned(),
            )
        })
}

fn to_unicode_cmap(
    mappings: &[PdfGlyphMapping],
    base_font: &str,
) -> Result<String, FormatEncodeError> {
    let mut cmap = String::new();
    let capacity = mappings
        .len()
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(512))
        .ok_or_else(pdf_size_error)?;
    cmap.try_reserve_exact(capacity)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_fallback_to_unicode_cmap",
        })?;
    cmap.push_str(concat!(
        "/CIDInit /ProcSet findresource begin\n",
        "12 dict begin\nbegincmap\n",
        "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n",
        "/CMapName /"
    ));
    cmap.push_str(base_font);
    cmap.push_str(
        "-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in mappings.chunks(100) {
        writeln!(cmap, "{} beginbfchar", chunk.len()).map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF ToUnicode count".to_owned())
        })?;
        for mapping in chunk {
            write!(cmap, "<{:04X}> <", mapping.code).map_err(|_| {
                FormatEncodeError::Encoding("failed to write PDF ToUnicode source".to_owned())
            })?;
            if mapping.unicode.is_empty() {
                cmap.push_str("FFFD");
            } else {
                for unit in &mapping.unicode {
                    write!(cmap, "{unit:04X}").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write PDF ToUnicode destination".to_owned(),
                        )
                    })?;
                }
            }
            cmap.push_str(">\n");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend");
    Ok(cmap)
}
