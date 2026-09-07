use std::collections::TryReserveError;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder, Tag};

use crate::font_metadata::FamilyMetrics;

const LIBERATION_SANS_PATH: &str = "fonts/Liberation/LiberationSans-Regular.woff2";
const EXCALIFONT_LATIN_PATH: &str =
    "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2";
const LIBERATION_SANS_IDENTITY: &str = concat!(
    "fonts/Liberation/LiberationSans-Regular.woff2@sha256:",
    "006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8"
);
pub(super) const FALLBACK_FONT_SIZE: f64 = 14.0;

/// Excalidraw's `FRAME_STYLE`, as far as a frame's visible name goes. The
/// export synthesizes the name as a Helvetica text element
/// (`addFrameLabelsAsTextElements`): `nameFontSize` 14 at `nameLineHeight`
/// 1.25 gives a 17.5 px line box, placed `nameOffsetY` 3 px above the frame's
/// top edge and truncated with `...` to the frame's width.
pub(super) const FRAME_NAME_FONT_FAMILY: i32 = 2;
pub(super) const FRAME_NAME_FONT_SIZE: f64 = 14.0;
pub(super) const FRAME_NAME_LINE_HEIGHT: f64 = 1.25;
pub(super) const FRAME_NAME_OFFSET_Y: f64 = 3.0;
pub(super) const FRAME_NAME_BOX_HEIGHT: f64 = FRAME_NAME_FONT_SIZE * FRAME_NAME_LINE_HEIGHT;

/// A frame's visible name laid out as the reference's synthesized text element:
/// left-aligned in a `FRAME_NAME_BOX_HEIGHT` line box, on the baseline
/// Helvetica's metadata metrics give that box, shaped with the Liberation Sans
/// substitute.
pub(super) struct FrameName {
    pub(super) label: ResolvedLabel,
    /// The synthesized text element's width: the measured advance of the name,
    /// or the frame's width once the name had to be truncated to fit it.
    pub(super) width: f64,
}

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
    /// The source family whose metadata metrics position the text. It is the
    /// element's own family, not the loaded font's: Helvetica and Liberation
    /// Sans share one font and must not share a baseline.
    pub(super) font_family: i32,
}

impl TextLayout<'_> {
    /// The first baseline for this layout, from the positioning family's
    /// metadata metrics. Every family a text element can resolve to is in the
    /// table, so a miss is the same invalid font `OfficialFont::load` rejects.
    fn first_baseline(&self) -> Result<f64, LabelError> {
        let metrics =
            FamilyMetrics::for_source_family(self.font_family).ok_or(LabelError::InvalidFont)?;
        Ok(metrics.first_baseline(self.font_size, self.font_size * self.line_height))
    }
}

/// `FONT_FAMILY.Excalifont`: the only source family whose fallback chain the
/// CJK subset trigger extends with Xiaolai.
pub(super) const EXCALIFONT_FONT_FAMILY: i32 = 5;

/// The CJK subset trigger's outcome for one resolved scene.
///
/// Xiaolai is selected by this trigger alone: when the scene's Excalifont font
/// family scalar set intersects the frozen CJK scalar table (spec §8.3.1),
/// Xiaolai follows Excalifont's own chunks for every Excalifont occurrence, in
/// glyph selection and in the SVG font demand alike. Whether a Xiaolai chunk
/// happens to carry a glyph never selects it. Evaluated once per scene so the
/// PNG and PDF glyphs and the SVG `@font-face` set cannot disagree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CjkSubsetTrigger {
    xiaolai_selected: bool,
}

impl CjkSubsetTrigger {
    /// Evaluates the trigger over the scene's Excalifont occurrences' text.
    pub(super) fn evaluate<'a>(excalifont_texts: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            xiaolai_selected: excalifont_texts
                .into_iter()
                .flat_map(str::chars)
                .any(|scalar| excalidraw_render_assets_base::is_cjk_scalar_v1(u32::from(scalar))),
        }
    }

    pub(super) fn selects_xiaolai(self) -> bool {
        self.xiaolai_selected
    }
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
    /// For each scalar of `display_text` in logical order, the index into the
    /// line's glyphs of the glyph that shows it. The glyphs themselves sit in
    /// visual order; SVG assigns the nth `x` and `y` to the nth character of
    /// the run's content, which is logical, so it positions through this.
    pub(super) logical_glyphs: Vec<usize>,
}

pub(super) struct ResolvedGlyph {
    pub(super) id: u16,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) advance_1000: u16,
    pub(super) unicode: Vec<u16>,
    /// The byte offset in the shaped display text where this glyph's cluster
    /// starts.
    pub(super) cluster: usize,
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
        let record = excalidraw_render_assets_base::find_asset(LIBERATION_SANS_PATH)
            .ok_or(LabelError::InvalidFont)?;
        let asset = excalidraw_render_assets_base::load_asset(LIBERATION_SANS_PATH)
            .map_err(|_| LabelError::InvalidFont)?
            .ok_or(LabelError::InvalidFont)?;
        let mut woff2 = Vec::new();
        woff2.try_reserve_exact(asset.bytes().len())?;
        woff2.extend_from_slice(asset.bytes());
        let sfnt = wuff::decompress_woff2(&woff2).map_err(|_| LabelError::InvalidFont)?;
        let sfnt = reviewed_decode(record, sfnt)?;
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

    /// Lays out a frame's name the way `addFrameLabelsAsTextElements` and
    /// `truncateText` do: measured at 14 px, and when wider than the frame,
    /// cut to the longest prefix that fits with `...` appended. When even one
    /// character with the dots does not fit, the reference keeps the whole
    /// name at the frame's width, and so does this.
    ///
    /// The reference measures each candidate afresh, so it also sees the kern
    /// pair between the last kept character and the first dot; this measures
    /// prefixes of the one shaped name and adds the dots' own advance, which
    /// can differ from that by one kern pair (a fraction of a pixel) exactly
    /// at the cut. It also cuts between scalar values where the reference cuts
    /// between UTF-16 code units, so a cut never splits a surrogate pair.
    pub(super) fn resolve_frame_name(
        &self,
        name: &str,
        frame_width: f64,
    ) -> Result<FrameName, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let scale = FRAME_NAME_FONT_SIZE / f64::from(face.units_per_em());
        let baseline = FamilyMetrics::for_source_family(FRAME_NAME_FONT_FAMILY)
            .ok_or(LabelError::InvalidFont)?
            .first_baseline(FRAME_NAME_FONT_SIZE, FRAME_NAME_BOX_HEIGHT);
        let (display_text, missing_codepoints) = fallback_display_text(&face, name)?;
        let glyphs = shape_text(&face, &display_text, ShapeDirection::Guessed);
        // `measureText` measures a blank line as one space.
        let measured = if display_text.is_empty() {
            shaped_advance(&face, " ", scale)
        } else {
            shaped_advance_units(&glyphs) as f64 * scale
        };
        if measured <= frame_width {
            let label = build_label(
                &face,
                &glyphs,
                name,
                display_text,
                missing_codepoints,
                0.0,
                baseline,
                scale,
            )?;
            return Ok(FrameName {
                label,
                width: measured,
            });
        }
        let dots = shaped_advance(&face, "...", scale);
        let prefix_widths = shaped_prefix_widths(&glyphs, &display_text, scale)?;
        // `truncateText` tries every prefix from the whole name down to its
        // first character; `prefix_widths[i]` is the width of the first `i`
        // characters, so index 0 (the empty prefix) is never a candidate.
        let cut = (1..prefix_widths.len())
            .rev()
            .find(|&count| prefix_widths[count] + dots <= frame_width);
        let text = match cut {
            Some(count) => {
                let end = char_boundary(name, count)?;
                let mut text = String::new();
                text.try_reserve_exact(end.saturating_add(3))?;
                text.push_str(&name[..end]);
                text.push_str("...");
                text
            }
            None => try_owned(name)?,
        };
        // The cut text has its own replaced scalars: the ones the cut removed
        // are not reported.
        let (display_text, missing_codepoints) = fallback_display_text(&face, &text)?;
        let glyphs = shape_text(&face, &display_text, ShapeDirection::Guessed);
        let label = build_label(
            &face,
            &glyphs,
            &text,
            display_text,
            missing_codepoints,
            0.0,
            baseline,
            scale,
        )?;
        Ok(FrameName {
            label,
            width: frame_width,
        })
    }

    fn shape(
        &self,
        text: &str,
        width: f64,
        height: f64,
    ) -> Result<Option<ResolvedLabel>, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let (display_text, missing_codepoints) = fallback_display_text(&face, text)?;
        let glyphs = shape_text(&face, &display_text, ShapeDirection::Guessed);
        let units_per_em = f64::from(face.units_per_em());
        let scale = FALLBACK_FONT_SIZE / units_per_em;
        let advance = shaped_advance_units(&glyphs) as f64 * scale;
        if advance > width {
            return Ok(None);
        }
        let origin_x = (width - advance) / 2.0;
        let baseline = height / 2.0 + f64::from(face.ascender() + face.descender()) * scale / 2.0;
        build_label(
            &face,
            &glyphs,
            text,
            display_text,
            missing_codepoints,
            origin_x,
            baseline,
            scale,
        )
        .map(Some)
    }
}

/// The text `face` can show: every character it has a glyph for, and `?` for
/// each it has not, with the replaced scalars in source order. `text` is one
/// line's content; a line terminator never reaches this.
fn fallback_display_text(
    face: &rustybuzz::Face<'_>,
    text: &str,
) -> Result<(String, Vec<u32>), LabelError> {
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
    Ok((display_text, missing_codepoints))
}

fn shaped_advance_units(glyphs: &rustybuzz::GlyphBuffer) -> i64 {
    glyphs
        .glyph_positions()
        .iter()
        .fold(0_i64, |advance, position| {
            advance.saturating_add(i64::from(position.x_advance))
        })
}

/// The advance of `text` shaped with `face`, in logical pixels at `scale`.
fn shaped_advance(face: &rustybuzz::Face<'_>, text: &str, scale: f64) -> f64 {
    shaped_advance_units(&shape_text(face, text, ShapeDirection::Guessed)) as f64 * scale
}

/// The advance of every character-count prefix of `display_text`, from the
/// empty prefix to the whole text, in logical pixels. A glyph belongs to the
/// prefix that contains the character its cluster starts at, so the widths
/// keep the kerning the shaper applied inside each prefix.
fn shaped_prefix_widths(
    glyphs: &rustybuzz::GlyphBuffer,
    display_text: &str,
    scale: f64,
) -> Result<Vec<f64>, LabelError> {
    let mut advances = Vec::new();
    advances.try_reserve_exact(glyphs.len())?;
    for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
        let cluster = usize::try_from(info.cluster).map_err(|_| LabelError::InvalidFont)?;
        advances.push((cluster, i64::from(position.x_advance)));
    }
    advances.sort_unstable_by_key(|(cluster, _)| *cluster);
    let mut widths = Vec::new();
    widths.try_reserve_exact(display_text.chars().count().saturating_add(1))?;
    let mut next = advances.iter().peekable();
    let mut units = 0_i64;
    for boundary in display_text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(display_text.len()))
    {
        while let Some((_, advance)) = next.next_if(|(cluster, _)| *cluster < boundary) {
            units = units.saturating_add(*advance);
        }
        widths.push(units as f64 * scale);
    }
    Ok(widths)
}

/// The byte offset where the first `count` characters of `text` end.
fn char_boundary(text: &str, count: usize) -> Result<usize, LabelError> {
    text.char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .nth(count)
        .ok_or(LabelError::InvalidFont)
}

/// Builds the label for one shaped line of fallback-face text with its pen
/// origin at `origin_x` on `baseline`.
#[allow(clippy::too_many_arguments)]
fn build_label(
    face: &rustybuzz::Face<'_>,
    glyphs: &rustybuzz::GlyphBuffer,
    text: &str,
    display_text: String,
    missing_codepoints: Vec<u32>,
    origin_x: f64,
    baseline: f64,
    scale: f64,
) -> Result<ResolvedLabel, LabelError> {
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
        let cluster_text = glyph_cluster_text(glyphs, &display_text, cluster)?;
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
            cluster,
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
    Ok(ResolvedLabel {
        text: owned_text,
        display_text,
        baseline,
        path: LabelPath { operations },
        glyph_count: u64::try_from(glyphs.glyph_infos().len()).unwrap_or(u64::MAX),
        missing_codepoints,
        glyphs: resolved_glyphs,
        runs: Vec::new(),
    })
}

impl OfficialFont {
    pub(super) fn load(font_family: i32, text: &str) -> Result<Self, LabelError> {
        let (catalog_family, family, pdf_name) = match font_family {
            1 => ("Virgil", "Virgil", "Virgil"),
            2 | 9 => ("Liberation", "Liberation Sans", "LiberationSans"),
            3 => ("Cascadia", "Cascadia Code", "CascadiaCode"),
            EXCALIFONT_FONT_FAMILY => ("Excalifont", "Excalifont", "Excalifont"),
            6 => ("Nunito", "Nunito", "Nunito"),
            7 => ("Lilita", "Lilita One", "LilitaOne"),
            8 => ("ComicShanns", "Comic Shanns", "ComicShanns"),
            _ => return Err(LabelError::InvalidFont),
        };
        let mut chunks = OfficialFontChunks::load(catalog_family)?;
        let shaped = || {
            text.chars()
                .filter(|character| !is_line_terminator(*character))
        };
        // The face for the whole text is the first official font chunk, in
        // pinned face order, whose declared Unicode ranges cover every shaped
        // scalar; failing that, the family's basic-Latin chunk shapes what it
        // can and leaves the rest to per-scalar fallback. A chunk's cmap never
        // selects it for scalars outside its declared ranges.
        let mut selected = chunks.first_declaring(shaped)?;
        if selected.is_none() {
            selected = chunks.first_declaring(|| ['?', 'A'].into_iter())?;
        }
        let (record, sfnt) = chunks.take(selected.ok_or(LabelError::InvalidFont)?)?;
        Self::from_chunk(font_family, catalog_family, family, pdf_name, record, sfnt)
    }

    fn from_chunk(
        source_family: i32,
        catalog_family: &'static str,
        family: &'static str,
        pdf_name: &'static str,
        record: &'static excalidraw_render_assets_base::AssetRecord,
        sfnt: Vec<u8>,
    ) -> Result<Self, LabelError> {
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .map_err(|_| LabelError::InvalidFont)?
            .ok_or(LabelError::InvalidFont)?;
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

    pub(super) fn layout(
        &self,
        text: &str,
        layout: TextLayout<'_>,
    ) -> Result<ResolvedText, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let text_align = ResolvedTextAlign::from_normalized(layout.text_align);
        let mut missing_codepoints = Vec::new();
        let mut stored_lines = Vec::new();
        for line in source_lines(text) {
            let (display, line_missing) = fallback_display_text(&face, line.content)?;
            missing_codepoints.try_reserve(line_missing.len())?;
            missing_codepoints.extend(line_missing);
            stored_lines.try_reserve(1)?;
            stored_lines.push(StoredLine {
                source: line.source()?,
                display,
            });
        }
        let line_break_steps = u64::try_from(stored_lines.len()).unwrap_or(u64::MAX);
        let line_step = layout.font_size * layout.line_height;
        let first_baseline = layout.first_baseline()?;
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
                ShapeDirection::Guessed,
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
            prepaid_glyphs: prospective_glyph_count(text),
            prepaid_line_break_steps: line_count(text),
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
            let width = shape_text(&face, line, ShapeDirection::Guessed)
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

    fn advance(
        &self,
        text: &str,
        font_size: f64,
        direction: ShapeDirection,
    ) -> Result<f64, LabelError> {
        let face = rustybuzz::Face::from_slice(&self.sfnt, 0).ok_or(LabelError::InvalidFont)?;
        let units = shape_text(&face, text, direction)
            .glyph_positions()
            .iter()
            .fold(0_i64, |sum, position| {
                sum.saturating_add(i64::from(position.x_advance))
            });
        Ok(units as f64 * font_size / f64::from(face.units_per_em()))
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
    trigger: CjkSubsetTrigger,
) -> Result<(ResolvedText, bool), LabelError> {
    let text_align = ResolvedTextAlign::from_normalized(layout.text_align);
    let line_step = layout.font_size * layout.line_height;
    let first_baseline = layout.first_baseline()?;
    let mut lines = Vec::new();
    let mut missing_codepoints = Vec::new();
    let mut used_noto = false;
    let mut fallback_probes = 0_u64;
    let mut resolved_characters = Vec::<(char, usize, char, bool, bool, u64)>::new();

    for (line_index, line) in source_lines(text).enumerate() {
        let source_content = line.content;
        let baseline = first_baseline + line_index as f64 * line_step;
        let mut selections = Vec::<(usize, bool, usize, char)>::new();
        let mut line_missing = Vec::<(usize, u32)>::new();
        selections.try_reserve_exact(source_content.chars().count())?;
        let bidi = unicode_bidi::BidiInfo::new(source_content, None);
        // Spec 8.3 keeps the reference's line rule, so a UAX #9 paragraph
        // separator (U+2029, U+0085, U+001C..U+001E) stays inside the source
        // line while unicode-bidi splits the line into paragraphs at it (P1).
        // Every paragraph is content: each is laid out in order, with its own
        // base level, and the separator ends the paragraph it belongs to. The
        // paragraphs partition the line, so every scalar is in some visual run;
        // an empty line has none.
        let mut bidi_run = 0_usize;
        for paragraph in &bidi.paragraphs {
            let (levels, ranges) = bidi.visual_runs(paragraph, paragraph.range.clone());
            for range in ranges {
                let rtl = levels.get(range.start).is_some_and(|level| level.is_rtl());
                let run_text = source_content
                    .get(range.clone())
                    .ok_or(LabelError::InvalidFont)?;
                for (offset, character) in run_text.char_indices() {
                    let (font_index, display, noto, missing) = resolve_character_cached(
                        primary_index,
                        fonts,
                        character,
                        trigger,
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
                bidi_run = bidi_run.saturating_add(1);
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
        // A visual run's face runs were grouped in logical order. Every glyph
        // run is placed left to right in visual order (spec 8.3), and a
        // right-to-left visual run reads its faces right to left: the first
        // in logical order is the rightmost, so its face runs are reversed.
        let mut start = 0;
        while let Some((bidi_run, rtl, ..)) = grouped.get(start) {
            let (bidi_run, rtl) = (*bidi_run, *rtl);
            let mut end = start.saturating_add(1);
            while grouped.get(end).is_some_and(|(next, ..)| *next == bidi_run) {
                end = end.saturating_add(1);
            }
            if rtl {
                grouped
                    .get_mut(start..end)
                    .ok_or(LabelError::InvalidFont)?
                    .reverse();
            }
            start = end;
        }

        let mut total_advance = 0.0;
        for (_, rtl, font_index, run_text) in &grouped {
            total_advance += fonts
                .get(*font_index)
                .ok_or(LabelError::InvalidFont)?
                .advance(
                    run_text,
                    layout.font_size,
                    ShapeDirection::Run { rtl: *rtl },
                )?;
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
            let direction = ShapeDirection::Run { rtl };
            let mut run = shape_official_line(
                &face,
                &run_text,
                &run_text,
                layout.width,
                layout.font_size,
                baseline,
                ResolvedTextAlign::Left,
                direction,
            )?;
            shift_label_x(&mut run, pen_x);
            let glyph_start = glyphs.len();
            let logical_glyphs = logical_glyph_indexes(&run.glyphs, &run_text, rtl, glyph_start)?;
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
                logical_glyphs,
            });
            pen_x += font.advance(&run_text, layout.font_size, direction)?;
        }
        lines.try_reserve(1)?;
        lines.push(ResolvedLabel {
            text: line.source()?,
            display_text,
            baseline,
            path: LabelPath { operations },
            glyph_count: u64::try_from(glyphs.len()).unwrap_or(u64::MAX),
            missing_codepoints: Vec::new(),
            glyphs,
            runs,
        });
    }
    let line_break_steps = u64::try_from(lines.len()).unwrap_or(u64::MAX);
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
            prepaid_glyphs: prospective_glyph_count(text),
            prepaid_line_break_steps: line_count(text),
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
    trigger: CjkSubsetTrigger,
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
    if source_family == EXCALIFONT_FONT_FAMILY && trigger.selects_xiaolai() {
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
    // The Noto Emoji consultation is one font fallback probe per scalar
    // occurrence (spec 8.3.1), whether the complete face is selected now or
    // was selected by an earlier scalar: a face that already serves the
    // target is asked once and never decoded again, so what a scalar pays
    // never depends on which scalars came before it.
    *fallback_probes = fallback_probes.saturating_add(1);
    if let Some(index) = fonts
        .iter()
        .position(|font| font.catalog_family == "NotoEmoji")
    {
        if fonts
            .get(index)
            .is_some_and(|font| font.supports(character))
        {
            return Ok((index, character, true, false));
        }
    } else if let Some(font) = OfficialFont::load_noto_emoji(character)? {
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
    trigger: CjkSubsetTrigger,
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
    let resolved =
        resolve_character_font(primary_index, fonts, character, trigger, fallback_probes)?;
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
    let mut chunks = OfficialFontChunks::load(catalog_family)?;
    // Only the official font chunks whose declared Unicode range covers the
    // scalar are probed, in pinned face order; a chunk's cmap never selects it
    // for a scalar outside its declared ranges. A chunk already loaded for the
    // scene is reused rather than decoded again.
    for index in 0..chunks.len() {
        if !chunks.declares(index, character) {
            continue;
        }
        *fallback_probes = fallback_probes.saturating_add(1);
        let identity = chunks.record(index)?.identity;
        if let Some(loaded) = fonts.iter().position(|font| font.identity == identity) {
            if fonts
                .get(loaded)
                .is_some_and(|font| font.supports(character))
            {
                return Ok(Some(loaded));
            }
            continue;
        }
        if !sfnt_maps(chunks.decode(index)?, character) {
            continue;
        }
        let (record, sfnt) = chunks.take(index)?;
        fonts.try_reserve(1)?;
        fonts.push(OfficialFont::from_chunk(
            source_family,
            catalog_family,
            family,
            pdf_name,
            record,
            sfnt,
        )?);
        return Ok(Some(fonts.len() - 1));
    }
    Ok(None)
}

/// Whether one decoded face maps a scalar to a glyph.
fn sfnt_maps(sfnt: &[u8], character: char) -> bool {
    rustybuzz::Face::from_slice(sfnt, 0)
        .and_then(|face| face.glyph_index(character))
        .is_some()
}

/// Admits decoded bytes as one official font chunk's reviewed decode (spec
/// 8.6): only the sfnt whose length and digest the catalog records for the
/// chunk may shape text or be embedded. Anything else is a sealed-asset
/// failure, never a font.
fn reviewed_decode(
    record: &'static excalidraw_render_assets_base::AssetRecord,
    decoded: Vec<u8>,
) -> Result<Vec<u8>, LabelError> {
    excalidraw_render_assets_base::verify_reviewed_decode(record, decoded)
        .map(excalidraw_render_assets_base::VerifiedAsset::into_bytes)
        .map_err(|_| LabelError::InvalidFont)
}

/// One resolved font family's official font chunks in pinned face order, each
/// with its declared Unicode ranges and its sfnt decoded at most once, on
/// demand. Only chunks whose declared range is consulted ever decode.
struct OfficialFontChunks {
    chunks: Vec<OfficialFontChunk>,
}

struct OfficialFontChunk {
    order: usize,
    record: &'static excalidraw_render_assets_base::AssetRecord,
    declared: &'static [(u32, u32)],
    sfnt: Option<Vec<u8>>,
}

impl OfficialFontChunk {
    fn declares(&self, character: char) -> bool {
        let scalar = u32::from(character);
        self.declared
            .iter()
            .any(|(start, end)| (*start..=*end).contains(&scalar))
    }
}

impl OfficialFontChunks {
    fn load(catalog_family: &'static str) -> Result<Self, LabelError> {
        let mut chunks = Vec::new();
        for record in excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .filter(|record| record.family == catalog_family)
        {
            let order = excalidraw_render_assets_base::font_face_order(
                catalog_family,
                record.upstream_path,
            )
            .ok_or(LabelError::InvalidFont)?;
            let declared = excalidraw_render_assets_base::font_face_unicode_ranges(
                catalog_family,
                record.upstream_path,
            )
            .ok_or(LabelError::InvalidFont)?;
            chunks.try_reserve(1)?;
            chunks.push(OfficialFontChunk {
                order,
                record,
                declared,
                sfnt: None,
            });
        }
        chunks.sort_unstable_by_key(|chunk| chunk.order);
        Ok(Self { chunks })
    }

    /// Whether the chunk at `index` declares a scalar in its Unicode ranges.
    fn declares(&self, index: usize, character: char) -> bool {
        self.chunks
            .get(index)
            .is_some_and(|chunk| chunk.declares(character))
    }

    fn len(&self) -> usize {
        self.chunks.len()
    }

    /// The first chunk, in pinned face order, whose declared ranges cover every
    /// scalar `scalars` yields and whose cmap maps them all.
    fn first_declaring<I: Iterator<Item = char>>(
        &mut self,
        scalars: impl Fn() -> I,
    ) -> Result<Option<usize>, LabelError> {
        for index in 0..self.chunks.len() {
            let chunk = self.chunks.get(index).ok_or(LabelError::InvalidFont)?;
            if !scalars().all(|character| chunk.declares(character)) {
                continue;
            }
            let sfnt = self.decode(index)?;
            if scalars().all(|character| sfnt_maps(sfnt, character)) {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn record(
        &self,
        index: usize,
    ) -> Result<&'static excalidraw_render_assets_base::AssetRecord, LabelError> {
        self.chunks
            .get(index)
            .map(|chunk| chunk.record)
            .ok_or(LabelError::InvalidFont)
    }

    fn decode(&mut self, index: usize) -> Result<&[u8], LabelError> {
        let chunk = self.chunks.get_mut(index).ok_or(LabelError::InvalidFont)?;
        if chunk.sfnt.is_none() {
            let sfnt = if chunk.record.upstream_path == EXCALIFONT_LATIN_PATH {
                let predecoded = excalidraw_render_assets_base::load_excalifont_basic_latin_sfnt()
                    .map_err(|_| LabelError::InvalidFont)?;
                let mut sfnt = Vec::new();
                sfnt.try_reserve_exact(predecoded.bytes().len())?;
                sfnt.extend_from_slice(predecoded.bytes());
                sfnt
            } else {
                let asset = excalidraw_render_assets_base::load_asset(chunk.record.upstream_path)
                    .map_err(|_| LabelError::InvalidFont)?
                    .ok_or(LabelError::InvalidFont)?;
                wuff::decompress_woff2(asset.bytes()).map_err(|_| LabelError::InvalidFont)?
            };
            let sfnt = reviewed_decode(chunk.record, sfnt)?;
            rustybuzz::Face::from_slice(&sfnt, 0).ok_or(LabelError::InvalidFont)?;
            chunk.sfnt = Some(sfnt);
        }
        chunk.sfnt.as_deref().ok_or(LabelError::InvalidFont)
    }

    /// The record and decoded bytes of one chunk, decoding it if needed.
    fn take(
        &mut self,
        index: usize,
    ) -> Result<(&'static excalidraw_render_assets_base::AssetRecord, Vec<u8>), LabelError> {
        self.decode(index)?;
        let chunk = self.chunks.get_mut(index).ok_or(LabelError::InvalidFont)?;
        let sfnt = chunk.sfnt.take().ok_or(LabelError::InvalidFont)?;
        Ok((chunk.record, sfnt))
    }
}

/// For each scalar of `display_text` in logical order, the index of the glyph
/// in `glyphs` that shows it, offset by `first_index` so the result indexes
/// the line's glyph list the run's glyphs are appended to.
///
/// The shaper groups scalars and glyphs into clusters keyed by the byte offset
/// of the cluster's first scalar. Within a cluster, the i-th scalar is shown
/// by the i-th glyph in logical order, and scalars beyond the cluster's glyph
/// count share its last glyph; a scalar that starts no cluster belongs to the
/// last cluster starting before it. A one-glyph-per-scalar run therefore maps
/// each scalar to its own glyph, and a mark keeps the position the shaper gave
/// it over its base. `glyphs` are in the order the line places them: buffer
/// order, which for a right-to-left run (`rtl`) is visual order, reversed from
/// logical order clusters included.
fn logical_glyph_indexes(
    glyphs: &[ResolvedGlyph],
    display_text: &str,
    rtl: bool,
    first_index: usize,
) -> Result<Vec<usize>, LabelError> {
    // `(cluster, logical position)` per glyph; sorting by that total key
    // groups each cluster's glyphs in logical order.
    let mut ordered = Vec::<(usize, usize, usize)>::new();
    ordered.try_reserve_exact(glyphs.len())?;
    for (index, glyph) in glyphs.iter().enumerate() {
        let logical = if rtl {
            glyphs.len().saturating_sub(1).saturating_sub(index)
        } else {
            index
        };
        ordered.push((glyph.cluster, logical, index));
    }
    ordered.sort_unstable_by_key(|(cluster, logical, _)| (*cluster, *logical));

    let mut indexes = Vec::new();
    indexes.try_reserve_exact(display_text.chars().count())?;
    // The glyphs of the cluster the current scalar belongs to, as a range of
    // `ordered`, and how many of its scalars have been mapped.
    let mut cluster = 0..0;
    let mut mapped = 0_usize;
    for (offset, _) in display_text.char_indices() {
        while let Some((start, ..)) = ordered.get(cluster.end) {
            if *start > offset {
                break;
            }
            let mut end = cluster.end.saturating_add(1);
            while ordered.get(end).is_some_and(|(next, ..)| next == start) {
                end = end.saturating_add(1);
            }
            cluster = cluster.end..end;
            mapped = 0;
        }
        let (.., index) = ordered
            .get(
                cluster
                    .start
                    .saturating_add(mapped.min(cluster.len().saturating_sub(1))),
            )
            .filter(|_| !cluster.is_empty())
            .ok_or(LabelError::InvalidFont)?;
        indexes.push(index.saturating_add(first_index));
        mapped = mapped.saturating_add(1);
    }
    Ok(indexes)
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

/// One line of a text element's `text`, split the way the reference renders
/// it. `drawElementOnCanvas` (`renderElement.ts`) and `renderElementToSvg`
/// (`staticSvgScene.ts`) both lay out
/// `element.text.replace(/\r\n?/g, "\n").split("\n")`, so `\r\n`, `\r`, and
/// `\n` each end a line and the text always has at least one. Neither rewrites
/// `text`, so the source bytes are kept, terminator included: the per-line
/// source text (`data-source-text`, `ActualText`) still concatenates to the
/// stored `text`.
struct SourceLine<'a> {
    /// The line without its terminator: what is shaped.
    content: &'a str,
    /// `"\r\n"`, `"\r"`, `"\n"`, or `""` on the last line.
    terminator: &'a str,
}

impl SourceLine<'_> {
    /// The line's source bytes, terminator included.
    fn source(&self) -> Result<String, LabelError> {
        let mut source = String::new();
        source.try_reserve_exact(self.content.len().saturating_add(self.terminator.len()))?;
        source.push_str(self.content);
        source.push_str(self.terminator);
        Ok(source)
    }
}

/// The `SourceLine`s of one text, in order; `None` once the last line, the one
/// with no terminator, has been yielded.
struct SourceLines<'a> {
    rest: Option<&'a str>,
}

impl<'a> Iterator for SourceLines<'a> {
    type Item = SourceLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let rest = self.rest?;
        // Terminators are ASCII, so every index here is a char boundary.
        let Some(index) = rest.find(is_line_terminator) else {
            self.rest = None;
            return Some(SourceLine {
                content: rest,
                terminator: "",
            });
        };
        let terminator_len = if rest[index..].starts_with("\r\n") {
            2
        } else {
            1
        };
        let (line, remaining) = rest.split_at(index + terminator_len);
        self.rest = Some(remaining);
        Some(SourceLine {
            content: &line[..index],
            terminator: &line[index..],
        })
    }
}

/// The lines of `text` by the reference's line-ending rule; see `SourceLine`.
fn source_lines(text: &str) -> SourceLines<'_> {
    SourceLines { rest: Some(text) }
}

/// Whether `character` ends a line in the reference's text rendering.
fn is_line_terminator(character: char) -> bool {
    matches!(character, '\n' | '\r')
}

/// How many lines `text` lays out to: one more than its line terminators,
/// counting `\r\n` once. The ledger prepays one line step per line.
pub(super) fn line_count(text: &str) -> u64 {
    u64::try_from(source_lines(text).count()).unwrap_or(u64::MAX)
}

/// The scalars of `text` that reach the shaper: everything but its line
/// terminators. The ledger prepays one glyph per such scalar.
pub(super) fn prospective_glyph_count(text: &str) -> u64 {
    u64::try_from(
        text.chars()
            .filter(|character| !is_line_terminator(*character))
            .count(),
    )
    .unwrap_or(u64::MAX)
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

#[allow(clippy::too_many_arguments)]
fn shape_official_line(
    face: &rustybuzz::Face<'_>,
    source_text: &str,
    display_text: &str,
    width: f64,
    font_size: f64,
    baseline: f64,
    text_align: ResolvedTextAlign,
    direction: ShapeDirection,
) -> Result<ResolvedLabel, LabelError> {
    let glyphs = shape_text(face, display_text, direction);
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
            cluster,
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

fn shape_text(
    face: &rustybuzz::Face<'_>,
    text: &str,
    direction: ShapeDirection,
) -> rustybuzz::GlyphBuffer {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    if let ShapeDirection::Run { rtl } = direction {
        buffer.set_direction(if rtl {
            rustybuzz::Direction::RightToLeft
        } else {
            rustybuzz::Direction::LeftToRight
        });
    }
    // Guesses the script (and the direction when none is set) from the text.
    buffer.guess_segment_properties();
    let features = [
        rustybuzz::Feature::new(Tag::from_bytes(b"liga"), 0, ..),
        rustybuzz::Feature::new(Tag::from_bytes(b"clig"), 0, ..),
    ];
    rustybuzz::shape(face, &features, buffer)
}

/// The direction text is shaped in.
#[derive(Clone, Copy)]
enum ShapeDirection {
    /// The shaper's guess from the text's script: left to right unless the
    /// text carries a right-to-left script. Text shaped this way has had no
    /// UAX #9 resolution: the primary path's whole line, which has no
    /// right-to-left content or it would take `layout_with_fallbacks`; the
    /// embeddable label wrapper's width measurements; and the frame name and
    /// placeholder labels `FallbackFont` shapes whole after `?`-replacement.
    Guessed,
    /// The resolved bidi level of one visual run (spec 8.3). A right-to-left
    /// run's glyphs come back in visual order, leftmost first, with mirrored
    /// characters showing their mirror glyph when the face has one (UAX #9
    /// L4); guessing from the run's display text would shape replacement
    /// glyphs and neutrals left to right instead.
    Run { rtl: bool },
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
