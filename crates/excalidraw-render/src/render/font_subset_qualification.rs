use super::{font_subset, svg_fonts};
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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

#[cfg(not(target_arch = "wasm32"))]
fn materialize_viewer_inputs() {
    use std::fs;
    use std::path::PathBuf;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/svg-font-subset-viewer-qualification");
    if output.exists() {
        fs::remove_dir_all(&output).expect("stale viewer qualification inputs can be removed");
    }
    fs::create_dir_all(output.join("inputs"))
        .expect("viewer qualification input directory can be created");

    let corpus: Corpus = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/qualification/font-subset-corpus.json"
    )))
    .expect("the font subset qualification corpus is valid JSON");
    let catalog = excalidraw_render_assets_base::revision_1();
    assert_eq!(corpus.sources.len(), catalog.assets.len());
    let mut executions = BTreeMap::<String, serde_json::Value>::new();

    for (source, record) in corpus.sources.iter().zip(catalog.assets) {
        assert_eq!(source.source_identity, record.identity);
        assert_eq!(source.upstream_path, record.upstream_path);
        assert_eq!(source.source_sha256, record.complete_sha256);
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("qualified source verifies")
            .expect("qualified source exists");
        for cell in &source.cells {
            if cell.status != CellStatus::Success {
                continue;
            }
            let scalars = cell
                .ordered_scalars
                .as_deref()
                .expect("success has scalars");
            let identity = cell
                .semantic_subset_identity
                .as_deref()
                .expect("success has semantic identity");
            let coverage = serde_json::json!({
                "class": cell.class,
                "ordered_scalars": scalars,
                "source_identity": source.source_identity,
            });
            if let Some(execution) = executions.get_mut(identity) {
                execution["coverage"]
                    .as_array_mut()
                    .expect("execution coverage is an array")
                    .push(coverage);
                continue;
            }

            let derived =
                font_subset::derive(asset.bytes(), scalars, &source.effective_alias, identity)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} {} viewer subset derives at {:?}",
                            record.upstream_path, cell.class, error.stage
                        )
                    });
            let source_sfnt =
                wuff::decompress_woff2(asset.bytes()).expect("qualified source decodes");
            let subset_sfnt =
                wuff::decompress_woff2(derived.woff2.as_ref()).expect("qualified subset decodes");
            let mapped_scalars = scalars
                .iter()
                .copied()
                .filter(|scalar| {
                    char::from_u32(*scalar).is_some_and(|character| {
                        rustybuzz::Face::from_slice(&source_sfnt, 0)
                            .is_some_and(|face| face.glyph_index(character).is_some())
                    })
                })
                .collect::<Vec<_>>();
            assert!(
                !mapped_scalars.is_empty(),
                "successful cell has mapped text"
            );

            let case_dir = output.join("inputs").join(identity);
            fs::create_dir_all(&case_dir).expect("viewer case directory can be created");
            let source_woff2_path = case_dir.join("source.woff2");
            let subset_woff2_path = case_dir.join("subset.woff2");
            let source_sfnt_path = case_dir.join("source.sfnt");
            let subset_sfnt_path = case_dir.join("subset.sfnt");
            fs::write(&source_woff2_path, asset.bytes()).expect("source WOFF2 can be staged");
            fs::write(&subset_woff2_path, derived.woff2.as_ref())
                .expect("subset WOFF2 can be staged");
            fs::write(&source_sfnt_path, &source_sfnt).expect("source sfnt can be staged");
            fs::write(&subset_sfnt_path, &subset_sfnt).expect("subset sfnt can be staged");

            let family = format!("Qualification-{}", &identity[..16]);
            let (source_family, source_properties) = fontdb_face(&source_sfnt);
            let (subset_family, subset_properties) = fontdb_face(&subset_sfnt);
            assert_eq!(
                source_properties, subset_properties,
                "subset preserves face properties"
            );
            let lines = qualification_lines(&mapped_scalars);
            let width = 384_u32;
            let height = 24_u32
                .checked_add(
                    u32::try_from(lines.len())
                        .expect("line count is a u32")
                        .checked_mul(28)
                        .expect("line height does not overflow"),
                )
                .expect("SVG height does not overflow");
            let source_svg = qualification_svg(
                &family,
                asset.bytes(),
                &lines,
                width,
                height,
                identity,
                &source_properties,
            );
            let subset_svg = qualification_svg(
                &family,
                derived.woff2.as_ref(),
                &lines,
                width,
                height,
                identity,
                &source_properties,
            );
            fs::write(case_dir.join("source.svg"), source_svg.as_bytes())
                .expect("source SVG can be staged");
            fs::write(case_dir.join("subset.svg"), subset_svg.as_bytes())
                .expect("subset SVG can be staged");

            let source_assets = qualification_assets(
                &root,
                asset.bytes(),
                &source_sfnt_path,
                &source_sfnt,
                &family,
                &source_family,
                &source_properties,
            );
            let subset_assets = qualification_assets(
                &root,
                derived.woff2.as_ref(),
                &subset_sfnt_path,
                &subset_sfnt,
                &family,
                &subset_family,
                &subset_properties,
            );
            write_json(&case_dir.join("source.assets.json"), &source_assets);
            write_json(&case_dir.join("subset.assets.json"), &subset_assets);

            let relative = |name: &str| {
                format!("target/svg-font-subset-viewer-qualification/inputs/{identity}/{name}")
            };
            executions.insert(
                identity.to_owned(),
                serde_json::json!({
                    "coverage": [coverage],
                    "dimensions": {
                        "byte_length": u64::from(width) * u64::from(height) * 4,
                        "height": height,
                        "layout": "top-left-row-major",
                        "pixel_format": "straight-alpha-srgb-rgba8",
                        "width": width,
                    },
                    "effective_alias": source.effective_alias,
                    "mapped_scalars": mapped_scalars,
                    "semantic_subset_identity": identity,
                    "source_assets": relative("source.assets.json"),
                    "source_assets_sha256": sha256(&json_bytes(&source_assets)),
                    "source_sfnt": relative("source.sfnt"),
                    "source_sfnt_sha256": sha256(&source_sfnt),
                    "source_svg": relative("source.svg"),
                    "source_svg_sha256": sha256(source_svg.as_bytes()),
                    "source_woff2": relative("source.woff2"),
                    "source_woff2_sha256": sha256(asset.bytes()),
                    "subset_assets": relative("subset.assets.json"),
                    "subset_assets_sha256": sha256(&json_bytes(&subset_assets)),
                    "subset_sfnt": relative("subset.sfnt"),
                    "subset_sfnt_sha256": sha256(&subset_sfnt),
                    "subset_svg": relative("subset.svg"),
                    "subset_svg_sha256": sha256(subset_svg.as_bytes()),
                    "subset_woff2": relative("subset.woff2"),
                    "subset_woff2_sha256": sha256(derived.woff2.as_ref()),
                }),
            );
        }
    }

    let cases = serde_json::json!({
        "cases": executions.into_values().collect::<Vec<_>>(),
        "corpus_sha256": sha256(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/qualification/font-subset-corpus.json"
        ))),
        "schema": "excalidraw-render/svg-font-subset-viewer-inputs/v1",
        "subset_profile_sha256": PROFILE_SHA256,
    });
    write_json(&output.join("inputs.json"), &cases);
}

#[cfg(not(target_arch = "wasm32"))]
fn qualification_lines(scalars: &[u32]) -> Vec<String> {
    scalars
        .chunks(12)
        .map(|line| {
            line.iter()
                .map(|scalar| char::from_u32(*scalar).expect("manifested scalar is Unicode"))
                .collect()
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn qualification_svg(
    family: &str,
    woff2: &[u8],
    lines: &[String],
    width: u32,
    height: u32,
    identity: &str,
    properties: &FaceProperties,
) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(woff2);
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
        let y = 24 + u32::try_from(index).expect("line index is a u32") * 28;
        text.push_str(&format!(
            "<tspan x=\"12\" y=\"{y}\">{}</tspan>",
            xml_escape(line)
        ));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><style>@font-face{{font-family:'{family}';font-style:{};font-weight:{};font-stretch:{};src:url(data:font/woff2;base64,{encoded}) format(woff2);unicode-range:U+0-10FFFF}}</style><text data-semantic-subset-identity=\"{identity}\" font-family=\"{family}\" font-style=\"{}\" font-weight=\"{}\" font-stretch=\"{}\" font-size=\"20\" fill=\"#111111\">{text}</text></svg>",
        properties.style,
        properties.weight,
        properties.stretch,
        properties.style,
        properties.weight,
        properties.stretch,
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Eq, PartialEq)]
struct FaceProperties {
    stretch: &'static str,
    style: &'static str,
    weight: u16,
}

#[cfg(not(target_arch = "wasm32"))]
fn fontdb_face(sfnt: &[u8]) -> (String, FaceProperties) {
    use std::sync::Arc;

    let mut database = usvg::fontdb::Database::new();
    let ids = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(sfnt.to_vec())));
    let face = ids
        .iter()
        .filter_map(|id| database.face(*id))
        .find(|face| face.index == 0)
        .expect("qualified sfnt has face index zero");
    let stretch = match face.stretch {
        usvg::fontdb::Stretch::UltraCondensed => "ultra-condensed",
        usvg::fontdb::Stretch::ExtraCondensed => "extra-condensed",
        usvg::fontdb::Stretch::Condensed => "condensed",
        usvg::fontdb::Stretch::SemiCondensed => "semi-condensed",
        usvg::fontdb::Stretch::Normal => "normal",
        usvg::fontdb::Stretch::SemiExpanded => "semi-expanded",
        usvg::fontdb::Stretch::Expanded => "expanded",
        usvg::fontdb::Stretch::ExtraExpanded => "extra-expanded",
        usvg::fontdb::Stretch::UltraExpanded => "ultra-expanded",
    };
    let style = match face.style {
        usvg::fontdb::Style::Normal => "normal",
        usvg::fontdb::Style::Italic => "italic",
        usvg::fontdb::Style::Oblique => "oblique",
    };
    (
        face.families
            .first()
            .map(|(family, _)| family.clone())
            .expect("qualified font has a family name"),
        FaceProperties {
            stretch,
            style,
            weight: face.weight.0,
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn qualification_assets(
    root: &std::path::Path,
    woff2: &[u8],
    sfnt_path: &std::path::Path,
    sfnt: &[u8],
    css_family: &str,
    sfnt_family: &str,
    properties: &FaceProperties,
) -> serde_json::Value {
    let relative = |path: &std::path::Path| {
        path.strip_prefix(root)
            .expect("qualification input is under repository root")
            .to_string_lossy()
            .replace('\\', "/")
    };
    serde_json::json!({
        "fonts": [{
            "css": {
                "family": css_family,
                "stretch": properties.stretch,
                "style": properties.style,
                "unicode_range": ["U+0-10FFFF"],
                "weight": properties.weight,
            },
            "sfnt": {
                "face_index": 0,
                "family": sfnt_family,
                "media_type": "font/ttf",
                "path": relative(sfnt_path),
                "sha256": sha256(sfnt),
                "stretch": properties.stretch,
                "style": properties.style,
                "weight": properties.weight,
            },
            "source": {
                "media_type": "font/woff2",
                "sha256": sha256(woff2),
            }
        }],
        "images": [],
        "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(not(target_arch = "wasm32"))]
fn write_json(path: &std::path::Path, value: &serde_json::Value) {
    std::fs::write(path, json_bytes(value)).expect("qualification JSON can be written");
}

#[cfg(not(target_arch = "wasm32"))]
fn json_bytes(value: &serde_json::Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("qualification JSON serializes");
    bytes.push(b'\n');
    bytes
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

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "explicitly materializes canonical viewer qualification inputs"]
fn materialize_complete_font_subset_viewer_inputs() {
    materialize_viewer_inputs();
}
