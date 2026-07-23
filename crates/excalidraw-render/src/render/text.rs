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
    family: &'static str,
    pdf_name: &'static str,
    identity: &'static str,
    woff2: Vec<u8>,
    sfnt: Vec<u8>,
}

pub(super) struct ResolvedText {
    pub(super) font_index: usize,
    pub(super) font_family: &'static str,
    pub(super) font_size: f64,
    pub(super) lines: Vec<ResolvedLabel>,
    pub(super) scalar_count: u64,
    pub(super) line_break_steps: u64,
    pub(super) missing_codepoints: Vec<u32>,
    pub(super) prepaid_glyphs: u64,
    pub(super) prepaid_line_break_steps: u64,
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
        let glyphs = shape_text(&face, text);
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
        let mut missing_codepoints = Vec::new();
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
            let cluster_text = glyph_cluster_text(&glyphs, text, cluster)?;
            if glyph_id == 0 {
                if let Some(codepoint) = cluster_text.chars().next().map(u32::from) {
                    missing_codepoints.try_reserve(1)?;
                    missing_codepoints.push(codepoint);
                }
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
            display_text: try_owned(text)?,
            baseline,
            path: LabelPath { operations },
            glyph_count: u64::try_from(glyphs.glyph_infos().len()).unwrap_or(u64::MAX),
            missing_codepoints,
            glyphs: resolved_glyphs,
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
            family,
            pdf_name,
            identity: record.identity,
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
                layout.text_align,
            )?);
        }
        Ok(ResolvedText {
            font_index: 0,
            font_family: self.family,
            font_size: layout.font_size,
            lines,
            scalar_count: u64::try_from(text.chars().count()).unwrap_or(u64::MAX),
            line_break_steps,
            missing_codepoints,
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

    pub(super) fn into_parts(self) -> (&'static str, &'static str, &'static str, Vec<u8>, Vec<u8>) {
        (
            self.family,
            self.pdf_name,
            self.identity,
            self.woff2,
            self.sfnt,
        )
    }

    pub(super) fn matches_family(&self, font_family: i32) -> bool {
        self.source_family == font_family
            || matches!((self.source_family, font_family), (2, 9) | (9, 2))
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
}

fn shape_official_line(
    face: &rustybuzz::Face<'_>,
    source_text: &str,
    display_text: &str,
    width: f64,
    font_size: f64,
    baseline: f64,
    text_align: &str,
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
        "center" => (width - advance) / 2.0,
        "right" => width - advance,
        _ => 0.0,
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
) -> Result<String, LabelError> {
    let full_ofl_notice = liberation_sans_notice().ok_or(LabelError::InvalidFont)?;
    let excalifont_attribution = "Copyright (c) 2024 by Excalidraw. Excalifont is derived from Virgil by Your Own Font Foundry, with modifications by Jan Filipek / DizajnDesign. Excalifont and Virgil provenance is retained together.";
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
        let attribution = match font.family {
            "Cascadia Code" => "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code.",
            "Comic Shanns" => comic_shanns_notice().ok_or(LabelError::InvalidFont)?,
            "Excalifont" => excalifont_attribution,
            "Liberation Sans" => "Digitized data copyright (c) 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine. Copyright (c) 2012 Red Hat, Inc. with Reserved Font Name Liberation.",
            "Lilita One" => "Copyright (c) 2011 Juan Montoreano (juan@remolacha.biz), with Reserved Font Name Lilita.",
            "Nunito" => "Copyright 2014 The Nunito Project Authors (https://github.com/googlefonts/nunito).",
            "Virgil" => "Virgil by Your Own Font Foundry, with Reserved Font Name Virgil.",
            _ => return Err(LabelError::InvalidFont),
        };
        let mut official_notice = String::new();
        official_notice.try_reserve_exact(
            attribution
                .len()
                .saturating_add(2)
                .saturating_add(ofl_start.len()),
        )?;
        official_notice.push_str(attribution);
        if font.family != "Comic Shanns" {
            official_notice.push_str("\n\n");
            official_notice.push_str(ofl_start);
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
    Ok(manifest)
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
    push_json_string_content(manifest, notice);
    manifest.push_str("\"}");
    *separator = ",";
}

fn comic_shanns_notice() -> Option<&'static str> {
    const START: &str = "Comic Shanns MIT license\n==============================================================================\n\n";
    const END: &str = "\n\n==============================================================================\nSIL Open Font License 1.1";
    let bundle = excalidraw_render_assets_base::license_bundle();
    let start = bundle.find(START)?.checked_add(START.len())?;
    let tail = bundle.get(start..)?;
    tail.get(..tail.find(END)?)
}

fn liberation_sans_notice() -> Option<&'static str> {
    const START: &str = "liberation-sans.ofl-full\n";
    const END: &str = "\n\nnoto-emoji.ofl-full\n";
    let bundle = excalidraw_render_assets_base::license_bundle();
    let start = bundle.find(START)?.checked_add(START.len())?;
    let tail = bundle.get(start..)?;
    let end = tail.find(END)?;
    tail.get(..end)
}

fn push_json_string_content(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character < '\u{20}' => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let value = character as u32;
                output.push_str("\\u00");
                output.push(char::from(HEX[((value >> 4) & 0x0f) as usize]));
                output.push(char::from(HEX[(value & 0x0f) as usize]));
            }
            character => output.push(character),
        }
    }
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
