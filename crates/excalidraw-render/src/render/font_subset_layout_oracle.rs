//! The retired traversal layout validator, kept only as the test oracle the
//! typed walker is compared against (#210, ADR-0020).
//!
//! This is the walker `font_subset::validate_layout_closure` ran on read-fonts
//! 0.42.1, unchanged except for its parser: read-fonts 0.43.3 is the last
//! release with the untyped `traversal` API. It crosses parser versions only
//! through the sfnt bytes. It leaves with its test-only dependency once the
//! typed walker has matched it.

use read_fonts_traversal::traversal::{FieldType, SomeArray, SomeTable};
use read_fonts_traversal::types::Tag;
use read_fonts_traversal::{FontRef, TableProvider};

const MAX_LAYOUT_TRAVERSAL_DEPTH: usize = 64;
const MAX_LAYOUT_TRAVERSAL_NODES: usize = 1_000_000;

/// The oracle's verdict on the GSUB, GPOS and GDEF tables of `sfnt`: whether
/// the retired walker accepted them.
pub(super) fn accepts(sfnt: &[u8]) -> bool {
    validate(sfnt).is_ok()
}

fn validate(sfnt: &[u8]) -> Result<(), ()> {
    let font = FontRef::new(sfnt).map_err(|_| ())?;
    let glyph_count = u32::from(font.maxp().map_err(|_| ())?.num_glyphs());
    let mut glyphs = vec![false; usize::try_from(glyph_count).map_err(|_| ())?];
    let mut budget = MAX_LAYOUT_TRAVERSAL_NODES;

    if font.data_for_tag(Tag::new(b"GSUB")).is_some() {
        let table = font.gsub().map_err(|_| ())?;
        validate_layout_table(&table, glyph_count, &mut glyphs, &mut budget, 0)?;
        validate_single_substitution_deltas(&table, glyph_count)?;
    }
    if font.data_for_tag(Tag::new(b"GPOS")).is_some() {
        let table = font.gpos().map_err(|_| ())?;
        validate_layout_table(&table, glyph_count, &mut glyphs, &mut budget, 0)?;
    }
    if font.data_for_tag(Tag::new(b"GDEF")).is_some() {
        let table = font.gdef().map_err(|_| ())?;
        validate_layout_table(&table, glyph_count, &mut glyphs, &mut budget, 0)?;
    }
    Ok(())
}

fn validate_layout_table<'a>(
    table: &(dyn SomeTable<'a> + 'a),
    glyph_count: u32,
    glyphs: &mut [bool],
    budget: &mut usize,
    depth: usize,
) -> Result<(), ()> {
    if depth > MAX_LAYOUT_TRAVERSAL_DEPTH {
        return Err(());
    }
    let validates_class_range = table.type_name() == "ClassDefFormat1";
    let mut class_start = None;
    let mut class_count = None;
    for field in table.iter() {
        if validates_class_range {
            match (&field.name, &field.value) {
                (&"start_glyph_id", FieldType::GlyphId16(glyph)) => {
                    class_start = Some(glyph.to_u32());
                }
                (&"class_value_array", FieldType::Array(array)) => {
                    class_count = Some(array.len());
                }
                _ => {}
            }
        }
        validate_layout_field(field.value, glyph_count, glyphs, budget, depth)?;
    }
    if let (Some(start), Some(count)) = (class_start, class_count) {
        let end = start
            .checked_add(u32::try_from(count).map_err(|_| ())?)
            .ok_or(())?;
        if end > glyph_count {
            return Err(());
        }
    }
    Ok(())
}

fn validate_layout_field<'a>(
    field: FieldType<'a>,
    glyph_count: u32,
    glyphs: &mut [bool],
    budget: &mut usize,
    depth: usize,
) -> Result<(), ()> {
    *budget = budget.checked_sub(1).ok_or(())?;
    match field {
        FieldType::GlyphId16(glyph) => {
            if glyph.to_u32() >= glyph_count {
                return Err(());
            }
            *glyphs.get_mut(usize::from(glyph.to_u16())).ok_or(())? = true;
        }
        FieldType::GlyphId24(glyph) if glyph.to_u32() >= glyph_count => {
            return Err(());
        }
        FieldType::GlyphId24(_) => {}
        FieldType::ResolvedOffset(offset) => {
            let target = offset.target.map_err(|_| ())?;
            validate_layout_table(target.as_ref(), glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::ArrayOffset(offset) => {
            let target = offset.target.map_err(|_| ())?;
            validate_layout_array(target.as_ref(), glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::Record(record) => {
            validate_layout_table(&record, glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::Array(array) => {
            validate_layout_array(array.as_ref(), glyph_count, glyphs, budget, depth + 1)?;
        }
        FieldType::BareOffset(offset) if offset.to_u32() != 0 => {
            return Err(());
        }
        _ => {}
    }
    Ok(())
}

fn validate_layout_array<'a>(
    array: &(dyn SomeArray<'a> + 'a),
    glyph_count: u32,
    glyphs: &mut [bool],
    budget: &mut usize,
    depth: usize,
) -> Result<(), ()> {
    if depth > MAX_LAYOUT_TRAVERSAL_DEPTH || array.len() > *budget {
        return Err(());
    }
    for index in 0..array.len() {
        let field = array.get(index).ok_or(())?;
        validate_layout_field(field, glyph_count, glyphs, budget, depth)?;
    }
    Ok(())
}

fn validate_single_substitution_deltas(
    gsub: &read_fonts_traversal::tables::gsub::Gsub<'_>,
    glyph_count: u32,
) -> Result<(), ()> {
    use read_fonts_traversal::tables::gsub::{SingleSubst, SubstitutionSubtables};
    for lookup in gsub.lookup_list().map_err(|_| ())?.lookups().iter() {
        let lookup = lookup.map_err(|_| ())?;
        let SubstitutionSubtables::Single(subtables) = lookup.subtables().map_err(|_| ())? else {
            continue;
        };
        for subtable in subtables.iter() {
            if let SingleSubst::Format1(single) = subtable.map_err(|_| ())? {
                let delta = i32::from(single.delta_glyph_id());
                for glyph in single.coverage().map_err(|_| ())?.iter() {
                    let substitute = (i32::from(glyph.to_u16()) + delta) & 0xffff;
                    if u32::try_from(substitute).map_err(|_| ())? >= glyph_count {
                        return Err(());
                    }
                }
            }
        }
    }
    Ok(())
}
