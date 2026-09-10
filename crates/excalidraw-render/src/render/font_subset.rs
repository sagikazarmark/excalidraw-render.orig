use crate::resource_accounting::{ResourceAccount, ResourceContext, ResourceDelta, ResourcePhase};
use excalidraw_render_assets_base::{FontSubsetTableAction, FONT_SUBSET_TABLE_RULES_V1};
use skera::{subset_font, Plan, SubsetFlags};
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Arc;
use write_fonts::from_obj::FromTableRef;
use write_fonts::read::{
    collections::IntSet,
    tables::glyf::CurvePoint,
    types::{GlyphId, NameId, Tag},
    FontRef, TableProvider,
};
use write_fonts::tables::glyf::{Contour, GlyfLocaBuilder, Glyph, SimpleGlyph};
use write_fonts::FontBuilder;

const SUBSET_FLAGS: SubsetFlags = SubsetFlags::SUBSET_FLAGS_DEFAULT;
const ENCODER_THREADS: Option<NonZeroUsize> = NonZeroUsize::new(1);
const MAX_CACHE_ENTRIES: usize = 230;
const MAX_LAYOUT_TRAVERSAL_DEPTH: usize = 64;
const MAX_LAYOUT_TRAVERSAL_NODES: usize = 1_000_000;

#[derive(Debug)]
pub(super) struct Error {
    pub(super) stage: crate::SvgFontSubsetStage,
}

/// The container the source bytes of a subset arrive in. Every official font
/// chunk is a WOFF2 file whose reviewed decode is the sfnt the subsetter
/// plans over; the Noto Emoji fallback (#150) is shipped as the TrueType sfnt
/// itself, so its decode stage is the identity and its decoded length is its
/// manifested length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceContainer {
    Woff2,
    Sfnt,
}

#[derive(Clone, Debug)]
pub(super) struct Derived {
    pub(super) woff2: Arc<[u8]>,
    pub(super) decoded_sfnt_bytes: usize,
    pub(super) subset_sfnt_bytes: usize,
    pub(super) subset_tables: usize,
}

pub(super) struct DerivationMetrics {
    asset_identity: &'static str,
    decoded_sfnt_bytes: usize,
    subset_sfnt_bytes: usize,
    subset_woff2_bytes: usize,
    subset_tables: usize,
}

impl DerivationMetrics {
    pub(super) fn new(asset_identity: &'static str, derived: &Derived) -> Self {
        Self {
            asset_identity,
            decoded_sfnt_bytes: derived.decoded_sfnt_bytes,
            subset_sfnt_bytes: derived.subset_sfnt_bytes,
            subset_woff2_bytes: derived.woff2.len(),
            subset_tables: derived.subset_tables,
        }
    }

    pub(super) const fn asset_identity(&self) -> &'static str {
        self.asset_identity
    }
}

pub(super) fn account_derivation<A: ResourceAccount>(
    account: &mut A,
    metrics: &DerivationMetrics,
) -> Result<(), A::Error> {
    let decoded = u64::try_from(metrics.decoded_sfnt_bytes).unwrap_or(u64::MAX);
    let subset = u64::try_from(metrics.subset_sfnt_bytes).unwrap_or(u64::MAX);
    let encoded = u64::try_from(metrics.subset_woff2_bytes).unwrap_or(u64::MAX);
    let tables = u64::try_from(metrics.subset_tables).unwrap_or(u64::MAX);
    let sfnt_bound = decoded
        .checked_mul(256)
        .ok_or_else(|| account.internal_failure("SVG font subset sfnt envelope overflowed"))?;
    let woff2_bound = decoded
        .checked_mul(2)
        .ok_or_else(|| account.internal_failure("SVG font subset WOFF2 envelope overflowed"))?;
    if subset > sfnt_bound || encoded > woff2_bound || tables > decoded / 16 {
        return Err(account.internal_failure("SVG font dependency exceeded its admitted envelope"));
    }
    for event in [
        "svg_font_woff2_decode",
        "svg_font_subset_plan",
        "svg_font_subset_sfnt",
        "svg_font_woff2_encode",
        "svg_font_validate",
    ] {
        account.transact_resource(
            ResourceContext::new(ResourcePhase::EncodingAndOutcomeAssembly, event, "")
                .with_semantic_identity(metrics.asset_identity),
            ResourceDelta::default(),
        )?;
    }
    Ok(())
}

#[derive(Eq, PartialEq)]
struct CacheKey {
    asset_identity: &'static str,
    effective_alias: String,
    semantic_identity: String,
}

#[derive(Default)]
pub(super) struct Session {
    derived: VecDeque<(CacheKey, Derived)>,
    #[cfg(test)]
    hits: usize,
    #[cfg(test)]
    misses: usize,
}

impl Session {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn derive(
        &mut self,
        asset_identity: &'static str,
        source: &[u8],
        container: SourceContainer,
        scalars: &[u32],
        effective_alias: &str,
        semantic_identity: &str,
    ) -> Result<Derived, Error> {
        let key = CacheKey {
            asset_identity,
            effective_alias: effective_alias.to_owned(),
            semantic_identity: semantic_identity.to_owned(),
        };
        if let Some((_, derived)) = self.derived.iter().find(|(candidate, _)| candidate == &key) {
            #[cfg(test)]
            {
                self.hits = self.hits.saturating_add(1);
            }
            return Ok(derived.clone());
        }
        #[cfg(test)]
        {
            self.misses = self.misses.saturating_add(1);
        }
        let derived = derive(
            source,
            container,
            scalars,
            effective_alias,
            semantic_identity,
        )?;
        self.retain(key, derived.clone());
        Ok(derived)
    }

    fn retain(&mut self, key: CacheKey, derived: Derived) {
        if self.derived.len() >= MAX_CACHE_ENTRIES {
            self.derived.pop_front();
        }
        self.derived.push_back((key, derived));
    }

    #[cfg(test)]
    pub(super) const fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

pub(super) fn derive(
    source: &[u8],
    container: SourceContainer,
    scalars: &[u32],
    effective_alias: &str,
    semantic_identity: &str,
) -> Result<Derived, Error> {
    let source_sfnt = decode_source(source, container)?;
    let font = FontRef::new(&source_sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Plan,
    })?;
    validate_source_profile(&font)?;

    let input_glyphs = IntSet::<GlyphId>::empty();
    let input_unicodes = scalars.iter().copied().collect::<IntSet<u32>>();
    let drop_tables = FONT_SUBSET_TABLE_RULES_V1
        .iter()
        .filter(|rule| rule.action == FontSubsetTableAction::Drop)
        .map(|rule| Tag::new(&rule.tag))
        .collect::<IntSet<Tag>>();
    let layout_scripts = IntSet::<Tag>::all();
    let layout_features = IntSet::<Tag>::all();
    let name_ids = (0_u16..=6)
        .chain([13, 14])
        .map(NameId::new)
        .collect::<IntSet<NameId>>();
    let name_languages = [0x0409_u16].into_iter().collect::<IntSet<u16>>();
    let plan = Plan::new(
        &input_glyphs,
        &input_unicodes,
        &font,
        SUBSET_FLAGS,
        &drop_tables,
        &layout_scripts,
        &layout_features,
        &name_ids,
        &name_languages,
    );
    let subset_sfnt = subset_font(&font, &plan).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let subset_sfnt = ensure_required_cmap(&subset_sfnt)?;
    let subset_sfnt = rewrite_names(&subset_sfnt, effective_alias, semantic_identity)?;
    let subset_sfnt = normalize_simple_glyph_overlap(&subset_sfnt)?;
    let subset_sfnt = ensure_nonempty_glyf(&subset_sfnt)?;

    let subset_woff2: Arc<[u8]> = encode_woff2(&subset_sfnt)?.into();
    let decoded = wuff::decompress_woff2(&subset_woff2).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let decoded_font = FontRef::new(&decoded).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    validate_checksums(&subset_sfnt)?;
    validate_checksums(&decoded)?;
    validate_canonical_agreement(&subset_sfnt, &decoded)?;
    validate_table_policy(&font, &decoded_font)?;
    validate_layout_closure(&decoded_font)?;
    validate_mappings_and_metrics(&source_sfnt, &decoded, scalars)?;
    validate_names(&source_sfnt, &decoded, effective_alias, semantic_identity)?;
    Ok(Derived {
        woff2: subset_woff2,
        decoded_sfnt_bytes: source_sfnt.len(),
        subset_sfnt_bytes: subset_sfnt.len(),
        subset_tables: decoded_font.table_directory().table_records().len(),
    })
}

/// The encode stage: the pinned `ttf2woff2` at quality 11 with the `glyf`/`loca`
/// transformation on one thread and no metadata or private block (spec
/// 8.3.3). The qualification corpus also encodes the Noto Emoji fallback's
/// complete sfnt with it, so the canonical viewers, which admit only WOFF2,
/// see the source face through the same container the subsets use.
pub(super) fn encode_woff2(sfnt: &[u8]) -> Result<Vec<u8>, Error> {
    ttf2woff2::encode_with_options(
        sfnt,
        ttf2woff2::EncodeOptions {
            quality: ttf2woff2::BrotliQuality::from(11),
            transform_glyf_loca: true,
            threads: ENCODER_THREADS,
        },
    )
    .map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Encode,
    })
}

/// The decode stage: the sfnt the subsetter plans over. A WOFF2 chunk is
/// decoded by the pinned `wuff`; an sfnt source must parse as one whose every
/// table lies inside the bytes, so a malformed or truncated fallback face
/// fails at the same stage a malformed chunk does, and is otherwise the
/// decode of itself.
fn decode_source(source: &[u8], container: SourceContainer) -> Result<Vec<u8>, Error> {
    let decode_error = || Error {
        stage: crate::SvgFontSubsetStage::Decode,
    };
    match container {
        SourceContainer::Woff2 => wuff::decompress_woff2(source).map_err(|_| decode_error()),
        SourceContainer::Sfnt => {
            let font = FontRef::new(source).map_err(|_| decode_error())?;
            for record in font.table_directory().table_records() {
                let end = usize::try_from(record.offset())
                    .ok()
                    .zip(usize::try_from(record.length()).ok())
                    .and_then(|(offset, length)| offset.checked_add(length))
                    .ok_or_else(decode_error)?;
                if end > source.len() {
                    return Err(decode_error());
                }
            }
            let mut sfnt = Vec::new();
            sfnt.try_reserve_exact(source.len())
                .map_err(|_| decode_error())?;
            sfnt.extend_from_slice(source);
            Ok(sfnt)
        }
    }
}

fn ensure_required_cmap(sfnt: &[u8]) -> Result<Vec<u8>, Error> {
    let font = FontRef::new(sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    if font.data_for_tag(Tag::new(b"cmap")).is_some() {
        return Ok(sfnt.to_vec());
    }
    let mut builder = FontBuilder::new();
    builder.add_raw(Tag::new(b"cmap"), vec![0, 0, 0, 0]);
    builder.copy_missing_tables(font);
    Ok(builder.build())
}

fn ensure_nonempty_glyf(sfnt: &[u8]) -> Result<Vec<u8>, Error> {
    let font = FontRef::new(sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let glyf = font.data_for_tag(Tag::new(b"glyf")).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    if !glyf.as_bytes().is_empty() {
        return Ok(sfnt.to_vec());
    }
    // Chromium OTS rejects a zero-length glyf table even when every retained
    // glyph is empty (for example, a subset containing only U+0020). Encode an
    // outline-free simple .notdef glyph so WOFF2 glyf transformation retains a
    // structurally nonempty table without changing any selected visible glyph.
    let notdef = Glyph::Simple(SimpleGlyph {
        contours: vec![Contour::from(vec![CurvePoint::on_curve(0, 0)])],
        ..SimpleGlyph::default()
    });
    let glyph_count = font
        .maxp()
        .map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?
        .num_glyphs();
    let mut glyf_loca = GlyfLocaBuilder::new();
    for glyph_id in 0..glyph_count {
        let glyph = if glyph_id == 0 {
            notdef.clone()
        } else {
            Glyph::Empty
        };
        glyf_loca.add_glyph(&glyph).map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?;
    }
    let (glyf, loca, _) = glyf_loca.build();
    let mut builder = FontBuilder::new();
    builder.add_table(&glyf).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    builder.add_table(&loca).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    builder.copy_missing_tables(font);
    Ok(builder.build())
}

fn rewrite_names(sfnt: &[u8], alias: &str, identity: &str) -> Result<Vec<u8>, Error> {
    let font = FontRef::new(sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let source_name = font.data_for_tag(Tag::new(b"name")).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let prefix = identity.get(..32).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let postscript_alias = alias.replace(' ', "-");
    let rewritten = [
        (1_u16, alias.to_owned()),
        (2, "Regular".to_owned()),
        (3, format!("excalidraw-render;{alias};{identity}")),
        (4, format!("{alias} Regular {prefix}")),
        (6, format!("{postscript_alias}-Regular-{prefix}")),
    ];
    let mut records = Vec::<(u16, Vec<u8>)>::new();
    for name_id in [0_u16, 1, 2, 3, 4, 5, 6, 13, 14] {
        let bytes = if let Some((_, value)) = rewritten.iter().find(|(id, _)| *id == name_id) {
            utf16be(value)
        } else if let Some(bytes) = english_name(source_name.as_bytes(), name_id) {
            bytes.to_vec()
        } else {
            continue;
        };
        records.push((name_id, bytes));
    }
    let count = u16::try_from(records.len()).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let string_offset = 6_usize
        .checked_add(records.len().checked_mul(12).ok_or(Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?)
        .ok_or(Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?;
    let mut table = Vec::new();
    let strings_len = records
        .iter()
        .try_fold(0_usize, |total, (_, bytes)| total.checked_add(bytes.len()))
        .ok_or(Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?;
    table
        .try_reserve_exact(string_offset.checked_add(strings_len).ok_or(Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?)
        .map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?;
    table.extend_from_slice(&0_u16.to_be_bytes());
    table.extend_from_slice(&count.to_be_bytes());
    table.extend_from_slice(
        &u16::try_from(string_offset)
            .map_err(|_| Error {
                stage: crate::SvgFontSubsetStage::Sfnt,
            })?
            .to_be_bytes(),
    );
    let mut offset = 0_usize;
    for (name_id, bytes) in &records {
        table.extend_from_slice(&3_u16.to_be_bytes());
        table.extend_from_slice(&1_u16.to_be_bytes());
        table.extend_from_slice(&0x0409_u16.to_be_bytes());
        table.extend_from_slice(&name_id.to_be_bytes());
        table.extend_from_slice(
            &u16::try_from(bytes.len())
                .map_err(|_| Error {
                    stage: crate::SvgFontSubsetStage::Sfnt,
                })?
                .to_be_bytes(),
        );
        table.extend_from_slice(
            &u16::try_from(offset)
                .map_err(|_| Error {
                    stage: crate::SvgFontSubsetStage::Sfnt,
                })?
                .to_be_bytes(),
        );
        offset = offset.checked_add(bytes.len()).ok_or(Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        })?;
    }
    for (_, bytes) in records {
        table.extend_from_slice(&bytes);
    }
    let mut builder = FontBuilder::new();
    builder.add_raw(Tag::new(b"name"), table);
    builder.copy_missing_tables(font);
    let rewritten = builder.build();
    FontRef::new(&rewritten).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    Ok(rewritten)
}

fn english_name(table: &[u8], name_id: u16) -> Option<&[u8]> {
    let count = usize::from(u16::from_be_bytes(table.get(2..4)?.try_into().ok()?));
    let strings = usize::from(u16::from_be_bytes(table.get(4..6)?.try_into().ok()?));
    for index in 0..count {
        let start = 6_usize.checked_add(index.checked_mul(12)?)?;
        let record = table.get(start..start.checked_add(12)?)?;
        let platform = u16::from_be_bytes(record.get(0..2)?.try_into().ok()?);
        let language = u16::from_be_bytes(record.get(4..6)?.try_into().ok()?);
        let candidate_id = u16::from_be_bytes(record.get(6..8)?.try_into().ok()?);
        if platform != 3 || language != 0x0409 || candidate_id != name_id {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(record.get(8..10)?.try_into().ok()?));
        let offset = usize::from(u16::from_be_bytes(record.get(10..12)?.try_into().ok()?));
        let value_start = strings.checked_add(offset)?;
        return table.get(value_start..value_start.checked_add(length)?);
    }
    None
}

fn utf16be(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>()
}

fn validate_names(source: &[u8], sfnt: &[u8], alias: &str, identity: &str) -> Result<(), Error> {
    let source = FontRef::new(source).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let source_table = source.data_for_tag(Tag::new(b"name")).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let font = FontRef::new(sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let table = font.data_for_tag(Tag::new(b"name")).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    for (name_id, expected) in [
        (1_u16, alias.to_owned()),
        (2, "Regular".to_owned()),
        (3, format!("excalidraw-render;{alias};{identity}")),
        (4, format!("{alias} Regular {}", &identity[..32])),
        (
            6,
            format!("{}-Regular-{}", alias.replace(' ', "-"), &identity[..32]),
        ),
    ] {
        if english_name(table.as_bytes(), name_id) != Some(utf16be(&expected).as_slice()) {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        }
    }
    for name_id in [0_u16, 5, 13, 14] {
        if let Some(expected) = english_name(source_table.as_bytes(), name_id) {
            if english_name(table.as_bytes(), name_id) != Some(expected) {
                return Err(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                });
            }
        }
    }
    Ok(())
}

fn validate_mappings_and_metrics(
    source: &[u8],
    subset: &[u8],
    scalars: &[u32],
) -> Result<(), Error> {
    let source = rustybuzz::Face::from_slice(source, 0).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let subset = rustybuzz::Face::from_slice(subset, 0).ok_or(Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    if source.units_per_em() != subset.units_per_em()
        || source.ascender() != subset.ascender()
        || source.descender() != subset.descender()
        || source.line_gap() != subset.line_gap()
    {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        });
    }
    let mut retained_text = String::new();
    retained_text
        .try_reserve_exact(
            scalars
                .len()
                .checked_mul(char::MAX.len_utf8())
                .ok_or_else(validate_error)?,
        )
        .map_err(|_| validate_error())?;
    for scalar in scalars {
        let Some(character) = char::from_u32(*scalar) else {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        };
        if let Some(source_glyph) = source.glyph_index(character) {
            let subset_glyph = subset.glyph_index(character).ok_or(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            })?;
            if source.glyph_hor_advance(source_glyph) != subset.glyph_hor_advance(subset_glyph) {
                return Err(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                });
            }
            let mut source_outline = Outline::default();
            let mut subset_outline = Outline::default();
            let source_bounds = source.outline_glyph(source_glyph, &mut source_outline);
            let subset_bounds = subset.outline_glyph(subset_glyph, &mut subset_outline);
            if source_bounds != subset_bounds || source_outline != subset_outline {
                return Err(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                });
            }
            retained_text.push(character);
        }
    }
    for character in retained_text.chars() {
        let mut repeated = String::new();
        repeated
            .try_reserve(character.len_utf8() * 3)
            .map_err(|_| Error {
                stage: crate::SvgFontSubsetStage::Validate,
            })?;
        repeated.extend([character, character, character]);
        validate_shaping_probe(
            &source,
            &subset,
            &repeated,
            Some(rustybuzz::Direction::LeftToRight),
        )?;
        validate_shaping_probe(
            &source,
            &subset,
            &repeated,
            Some(rustybuzz::Direction::RightToLeft),
        )?;
    }
    for probe in ["fi", "fl", "ffi", "office", "لا"] {
        if probe
            .chars()
            .all(|character| retained_text.contains(character))
        {
            validate_shaping_probe(&source, &subset, probe, None)?;
        }
    }
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(retained_text.chars().count())
        .map_err(|_| validate_error())?;
    retained.extend(retained_text.chars());
    for chunk in retained.chunks(32).filter(|chunk| chunk.len() > 1) {
        let probe = fallible_probe(chunk)?;
        validate_shaping_probe(&source, &subset, &probe, None)?;
    }
    for width in 2..=4 {
        for window in retained.windows(width) {
            let probe = fallible_probe(window)?;
            validate_shaping_probe(&source, &subset, &probe, None)?;
        }
    }
    Ok(())
}

fn validate_layout_closure(font: &FontRef<'_>) -> Result<(), Error> {
    let glyph_count = u32::from(font.maxp().map_err(|_| validate_error())?.num_glyphs());
    let mut glyphs = Vec::new();
    glyphs
        .try_reserve_exact(usize::try_from(glyph_count).map_err(|_| validate_error())?)
        .map_err(|_| validate_error())?;
    glyphs.resize(
        usize::try_from(glyph_count).map_err(|_| validate_error())?,
        false,
    );
    let mut budget = MAX_LAYOUT_TRAVERSAL_NODES;

    if font.data_for_tag(Tag::new(b"GSUB")).is_some() {
        let table = font.gsub().map_err(|_| validate_error())?;
        validate_layout_table(&table, glyph_count, &mut glyphs, &mut budget, 0)?;
        validate_single_substitution_deltas(&table, glyph_count)?;
    }
    if font.data_for_tag(Tag::new(b"GPOS")).is_some() {
        let table = font.gpos().map_err(|_| validate_error())?;
        validate_layout_table(&table, glyph_count, &mut glyphs, &mut budget, 0)?;
    }
    if font.data_for_tag(Tag::new(b"GDEF")).is_some() {
        let table = font.gdef().map_err(|_| validate_error())?;
        validate_layout_table(&table, glyph_count, &mut glyphs, &mut budget, 0)?;
    }
    Ok(())
}

fn fallible_probe(characters: &[char]) -> Result<String, Error> {
    let bytes = characters
        .iter()
        .try_fold(0_usize, |total, character| {
            total.checked_add(character.len_utf8())
        })
        .ok_or_else(validate_error)?;
    let mut probe = String::new();
    probe
        .try_reserve_exact(bytes)
        .map_err(|_| validate_error())?;
    probe.extend(characters);
    Ok(probe)
}

#[cfg(test)]
pub(super) fn layout_glyph_ids(font: &FontRef<'_>) -> Option<Vec<bool>> {
    let glyph_count = u32::from(font.maxp().ok()?.num_glyphs());
    let mut glyphs = vec![false; usize::try_from(glyph_count).ok()?];
    let mut budget = MAX_LAYOUT_TRAVERSAL_NODES;
    if font.data_for_tag(Tag::new(b"GSUB")).is_some() {
        validate_layout_table(&font.gsub().ok()?, glyph_count, &mut glyphs, &mut budget, 0).ok()?;
    }
    if font.data_for_tag(Tag::new(b"GPOS")).is_some() {
        validate_layout_table(&font.gpos().ok()?, glyph_count, &mut glyphs, &mut budget, 0).ok()?;
    }
    if font.data_for_tag(Tag::new(b"GDEF")).is_some() {
        validate_layout_table(&font.gdef().ok()?, glyph_count, &mut glyphs, &mut budget, 0).ok()?;
    }
    Some(glyphs)
}

fn validate_layout_table<'a>(
    table: &(dyn write_fonts::read::traversal::SomeTable<'a> + 'a),
    glyph_count: u32,
    glyphs: &mut [bool],
    budget: &mut usize,
    depth: usize,
) -> Result<(), Error> {
    if depth > MAX_LAYOUT_TRAVERSAL_DEPTH {
        return Err(validate_error());
    }
    let validates_class_range = table.type_name() == "ClassDefFormat1";
    let mut class_start = None;
    let mut class_count = None;
    for field in table.iter() {
        if validates_class_range {
            match (&field.name, &field.value) {
                (&"start_glyph_id", write_fonts::read::traversal::FieldType::GlyphId16(glyph)) => {
                    class_start = Some(glyph.to_u32());
                }
                (&"class_value_array", write_fonts::read::traversal::FieldType::Array(array)) => {
                    class_count = Some(array.len());
                }
                _ => {}
            }
        }
        validate_layout_field(field.value, glyph_count, glyphs, budget, depth)?;
    }
    if let (Some(start), Some(count)) = (class_start, class_count) {
        let end = start
            .checked_add(u32::try_from(count).map_err(|_| validate_error())?)
            .ok_or_else(validate_error)?;
        if end > glyph_count {
            return Err(validate_error());
        }
    }
    Ok(())
}

fn validate_layout_field<'a>(
    field: write_fonts::read::traversal::FieldType<'a>,
    glyph_count: u32,
    glyphs: &mut [bool],
    budget: &mut usize,
    depth: usize,
) -> Result<(), Error> {
    *budget = budget.checked_sub(1).ok_or_else(validate_error)?;
    use write_fonts::read::traversal::FieldType;
    match field {
        FieldType::GlyphId16(glyph) => {
            if glyph.to_u32() >= glyph_count {
                return Err(validate_error());
            }
            *glyphs
                .get_mut(usize::from(glyph.to_u16()))
                .ok_or_else(validate_error)? = true;
        }
        FieldType::GlyphId24(glyph) if glyph.to_u32() >= glyph_count => {
            return Err(validate_error());
        }
        FieldType::GlyphId24(_) => {}
        FieldType::ResolvedOffset(offset) => {
            let target = offset.target.map_err(|_| validate_error())?;
            validate_layout_table(target.as_ref(), glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::ArrayOffset(offset) => {
            let target = offset.target.map_err(|_| validate_error())?;
            validate_layout_array(target.as_ref(), glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::Record(record) => {
            validate_layout_table(&record, glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::Array(array) => {
            validate_layout_array(array.as_ref(), glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::BareOffset(offset) if offset.to_u32() != 0 => {
            return Err(validate_error());
        }
        _ => {}
    }
    Ok(())
}

fn validate_layout_array<'a>(
    array: &(dyn write_fonts::read::traversal::SomeArray<'a> + 'a),
    glyph_count: u32,
    glyphs: &mut [bool],
    budget: &mut usize,
    depth: usize,
) -> Result<(), Error> {
    if depth > MAX_LAYOUT_TRAVERSAL_DEPTH || array.len() > *budget {
        return Err(validate_error());
    }
    for index in 0..array.len() {
        let field = array.get(index).ok_or_else(validate_error)?;
        validate_layout_field(field, glyph_count, glyphs, budget, depth)?;
    }
    Ok(())
}

fn validate_single_substitution_deltas(
    gsub: &write_fonts::read::tables::gsub::Gsub<'_>,
    glyph_count: u32,
) -> Result<(), Error> {
    use write_fonts::read::tables::gsub::{SingleSubst, SubstitutionSubtables};
    for lookup in gsub
        .lookup_list()
        .map_err(|_| validate_error())?
        .lookups()
        .iter()
    {
        let lookup = lookup.map_err(|_| validate_error())?;
        let SubstitutionSubtables::Single(subtables) =
            lookup.subtables().map_err(|_| validate_error())?
        else {
            continue;
        };
        for subtable in subtables.iter() {
            if let SingleSubst::Format1(single) = subtable.map_err(|_| validate_error())? {
                let delta = i32::from(single.delta_glyph_id());
                for glyph in single.coverage().map_err(|_| validate_error())?.iter() {
                    let substitute = (i32::from(glyph.to_u16()) + delta) & 0xffff;
                    if u32::try_from(substitute).map_err(|_| validate_error())? >= glyph_count {
                        return Err(validate_error());
                    }
                }
            }
        }
    }
    Ok(())
}

const fn validate_error() -> Error {
    Error {
        stage: crate::SvgFontSubsetStage::Validate,
    }
}

fn validate_shaping_probe(
    source: &rustybuzz::Face<'_>,
    subset: &rustybuzz::Face<'_>,
    text: &str,
    direction: Option<rustybuzz::Direction>,
) -> Result<(), Error> {
    let shape = |face: &rustybuzz::Face<'_>| {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        if let Some(direction) = direction {
            buffer.set_direction(direction);
        }
        buffer.guess_segment_properties();
        rustybuzz::shape(face, &[], buffer)
    };
    let source_shape = shape(source);
    let subset_shape = shape(subset);
    if source_shape.glyph_infos().len() != subset_shape.glyph_infos().len()
        || source_shape
            .glyph_infos()
            .iter()
            .zip(subset_shape.glyph_infos())
            .any(|(source, subset)| source.cluster != subset.cluster)
        || source_shape
            .glyph_positions()
            .iter()
            .zip(subset_shape.glyph_positions())
            .any(|(source, subset)| {
                source.x_advance != subset.x_advance
                    || source.y_advance != subset.y_advance
                    || source.x_offset != subset.x_offset
                    || source.y_offset != subset.y_offset
            })
    {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        });
    }
    for (source_info, subset_info) in source_shape
        .glyph_infos()
        .iter()
        .zip(subset_shape.glyph_infos())
    {
        let source_id = u16::try_from(source_info.glyph_id).map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        let subset_id = u16::try_from(subset_info.glyph_id).map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        let mut source_outline = Outline::default();
        let mut subset_outline = Outline::default();
        let source_bounds = source.outline_glyph(
            rustybuzz::ttf_parser::GlyphId(source_id),
            &mut source_outline,
        );
        let subset_bounds = subset.outline_glyph(
            rustybuzz::ttf_parser::GlyphId(subset_id),
            &mut subset_outline,
        );
        if source_bounds != subset_bounds || source_outline != subset_outline {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        }
    }
    Ok(())
}

fn validate_source_profile(font: &FontRef<'_>) -> Result<(), Error> {
    for record in font.table_directory().table_records() {
        if table_rule(record.tag()).is_none() {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Plan,
            });
        }
    }
    for rule in FONT_SUBSET_TABLE_RULES_V1 {
        if rule.required && font.data_for_tag(Tag::new(&rule.tag)).is_none() {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Plan,
            });
        }
    }
    let glyph_count = usize::from(
        font.maxp()
            .map_err(|_| Error {
                stage: crate::SvgFontSubsetStage::Plan,
            })?
            .num_glyphs(),
    );
    let loca = font.loca(None).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Plan,
    })?;
    let glyf = font.glyf().map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Plan,
    })?;
    if loca.len() != glyph_count || !loca.all_offsets_are_ascending() {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Plan,
        });
    }
    for glyph_id in 0..glyph_count {
        let glyph_id = u32::try_from(glyph_id).map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Plan,
        })?;
        let glyph = loca
            .get_glyf(GlyphId::new(glyph_id), &glyf)
            .map_err(|_| Error {
                stage: crate::SvgFontSubsetStage::Plan,
            })?;
        if let Some(write_fonts::read::tables::glyf::Glyph::Composite(composite)) = glyph {
            for component in composite.components() {
                if usize::from(component.glyph.to_u16()) >= glyph_count
                    || loca
                        .get_glyf(GlyphId::new(component.glyph.to_u32()), &glyf)
                        .map_err(|_| Error {
                            stage: crate::SvgFontSubsetStage::Plan,
                        })?
                        .is_none()
                {
                    return Err(Error {
                        stage: crate::SvgFontSubsetStage::Plan,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_table_policy(source: &FontRef<'_>, subset: &FontRef<'_>) -> Result<(), Error> {
    for record in subset.table_directory().table_records() {
        if table_rule(record.tag()).is_none() {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        }
    }
    for rule in FONT_SUBSET_TABLE_RULES_V1 {
        let tag = Tag::new(&rule.tag);
        let source_data = source.data_for_tag(tag);
        let subset_data = subset.data_for_tag(tag);
        match rule.action {
            FontSubsetTableAction::Drop if subset_data.is_some() => {
                return Err(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                });
            }
            FontSubsetTableAction::PassThrough
                if subset_data.map(|data| data.as_bytes())
                    != source_data.map(|data| data.as_bytes()) =>
            {
                return Err(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                });
            }
            _ => {}
        }
        if rule.required && subset_data.is_none() {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        }
    }
    Ok(())
}

fn table_rule(tag: Tag) -> Option<&'static excalidraw_render_assets_base::FontSubsetTableRule> {
    FONT_SUBSET_TABLE_RULES_V1
        .iter()
        .find(|rule| Tag::new(&rule.tag) == tag)
}

fn canonicalize_sfnt(sfnt: &[u8]) -> Result<Vec<u8>, Error> {
    let font = FontRef::new(sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let mut builder = FontBuilder::new();
    builder.copy_missing_tables(font);
    Ok(builder.build())
}

fn validate_canonical_agreement(generated: &[u8], decoded: &[u8]) -> Result<(), Error> {
    let generated = canonicalize_sfnt(generated)?;
    let decoded = canonicalize_sfnt(decoded)?;
    let generated = FontRef::new(&generated).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let decoded = FontRef::new(&decoded).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Validate,
    })?;
    let generated_tags = generated
        .table_directory()
        .table_records()
        .iter()
        .map(|record| record.tag())
        .collect::<Vec<_>>();
    let decoded_tags = decoded
        .table_directory()
        .table_records()
        .iter()
        .map(|record| record.tag())
        .collect::<Vec<_>>();
    if generated_tags != decoded_tags {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        });
    }
    if canonical_glyf_loca(&generated)? != canonical_glyf_loca(&decoded)? {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        });
    }
    for tag in generated_tags {
        if tag == Tag::new(b"glyf") || tag == Tag::new(b"loca") {
            continue;
        }
        let generated_data = generated.data_for_tag(tag).ok_or(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        let decoded_data = decoded.data_for_tag(tag).ok_or(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        if tag == Tag::new(b"head") {
            let mut generated_head = generated_data.as_bytes().to_vec();
            let mut decoded_head = decoded_data.as_bytes().to_vec();
            generated_head
                .get_mut(8..12)
                .ok_or(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                })?
                .fill(0);
            decoded_head
                .get_mut(8..12)
                .ok_or(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                })?
                .fill(0);
            if generated_head != decoded_head {
                return Err(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                });
            }
        } else if generated_data.as_bytes() != decoded_data.as_bytes() {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        }
    }
    Ok(())
}

fn canonical_glyf_loca(font: &FontRef<'_>) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let (glyf, loca) = rebuild_glyf_loca(font, false, crate::SvgFontSubsetStage::Validate)?;
    Ok((
        write_fonts::dump_table(&glyf).map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?,
        write_fonts::dump_table(&loca).map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?,
    ))
}

fn normalize_simple_glyph_overlap(sfnt: &[u8]) -> Result<Vec<u8>, Error> {
    let font = FontRef::new(sfnt).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    let (glyf, loca) = rebuild_glyf_loca(&font, true, crate::SvgFontSubsetStage::Sfnt)?;
    if i16::from(loca.format() as u8)
        != font
            .head()
            .map_err(|_| Error {
                stage: crate::SvgFontSubsetStage::Sfnt,
            })?
            .index_to_loc_format()
    {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Sfnt,
        });
    }
    let mut builder = FontBuilder::new();
    builder.add_table(&glyf).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    builder.add_table(&loca).map_err(|_| Error {
        stage: crate::SvgFontSubsetStage::Sfnt,
    })?;
    builder.copy_missing_tables(font);
    Ok(builder.build())
}

fn rebuild_glyf_loca(
    font: &FontRef<'_>,
    clear_simple_overlap: bool,
    stage: crate::SvgFontSubsetStage,
) -> Result<
    (
        write_fonts::tables::glyf::Glyf,
        write_fonts::tables::loca::Loca,
    ),
    Error,
> {
    let glyf = font.glyf().map_err(|_| Error { stage })?;
    let loca = font.loca(None).map_err(|_| Error { stage })?;
    let glyph_count = font.maxp().map_err(|_| Error { stage })?.num_glyphs();
    let mut builder = GlyfLocaBuilder::new();
    for glyph_id in 0..glyph_count {
        let glyph_ref = loca
            .get_glyf(GlyphId::new(u32::from(glyph_id)), &glyf)
            .map_err(|_| Error { stage })?;
        if let Some(write_fonts::read::tables::glyf::Glyph::Composite(composite)) = &glyph_ref {
            for component in composite.components() {
                if component.glyph.to_u32() >= u32::from(glyph_count)
                    || loca
                        .get_glyf(GlyphId::new(component.glyph.to_u32()), &glyf)
                        .map_err(|_| Error { stage })?
                        .is_none()
                {
                    return Err(Error { stage });
                }
            }
        }
        let mut glyph = glyph_ref
            .map(|glyph| Glyph::from_table_ref(&glyph))
            .unwrap_or(Glyph::Empty);
        if clear_simple_overlap {
            if let Glyph::Simple(simple) = &mut glyph {
                simple.overlaps = false;
            }
        }
        builder.add_glyph(&glyph).map_err(|_| Error { stage })?;
    }
    let (glyf, loca, _) = builder.build();
    Ok((glyf, loca))
}

#[derive(Default, PartialEq)]
struct Outline(Vec<OutlineCommand>);

#[derive(PartialEq)]
enum OutlineCommand {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Curve(f32, f32, f32, f32, f32, f32),
    Close,
}

impl rustybuzz::ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCommand::Move(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCommand::Line(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(OutlineCommand::Quad(x1, y1, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(OutlineCommand::Curve(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.0.push(OutlineCommand::Close);
    }
}

pub(super) fn validate_checksums(sfnt: &[u8]) -> Result<(), Error> {
    if !sfnt.len().is_multiple_of(4)
        || sfnt.as_chunks::<4>().0.iter().fold(0_u32, |sum, word| {
            sum.wrapping_add(u32::from_be_bytes(*word))
        }) != 0xb1b0_afba
    {
        return Err(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        });
    }
    let count = usize::from(u16::from_be_bytes(
        sfnt.get(4..6)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            })?,
    ));
    for index in 0..count {
        let start = 12_usize
            .checked_add(index.checked_mul(16).ok_or(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            })?)
            .ok_or(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            })?;
        let record = sfnt.get(start..start + 16).ok_or(Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        let is_head = record.get(0..4) == Some(b"head");
        let expected = u32::from_be_bytes(record[4..8].try_into().map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?);
        let offset = usize::try_from(u32::from_be_bytes(record[8..12].try_into().map_err(
            |_| Error {
                stage: crate::SvgFontSubsetStage::Validate,
            },
        )?))
        .map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        let length = usize::try_from(u32::from_be_bytes(record[12..16].try_into().map_err(
            |_| Error {
                stage: crate::SvgFontSubsetStage::Validate,
            },
        )?))
        .map_err(|_| Error {
            stage: crate::SvgFontSubsetStage::Validate,
        })?;
        let data = sfnt
            .get(
                offset..offset.checked_add(length).ok_or(Error {
                    stage: crate::SvgFontSubsetStage::Validate,
                })?,
            )
            .ok_or(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            })?;
        if table_checksum(data, is_head) != expected {
            return Err(Error {
                stage: crate::SvgFontSubsetStage::Validate,
            });
        }
    }
    Ok(())
}

fn table_checksum(data: &[u8], zero_adjustment: bool) -> u32 {
    let mut sum = 0_u32;
    for (index, chunk) in data.chunks(4).enumerate() {
        let mut word = [0_u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        if zero_adjustment && index == 2 {
            word = [0; 4];
        }
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifested_zero_subset_flags_disable_every_skera_option() {
        for flag in [
            SubsetFlags::SUBSET_FLAGS_NO_HINTING,
            SubsetFlags::SUBSET_FLAGS_RETAIN_GIDS,
            SubsetFlags::SUBSET_FLAGS_DESUBROUTINIZE,
            SubsetFlags::SUBSET_FLAGS_NAME_LEGACY,
            SubsetFlags::SUBSET_FLAGS_SET_OVERLAPS_FLAG,
            SubsetFlags::SUBSET_FLAGS_PASSTHROUGH_UNRECOGNIZED,
            SubsetFlags::SUBSET_FLAGS_NOTDEF_OUTLINE,
            SubsetFlags::SUBSET_FLAGS_GLYPH_NAMES,
            SubsetFlags::SUBSET_FLAGS_NO_PRUNE_UNICODE_RANGES,
            SubsetFlags::SUBSET_FLAGS_NO_LAYOUT_CLOSURE,
            SubsetFlags::SUBSET_FLAGS_OPTIMIZE_IUP_DELTAS,
            SubsetFlags::SUBSET_FLAGS_NO_BIDI_CLOSURE,
        ] {
            assert!(!SUBSET_FLAGS.contains(flag));
        }
        assert_eq!(ENCODER_THREADS, NonZeroUsize::new(1));
    }

    #[test]
    fn subset_cache_evicts_in_insertion_order() {
        let mut session = Session::new();
        for index in 0..=MAX_CACHE_ENTRIES {
            session.retain(
                CacheKey {
                    asset_identity: "test-font",
                    effective_alias: "Test".to_owned(),
                    semantic_identity: format!("{index:03}"),
                },
                Derived {
                    woff2: Arc::from([u8::try_from(index % 256).unwrap_or_default()]),
                    decoded_sfnt_bytes: 0,
                    subset_sfnt_bytes: 0,
                    subset_tables: 0,
                },
            );
        }

        assert_eq!(session.derived.len(), MAX_CACHE_ENTRIES);
        assert_eq!(session.derived.front().unwrap().0.semantic_identity, "001");
        assert_eq!(session.derived.back().unwrap().0.semantic_identity, "230");
    }

    #[test]
    fn whitespace_only_subsets_keep_a_browser_admissible_glyf_table() {
        let record = excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .find(|record| record.family == "Liberation")
            .expect("Liberation source is manifested");
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("Liberation source verifies")
            .expect("Liberation source exists");
        let derived = derive(
            asset.bytes(),
            SourceContainer::Woff2,
            &[u32::from(' ')],
            "ExcalidrawRender-9",
            &"0".repeat(64),
        )
        .expect("whitespace-only subset derives");
        let sfnt = wuff::decompress_woff2(&derived.woff2).expect("subset decodes");
        let font = FontRef::new(&sfnt).expect("subset sfnt parses");

        assert!(
            font.data_for_tag(Tag::new(b"glyf"))
                .is_some_and(|table| !table.as_bytes().is_empty()),
            "Chromium OTS rejects zero-length glyf tables"
        );
    }

    #[test]
    fn malformed_woff2_sources_return_typed_failures_without_panicking() {
        let record = &excalidraw_render_assets_base::revision_1().assets[0];
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("qualified source verifies")
            .expect("qualified source exists");
        let mut bad_signature = asset.bytes().to_vec();
        bad_signature[0] ^= 0xff;
        for malformed in [
            &[][..],
            &asset.bytes()[..4],
            &asset.bytes()[..asset.bytes().len() / 2],
            bad_signature.as_slice(),
        ] {
            let result = std::panic::catch_unwind(|| {
                derive(
                    malformed,
                    SourceContainer::Woff2,
                    &[u32::from('A')],
                    "Test",
                    &"0".repeat(64),
                )
            });
            let failure = result.expect("malformed source handling does not panic");
            let error = match failure {
                Ok(_) => panic!("malformed WOFF2 is rejected"),
                Err(error) => error,
            };
            assert_eq!(error.stage, crate::SvgFontSubsetStage::Decode);
        }
    }

    /// The Noto Emoji fallback is an sfnt, not a WOFF2 chunk (#150): its
    /// decode stage is the identity, the frozen table policy admits every
    /// table it carries, and the derived subset keeps the source's outlines,
    /// advances and metrics for the requested scalars under the canonical
    /// family name, since Noto Emoji reserves no font name.
    #[test]
    fn noto_emoji_sfnt_source_derives_a_validated_woff2_subset() {
        let asset = excalidraw_render_assets_base::load_noto_emoji_sfnt()
            .expect("Noto Emoji fallback verifies");
        let scalars = [0x1f600, 0x1f389];
        let identity = "1".repeat(64);
        let derived = derive(
            asset.bytes(),
            SourceContainer::Sfnt,
            &scalars,
            "Noto Emoji",
            &identity,
        )
        .expect("Noto Emoji subset derives");

        assert_eq!(derived.decoded_sfnt_bytes, asset.bytes().len());
        assert!(derived.woff2.len() < asset.bytes().len() / 64);
        let sfnt = wuff::decompress_woff2(&derived.woff2).expect("subset decodes");
        let subset = rustybuzz::Face::from_slice(&sfnt, 0).expect("subset parses");
        let source = rustybuzz::Face::from_slice(asset.bytes(), 0).expect("source parses");
        for scalar in scalars {
            let character = char::from_u32(scalar).expect("scalar is a character");
            let source_glyph = source
                .glyph_index(character)
                .expect("source maps the emoji");
            let subset_glyph = subset
                .glyph_index(character)
                .expect("subset maps the emoji");
            assert_eq!(
                source.glyph_hor_advance(source_glyph),
                subset.glyph_hor_advance(subset_glyph)
            );
        }
        let font = FontRef::new(&sfnt).expect("subset sfnt parses");
        let table = font
            .data_for_tag(Tag::new(b"name"))
            .expect("subset has names");
        assert_eq!(
            english_name(table.as_bytes(), 1),
            Some(utf16be("Noto Emoji").as_slice())
        );
        assert_eq!(
            english_name(table.as_bytes(), 0),
            Some(utf16be("Copyright 2013 Google LLC").as_slice())
        );
        assert!(font.data_for_tag(Tag::new(b"FFTM")).is_none());
    }

    /// A truncated or non-sfnt fallback source fails at the decode stage like
    /// a malformed WOFF2 chunk does, without panicking.
    #[test]
    fn malformed_sfnt_sources_return_typed_decode_failures() {
        let asset = excalidraw_render_assets_base::load_noto_emoji_sfnt()
            .expect("Noto Emoji fallback verifies");
        for malformed in [&[][..], &asset.bytes()[..4], &asset.bytes()[..4096]] {
            let error = derive(
                malformed,
                SourceContainer::Sfnt,
                &[0x1f600],
                "Noto Emoji",
                &"1".repeat(64),
            )
            .expect_err("malformed sfnt is rejected");
            assert_eq!(error.stage, crate::SvgFontSubsetStage::Decode);
        }
    }

    #[test]
    fn malformed_generated_sfnt_validation_is_bounded_and_typed() {
        for malformed in [&[][..], &[0, 1, 0, 0][..], &[0; 12][..], &[0; 64][..]] {
            assert_eq!(
                validate_checksums(malformed)
                    .expect_err("malformed sfnt is rejected")
                    .stage,
                crate::SvgFontSubsetStage::Validate
            );
        }
    }

    #[test]
    fn malformed_layout_closure_returns_a_typed_validate_failure() {
        let record = excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .find(|record| record.family == "Liberation")
            .expect("Liberation source is manifested");
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("Liberation source verifies")
            .expect("Liberation source exists");
        let derived = derive(
            asset.bytes(),
            SourceContainer::Woff2,
            &[u32::from('A')],
            "ExcalidrawRender-9",
            &"0".repeat(64),
        )
        .expect("fixture subset derives");
        let sfnt = wuff::decompress_woff2(&derived.woff2).expect("fixture subset decodes");

        let outside_maxp = [
            0x00, 0x01, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0xff, 0xff, 0x00, 0x01, 0x00, 0x01,
        ];
        let unresolved_offset = [
            0x00, 0x01, 0x00, 0x00, 0x7f, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        for malformed_gdef in [&outside_maxp[..], &unresolved_offset[..]] {
            let font = FontRef::new(&sfnt).expect("fixture sfnt parses");
            let mut builder = FontBuilder::new();
            builder.add_raw(Tag::new(b"GDEF"), malformed_gdef.to_vec());
            builder.copy_missing_tables(font);
            let malformed = builder.build();
            let malformed = FontRef::new(&malformed).expect("outer malformed sfnt parses");

            assert_eq!(
                validate_layout_closure(&malformed)
                    .expect_err("broken layout closure is rejected")
                    .stage,
                crate::SvgFontSubsetStage::Validate
            );
        }
    }
}
