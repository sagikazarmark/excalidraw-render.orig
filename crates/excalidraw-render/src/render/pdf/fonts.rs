//! PDF fonts: each embedded face becomes six objects — Type0 font, CIDFontType2
//! descendant, descriptor, FontFile2 stream, ToUnicode CMap and CIDToGIDMap —
//! and every shaped glyph the content stream shows is assigned a deterministic
//! Identity-H character code in first-use order, one mapping table for the
//! Liberation Sans fallback face and one per official font. An official font
//! chunk's program is named with a subset tag derived from the chunk's
//! manifested digest (ADR-0008), so two chunks of one family never share a
//! `/BaseFont`.

use std::fmt::Write as _;

use excalidraw_render_assets_base::AssetRecord;

use super::super::text::{ResolvedGlyph, ResolvedLabel};
use super::super::FormatEncodeError;
use super::streams::deflate_stream;
use super::writer::{StreamEncoding, Writer};
use super::{pdf_size_error, PdfElementKind, PdfPlan};

#[derive(Debug)]
pub(super) struct PdfGlyphMapping {
    pub(super) code: u16,
    pub(super) glyph_id: u16,
    pub(super) advance_1000: u16,
    pub(super) unicode: Vec<u16>,
}

/// The `/BaseFont` and `/FontName` of one embedded program: a subset-tagged
/// name for an official font chunk, the family's PDF name alone for a
/// complete face.
///
/// The tag is the one PDF reserves for embedded subsets — six uppercase
/// letters and `+` — and here it spells the first six nibbles of the chunk's
/// manifested WOFF2 digest, `A` for 0 through `P` for 15, so it is a pure
/// function of the chunk and the three Xiaolai chunks a scene can embed carry
/// three names. A program that is not a Unicode-range chunk of its family, the
/// complete Noto Emoji face, carries no tag. The CMap name stays untagged.
pub(super) fn embedded_font_name(
    identity: &str,
    pdf_name: &str,
) -> Result<String, FormatEncodeError> {
    let Some(record) = official_chunk_record(identity) else {
        return Ok(pdf_name.to_owned());
    };
    let mut name = String::new();
    name.try_reserve_exact(7_usize.saturating_add(pdf_name.len()))
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_font_name",
        })?;
    for nibble in record.complete_sha256.bytes().take(6) {
        let value = (nibble as char).to_digit(16).ok_or_else(|| {
            FormatEncodeError::Encoding("official font chunk digest is not hexadecimal".to_owned())
        })?;
        name.push(char::from(b'A' + value as u8));
    }
    if name.len() != 6 {
        return Err(FormatEncodeError::Encoding(
            "official font chunk digest is shorter than a subset tag".to_owned(),
        ));
    }
    name.push('+');
    name.push_str(pdf_name);
    Ok(name)
}

/// The catalog record of the official font chunk `identity` names
/// (`<upstream path>@sha256:<manifested digest>`), or `None` for a face the
/// catalog does not list as a chunk.
fn official_chunk_record(identity: &str) -> Option<&'static AssetRecord> {
    let (upstream_path, _) = identity.split_once("@sha256:")?;
    excalidraw_render_assets_base::find_asset(upstream_path)
        .filter(|record| record.identity == identity)
}

impl PdfPlan {
    /// Writes the six objects of one embedded face: `font` is the complete
    /// sfnt the descriptor's metrics and `/Length1` come from, `program` its
    /// deflated `FontFile2` body, `base_font` the tagged or complete name the
    /// Type0 font, its descendant and the descriptor carry, and `cmap_name`
    /// the family's PDF name the ToUnicode CMap is registered under.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_font_objects(
        &self,
        pdf: &mut Writer,
        font_object: usize,
        font: &[u8],
        program: &[u8],
        base_font: &str,
        cmap_name: &str,
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

        pdf.object(font_object, |pdf| {
            write!(
                pdf,
                concat!(
                    "<<\n/Type /Font\n/Subtype /Type0\n/BaseFont /{}\n",
                    "/Encoding /Identity-H\n/DescendantFonts [{} 0 R]\n",
                    "/ToUnicode {} 0 R\n>>\n"
                ),
                base_font, cid_font_object, to_unicode_object,
            )
        })?;

        pdf.object(cid_font_object, |pdf| {
            write!(
                pdf,
                concat!(
                    "<<\n/Type /Font\n/Subtype /CIDFontType2\n/BaseFont /{}\n",
                    "/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>\n",
                    "/FontDescriptor {} 0 R\n/CIDToGIDMap {} 0 R\n/DW 1000\n/W ["
                ),
                base_font, descriptor_object, cid_to_gid_object,
            )?;
            for mapping in mappings {
                write!(pdf, " {} [{}]", mapping.code, mapping.advance_1000)?;
            }
            pdf.bytes(b" ]\n>>\n")
        })?;

        pdf.object(descriptor_object, |pdf| {
            write!(
                pdf,
                concat!(
                    "<<\n/Type /FontDescriptor\n/FontName /{}\n/Flags 32\n",
                    "/FontBBox [{} {} {} {}]\n/ItalicAngle 0\n/Ascent {}\n/Descent {}\n",
                    "/CapHeight {}\n/StemV 80\n/FontFile2 {} 0 R\n>>\n"
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
        })?;

        pdf.stream(
            font_file_object,
            StreamEncoding::Flate,
            program,
            |_| Ok(()),
            format_args!("/Length1 {}\n", font.len()),
        )?;

        let cmap = deflate_stream(
            to_unicode_cmap(mappings, cmap_name)?.as_bytes(),
            "pdf_to_unicode_stream",
        )?;
        pdf.flate_stream(to_unicode_object, "", &cmap)?;

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
        let cid_to_gid = deflate_stream(&cid_to_gid, "pdf_cid_to_gid_stream")?;
        pdf.flate_stream(cid_to_gid_object, "", &cid_to_gid)?;
        Ok(())
    }

    pub(super) fn fallback_glyph_mappings(
        &self,
    ) -> Result<Vec<PdfGlyphMapping>, FormatEncodeError> {
        glyph_mappings(
            self.elements
                .iter()
                .filter_map(|element| match &element.kind {
                    PdfElementKind::Fallback {
                        label: Some(label), ..
                    } => Some(label),
                    PdfElementKind::Frame {
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
                let text = match &element.kind {
                    PdfElementKind::Text { text, .. }
                    | PdfElementKind::RoughOutline {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_chunk_programs_carry_a_digest_derived_subset_tag() {
        // Liberation Sans: manifested digest 006a2b28… → nibbles 0 0 6 a 2 b.
        assert_eq!(
            embedded_font_name(
                super::super::super::text::LIBERATION_SANS_IDENTITY,
                "LiberationSans"
            )
            .expect("tag derives"),
            "AAGKCL+LiberationSans"
        );
        let excalifont = excalidraw_render_assets_base::find_asset(
            "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
        )
        .expect("the Excalifont basic-Latin chunk is catalogued");
        assert!(excalifont.complete_sha256.starts_with("e4423b"));
        assert_eq!(
            embedded_font_name(excalifont.identity, "Excalifont").expect("tag derives"),
            "OEECDL+Excalifont"
        );
    }

    #[test]
    fn the_complete_noto_emoji_face_is_not_a_subset_and_carries_no_tag() {
        let identity = concat!(
            "scripts/woff2/assets/NotoEmoji-Regular.ttf@sha256:",
            "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"
        );
        assert_eq!(
            embedded_font_name(identity, "NotoEmoji").expect("name derives"),
            "NotoEmoji"
        );
    }

    #[test]
    fn an_identity_whose_digest_disagrees_with_the_catalog_is_not_a_chunk() {
        let identity = concat!(
            "fonts/Liberation/LiberationSans-Regular.woff2@sha256:",
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert!(official_chunk_record(identity).is_none());
    }
}
