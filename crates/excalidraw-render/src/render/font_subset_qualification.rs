use super::{font_subset, svg_fonts};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use write_fonts::read::{types::GlyphId, FontRef, TableProvider};

#[cfg(not(target_arch = "wasm32"))]
const CORPUS_PATH: &str = concat!(
    "tests/oracle/compatibility-artifacts/",
    "revision-1-svg-font-subset-qualification-corpus.json"
);
#[cfg(not(target_arch = "wasm32"))]
const PACKAGED_CORPUS_PATH: &str =
    "crates/excalidraw-render/tests/qualification/font-subset-corpus.json";
const CORPUS_SCHEMA: &str = "excalidraw-render/svg-font-subset-qualification-corpus/v1";
const SOURCE_MANIFEST_SHA256: &str =
    "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const PROFILE_SHA256: &str = "c79d0f7905709f5177a2ec8cd502be2eed9dd0c35806dfae06642e6cf4dd4ff5";
const CLASS_IDS: [&str; 8] = [
    "mapped-scalar",
    "range-boundary",
    "empty-intersection",
    "full-intersection",
    "non-bmp",
    "combining-mark",
    "composite-closure",
    "layout-closure-heavy",
];
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Corpus {
    schema: String,
    compatibility_revision: u64,
    originating_issue: u64,
    source_manifest_sha256: String,
    subset_profile_sha256: String,
    classes: Vec<ClassDefinition>,
    sources: Vec<SourceQualification>,
    failures: Vec<FailureQualification>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClassDefinition {
    id: String,
    definition: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceQualification {
    source_identity: String,
    upstream_path: String,
    source_sha256: String,
    effective_alias: String,
    cells: Vec<QualificationCell>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualificationCell {
    class: String,
    status: CellStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordered_scalars: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic_subset_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated_woff2_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_resource_trace_sha256: Option<String>,
    expected_validation_outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CellStatus {
    Success,
    NotSelected,
    NotApplicable,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FailureQualification {
    id: String,
    expected_error_code: String,
    expected_stage: String,
    max_source_bytes: usize,
    max_scalars: usize,
    expected_generated_woff2_bytes: usize,
    complete_chunk_fallback: bool,
    partial_artifact: bool,
}

pub(crate) fn assert_complete_font_subset_qualification_corpus() {
    let catalog = excalidraw_render_assets_base::revision_1();
    let corpus_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/qualification/font-subset-corpus.json"
    ));
    let corpus: Corpus = serde_json::from_slice(corpus_bytes)
        .expect("the font subset qualification corpus is valid JSON");

    assert_eq!(corpus.schema, CORPUS_SCHEMA);
    assert_eq!(corpus.compatibility_revision, 1);
    assert_eq!(corpus.originating_issue, 98);
    assert_eq!(corpus.source_manifest_sha256, SOURCE_MANIFEST_SHA256);
    assert_eq!(corpus.subset_profile_sha256, PROFILE_SHA256);
    assert_eq!(corpus.classes.len(), CLASS_IDS.len());
    for (definition, expected) in corpus.classes.iter().zip(CLASS_IDS) {
        assert_eq!(definition.id, expected);
        assert!(!definition.definition.is_empty());
    }
    assert_eq!(corpus.sources.len(), 230);
    assert_eq!(corpus.sources.len(), catalog.assets.len());

    let mut source_identities = BTreeSet::new();
    let mut cell_identities = BTreeSet::new();
    for (source, record) in corpus.sources.iter().zip(catalog.assets) {
        assert_eq!(source.source_identity, record.identity);
        assert_eq!(source.upstream_path, record.upstream_path);
        assert_eq!(source.source_sha256, record.complete_sha256);
        assert_eq!(source.effective_alias, effective_alias(record.family));
        assert!(source_identities.insert(source.source_identity.as_str()));
        assert_eq!(source.cells.len(), CLASS_IDS.len());

        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("qualified source verifies")
            .expect("qualified source exists");
        let expected_cells = classify_source(record, asset.bytes());
        for ((cell, expected), class) in source.cells.iter().zip(expected_cells).zip(CLASS_IDS) {
            assert_eq!(cell.class, class);
            assert!(cell_identities.insert((source.source_identity.as_str(), cell.class.as_str())));
            assert_cell(record, asset.bytes(), source, cell, expected);
        }
    }
    assert_eq!(source_identities.len(), 230);
    assert_eq!(cell_identities.len(), 230 * CLASS_IDS.len());
    assert_failure_cells(&corpus.failures);
}

fn assert_cell(
    record: &'static excalidraw_render_assets_base::AssetRecord,
    source_woff2: &[u8],
    source: &SourceQualification,
    cell: &QualificationCell,
    expected: ClassifiedCell,
) {
    assert_eq!(cell.status, expected.status);
    assert_eq!(cell.ordered_scalars.as_deref(), expected.scalars.as_deref());
    match expected.status {
        CellStatus::Success => {
            assert_eq!(cell.expected_validation_outcome, "valid");
            assert!(cell.reason.is_none());
            let scalars = expected.scalars.as_deref().expect("success has scalars");
            let semantic_identity = svg_fonts::semantic_identity(record, scalars)
                .expect("qualified semantic identity can be represented");
            assert_eq!(
                cell.semantic_subset_identity.as_deref(),
                Some(semantic_identity.as_str())
            );
            let mut session = font_subset::Session::new();
            let derived = session
                .derive(
                    record.identity,
                    source_woff2,
                    scalars,
                    &source.effective_alias,
                    &semantic_identity,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{} {} derives at {:?}",
                        record.upstream_path, cell.class, error.stage
                    )
                });
            let cached = session
                .derive(
                    record.identity,
                    source_woff2,
                    scalars,
                    &source.effective_alias,
                    &semantic_identity,
                )
                .expect("qualified request-cache replay succeeds");
            assert_eq!(derived.woff2, cached.woff2);
            assert_eq!(derived.decoded_sfnt_bytes, cached.decoded_sfnt_bytes);
            assert_eq!(derived.subset_sfnt_bytes, cached.subset_sfnt_bytes);
            assert_eq!(derived.subset_tables, cached.subset_tables);
            let output_sha256 = sha256(derived.woff2.as_ref());
            if cell.generated_woff2_sha256.as_deref() != Some(output_sha256.as_str()) {
                let decoded = wuff::decompress_woff2(derived.woff2.as_ref())
                    .expect("divergent WOFF2 still decodes");
                panic!(
                    "{} {} generated WOFF2 diverged: expected {:?}, actual {}, decoded sfnt {}",
                    record.upstream_path,
                    cell.class,
                    cell.generated_woff2_sha256,
                    output_sha256,
                    sha256(&decoded)
                );
            }
            let trace_sha256 = canonical_trace_sha256(record, scalars, Some(&derived));
            assert_eq!(
                cell.canonical_resource_trace_sha256.as_deref(),
                Some(trace_sha256.as_str())
            );
        }
        CellStatus::NotSelected => {
            assert_eq!(cell.expected_validation_outcome, "not_selected");
            assert!(cell.semantic_subset_identity.is_none());
            assert!(cell.generated_woff2_sha256.is_none());
            let scalars = expected
                .scalars
                .as_deref()
                .expect("not-selected has scalars");
            let trace_sha256 = canonical_trace_sha256(record, scalars, None);
            assert_eq!(
                cell.canonical_resource_trace_sha256.as_deref(),
                Some(trace_sha256.as_str())
            );
        }
        CellStatus::NotApplicable => {
            assert_eq!(cell.expected_validation_outcome, "not_applicable");
            assert!(cell.ordered_scalars.is_none());
            assert!(cell.semantic_subset_identity.is_none());
            assert!(cell.generated_woff2_sha256.is_none());
            assert!(cell.canonical_resource_trace_sha256.is_none());
            assert_eq!(cell.reason.as_deref(), expected.reason);
        }
    }
}

fn assert_failure_cells(failures: &[FailureQualification]) {
    let expected = [
        ("empty-source", "decode"),
        ("short-semantic-identity", "sfnt"),
        ("empty-generated-sfnt", "validate"),
    ];
    assert_eq!(failures.len(), expected.len());
    let record = &excalidraw_render_assets_base::revision_1().assets[0];
    let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
        .expect("failure fixture source verifies")
        .expect("failure fixture source exists");
    for (failure, (id, stage)) in failures.iter().zip(expected) {
        assert_eq!(failure.id, id);
        assert_eq!(failure.expected_error_code, "encode.svg-font-subset-failed");
        assert_eq!(failure.expected_stage, stage);
        assert!(!failure.complete_chunk_fallback);
        assert!(!failure.partial_artifact);
        let (source_bytes, scalar_count) = match id {
            "empty-source" => (0, 1),
            "short-semantic-identity" => (asset.bytes().len(), 1),
            "empty-generated-sfnt" => (0, 0),
            _ => panic!("unknown failure fixture"),
        };
        assert!(source_bytes <= failure.max_source_bytes);
        assert!(scalar_count <= failure.max_scalars);
        let (actual, generated_woff2_bytes) = match id {
            "empty-source" => {
                match font_subset::derive(&[], &[u32::from('A')], "Test", &"0".repeat(64)) {
                    Ok(derived) => (None, Some(derived.woff2.len())),
                    Err(error) => (Some(error.stage), None),
                }
            }
            "short-semantic-identity" => {
                match font_subset::derive(asset.bytes(), &[u32::from('A')], "Test", "short") {
                    Ok(derived) => (None, Some(derived.woff2.len())),
                    Err(error) => (Some(error.stage), None),
                }
            }
            "empty-generated-sfnt" => match font_subset::validate_checksums(&[]) {
                Ok(()) => panic!("empty generated sfnt passed validation"),
                Err(error) => (Some(error.stage), None),
            },
            _ => panic!("unknown failure fixture"),
        };
        let actual = actual.expect("failure fixture returns no successful subset");
        assert_eq!(stage_name(actual), stage);
        assert_eq!(
            generated_woff2_bytes.unwrap_or_default(),
            failure.expected_generated_woff2_bytes
        );
        assert!(generated_woff2_bytes.is_none());
        let public_error =
            super::font_subset_qualification_error(actual, "Cascadia Code", record.identity);
        assert_eq!(
            public_error.code(),
            crate::ErrorCode::ENCODE_SVG_FONT_SUBSET_FAILED
        );
        let crate::ErrorDetails::SvgFontSubsetFailure {
            stage: public_stage,
            ..
        } = public_error
            .details()
            .expect("font subset error has details")
        else {
            panic!("font subset error has typed subset details");
        };
        assert_eq!(*public_stage, actual);
    }
}

struct ClassifiedCell {
    status: CellStatus,
    scalars: Option<Vec<u32>>,
    reason: Option<&'static str>,
}

fn classify_source(
    record: &excalidraw_render_assets_base::AssetRecord,
    source_woff2: &[u8],
) -> Vec<ClassifiedCell> {
    let ranges = excalidraw_render_assets_base::font_face_unicode_ranges(
        record.family,
        record.upstream_path,
    )
    .expect("qualified source has declared ranges");
    let sfnt = wuff::decompress_woff2(source_woff2).expect("qualified source decodes");
    let face = rustybuzz::Face::from_slice(&sfnt, 0).expect("qualified source parses");
    let font = FontRef::new(&sfnt).expect("qualified source is an sfnt");

    let mapped = first_mapped(&face, &ranges, |_| true).expect("source declares a mapped scalar");
    let boundaries = range_boundaries(&face, &ranges);
    let outside = first_outside(record.family, &ranges);
    let full = full_intersection(&face, &ranges);
    let non_bmp = first_mapped(&face, &ranges, |scalar| scalar > 0xffff);
    let combining = first_mapped(&face, &ranges, is_combining_mark);
    let composite = first_mapped(&face, &ranges, |scalar| {
        mapped_glyph_is_composite(&face, &font, scalar)
    });
    let layout = layout_probe(&face, &font);

    vec![
        success(vec![mapped]),
        success(boundaries),
        outside.map_or_else(
            || not_applicable("declared ranges cover every Unicode scalar"),
            |scalar| not_selected(vec![scalar]),
        ),
        success(full),
        non_bmp.map_or_else(
            || not_applicable("source has no mapped non-BMP scalar"),
            |scalar| success(vec![scalar]),
        ),
        combining.map_or_else(
            || not_applicable("source has no mapped combining mark"),
            |scalar| success(vec![scalar]),
        ),
        composite.map_or_else(
            || not_applicable("source has no mapped composite glyph"),
            |scalar| success(vec![scalar]),
        ),
        layout.map_or_else(
            || not_applicable("source has no complete manifested layout probe"),
            success,
        ),
    ]
}

fn success(scalars: Vec<u32>) -> ClassifiedCell {
    ClassifiedCell {
        status: CellStatus::Success,
        scalars: Some(scalars),
        reason: None,
    }
}

fn not_selected(scalars: Vec<u32>) -> ClassifiedCell {
    ClassifiedCell {
        status: CellStatus::NotSelected,
        scalars: Some(scalars),
        reason: None,
    }
}

fn not_applicable(reason: &'static str) -> ClassifiedCell {
    ClassifiedCell {
        status: CellStatus::NotApplicable,
        scalars: None,
        reason: Some(reason),
    }
}

fn first_mapped(
    face: &rustybuzz::Face<'_>,
    ranges: &[(u32, u32)],
    predicate: impl Fn(u32) -> bool,
) -> Option<u32> {
    for (start, end) in ranges {
        for scalar in *start..=*end {
            if predicate(scalar)
                && qualification_character(scalar)
                    .is_some_and(|character| face.glyph_index(character).is_some())
            {
                return Some(scalar);
            }
        }
    }
    None
}

fn range_boundaries(face: &rustybuzz::Face<'_>, ranges: &[(u32, u32)]) -> Vec<u32> {
    let mut scalars = Vec::new();
    let range = ranges.first().copied().expect("source declares a range");
    if let Some(scalar) = first_mapped(face, &[range], |scalar| !is_combining_mark(scalar)) {
        push_scalar(&mut scalars, scalar);
    }
    if let Some(scalar) = (range.0..=range.1).rev().find(|scalar| {
        !is_combining_mark(*scalar)
            && qualification_character(*scalar)
                .is_some_and(|character| face.glyph_index(character).is_some())
    }) {
        push_scalar(&mut scalars, scalar);
    }
    if scalars.is_empty() {
        push_scalar(
            &mut scalars,
            first_mapped(face, ranges, |_| true).expect("source declares a mapped scalar"),
        );
    }
    scalars
}

fn full_intersection(face: &rustybuzz::Face<'_>, ranges: &[(u32, u32)]) -> Vec<u32> {
    let mut scalars =
        vec![first_mapped(face, ranges, |_| true).expect("source declares a mapped scalar")];
    for (start, end) in ranges {
        let scalar = (*start..=*end)
            .find(|scalar| {
                qualification_character(*scalar)
                    .is_some_and(|character| face.glyph_index(character).is_none())
            })
            .or_else(|| {
                (*start..=*end).find(|scalar| {
                    qualification_character(*scalar)
                        .is_some_and(|character| face.glyph_index(character).is_some())
                })
            })
            .or_else(|| (*start..=*end).find(|scalar| char::from_u32(*scalar).is_some()))
            .expect("declared range includes a Unicode scalar");
        push_scalar(&mut scalars, scalar);
    }
    scalars.sort_by_key(|scalar| !is_combining_mark(*scalar));
    scalars
}

fn push_scalar(scalars: &mut Vec<u32>, scalar: u32) {
    if char::from_u32(scalar).is_some() && !scalars.contains(&scalar) {
        scalars.push(scalar);
    }
}

fn first_outside(family: &str, ranges: &[(u32, u32)]) -> Option<u32> {
    let candidates: Box<dyn Iterator<Item = u32>> = if family == "Xiaolai" {
        Box::new(
            excalidraw_render_assets_base::CJK_SCALAR_RANGES_V1
                .iter()
                .flat_map(|(start, end)| *start..=*end),
        )
    } else {
        Box::new((0x20..=0x10ffff).chain(0..0x20))
    };
    candidates.into_iter().find(|scalar| {
        char::from_u32(*scalar).is_some()
            && !ranges
                .iter()
                .any(|(start, end)| start <= scalar && scalar <= end)
    })
}

fn is_combining_mark(scalar: u32) -> bool {
    char::from_u32(scalar).is_some_and(|character| {
        unicode_normalization::char::canonical_combining_class(character) != 0
    })
}

fn qualification_character(scalar: u32) -> Option<char> {
    let character = char::from_u32(scalar)?;
    let low = scalar & 0xffff;
    if character.is_control() || matches!(scalar, 0xfdd0..=0xfdef) || matches!(low, 0xfffe | 0xffff)
    {
        None
    } else {
        Some(character)
    }
}

fn mapped_glyph_is_composite(face: &rustybuzz::Face<'_>, font: &FontRef<'_>, scalar: u32) -> bool {
    let Some(character) = char::from_u32(scalar) else {
        return false;
    };
    let Some(glyph_id) = face.glyph_index(character) else {
        return false;
    };
    let Ok(loca) = font.loca(None) else {
        return false;
    };
    let Ok(glyf) = font.glyf() else {
        return false;
    };
    matches!(
        loca.get_glyf(GlyphId::new(u32::from(glyph_id.0)), &glyf),
        Ok(Some(write_fonts::read::tables::glyf::Glyph::Composite(_)))
    )
}

fn layout_probe(face: &rustybuzz::Face<'_>, font: &FontRef<'_>) -> Option<Vec<u32>> {
    if font.gsub().is_err() && font.gpos().is_err() {
        return None;
    }
    [
        "office", "ffi", "fl", "AV", "To", "Ta", "Wa", "Yo", "لا", "سلام",
    ]
    .iter()
    .find(|probe| {
        probe
            .chars()
            .all(|character| face.glyph_index(character).is_some())
            && probe_exercises_layout(face, probe)
    })
    .map(|probe| {
        let mut scalars = Vec::new();
        for scalar in probe.chars().map(u32::from) {
            push_scalar(&mut scalars, scalar);
        }
        scalars
    })
}

fn probe_exercises_layout(face: &rustybuzz::Face<'_>, probe: &str) -> bool {
    let mapped = probe
        .chars()
        .filter_map(|character| face.glyph_index(character))
        .collect::<Vec<_>>();
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(probe);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(face, &[], buffer);
    if shaped.glyph_infos().len() != mapped.len()
        || shaped
            .glyph_infos()
            .iter()
            .zip(&mapped)
            .any(|(info, glyph)| info.glyph_id != u32::from(glyph.0))
    {
        return true;
    }
    shaped
        .glyph_positions()
        .iter()
        .any(|position| position.y_advance != 0 || position.x_offset != 0 || position.y_offset != 0)
}

fn canonical_trace_sha256(
    record: &'static excalidraw_render_assets_base::AssetRecord,
    scalars: &[u32],
    derived: Option<&font_subset::Derived>,
) -> String {
    let selected = derived.is_some();
    let (outcome, trace) = super::render_font_subset_qualification_trace(record, scalars);
    outcome.unwrap_or_else(|error| {
        panic!(
            "{} {scalars:?} canonical trace render fails with {error:?} / {:?}; {}",
            record.upstream_path,
            error.code(),
            family_failure_diagnosis(record, scalars)
        )
    });
    let trace_value: serde_json::Value =
        serde_json::from_slice(&trace).expect("canonical trace is JSON");
    let entries = trace_value["entries"]
        .as_array()
        .expect("canonical trace entries are an array");
    let has_event = |event: &str| {
        entries
            .iter()
            .any(|entry| entry["event"] == event && entry["semantic_identity"] == record.identity)
    };
    assert!(has_event("svg_font_chunk_considered"));
    assert_eq!(has_event("svg_font_chunk_selected"), selected);
    if selected {
        for event in [
            "svg_font_woff2_decode",
            "svg_font_subset_plan",
            "svg_font_subset_sfnt",
            "svg_font_woff2_encode",
            "svg_font_validate",
            "svg_font_css_data_url",
        ] {
            assert!(has_event(event), "{} lacks {event}", record.upstream_path);
        }
    }
    sha256(&trace)
}

fn family_failure_diagnosis(
    target: &excalidraw_render_assets_base::AssetRecord,
    scalars: &[u32],
) -> String {
    for record in excalidraw_render_assets_base::revision_1()
        .assets
        .iter()
        .filter(|record| record.family == target.family)
    {
        let ranges = excalidraw_render_assets_base::font_face_unicode_ranges(
            record.family,
            record.upstream_path,
        )
        .expect("diagnosed source has ranges");
        if !scalars.iter().any(|scalar| {
            ranges
                .iter()
                .any(|(start, end)| start <= scalar && scalar <= end)
        }) {
            continue;
        }
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("diagnosed source verifies")
            .expect("diagnosed source exists");
        let identity = svg_fonts::semantic_identity(record, scalars).expect("diagnosed identity");
        if let Err(error) = font_subset::derive(
            asset.bytes(),
            scalars,
            effective_alias(record.family),
            &identity,
        ) {
            let prefix = (1..=scalars.len()).find(|length| {
                let values = &scalars[..*length];
                let identity =
                    svg_fonts::semantic_identity(record, values).expect("diagnosed identity");
                font_subset::derive(
                    asset.bytes(),
                    values,
                    effective_alias(record.family),
                    &identity,
                )
                .is_err()
            });
            return format!(
                "{} fails at {:?}, first bad prefix {prefix:?}",
                record.upstream_path, error.stage
            );
        }
    }
    "no selected source reproduces the failure".to_owned()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn effective_alias(family: &str) -> &str {
    match family {
        "Cascadia" => "ExcalidrawRender-3",
        "ComicShanns" => "Comic Shanns",
        "Liberation" => "ExcalidrawRender-9",
        "Lilita" => "ExcalidrawRender-7",
        "Virgil" => "ExcalidrawRender-1",
        family => family,
    }
}

fn stage_name(stage: crate::SvgFontSubsetStage) -> &'static str {
    match stage {
        crate::SvgFontSubsetStage::Decode => "decode",
        crate::SvgFontSubsetStage::Plan => "plan",
        crate::SvgFontSubsetStage::Sfnt => "sfnt",
        crate::SvgFontSubsetStage::Encode => "encode",
        crate::SvgFontSubsetStage::Validate => "validate",
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_corpus() -> Corpus {
    let catalog = excalidraw_render_assets_base::revision_1();
    let mut sources = Vec::with_capacity(catalog.assets.len());
    for record in catalog.assets {
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("qualified source verifies")
            .expect("qualified source exists");
        let alias = effective_alias(record.family);
        let cells = CLASS_IDS
            .into_iter()
            .zip(classify_source(record, asset.bytes()))
            .map(|(class, classified)| {
                materialize_cell(record, asset.bytes(), alias, class, classified)
            })
            .collect();
        sources.push(SourceQualification {
            source_identity: record.identity.to_owned(),
            upstream_path: record.upstream_path.to_owned(),
            source_sha256: record.complete_sha256.to_owned(),
            effective_alias: alias.to_owned(),
            cells,
        });
    }
    Corpus {
        schema: CORPUS_SCHEMA.to_owned(),
        compatibility_revision: 1,
        originating_issue: 98,
        source_manifest_sha256: SOURCE_MANIFEST_SHA256.to_owned(),
        subset_profile_sha256: PROFILE_SHA256.to_owned(),
        classes: class_definitions(),
        sources,
        failures: vec![
            failure("empty-source", "decode", 0, 1),
            failure(
                "short-semantic-identity",
                "sfnt",
                catalog.assets[0].complete_length,
                1,
            ),
            failure("empty-generated-sfnt", "validate", 0, 0),
        ],
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn materialize_cell(
    record: &'static excalidraw_render_assets_base::AssetRecord,
    source_woff2: &[u8],
    alias: &str,
    class: &str,
    classified: ClassifiedCell,
) -> QualificationCell {
    match classified.status {
        CellStatus::Success => {
            let scalars = classified.scalars.expect("success has scalars");
            let semantic_identity =
                svg_fonts::semantic_identity(record, &scalars).expect("semantic identity");
            let derived = font_subset::derive(source_woff2, &scalars, alias, &semantic_identity)
                .unwrap_or_else(|error| {
                    let first_bad_prefix = (1..=scalars.len()).find(|length| {
                        let prefix = &scalars[..*length];
                        let identity = svg_fonts::semantic_identity(record, prefix)
                            .expect("prefix semantic identity");
                        font_subset::derive(source_woff2, prefix, alias, &identity).is_err()
                    });
                    panic!(
                        "{} {class} {scalars:?}, first bad prefix {first_bad_prefix:?}: {:?}",
                        record.upstream_path, error.stage
                    )
                });
            let output_sha256 = sha256(derived.woff2.as_ref());
            let trace_sha256 = canonical_trace_sha256(record, &scalars, Some(&derived));
            QualificationCell {
                class: class.to_owned(),
                status: CellStatus::Success,
                ordered_scalars: Some(scalars),
                semantic_subset_identity: Some(semantic_identity),
                generated_woff2_sha256: Some(output_sha256),
                canonical_resource_trace_sha256: Some(trace_sha256),
                expected_validation_outcome: "valid".to_owned(),
                reason: None,
            }
        }
        CellStatus::NotSelected => {
            let scalars = classified.scalars.expect("not-selected has scalars");
            QualificationCell {
                class: class.to_owned(),
                status: CellStatus::NotSelected,
                canonical_resource_trace_sha256: Some(canonical_trace_sha256(
                    record, &scalars, None,
                )),
                ordered_scalars: Some(scalars),
                semantic_subset_identity: None,
                generated_woff2_sha256: None,
                expected_validation_outcome: "not_selected".to_owned(),
                reason: None,
            }
        }
        CellStatus::NotApplicable => QualificationCell {
            class: class.to_owned(),
            status: CellStatus::NotApplicable,
            ordered_scalars: None,
            semantic_subset_identity: None,
            generated_woff2_sha256: None,
            canonical_resource_trace_sha256: None,
            expected_validation_outcome: "not_applicable".to_owned(),
            reason: classified.reason.map(str::to_owned),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn failure(
    id: &str,
    stage: &str,
    max_source_bytes: usize,
    max_scalars: usize,
) -> FailureQualification {
    FailureQualification {
        id: id.to_owned(),
        expected_error_code: "encode.svg-font-subset-failed".to_owned(),
        expected_stage: stage.to_owned(),
        max_source_bytes,
        max_scalars,
        expected_generated_woff2_bytes: 0,
        complete_chunk_fallback: false,
        partial_artifact: false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn class_definitions() -> Vec<ClassDefinition> {
    [
        "first source-mapped Unicode scalar inside the chunk's declared ranges",
        "ordered unique source-mapped scalars nearest the first declared range boundaries",
        "first Unicode scalar outside every declared range; the chunk must not be selected",
        "ordered unique scalar representatives intersecting every declared range, preferring unmapped scalars to isolate selection",
        "first source-mapped scalar above U+FFFF",
        "first source-mapped scalar with a nonzero canonical combining class",
        "first source-mapped scalar whose TrueType glyph is composite",
        "first complete manifested shaping probe whose default GSUB/GPOS shaping changes glyph choice, count, vertical advance, or offset",
    ]
    .into_iter()
    .zip(CLASS_IDS)
    .map(|(definition, id)| ClassDefinition {
        id: id.to_owned(),
        definition: definition.to_owned(),
    })
    .collect()
}

#[test]
fn complete_font_subset_qualification_corpus_matches_generated_bytes_and_traces() {
    assert_complete_font_subset_qualification_corpus();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "explicitly refreshes immutable qualification evidence"]
fn materialize_complete_font_subset_qualification_corpus() {
    use std::fs;
    use std::path::PathBuf;

    let value = serde_json::to_value(build_corpus()).expect("corpus is serializable");
    let bytes = serde_json::to_vec(&value).expect("corpus value is serializable");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(CORPUS_PATH);
    fs::write(&path, &bytes).expect("corpus can be written");
    fs::write(root.join(PACKAGED_CORPUS_PATH), &bytes)
        .expect("packaged corpus copy can be written");
    fs::write(
        path.with_extension("sha256"),
        format!("{}\n", sha256(&bytes)),
    )
    .expect("corpus sidecar can be written");
}
