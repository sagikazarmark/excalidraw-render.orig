use std::collections::TryReserveError;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder, Tag};

const LIBERATION_SANS_PATH: &str = "fonts/Liberation/LiberationSans-Regular.woff2";
const EXCALIFONT_LATIN_PATH: &str =
    "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2";
const LIBERATION_SANS_IDENTITY: &str = concat!(
    "fonts/Liberation/LiberationSans-Regular.woff2@sha256:",
    "006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8"
);
pub(super) const FALLBACK_FONT_SIZE: f64 = 14.0;

pub(super) struct FallbackFont {
    woff2: Vec<u8>,
    sfnt: Vec<u8>,
}

pub(super) struct OfficialFont {
    source_family: i32,
    catalog_family: &'static str,
    family: &'static str,
    pdf_name: &'static str,
    identity: &'static str,
    svg_mime: &'static str,
    woff2: Vec<u8>,
    sfnt: Vec<u8>,
}

pub(super) struct ResolvedText {
    pub(super) font_index: usize,
    pub(super) font_family: &'static str,
    pub(super) font_size: f64,
    pub(super) text_align: ResolvedTextAlign,
    pub(super) lines: Vec<ResolvedLabel>,
    pub(super) scalar_count: u64,
    pub(super) line_break_steps: u64,
    pub(super) missing_codepoints: Vec<u32>,
    pub(super) noto_emoji_substituted: bool,
    pub(super) fallback_probes: u64,
    fallback_resources: u64,
    pub(super) prepaid_glyphs: u64,
    pub(super) prepaid_line_break_steps: u64,
}

#[derive(Clone, Copy)]
pub(super) enum ResolvedTextAlign {
    Left,
    Center,
    Right,
}

impl ResolvedTextAlign {
    fn from_normalized(value: &str) -> Self {
        match value {
            "center" => Self::Center,
            "right" => Self::Right,
            _ => Self::Left,
        }
    }
}

pub(super) struct TextLayout<'a> {
    pub(super) width: f64,
    pub(super) font_size: f64,
    pub(super) line_height: f64,
    pub(super) text_align: &'a str,
}

pub(super) struct ResolvedLabel {
    pub(super) text: String,
    pub(super) display_text: String,
    pub(super) baseline: f64,
    pub(super) path: LabelPath,
    pub(super) glyph_count: u64,
    pub(super) missing_codepoints: Vec<u32>,
    pub(super) glyphs: Vec<ResolvedGlyph>,
    pub(super) runs: Vec<ResolvedTextRun>,
}

pub(super) struct ResolvedTextRun {
    pub(super) font_index: usize,
    pub(super) font_family: &'static str,
    pub(super) display_text: String,
    pub(super) glyph_start: usize,
    pub(super) glyph_end: usize,
    pub(super) rtl: bool,
}

pub(super) struct ResolvedGlyph {
    pub(super) id: u16,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) advance_1000: u16,
    pub(super) unicode: Vec<u16>,
}

pub(super) struct LabelPath {
    pub(super) operations: Vec<LabelPathOperation>,
}

pub(super) enum LabelPathOperation {
    Move {
        x: f64,
        y: f64,
    },
    Line {
        x: f64,
        y: f64,
    },
    Quadratic {
        control_x: f64,
        control_y: f64,
        x: f64,
        y: f64,
    },
    Cubic {
        control_1_x: f64,
        control_1_y: f64,
        control_2_x: f64,
        control_2_y: f64,
        x: f64,
        y: f64,
    },
    Close,
}

pub(super) enum LabelError {
    Memory,
    InvalidFont,
}

impl From<TryReserveError> for LabelError {
    fn from(_: TryReserveError) -> Self {
        Self::Memory
    }
}

impl FallbackFont {
    pub(super) fn load() -> Result<Self, LabelError> {
        let asset = excalidraw_render_assets_base::load_asset(LIBERATION_SANS_PATH)
            .map_err(|_| LabelError::InvalidFont)?
            .ok_or(LabelError::InvalidFont)?;
        let mut woff2 = Vec::new();
        woff2.try_reserve_exact(asset.bytes().len())?;
        woff2.extend_from_slice(asset.bytes());
        let sfnt = wuff::decompress_woff2(&woff2).map_err(|_| LabelError::InvalidFont)?;
        rustybuzz::Face::from_slice(&sfnt, 0).ok_or(LabelError::InvalidFont)?;
        Ok(Self { woff2, sfnt })
    }

    pub(super) fn resolve(
        &self,
        full_text: &str,
        width: f64,
        height: f64,
    ) -> Result<Option<ResolvedLabel>, LabelError> {
        if height < FALLBACK_FONT_SIZE {
            return Ok(None);
        }
        let full = self.shape(full_text, width, height)?;
        if full.is_some() {
            return Ok(full);
        }
        self.shape("\u{2026}", width, height)
    }

    pub(super) fn into_parts(self) -> (Vec<u8>, Vec<u8>) {
        (self.woff2, self.sfnt)
    }

    fn shape(
        &self,
        text: &str,
        width: f64,
        height: f64,
    ) -> Result<Option<ResolvedLabel>, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let mut display_text = String::new();
        display_text.try_reserve_exact(text.len())?;
        let mut missing_codepoints = Vec::new();
        for character in text.chars() {
            if face.glyph_index(character).is_some() {
                display_text.push(character);
            } else {
                display_text.push('?');
                missing_codepoints.try_reserve(1)?;
                missing_codepoints.push(u32::from(character));
            }
        }
        let glyphs = shape_text(&face, &display_text);
        let advance_units = glyphs
            .glyph_positions()
            .iter()
            .fold(0_i64, |advance, position| {
                advance.saturating_add(i64::from(position.x_advance))
            });
        let units_per_em = f64::from(face.units_per_em());
        let scale = FALLBACK_FONT_SIZE / units_per_em;
        let advance = advance_units as f64 * scale;
        if advance > width {
            return Ok(None);
        }
        let origin_x = (width - advance) / 2.0;
        let baseline = height / 2.0 + f64::from(face.ascender() + face.descender()) * scale / 2.0;
        let mut operations = Vec::new();
        let estimated_operations = glyphs.glyph_infos().len().saturating_mul(32);
        operations.try_reserve(estimated_operations)?;
        let mut pen_x = 0_i64;
        let mut pen_y = 0_i64;
        let mut resolved_glyphs = Vec::new();
        resolved_glyphs.try_reserve_exact(glyphs.glyph_infos().len())?;
        for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            let mut builder = GlyphOutlineBuilder {
                operations: &mut operations,
                failed: false,
                origin_x,
                baseline,
                glyph_x: (pen_x + i64::from(position.x_offset)) as f64,
                glyph_y: (pen_y + i64::from(position.y_offset)) as f64,
                scale,
            };
            let glyph_id = u16::try_from(info.glyph_id).map_err(|_| LabelError::InvalidFont)?;
            let cluster = usize::try_from(info.cluster).map_err(|_| LabelError::InvalidFont)?;
            let cluster_text = glyph_cluster_text(&glyphs, &display_text, cluster)?;
            if glyph_id == 0 {
                return Err(LabelError::InvalidFont);
            }
            let mut unicode = Vec::new();
            unicode.try_reserve_exact(cluster_text.encode_utf16().count())?;
            unicode.extend(cluster_text.encode_utf16());
            let advance = face
                .glyph_hor_advance(GlyphId(glyph_id))
                .unwrap_or_default();
            let units_per_em =
                u64::try_from(face.units_per_em()).map_err(|_| LabelError::InvalidFont)?;
            let advance_1000 =
                u16::try_from((u64::from(advance) * 1_000 + units_per_em / 2) / units_per_em)
                    .map_err(|_| LabelError::InvalidFont)?;
            resolved_glyphs.push(ResolvedGlyph {
                id: glyph_id,
                x: origin_x + (pen_x + i64::from(position.x_offset)) as f64 * scale,
                y: baseline - (pen_y + i64::from(position.y_offset)) as f64 * scale,
                advance_1000,
                unicode,
            });
            if face
                .outline_glyph(GlyphId(glyph_id), &mut builder)
                .is_none()
                && info.glyph_id != 3
            {
                return Err(LabelError::InvalidFont);
            }
            if builder.failed {
                return Err(LabelError::Memory);
            }
            pen_x = pen_x.saturating_add(i64::from(position.x_advance));
            pen_y = pen_y.saturating_add(i64::from(position.y_advance));
        }
        let mut owned_text = String::new();
        owned_text.try_reserve_exact(text.len())?;
        owned_text.push_str(text);
        Ok(Some(ResolvedLabel {
            text: owned_text,
            display_text,
            baseline,
            path: LabelPath { operations },
            glyph_count: u64::try_from(glyphs.glyph_infos().len()).unwrap_or(u64::MAX),
            missing_codepoints,
            glyphs: resolved_glyphs,
            runs: Vec::new(),
        }))
    }
}

impl OfficialFont {
    pub(super) fn load(font_family: i32, text: &str) -> Result<Self, LabelError> {
        let (catalog_family, family, pdf_name) = match font_family {
            1 => ("Virgil", "Virgil", "Virgil"),
            2 | 9 => ("Liberation", "Liberation Sans", "LiberationSans"),
            3 => ("Cascadia", "Cascadia Code", "CascadiaCode"),
            5 => ("Excalifont", "Excalifont", "Excalifont"),
            6 => ("Nunito", "Nunito", "Nunito"),
            7 => ("Lilita", "Lilita One", "LilitaOne"),
            8 => ("ComicShanns", "Comic Shanns", "ComicShanns"),
            _ => return Err(LabelError::InvalidFont),
        };
        let mut selected = None;
        let mut replacement_face = None;
        for record in excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .filter(|record| record.family == catalog_family)
        {
            let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
                .map_err(|_| LabelError::InvalidFont)?
                .ok_or(LabelError::InvalidFont)?;
            let sfnt = if record.upstream_path == EXCALIFONT_LATIN_PATH {
                let predecoded = excalidraw_render_assets_base::load_excalifont_basic_latin_sfnt()
                    .map_err(|_| LabelError::InvalidFont)?;
                let mut sfnt = Vec::new();
                sfnt.try_reserve_exact(predecoded.bytes().len())?;
                sfnt.extend_from_slice(predecoded.bytes());
                sfnt
            } else {
                wuff::decompress_woff2(asset.bytes()).map_err(|_| LabelError::InvalidFont)?
            };
            let face = rustybuzz::Face::from_slice(&sfnt, 0).ok_or(LabelError::InvalidFont)?;
            if text
                .chars()
                .filter(|character| *character != '\n')
                .all(|character| face.glyph_index(character).is_some())
            {
                selected = Some((record, asset, sfnt));
                break;
            }
            if face.glyph_index('?').is_some()
                && face.glyph_index('A').is_some()
                && (replacement_face.is_none() || record.upstream_path == EXCALIFONT_LATIN_PATH)
            {
                replacement_face = Some((record, asset, sfnt));
            }
        }
        let (record, asset, sfnt) = selected
            .or(replacement_face)
            .ok_or(LabelError::InvalidFont)?;
        let mut woff2 = Vec::new();
        woff2.try_reserve_exact(asset.bytes().len())?;
        woff2.extend_from_slice(asset.bytes());
        rustybuzz::Face::from_slice(&sfnt, 0).ok_or(LabelError::InvalidFont)?;
        Ok(Self {
            source_family: font_family,
            catalog_family,
            family,
            pdf_name,
            identity: record.identity,
            svg_mime: "font/woff2",
            woff2,
            sfnt,
        })
    }

    pub(super) fn layout(
        &self,
        text: &str,
        layout: TextLayout<'_>,
    ) -> Result<ResolvedText, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let text_align = ResolvedTextAlign::from_normalized(layout.text_align);
        let mut display_text = String::new();
        display_text.try_reserve_exact(text.len())?;
        let mut missing_codepoints = Vec::new();
        for character in text.chars() {
            if character == '\n' || face.glyph_index(character).is_some() {
                display_text.push(character);
            } else {
                display_text.push('?');
                missing_codepoints.try_reserve(1)?;
                missing_codepoints.push(u32::from(character));
            }
        }
        let mut stored_lines = Vec::new();
        let mut line_break_steps = 0_u64;
        let mut source_lines = text.split_inclusive('\n');
        let mut display_lines = display_text.split_inclusive('\n');
        loop {
            let source_line = source_lines.next();
            let display_line = display_lines.next();
            let (Some(source_line), Some(display_line)) = (source_line, display_line) else {
                break;
            };
            let has_newline = source_line.ends_with('\n');
            let source_content = source_line.strip_suffix('\n').unwrap_or(source_line);
            let display_content = display_line.strip_suffix('\n').unwrap_or(display_line);
            stored_lines.try_reserve(1)?;
            stored_lines.push(StoredLine {
                source: try_owned(source_content)?,
                display: try_owned(display_content)?,
            });
            line_break_steps = line_break_steps.saturating_add(1);
            if has_newline {
                let last = stored_lines.last_mut().ok_or(LabelError::InvalidFont)?;
                last.source.push('\n');
            }
        }
        if text.is_empty() || text.ends_with('\n') {
            stored_lines.try_reserve(1)?;
            stored_lines.push(StoredLine {
                source: String::new(),
                display: String::new(),
            });
            line_break_steps = line_break_steps.saturating_add(1);
        }
        let line_step = layout.font_size * layout.line_height;
        let scale = layout.font_size / f64::from(face.units_per_em());
        // Canvas centers the font's ascender/descender box within each line box.
        let first_baseline =
            line_step / 2.0 + f64::from(face.ascender() + face.descender()) * scale / 2.0;
        let mut lines = Vec::new();
        lines.try_reserve_exact(stored_lines.len())?;
        for (index, line) in stored_lines.into_iter().enumerate() {
            let baseline = first_baseline + index as f64 * line_step;
            lines.push(shape_official_line(
                &face,
                &line.source,
                &line.display,
                layout.width,
                layout.font_size,
                baseline,
                text_align,
            )?);
        }
        Ok(ResolvedText {
            font_index: 0,
            font_family: self.family,
            font_size: layout.font_size,
            text_align,
            lines,
            scalar_count: u64::try_from(text.chars().count()).unwrap_or(u64::MAX),
            line_break_steps,
            missing_codepoints,
            noto_emoji_substituted: false,
            fallback_probes: 0,
            fallback_resources: 0,
            prepaid_glyphs: u64::try_from(
                text.chars().filter(|character| *character != '\n').count(),
            )
            .unwrap_or(u64::MAX),
            prepaid_line_break_steps: u64::try_from(text.split('\n').count()).unwrap_or(u64::MAX),
        })
    }

    pub(super) fn wrap_embeddable_label(
        &self,
        text: &str,
        font_size: f64,
        max_width: f64,
    ) -> Result<String, LabelError> {
        if max_width < 0.0 || !max_width.is_finite() {
            return try_owned(text);
        }
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let scale = font_size / f64::from(face.units_per_em());
        let mut wrapped = String::new();
        wrapped.try_reserve_exact(text.len().saturating_add(text.chars().count()))?;
        for (line_index, line) in text.split('\n').enumerate() {
            if line_index != 0 {
                wrapped.push('\n');
            }
            let width = shape_text(&face, line)
                .glyph_positions()
                .iter()
                .fold(0_i64, |sum, position| {
                    sum.saturating_add(i64::from(position.x_advance))
                }) as f64
                * scale;
            if width <= max_width {
                wrapped.push_str(line);
                continue;
            }
            let mut line_width = 0.0;
            let mut has_character = false;
            for character in line.chars() {
                let advance = face
                    .glyph_index(character)
                    .and_then(|glyph| face.glyph_hor_advance(glyph))
                    .map_or(0.0, |advance| f64::from(advance) * scale);
                if has_character && line_width + advance > max_width {
                    wrapped.push('\n');
                    line_width = 0.0;
                }
                wrapped.push(character);
                line_width += advance;
                has_character = true;
            }
        }
        Ok(wrapped)
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        Vec<u8>,
        Vec<u8>,
    ) {
        (
            self.family,
            self.pdf_name,
            self.identity,
            self.svg_mime,
            self.woff2,
            self.sfnt,
        )
    }

    fn supports(&self, character: char) -> bool {
        rustybuzz::Face::from_slice(&self.sfnt, 0)
            .and_then(|face| face.glyph_index(character))
            .is_some()
    }

    fn advance(&self, text: &str, font_size: f64) -> Result<f64, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let units = shape_text(&face, text)
            .glyph_positions()
            .iter()
            .fold(0_i64, |sum, position| {
                sum.saturating_add(i64::from(position.x_advance))
            });
        Ok(units as f64 * font_size / f64::from(face.units_per_em()))
    }

    fn load_catalog_record(
        source_family: i32,
        catalog_family: &'static str,
        family: &'static str,
        pdf_name: &'static str,
        record: &'static excalidraw_render_assets_base::AssetRecord,
    ) -> Result<Self, LabelError> {
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .map_err(|_| LabelError::InvalidFont)?
            .ok_or(LabelError::InvalidFont)?;
        let sfnt = wuff::decompress_woff2(asset.bytes()).map_err(|_| LabelError::InvalidFont)?;
        rustybuzz::Face::from_slice(&sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let mut woff2 = Vec::new();
        woff2.try_reserve_exact(asset.bytes().len())?;
        woff2.extend_from_slice(asset.bytes());
        Ok(Self {
            source_family,
            catalog_family,
            family,
            pdf_name,
            identity: record.identity,
            svg_mime: "font/woff2",
            woff2,
            sfnt,
        })
    }

    fn load_noto_emoji(character: char) -> Result<Option<Self>, LabelError> {
        let asset = excalidraw_render_assets_base::load_noto_emoji_sfnt()
            .map_err(|_| LabelError::InvalidFont)?;
        let face = rustybuzz::Face::from_slice(asset.bytes(), 0).ok_or(LabelError::InvalidFont)?;
        if face.glyph_index(character).is_none() {
            return Ok(None);
        }
        let mut sfnt = Vec::new();
        sfnt.try_reserve_exact(asset.bytes().len())?;
        sfnt.extend_from_slice(asset.bytes());
        Ok(Some(Self {
            source_family: 1_000,
            catalog_family: "NotoEmoji",
            family: "Noto Emoji",
            pdf_name: "NotoEmoji",
            identity: concat!(
                "scripts/woff2/assets/NotoEmoji-Regular.ttf@sha256:",
                "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"
            ),
            svg_mime: "font/ttf",
            woff2: sfnt.clone(),
            sfnt,
        }))
    }

    pub(super) fn matches_family(&self, font_family: i32) -> bool {
        self.source_family == font_family
            || matches!((self.source_family, font_family), (2, 9) | (9, 2))
    }
}

pub(super) fn layout_with_fallbacks(
    primary_index: usize,
    fonts: &mut Vec<OfficialFont>,
    text: &str,
    layout: TextLayout<'_>,
) -> Result<(ResolvedText, bool), LabelError> {
    let primary = fonts.get(primary_index).ok_or(LabelError::InvalidFont)?;
    let primary_face =
        rustybuzz::Face::from_slice(&primary.sfnt, 0).ok_or(LabelError::InvalidFont)?;
    let text_align = ResolvedTextAlign::from_normalized(layout.text_align);
    let line_step = layout.font_size * layout.line_height;
    let scale = layout.font_size / f64::from(primary_face.units_per_em());
    let first_baseline = line_step / 2.0
        + f64::from(primary_face.ascender() + primary_face.descender()) * scale / 2.0;
    let mut lines = Vec::new();
    let mut missing_codepoints = Vec::new();
    let mut used_noto = false;
    let mut fallback_probes = 0_u64;
    let mut resolved_characters = Vec::<(char, usize, char, bool, bool, u64)>::new();
    let mut line_break_steps = 0_u64;

    for (line_index, source_line) in text.split_inclusive('\n').enumerate() {
        let has_newline = source_line.ends_with('\n');
        let source_content = source_line.strip_suffix('\n').unwrap_or(source_line);
        let baseline = first_baseline + line_index as f64 * line_step;
        let mut selections = Vec::<(usize, bool, usize, char)>::new();
        let mut line_missing = Vec::<(usize, u32)>::new();
        selections.try_reserve_exact(source_content.chars().count())?;
        let bidi = unicode_bidi::BidiInfo::new(source_content, None);
        let visual_ranges = if let Some(paragraph) = bidi.paragraphs.first() {
            let (levels, ranges) = bidi.visual_runs(paragraph, paragraph.range.clone());
            ranges
                .into_iter()
                .map(|range| {
                    let rtl = levels.get(range.start).is_some_and(|level| level.is_rtl());
                    (range, rtl)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if visual_ranges.is_empty() {
            for (offset, character) in source_content.char_indices() {
                let (font_index, display, noto, missing) = resolve_character_cached(
                    primary_index,
                    fonts,
                    character,
                    &mut resolved_characters,
                    &mut fallback_probes,
                )?;
                selections.push((0, false, font_index, display));
                used_noto |= noto;
                if missing {
                    line_missing.try_reserve(1)?;
                    line_missing.push((offset, u32::from(character)));
                }
            }
        } else {
            for (bidi_run, (range, rtl)) in visual_ranges.into_iter().enumerate() {
                let run_text = source_content
                    .get(range.clone())
                    .ok_or(LabelError::InvalidFont)?;
                for (offset, character) in run_text.char_indices() {
                    let (font_index, display, noto, missing) = resolve_character_cached(
                        primary_index,
                        fonts,
                        character,
                        &mut resolved_characters,
                        &mut fallback_probes,
                    )?;
                    selections.push((bidi_run, rtl, font_index, display));
                    used_noto |= noto;
                    if missing {
                        line_missing.try_reserve(1)?;
                        line_missing
                            .push((range.start.saturating_add(offset), u32::from(character)));
                    }
                }
            }
        }
        line_missing.sort_unstable_by_key(|(offset, _)| *offset);
        missing_codepoints.try_reserve(line_missing.len())?;
        missing_codepoints.extend(line_missing.into_iter().map(|(_, codepoint)| codepoint));

        let mut grouped = Vec::<(usize, bool, usize, String)>::new();
        for (bidi_run, rtl, font_index, character) in selections {
            if let Some((last_bidi_run, _, last_index, run_text)) = grouped.last_mut() {
                if *last_bidi_run == bidi_run && *last_index == font_index {
                    run_text.push(character);
                    continue;
                }
            }
            grouped.try_reserve(1)?;
            let mut run_text = String::new();
            run_text.try_reserve(character.len_utf8())?;
            run_text.push(character);
            grouped.push((bidi_run, rtl, font_index, run_text));
        }

        let mut total_advance = 0.0;
        for (_, _, font_index, run_text) in &grouped {
            total_advance += fonts
                .get(*font_index)
                .ok_or(LabelError::InvalidFont)?
                .advance(run_text, layout.font_size)?;
        }
        let line_origin = match text_align {
            ResolvedTextAlign::Left => 0.0,
            ResolvedTextAlign::Center => (layout.width - total_advance) / 2.0,
            ResolvedTextAlign::Right => layout.width - total_advance,
        };
        let mut pen_x = line_origin;
        let mut operations = Vec::new();
        let mut glyphs = Vec::new();
        let mut runs = Vec::new();
        let mut display_text = String::new();
        for (_, rtl, font_index, run_text) in grouped {
            let font = fonts.get(font_index).ok_or(LabelError::InvalidFont)?;
            let face = rustybuzz::Face::from_slice(&font.sfnt, 0).ok_or(LabelError::InvalidFont)?;
            let mut run = shape_official_line(
                &face,
                &run_text,
                &run_text,
                layout.width,
                layout.font_size,
                baseline,
                ResolvedTextAlign::Left,
            )?;
            shift_label_x(&mut run, pen_x);
            let glyph_start = glyphs.len();
            operations.try_reserve(run.path.operations.len())?;
            operations.extend(run.path.operations);
            glyphs.try_reserve(run.glyphs.len())?;
            glyphs.extend(run.glyphs);
            let glyph_end = glyphs.len();
            display_text.try_reserve(run_text.len())?;
            display_text.push_str(&run_text);
            runs.try_reserve(1)?;
            runs.push(ResolvedTextRun {
                font_index,
                font_family: font.family,
                display_text: run_text.clone(),
                glyph_start,
                glyph_end,
                rtl,
            });
            pen_x += font.advance(&run_text, layout.font_size)?;
        }
        let mut owned_source = try_owned(source_content)?;
        if has_newline {
            owned_source.push('\n');
        }
        lines.try_reserve(1)?;
        lines.push(ResolvedLabel {
            text: owned_source,
            display_text,
            baseline,
            path: LabelPath { operations },
            glyph_count: u64::try_from(glyphs.len()).unwrap_or(u64::MAX),
            missing_codepoints: Vec::new(),
            glyphs,
            runs,
        });
        line_break_steps = line_break_steps.saturating_add(1);
    }
    if text.is_empty() || text.ends_with('\n') {
        lines.try_reserve(1)?;
        lines.push(ResolvedLabel {
            text: String::new(),
            display_text: String::new(),
            baseline: first_baseline + lines.len() as f64 * line_step,
            path: LabelPath {
                operations: Vec::new(),
            },
            glyph_count: 0,
            missing_codepoints: Vec::new(),
            glyphs: Vec::new(),
            runs: Vec::new(),
        });
        line_break_steps = line_break_steps.saturating_add(1);
    }
    let mut fallback_indexes = Vec::new();
    for run in lines.iter().flat_map(|line| &line.runs) {
        if run.font_index != primary_index && !fallback_indexes.contains(&run.font_index) {
            fallback_indexes.push(run.font_index);
        }
    }
    Ok((
        ResolvedText {
            font_index: primary_index,
            font_family: fonts
                .get(primary_index)
                .ok_or(LabelError::InvalidFont)?
                .family,
            font_size: layout.font_size,
            text_align,
            lines,
            scalar_count: u64::try_from(text.chars().count()).unwrap_or(u64::MAX),
            line_break_steps,
            missing_codepoints,
            noto_emoji_substituted: used_noto,
            fallback_probes,
            fallback_resources: u64::try_from(fallback_indexes.len()).unwrap_or(u64::MAX),
            prepaid_glyphs: u64::try_from(
                text.chars().filter(|character| *character != '\n').count(),
            )
            .unwrap_or(u64::MAX),
            prepaid_line_break_steps: u64::try_from(text.split('\n').count()).unwrap_or(u64::MAX),
        },
        used_noto,
    ))
}

pub(super) fn contains_bidirectional_text(text: &str) -> bool {
    unicode_bidi::BidiInfo::new(text, None).has_rtl()
}

fn resolve_character_font(
    primary_index: usize,
    fonts: &mut Vec<OfficialFont>,
    character: char,
    fallback_probes: &mut u64,
) -> Result<(usize, char, bool, bool), LabelError> {
    let primary = fonts.get(primary_index).ok_or(LabelError::InvalidFont)?;
    if primary.supports(character) {
        return Ok((primary_index, character, false, false));
    }
    let source_family = primary.source_family;
    let catalog_family = primary.catalog_family;
    let family = primary.family;
    let pdf_name = primary.pdf_name;
    if let Some(index) = select_catalog_font(
        fonts,
        source_family,
        catalog_family,
        family,
        pdf_name,
        character,
        fallback_probes,
    )? {
        return Ok((index, character, false, false));
    }
    if source_family == 5 {
        if let Some(index) = select_catalog_font(
            fonts,
            100,
            "Xiaolai",
            "Xiaolai",
            "Xiaolai",
            character,
            fallback_probes,
        )? {
            return Ok((index, character, false, false));
        }
    }
    for (index, font) in fonts.iter().enumerate() {
        if font.catalog_family == "NotoEmoji" {
            *fallback_probes = fallback_probes.saturating_add(1);
            if font.supports(character) {
                return Ok((index, character, true, false));
            }
        }
    }
    *fallback_probes = fallback_probes.saturating_add(1);
    if let Some(font) = OfficialFont::load_noto_emoji(character)? {
        fonts.try_reserve(1)?;
        fonts.push(font);
        return Ok((fonts.len() - 1, character, true, false));
    }
    let replacement = select_catalog_font(
        fonts,
        9,
        "Liberation",
        "Liberation Sans",
        "LiberationSans",
        '?',
        fallback_probes,
    )?
    .ok_or(LabelError::InvalidFont)?;
    Ok((replacement, '?', false, true))
}

fn resolve_character_cached(
    primary_index: usize,
    fonts: &mut Vec<OfficialFont>,
    character: char,
    cache: &mut Vec<(char, usize, char, bool, bool, u64)>,
    fallback_probes: &mut u64,
) -> Result<(usize, char, bool, bool), LabelError> {
    if let Some((_, font_index, display, noto, missing, probes)) =
        cache.iter().find(|(cached, ..)| *cached == character)
    {
        *fallback_probes = fallback_probes.saturating_add(*probes);
        return Ok((*font_index, *display, *noto, *missing));
    }
    let probes_before = *fallback_probes;
    let resolved = resolve_character_font(primary_index, fonts, character, fallback_probes)?;
    let probes = fallback_probes.saturating_sub(probes_before);
    cache.try_reserve(1)?;
    cache.push((
        character, resolved.0, resolved.1, resolved.2, resolved.3, probes,
    ));
    Ok(resolved)
}

fn select_catalog_font(
    fonts: &mut Vec<OfficialFont>,
    source_family: i32,
    catalog_family: &'static str,
    family: &'static str,
    pdf_name: &'static str,
    character: char,
    fallback_probes: &mut u64,
) -> Result<Option<usize>, LabelError> {
    let mut records = Vec::new();
    for record in excalidraw_render_assets_base::revision_1()
        .assets
        .iter()
        .filter(|record| record.family == catalog_family)
    {
        records.try_reserve(1)?;
        records.push(record);
    }
    records.sort_unstable_by_key(|record| {
        excalidraw_render_assets_base::font_face_order(catalog_family, record.upstream_path)
            .unwrap_or(usize::MAX)
    });
    for record in records {
        *fallback_probes = fallback_probes.saturating_add(1);
        if let Some(index) = fonts
            .iter()
            .position(|font| font.identity == record.identity)
        {
            if fonts[index].supports(character) {
                return Ok(Some(index));
            }
            continue;
        }
        let font = OfficialFont::load_catalog_record(
            source_family,
            catalog_family,
            family,
            pdf_name,
            record,
        )?;
        if font.supports(character) {
            fonts.try_reserve(1)?;
            fonts.push(font);
            return Ok(Some(fonts.len() - 1));
        }
    }
    Ok(None)
}

fn shift_label_x(label: &mut ResolvedLabel, delta: f64) {
    for operation in &mut label.path.operations {
        match operation {
            LabelPathOperation::Move { x, .. } | LabelPathOperation::Line { x, .. } => *x += delta,
            LabelPathOperation::Quadratic { control_x, x, .. } => {
                *control_x += delta;
                *x += delta;
            }
            LabelPathOperation::Cubic {
                control_1_x,
                control_2_x,
                x,
                ..
            } => {
                *control_1_x += delta;
                *control_2_x += delta;
                *x += delta;
            }
            LabelPathOperation::Close => {}
        }
    }
    for glyph in &mut label.glyphs {
        glyph.x += delta;
    }
}

struct StoredLine {
    source: String,
    display: String,
}

impl ResolvedText {
    pub(super) fn glyph_count(&self) -> u64 {
        self.lines
            .iter()
            .fold(0_u64, |total, line| total.saturating_add(line.glyph_count))
    }

    pub(super) fn outline_segments(&self) -> u64 {
        self.lines.iter().fold(0_u64, |total, line| {
            total.saturating_add(u64::try_from(line.path.operations.len()).unwrap_or(u64::MAX))
        })
    }

    pub(super) fn run_count(&self) -> u64 {
        self.lines.iter().fold(0_u64, |total, line| {
            total.saturating_add(if line.runs.is_empty() {
                1
            } else {
                u64::try_from(line.runs.len()).unwrap_or(u64::MAX)
            })
        })
    }

    pub(super) fn fallback_resource_count(&self) -> u64 {
        self.fallback_resources
    }

    pub(super) fn retain_font_indices(&self, indexes: &mut Vec<usize>) {
        if self.lines.iter().all(|line| line.runs.is_empty()) {
            if !indexes.contains(&self.font_index) {
                indexes.push(self.font_index);
            }
            return;
        }
        for run in self.lines.iter().flat_map(|line| &line.runs) {
            if !indexes.contains(&run.font_index) {
                indexes.push(run.font_index);
            }
        }
    }

    pub(super) fn remap_font_indices(
        &mut self,
        mapping: &[Option<usize>],
    ) -> Result<(), LabelError> {
        let remapped_primary = mapping.get(self.font_index).copied().flatten();
        for run in self.lines.iter_mut().flat_map(|line| &mut line.runs) {
            run.font_index = mapping
                .get(run.font_index)
                .copied()
                .flatten()
                .ok_or(LabelError::InvalidFont)?;
        }
        self.font_index = remapped_primary
            .or_else(|| {
                self.lines
                    .iter()
                    .flat_map(|line| &line.runs)
                    .next()
                    .map(|run| run.font_index)
            })
            .ok_or(LabelError::InvalidFont)?;
        Ok(())
    }
}

fn shape_official_line(
    face: &rustybuzz::Face<'_>,
    source_text: &str,
    display_text: &str,
    width: f64,
    font_size: f64,
    baseline: f64,
    text_align: ResolvedTextAlign,
) -> Result<ResolvedLabel, LabelError> {
    let glyphs = shape_text(face, display_text);
    let scale = font_size / f64::from(face.units_per_em());
    let advance = glyphs
        .glyph_positions()
        .iter()
        .fold(0_i64, |sum, position| {
            sum.saturating_add(i64::from(position.x_advance))
        }) as f64
        * scale;
    let origin_x = match text_align {
        ResolvedTextAlign::Center => (width - advance) / 2.0,
        ResolvedTextAlign::Right => width - advance,
        ResolvedTextAlign::Left => 0.0,
    };
    let mut operations = Vec::new();
    operations.try_reserve(glyphs.glyph_infos().len().saturating_mul(32))?;
    let mut missing_codepoints = Vec::new();
    let mut resolved_glyphs = Vec::new();
    resolved_glyphs.try_reserve_exact(glyphs.glyph_infos().len())?;
    let mut pen_x = 0_i64;
    let mut pen_y = 0_i64;
    for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
        let glyph_id = u16::try_from(info.glyph_id).map_err(|_| LabelError::InvalidFont)?;
        let cluster = usize::try_from(info.cluster).map_err(|_| LabelError::InvalidFont)?;
        let cluster_text = glyph_cluster_text(&glyphs, display_text, cluster)?;
        if glyph_id == 0 {
            if let Some(codepoint) = cluster_text.chars().next().map(u32::from) {
                missing_codepoints.try_reserve(1)?;
                missing_codepoints.push(codepoint);
            }
        }
        let mut unicode = Vec::new();
        unicode.try_reserve_exact(cluster_text.encode_utf16().count())?;
        unicode.extend(cluster_text.encode_utf16());
        let advance_units = face
            .glyph_hor_advance(GlyphId(glyph_id))
            .unwrap_or_default();
        let units_per_em =
            u64::try_from(face.units_per_em()).map_err(|_| LabelError::InvalidFont)?;
        let advance_1000 =
            u16::try_from((u64::from(advance_units) * 1_000 + units_per_em / 2) / units_per_em)
                .map_err(|_| LabelError::InvalidFont)?;
        resolved_glyphs.push(ResolvedGlyph {
            id: glyph_id,
            x: origin_x + (pen_x + i64::from(position.x_offset)) as f64 * scale,
            y: baseline - (pen_y + i64::from(position.y_offset)) as f64 * scale,
            advance_1000,
            unicode,
        });
        let mut builder = GlyphOutlineBuilder {
            operations: &mut operations,
            failed: false,
            origin_x,
            baseline,
            glyph_x: (pen_x + i64::from(position.x_offset)) as f64,
            glyph_y: (pen_y + i64::from(position.y_offset)) as f64,
            scale,
        };
        if face
            .outline_glyph(GlyphId(glyph_id), &mut builder)
            .is_none()
            && glyph_id != 0
            && !cluster_text.chars().all(char::is_whitespace)
        {
            return Err(LabelError::InvalidFont);
        }
        if builder.failed {
            return Err(LabelError::Memory);
        }
        pen_x = pen_x.saturating_add(i64::from(position.x_advance));
        pen_y = pen_y.saturating_add(i64::from(position.y_advance));
    }
    let mut owned_text = String::new();
    owned_text.try_reserve_exact(source_text.len())?;
    owned_text.push_str(source_text);
    let mut owned_display_text = String::new();
    owned_display_text.try_reserve_exact(display_text.len())?;
    owned_display_text.push_str(display_text);
    Ok(ResolvedLabel {
        text: owned_text,
        display_text: owned_display_text,
        baseline,
        path: LabelPath { operations },
        glyph_count: u64::try_from(glyphs.glyph_infos().len()).unwrap_or(u64::MAX),
        missing_codepoints,
        glyphs: resolved_glyphs,
        runs: Vec::new(),
    })
}

fn shape_text(face: &rustybuzz::Face<'_>, text: &str) -> rustybuzz::GlyphBuffer {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let features = [
        rustybuzz::Feature::new(Tag::from_bytes(b"liga"), 0, ..),
        rustybuzz::Feature::new(Tag::from_bytes(b"clig"), 0, ..),
    ];
    rustybuzz::shape(face, &features, buffer)
}

fn try_owned(value: &str) -> Result<String, LabelError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len())?;
    owned.push_str(value);
    Ok(owned)
}

fn glyph_cluster_text<'a>(
    glyphs: &rustybuzz::GlyphBuffer,
    text: &'a str,
    cluster: usize,
) -> Result<&'a str, LabelError> {
    let cluster_end = glyphs
        .glyph_infos()
        .iter()
        .filter_map(|candidate| usize::try_from(candidate.cluster).ok())
        .filter(|candidate| *candidate > cluster)
        .min()
        .unwrap_or(text.len());
    text.get(cluster..cluster_end)
        .ok_or(LabelError::InvalidFont)
}

pub(super) fn mandatory_notices_json(
    liberation: bool,
    official: &[super::ResolvedFont],
) -> Result<super::notices::MandatoryNotices, LabelError> {
    let full_ofl_notice =
        super::notices::liberation_sans_notice().ok_or(LabelError::InvalidFont)?;
    let ofl_start = full_ofl_notice
        .find("This Font Software is licensed")
        .map(|index| &full_ofl_notice[index..])
        .ok_or(LabelError::InvalidFont)?;
    let prefix = "[";
    let suffix = "]";
    let mut manifest = String::new();
    manifest.try_reserve_exact(
        prefix
            .len()
            .saturating_add(suffix.len())
            .saturating_add(256),
    )?;
    manifest.push_str(prefix);
    let mut separator = "";
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(official.len())?;
    ordered.extend(official);
    ordered.sort_unstable_by_key(|font| font.identity);
    let mut liberation_pending = liberation;
    for font in ordered {
        if liberation && font.identity == LIBERATION_SANS_IDENTITY {
            continue;
        }
        if liberation_pending && LIBERATION_SANS_IDENTITY < font.identity {
            push_manifest_notice(
                &mut manifest,
                &mut separator,
                LIBERATION_SANS_IDENTITY,
                full_ofl_notice,
            );
            liberation_pending = false;
        }
        if font.family == "Noto Emoji" {
            let notice = super::notices::noto_emoji_notice().ok_or(LabelError::InvalidFont)?;
            push_manifest_notice(&mut manifest, &mut separator, font.identity, notice);
            continue;
        }
        let provenance = crate::catalog::manifested_font_attribution(font.identity)
            .map_err(|_| LabelError::InvalidFont)?;
        let mut official_notice = String::new();
        if provenance.license_identity == "font.comic-shanns.mit" {
            let notice = super::notices::comic_shanns_notice().ok_or(LabelError::InvalidFont)?;
            official_notice.try_reserve_exact(notice.len())?;
            official_notice.push_str(notice);
        } else if provenance.license_identity.ends_with(".ofl-1.1") {
            official_notice.try_reserve_exact(
                provenance
                    .text
                    .len()
                    .saturating_add(2)
                    .saturating_add(ofl_start.len()),
            )?;
            official_notice.push_str(&provenance.text);
            official_notice.push_str("\n\n");
            official_notice.push_str(ofl_start);
        } else {
            return Err(LabelError::InvalidFont);
        }
        push_manifest_notice(
            &mut manifest,
            &mut separator,
            font.identity,
            &official_notice,
        );
    }
    if liberation_pending {
        push_manifest_notice(
            &mut manifest,
            &mut separator,
            LIBERATION_SANS_IDENTITY,
            full_ofl_notice,
        );
    }
    manifest.push_str(suffix);
    Ok(super::notices::MandatoryNotices::from_json(manifest))
}

fn push_manifest_notice(
    manifest: &mut String,
    separator: &mut &'static str,
    identity: &str,
    notice: &str,
) {
    manifest.push_str(separator);
    manifest.push_str("{\"asset_identity\":\"");
    manifest.push_str(identity);
    manifest.push_str("\",\"text\":\"");
    super::notices::push_json_string_content(manifest, notice);
    manifest.push_str("\"}");
    *separator = ",";
}

struct GlyphOutlineBuilder<'a> {
    operations: &'a mut Vec<LabelPathOperation>,
    failed: bool,
    origin_x: f64,
    baseline: f64,
    glyph_x: f64,
    glyph_y: f64,
    scale: f64,
}

impl GlyphOutlineBuilder<'_> {
    fn point(&self, x: f32, y: f32) -> (f64, f64) {
        (
            self.origin_x + (self.glyph_x + f64::from(x)) * self.scale,
            self.baseline - (self.glyph_y + f64::from(y)) * self.scale,
        )
    }

    fn push(&mut self, operation: LabelPathOperation) {
        if self.failed {
            return;
        }
        if self.operations.try_reserve(1).is_err() {
            self.failed = true;
        } else {
            self.operations.push(operation);
        }
    }
}

impl OutlineBuilder for GlyphOutlineBuilder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.push(LabelPathOperation::Move { x, y });
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.push(LabelPathOperation::Line { x, y });
    }

    fn quad_to(&mut self, control_x: f32, control_y: f32, x: f32, y: f32) {
        let (control_x, control_y) = self.point(control_x, control_y);
        let (x, y) = self.point(x, y);
        self.push(LabelPathOperation::Quadratic {
            control_x,
            control_y,
            x,
            y,
        });
    }

    fn curve_to(
        &mut self,
        control_1_x: f32,
        control_1_y: f32,
        control_2_x: f32,
        control_2_y: f32,
        x: f32,
        y: f32,
    ) {
        let (control_1_x, control_1_y) = self.point(control_1_x, control_1_y);
        let (control_2_x, control_2_y) = self.point(control_2_x, control_2_y);
        let (x, y) = self.point(x, y);
        self.push(LabelPathOperation::Cubic {
            control_1_x,
            control_1_y,
            control_2_x,
            control_2_y,
            x,
            y,
        });
    }

    fn close(&mut self) {
        self.push(LabelPathOperation::Close);
    }
}
