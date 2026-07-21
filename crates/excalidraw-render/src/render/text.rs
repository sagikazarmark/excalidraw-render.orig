use std::collections::TryReserveError;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};

const LIBERATION_SANS_PATH: &str = "fonts/Liberation/LiberationSans-Regular.woff2";
const LIBERATION_SANS_IDENTITY: &str = concat!(
    "fonts/Liberation/LiberationSans-Regular.woff2@sha256:",
    "006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8"
);
pub(super) const FALLBACK_FONT_SIZE: f64 = 14.0;

pub(super) struct FallbackFont {
    woff2: Vec<u8>,
    sfnt: Vec<u8>,
}

pub(super) struct ResolvedLabel {
    pub(super) text: String,
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
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let glyphs = rustybuzz::shape(&face, &[], buffer);
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
        for info in glyphs.glyph_infos() {
            if info.glyph_id != 0 {
                continue;
            }
            let cluster = usize::try_from(info.cluster).map_err(|_| LabelError::InvalidFont)?;
            if let Some(codepoint) = text
                .get(cluster..)
                .and_then(|cluster| cluster.chars().next())
                .map(u32::from)
            {
                missing_codepoints.try_reserve(1)?;
                missing_codepoints.push(codepoint);
            }
        }
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
            let cluster_end = glyphs
                .glyph_infos()
                .iter()
                .filter_map(|candidate| usize::try_from(candidate.cluster).ok())
                .filter(|candidate| *candidate > cluster)
                .min()
                .unwrap_or(text.len());
            let cluster_text = text
                .get(cluster..cluster_end)
                .ok_or(LabelError::InvalidFont)?;
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
            path: LabelPath { operations },
            glyph_count: u64::try_from(glyphs.glyph_infos().len()).unwrap_or(u64::MAX),
            missing_codepoints,
            glyphs: resolved_glyphs,
        }))
    }
}

pub(super) fn mandatory_notice_manifest() -> Result<String, LabelError> {
    let notice = liberation_sans_notice().ok_or(LabelError::InvalidFont)?;
    let prefix = concat!(
        "{\"schema_version\":1,\"provenance\":null,\"diagnostic_summary\":null,",
        "\"notices\":[{\"asset_identity\":\""
    );
    let middle = "\",\"text\":\"";
    let suffix = "\"}],\"embedded_scene\":null}";
    let mut manifest = String::new();
    let capacity = prefix
        .len()
        .saturating_add(LIBERATION_SANS_IDENTITY.len())
        .saturating_add(middle.len())
        .saturating_add(notice.len().saturating_mul(2))
        .saturating_add(suffix.len());
    manifest.try_reserve_exact(capacity)?;
    manifest.push_str(prefix);
    manifest.push_str(LIBERATION_SANS_IDENTITY);
    manifest.push_str(middle);
    push_json_string_content(&mut manifest, notice);
    manifest.push_str(suffix);
    Ok(manifest)
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
