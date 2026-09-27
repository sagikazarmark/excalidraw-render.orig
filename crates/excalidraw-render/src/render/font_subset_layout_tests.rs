//! Tests of the typed layout walker (`font_subset_layout.rs`, #210).

use super::font_subset_layout::{
    validate, validate_branches, LayoutCheck, LayoutFailure, LayoutTable, MAX_LAYOUT_WORK,
};
use super::font_subset_layout_fixtures::{layout_fixtures, LayoutFixture, MIN_GLYPHS};
use super::{font_subset, font_subset_fuzz, font_subset_layout_oracle, svg_fonts};
use std::collections::{BTreeMap, BTreeSet};
use write_fonts::read::{types::Tag, FontRef};
use write_fonts::tables::gpos::{
    Class1Record, Class2Record, Gpos, PairPos, PositionLookup, ValueRecord,
};
use write_fonts::tables::gsub::{Gsub, SingleSubst, SubstitutionLookup};
use write_fonts::tables::layout::{
    ClassDef, CoverageTable, Feature, FeatureList, FeatureRecord, LangSys, Lookup, LookupFlag,
    LookupList, RangeRecord, Script, ScriptList, ScriptRecord, SequenceContext,
    SequenceLookupRecord, SequenceRule, SequenceRuleSet,
};
use write_fonts::tables::maxp::Maxp;
use write_fonts::types::GlyphId16;
use write_fonts::FontBuilder;

/// Every branch the walker records: each lookup type and subtable format of
/// the OpenType layout specification.
const BRANCHES: [&str; 56] = [
    "anchor-1",
    "anchor-2",
    "anchor-3",
    "caret-value-1",
    "caret-value-2",
    "caret-value-3",
    "chained-context-1",
    "chained-context-2",
    "chained-context-3",
    "class-def-1",
    "class-def-2",
    "condition-1",
    "condition-2",
    "condition-3",
    "condition-4",
    "condition-5",
    "context-1",
    "context-2",
    "context-3",
    "coverage-1",
    "coverage-2",
    "device",
    "feature-params-character-variant",
    "feature-params-size",
    "feature-params-stylistic-set",
    "feature-variations",
    "gdef-attach-list",
    "gdef-glyph-class-def",
    "gdef-item-variation-store",
    "gdef-lig-caret-list",
    "gdef-mark-attach-class-def",
    "gdef-mark-glyph-sets",
    "gpos-1",
    "gpos-1-format-1",
    "gpos-1-format-2",
    "gpos-2",
    "gpos-2-format-1",
    "gpos-2-format-2",
    "gpos-3",
    "gpos-4",
    "gpos-5",
    "gpos-6",
    "gpos-7",
    "gpos-8",
    "gpos-9",
    "gsub-1",
    "gsub-1-format-1",
    "gsub-1-format-2",
    "gsub-2",
    "gsub-3",
    "gsub-4",
    "gsub-5",
    "gsub-6",
    "gsub-7",
    "gsub-8",
    "variation-index",
];

/// The checks ADR-0020 adds to the retired walker's: a rejection for one of
/// them is the only verdict on which the typed walker may differ from the
/// traversal oracle.
const NEW_CHECKS: [LayoutCheck; 6] = [
    LayoutCheck::LookupIndex,
    LayoutCheck::FeatureIndex,
    LayoutCheck::RangeOrder,
    LayoutCheck::ClassValue,
    LayoutCheck::Count,
    LayoutCheck::SequenceIndex,
];

/// A font holding only `maxp` with `glyphs` glyphs: all the walker reads
/// besides the layout tables a fixture installs.
fn maxp_font(glyphs: u16) -> Vec<u8> {
    let mut builder = FontBuilder::new();
    builder
        .add_table(&Maxp::new(glyphs))
        .expect("maxp fixture compiles");
    builder.build()
}

fn fixture_font(fixture: &LayoutFixture) -> Vec<u8> {
    fixture.install(&maxp_font(MIN_GLYPHS))
}

fn raw_font(tag: &[u8; 4], table: Vec<u8>) -> Vec<u8> {
    LayoutFixture {
        name: "raw",
        gsub: (tag == b"GSUB").then(|| table.clone()),
        gpos: (tag == b"GPOS").then(|| table.clone()),
        gdef: (tag == b"GDEF").then_some(table),
    }
    .install(&maxp_font(MIN_GLYPHS))
}

fn check(sfnt: &[u8]) -> Result<usize, LayoutFailure> {
    validate(&FontRef::new(sfnt).expect("test font is an sfnt"))
}

fn g(id: u16) -> GlyphId16 {
    GlyphId16::new(id)
}

/// A GSUB with one `liga` feature over all of `lookups` under a default
/// language system that also requires `required`.
fn gsub(lookups: Vec<SubstitutionLookup>, feature_lookups: Vec<u16>, required: u16) -> Vec<u8> {
    let mut lang_sys = LangSys::new(vec![0]);
    lang_sys.required_feature_index = required;
    let table = Gsub::new(
        ScriptList::new(vec![ScriptRecord::new(
            Tag::new(b"latn"),
            Script::new(Some(lang_sys), Vec::new()),
        )]),
        FeatureList::new(vec![FeatureRecord::new(
            Tag::new(b"liga"),
            Feature::new(None, feature_lookups),
        )]),
        LookupList::new(lookups),
    );
    raw_font(
        b"GSUB",
        write_fonts::dump_table(&table).expect("GSUB fixture compiles"),
    )
}

fn single(subtable: SingleSubst) -> SubstitutionLookup {
    SubstitutionLookup::Single(Lookup::new(LookupFlag::empty(), vec![subtable]))
}

#[test]
fn layout_fixtures_pass_and_cover_every_walker_branch() {
    let mut branches = BTreeSet::new();
    for fixture in layout_fixtures() {
        let sfnt = fixture_font(&fixture);
        let taken = validate_branches(&FontRef::new(&sfnt).expect("fixture is an sfnt"))
            .unwrap_or_else(|failure| panic!("fixture {} fails: {failure:?}", fixture.name));
        assert!(
            font_subset_layout_oracle::accepts(&sfnt),
            "the oracle rejects fixture {}",
            fixture.name
        );
        branches.extend(taken);
    }
    assert_eq!(branches, BRANCHES.into_iter().collect());
}

#[test]
fn layout_failures_carry_the_table_and_check() {
    let failure = |table, check| Err(LayoutFailure { table, check });
    let gsub_failure = |check| failure(LayoutTable::Gsub, check);

    assert_eq!(
        check(&gsub(
            vec![single(SingleSubst::format_2(
                CoverageTable::format_1(vec![g(1), g(9)]),
                vec![g(2), g(3)],
            ))],
            vec![0],
            0xffff,
        )),
        gsub_failure(LayoutCheck::GlyphId)
    );
    assert_eq!(
        check(&gsub(
            vec![single(SingleSubst::format_2(
                CoverageTable::format_2(vec![RangeRecord::new(g(3), g(1), 0)]),
                vec![g(2), g(3), g(1)],
            ))],
            vec![0],
            0xffff,
        )),
        gsub_failure(LayoutCheck::RangeOrder)
    );
    assert_eq!(
        check(&gsub(
            vec![single(SingleSubst::format_2(
                CoverageTable::format_1(vec![g(1), g(2)]),
                vec![g(2)],
            ))],
            vec![0],
            0xffff,
        )),
        gsub_failure(LayoutCheck::Count)
    );
    assert_eq!(
        check(&gsub(
            vec![single(SingleSubst::format_1(
                CoverageTable::format_1(vec![g(3)]),
                5,
            ))],
            vec![0],
            0xffff,
        )),
        gsub_failure(LayoutCheck::SingleSubstDelta)
    );
    let ok = || {
        single(SingleSubst::format_1(
            CoverageTable::format_1(vec![g(1)]),
            1,
        ))
    };
    assert!(check(&gsub(vec![ok()], vec![0], 0)).is_ok());
    assert_eq!(
        check(&gsub(vec![ok()], vec![1], 0xffff)),
        gsub_failure(LayoutCheck::LookupIndex)
    );
    assert_eq!(
        check(&gsub(vec![ok()], vec![0], 1)),
        gsub_failure(LayoutCheck::FeatureIndex)
    );
    let context = |sequence_index, lookup_index| {
        SubstitutionLookup::Contextual(Lookup::new(
            LookupFlag::empty(),
            vec![SequenceContext::format_1(
                CoverageTable::format_1(vec![g(1)]),
                vec![Some(SequenceRuleSet::new(vec![SequenceRule::new(
                    vec![g(2)],
                    vec![SequenceLookupRecord::new(sequence_index, lookup_index)],
                )]))],
            )
            .into()],
        ))
    };
    assert!(check(&gsub(vec![context(1, 0)], vec![0], 0xffff)).is_ok());
    assert_eq!(
        check(&gsub(vec![context(2, 0)], vec![0], 0xffff)),
        gsub_failure(LayoutCheck::SequenceIndex)
    );
    assert_eq!(
        check(&gsub(vec![context(0, 1)], vec![0], 0xffff)),
        gsub_failure(LayoutCheck::LookupIndex)
    );

    // write-fonts derives class1Count from ClassDef1, so the invalid count is
    // patched into a valid table: ClassDef1 assigns class 1, and one class 1
    // record fewer than it needs remains.
    let pair = Gpos::new(
        ScriptList::new(Vec::new()),
        FeatureList::new(Vec::new()),
        LookupList::new(vec![PositionLookup::Pair(Lookup::new(
            LookupFlag::empty(),
            vec![PairPos::format_2(
                CoverageTable::format_1(vec![g(1)]),
                ClassDef::format_1(g(1), vec![1]),
                ClassDef::format_1(g(1), vec![0]),
                (0..2)
                    .map(|_| {
                        Class1Record::new(vec![Class2Record::new(
                            ValueRecord::new().with_x_advance(1),
                            ValueRecord::new(),
                        )])
                    })
                    .collect(),
            )],
        ))]),
    );
    let mut pair = write_fonts::dump_table(&pair).expect("GPOS fixture compiles");
    assert!(check(&raw_font(b"GPOS", pair.clone())).is_ok());
    let class1_count = {
        use write_fonts::read::tables::gpos::{Gpos, PairPos, PositionSubtables};
        use write_fonts::read::FontRead;
        let gpos = Gpos::read(write_fonts::read::FontData::new(&pair)).expect("GPOS parses");
        let lookup = gpos
            .lookup_list()
            .and_then(|list| list.lookups().get(0))
            .and_then(|lookup| lookup.subtables())
            .expect("the lookup parses");
        let PositionSubtables::Pair(subtables) = lookup else {
            unreachable!("the lookup is a pair lookup")
        };
        let Ok(PairPos::Format2(subtable)) = subtables.get(0) else {
            unreachable!("the subtable is a class pair subtable")
        };
        subtable.offset_data().as_bytes().as_ptr() as usize - pair.as_ptr() as usize
            + subtable.class1_count_byte_range().start
    };
    assert_eq!(pair[class1_count..class1_count + 2], [0, 2]);
    pair[class1_count + 1] = 1;
    assert_eq!(
        check(&raw_font(b"GPOS", pair)),
        failure(LayoutTable::Gpos, LayoutCheck::ClassValue)
    );

    // A GSUB whose only lookup has type 9, which the specification does not
    // define.
    let unknown_lookup = vec![
        0x00, 0x01, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x0c, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x04, 0x00, 0x09, 0x00, 0x00, 0x00, 0x00,
    ];
    assert_eq!(
        check(&raw_font(b"GSUB", unknown_lookup)),
        gsub_failure(LayoutCheck::UnknownFormat)
    );
    // A GDEF 1.0 whose glyph ClassDef has format 3.
    let unknown_class_format = vec![
        0x00, 0x01, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00,
        0x00,
    ];
    assert_eq!(
        check(&raw_font(b"GDEF", unknown_class_format)),
        failure(LayoutTable::Gdef, LayoutCheck::UnknownFormat)
    );
    // A GDEF 1.0 whose glyph ClassDef format 1 starts at glyph 2 and
    // classifies three glyphs of four.
    let class_range = vec![
        0x00, 0x01, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x02, 0x00, 0x03, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01,
    ];
    assert_eq!(
        check(&raw_font(b"GDEF", class_range)),
        failure(LayoutTable::Gdef, LayoutCheck::ClassRange)
    );
    // A GDEF 1.0 whose glyph ClassDef format 1 counts two class values but
    // ends after the count. read-fonts reads the truncated array as empty,
    // which the traversal oracle accepted.
    let truncated = vec![
        0x00, 0x01, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x01, 0x00, 0x02,
    ];
    assert!(font_subset_layout_oracle::accepts(&raw_font(
        b"GDEF",
        truncated.clone()
    )));
    assert_eq!(
        check(&raw_font(b"GDEF", truncated)),
        failure(LayoutTable::Gdef, LayoutCheck::Count)
    );
    // A GDEF 1.0 whose glyph ClassDef offset points past the table.
    let unresolved = vec![
        0x00, 0x01, 0x00, 0x00, 0x7f, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    assert_eq!(
        check(&raw_font(b"GDEF", unresolved)),
        failure(LayoutTable::Gdef, LayoutCheck::Parse)
    );
}

#[test]
fn shared_offsets_exhaust_the_layout_work_budget() {
    // A GDEF 1.2 whose 40 mark glyph sets share one 30,000-glyph coverage:
    // 60 KB of table directing 1.2M visits.
    const SETS: u32 = 40;
    const GLYPHS: u16 = 30_000;
    let mut gdef = vec![
        0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00,
        0x01,
    ];
    gdef.extend(u16::try_from(SETS).expect("set count fits").to_be_bytes());
    for _ in 0..SETS {
        gdef.extend((4 + 4 * SETS).to_be_bytes());
    }
    gdef.extend([0x00, 0x01]);
    gdef.extend(GLYPHS.to_be_bytes());
    gdef.extend(std::iter::repeat_n(0, usize::from(GLYPHS) * 2));
    let sfnt = raw_font(b"GDEF", gdef);
    assert_eq!(
        check(&sfnt),
        Err(LayoutFailure {
            table: LayoutTable::Gdef,
            check: LayoutCheck::Budget,
        })
    );
    assert!(!font_subset_layout_oracle::accepts(&sfnt));
}

/// Every bundled source: the 230 official chunks, then the Noto Emoji
/// fallback, each as the sfnt the subsetter plans over.
fn bundled_sources() -> Vec<(svg_fonts::SubsetSource, Vec<u8>, Vec<u8>)> {
    crate::fonts::revision_1()
        .assets
        .iter()
        .map(svg_fonts::SubsetSource::chunk)
        .chain([svg_fonts::SubsetSource::noto_emoji()])
        .map(|source| {
            let bytes = source
                .load()
                .expect("bundled source verifies")
                .expect("bundled source exists")
                .bytes()
                .to_vec();
            let sfnt = match source.container() {
                font_subset::SourceContainer::Woff2 => {
                    wuff::decompress_woff2(&bytes).expect("bundled source decodes")
                }
                font_subset::SourceContainer::Sfnt => bytes.clone(),
            };
            (source, bytes, sfnt)
        })
        .collect()
}

#[test]
fn the_largest_bundled_layout_stays_under_the_work_budget() {
    let (identity, work) = bundled_sources()
        .into_iter()
        .map(|(source, _, sfnt)| {
            let work = check(&sfnt)
                .unwrap_or_else(|failure| panic!("{} fails: {failure:?}", source.identity()));
            (source.identity(), work)
        })
        .max_by_key(|(_, work)| *work)
        .expect("fonts are bundled");
    println!("largest bundled layout walk: {identity}, {work} of {MAX_LAYOUT_WORK}");
    assert!(
        work > 0 && work <= MAX_LAYOUT_WORK / 4,
        "{identity} uses {work}"
    );
}

/// How one input's typed verdict compared with the oracle's.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Comparison {
    BothAccept,
    BothReject,
    /// Rejected only by one of ADR-0020's new checks.
    NewCheck(&'static str),
    /// Any other disagreement, a bug in one walker: the typed walker's failure,
    /// or `None` when only the oracle rejects.
    Disagreement(Option<LayoutFailure>),
}

fn compare(sfnt: &[u8]) -> Comparison {
    let typed = FontRef::new(sfnt).map_or(Err(None), |font| validate(&font).map_err(Some));
    let oracle = font_subset_layout_oracle::accepts(sfnt);
    match (typed, oracle) {
        (Ok(_), true) => Comparison::BothAccept,
        (Err(_), false) => Comparison::BothReject,
        (Err(Some(failure)), true) if NEW_CHECKS.contains(&failure.check) => {
            Comparison::NewCheck(check_name(failure.check))
        }
        (Err(failure), true) => Comparison::Disagreement(failure),
        (Ok(_), false) => Comparison::Disagreement(None),
    }
}

const fn check_name(check: LayoutCheck) -> &'static str {
    match check {
        LayoutCheck::LookupIndex => "lookup-index",
        LayoutCheck::FeatureIndex => "feature-index",
        LayoutCheck::RangeOrder => "range-order",
        LayoutCheck::ClassValue => "class-value",
        LayoutCheck::Count => "count",
        LayoutCheck::SequenceIndex => "sequence-index",
        _ => "other",
    }
}

/// The comparison #210 requires before the typed walker replaces the
/// traversal one: identical verdicts over every bundled source, every corpus
/// subset, every fixture and seeded mutations of each, except rejections by
/// the new checks, which no bundled source or corpus subset may hit.
///
/// `EXCALIDRAW_LAYOUT_ORACLE_MUTATIONS` sets the mutations per font (default
/// 10,000). A disagreement is reported by its seed and case, which replay it.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "on-demand oracle comparison for #210"]
fn typed_layout_walker_matches_the_traversal_oracle() {
    let mutations = std::env::var("EXCALIDRAW_LAYOUT_ORACLE_MUTATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);
    let mut totals = BTreeMap::<(&str, Comparison), usize>::new();
    let mut disagreements = Vec::new();
    let mut record = |group: &'static str, label: &str, sfnt: &[u8]| {
        let comparison = compare(sfnt);
        *totals.entry((group, comparison)).or_default() += 1;
        if let Comparison::Disagreement(failure) = comparison {
            disagreements.push(format!("{group} {label}: {failure:?}"));
        }
        comparison
    };

    let sources = bundled_sources();
    for (index, (source, _, sfnt)) in sources.iter().enumerate() {
        let comparison = record("source", source.identity(), sfnt);
        assert_eq!(comparison, Comparison::BothAccept, "{}", source.identity());
        let Some(layout) = font_subset_fuzz::layout_sfnt(sfnt) else {
            continue;
        };
        let seed = 0x9e37_79b9_7f4a_7c15 ^ u64::try_from(index).expect("index fits");
        for (case, mutated) in font_subset_fuzz::layout_mutations(&layout, seed, mutations)
            .iter()
            .enumerate()
        {
            let label = format!("{} seed {seed:016x} case {case}", source.identity());
            record("source-mutation", &label, mutated);
        }
    }

    let corpus: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/qualification/font-subset-corpus.json"
    )))
    .expect("the qualification corpus is JSON");
    let corpus_sources = corpus["sources"]
        .as_array()
        .expect("the corpus lists sources");
    assert_eq!(corpus_sources.len(), sources.len());
    for ((source, bytes, _), qualified) in sources.iter().zip(corpus_sources) {
        assert_eq!(qualified["sourceIdentity"], source.identity());
        for cell in qualified["cells"].as_array().expect("sources have cells") {
            if cell["status"] != "success" {
                continue;
            }
            let scalars = cell["orderedScalars"]
                .as_array()
                .expect("success cells have scalars")
                .iter()
                .map(|scalar| {
                    u32::try_from(scalar.as_u64().expect("scalars are integers"))
                        .expect("scalars fit")
                })
                .collect::<Vec<_>>();
            let identity = svg_fonts::source_semantic_identity(*source, &scalars)
                .expect("corpus identity is representable");
            let label = format!("{} {}", source.identity(), cell["class"]);
            let derived = font_subset::derive(
                bytes,
                source.container(),
                &scalars,
                "ExcalidrawRender-9",
                &identity,
            )
            .unwrap_or_else(|error| panic!("{label} is newly rejected at {:?}", error.stage));
            let subset = wuff::decompress_woff2(&derived.woff2).expect("subset decodes");
            assert_eq!(
                record("corpus-subset", &label, &subset),
                Comparison::BothAccept,
                "{label}"
            );
        }
    }

    let base = maxp_font(MIN_GLYPHS);
    for (index, fixture) in layout_fixtures().iter().enumerate() {
        let sfnt = fixture.install(&base);
        assert_eq!(
            record("fixture", fixture.name, &sfnt),
            Comparison::BothAccept,
            "{}",
            fixture.name
        );
        let seed = 0x3c6e_f372_fe94_f82b ^ u64::try_from(index).expect("index fits");
        for (case, mutated) in font_subset_fuzz::layout_mutations(&sfnt, seed, mutations)
            .iter()
            .enumerate()
        {
            let label = format!("{} seed {seed:016x} case {case}", fixture.name);
            record("fixture-mutation", &label, mutated);
        }
    }

    for ((group, comparison), count) in &totals {
        println!("{group:>16} {comparison:?}: {count}");
    }
    for disagreement in &disagreements {
        println!("disagreement {disagreement}");
    }
    assert!(
        disagreements.is_empty(),
        "{} disagreements",
        disagreements.len()
    );
}
