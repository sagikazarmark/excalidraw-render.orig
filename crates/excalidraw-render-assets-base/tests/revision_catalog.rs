use std::collections::BTreeMap;

use excalidraw_render_assets_base::{
    excalifont_basic_latin_sfnt_transformation, load_asset, load_excalifont_basic_latin_sfnt,
    revision_1, FontSubsetTableAction, CJK_SCALAR_COUNT_V1, CJK_SCALAR_RANGES_V1,
    CJK_SCALAR_TABLE_BYTES_V1, CJK_SCALAR_TABLE_SHA256_V1, FONT_SUBSET_PROFILE_SETTINGS_V1,
    FONT_SUBSET_TABLE_RULES_V1, RAW_SHARD_LIMIT,
};
use sha2::{Digest, Sha256};

#[test]
fn every_catalog_asset_reassembles_and_verifies_before_use() {
    let catalog = revision_1();

    assert_eq!(catalog.compatibility_revision, 1);
    assert_eq!(catalog.excalidraw_release, "v0.18.1");
    assert_eq!(catalog.assets.len(), 230);
    assert_eq!(catalog.fallback_recipe_identities.len(), 5);
    assert_eq!(catalog.notice_identities.len(), 4);
    assert_eq!(catalog.provenance_identities, &["excalidraw.npm-v0.18.1"]);

    let mut shard_extents = BTreeMap::<&str, Vec<(usize, usize, usize)>>::new();
    for asset in catalog.assets {
        assert!(catalog
            .provenance_identities
            .contains(&asset.provenance_identity));
        let verified = load_asset(asset.upstream_path)
            .expect("checked-in segments retain their digests")
            .expect("every catalog path resolves");
        assert_eq!(verified.bytes().len(), asset.complete_length);
        assert_eq!(&verified.bytes()[..4], b"wOF2");
        assert!(
            excalidraw_render_assets_base::font_face_decoded_length(asset.upstream_path)
                .is_some_and(|length| length > 0)
        );
        assert!(asset.segments.iter().all(|segment| {
            segment.bytes.len() <= RAW_SHARD_LIMIT && segment.order < asset.segments.len()
        }));
        for segment in asset.segments {
            shard_extents.entry(segment.shard).or_default().push((
                segment.offset,
                segment.offset + segment.length,
                segment.bytes.len(),
            ));
        }
    }
    for extents in shard_extents.values_mut() {
        extents.sort_unstable();
        let mut covered_end = 0;
        for &(offset, end, _) in extents.iter() {
            assert_eq!(offset, covered_end, "shard segments must be contiguous");
            covered_end = end;
        }
        assert_eq!(
            covered_end,
            extents.last().expect("shard has at least one segment").2,
            "every shard byte must belong to a manifested segment"
        );
    }

    assert!(load_asset("fonts/Assistant/Assistant-Regular.woff2")
        .expect("missing lookup is not an integrity error")
        .is_none());
}

#[test]
fn predecoded_excalifont_basic_latin_sfnt_is_checked_in_and_digest_verified() {
    let source =
        load_asset("fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2")
            .expect("source integrity")
            .expect("source is manifested");
    let sfnt = load_excalifont_basic_latin_sfnt().expect("predecoded sfnt integrity");
    let transformation = excalifont_basic_latin_sfnt_transformation();

    assert_eq!(
        format!("{:x}", Sha256::digest(source.bytes())),
        transformation.source_sha256
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(sfnt.bytes())),
        transformation.output_sha256
    );
    assert_eq!(
        wuff::decompress_woff2(source.bytes()).expect("source decodes"),
        sfnt.bytes()
    );
    let face = rustybuzz::Face::from_slice(sfnt.bytes(), 0).expect("output is a valid sfnt");
    assert!(face.glyph_index('A').is_some());
    assert_eq!(transformation.tool, "wuff 0.2.8");
    assert_eq!(
        transformation.invocation,
        "wuff::decompress_woff2(source_bytes)"
    );
    assert_eq!(transformation.reviewer, "OpenAI gpt-5.6-sol");
    assert_eq!(transformation.review_date, "2026-07-22");
    assert!(transformation.exact_output_bytes_reviewed);
    assert!(std::str::from_utf8(transformation.metadata_json)
        .expect("transformation metadata is UTF-8")
        .contains("all 14 SFNT tables and declared checksums are identical"));
}

#[test]
fn noto_emoji_3003_fallback_is_digest_verified_and_parseable() {
    let asset = excalidraw_render_assets_base::load_noto_emoji_sfnt()
        .expect("approved Noto Emoji fallback verifies");
    assert_eq!(asset.bytes().len(), 862_680);
    let face = rustybuzz::Face::from_slice(asset.bytes(), 0).expect("Noto Emoji is valid SFNT");
    assert_eq!(face.units_per_em(), 1_000);
    assert!(face.glyph_index('\u{1f600}').is_some());
    assert!(face.glyph_index('A').is_none());

    let metadata = include_str!("../catalog/fallback/NotoEmoji-Regular.ttf.json");
    assert!(metadata.contains("Version 3.003"));
    assert!(metadata.contains("0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"));
}

#[test]
fn every_official_face_has_a_unique_pinned_family_order() {
    let catalog = excalidraw_render_assets_base::revision_1();
    for family in [
        "Cascadia",
        "ComicShanns",
        "Excalifont",
        "Liberation",
        "Lilita",
        "Nunito",
        "Virgil",
        "Xiaolai",
    ] {
        let mut orders = Vec::new();
        for asset in catalog.assets.iter().filter(|asset| asset.family == family) {
            let order = excalidraw_render_assets_base::font_face_order(family, asset.upstream_path)
                .unwrap_or_else(|| panic!("{family} face {} has an order", asset.upstream_path));
            assert!(!orders.contains(&order), "{family} order {order} is unique");
            orders.push(order);
        }
        orders.sort_unstable();
        assert_eq!(orders, (0..orders.len()).collect::<Vec<_>>());
    }
}

#[test]
fn every_official_face_exposes_its_pinned_declared_unicode_ranges() {
    let catalog = excalidraw_render_assets_base::revision_1();
    for asset in catalog.assets {
        let ranges = excalidraw_render_assets_base::font_face_unicode_ranges(
            asset.family,
            asset.upstream_path,
        )
        .unwrap_or_else(|| panic!("{} has declared ranges", asset.upstream_path));
        assert!(!ranges.is_empty());
        assert!(ranges.iter().all(|(start, end)| start <= end));
    }

    let latin = excalidraw_render_assets_base::font_face_unicode_ranges(
        "Excalifont",
        "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
    )
    .expect("Excalifont Latin ranges");
    assert!(latin
        .iter()
        .any(|(start, end)| *start <= u32::from('A') && u32::from('A') <= *end));
    assert!(!latin
        .iter()
        .any(|(start, end)| *start <= u32::from('Ж') && u32::from('Ж') <= *end));
}

#[test]
fn svg_font_subset_profile_classifies_each_table_once() {
    let mut tags = FONT_SUBSET_TABLE_RULES_V1
        .iter()
        .map(|rule| rule.tag)
        .collect::<Vec<_>>();
    tags.sort_unstable();
    tags.dedup();

    assert_eq!(tags.len(), FONT_SUBSET_TABLE_RULES_V1.len());
    assert!(FONT_SUBSET_PROFILE_SETTINGS_V1
        .starts_with(b"excalidraw-render:svg-font-subset-profile:v1\n"));
    let digest = excalidraw_render_assets_base::font_subset_profile_digest_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        digest,
        "49015dfb91f9e89fcf1e7f3e16fd892e4c1715f5d49b6488d67037b7aac21c23"
    );
}

#[test]
fn chromium_149_cjk_scalar_table_bytes_and_classes_are_frozen() {
    assert_eq!(&CJK_SCALAR_TABLE_BYTES_V1[..8], b"ER-CJK1\0");
    assert_eq!(CJK_SCALAR_RANGES_V1.len(), 63);
    assert_eq!(
        CJK_SCALAR_RANGES_V1
            .iter()
            .map(|(start, end)| end - start + 1)
            .sum::<u32>(),
        CJK_SCALAR_COUNT_V1
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(CJK_SCALAR_TABLE_BYTES_V1)),
        CJK_SCALAR_TABLE_SHA256_V1
    );
    assert!(CJK_SCALAR_RANGES_V1
        .windows(2)
        .all(|ranges| ranges[0].1 < ranges[1].0));
    for scalar in [
        0x4e2d,
        0x3042,
        0x30a2,
        0xac00,
        u32::from('〃'),
        u32::from('（'),
        u32::from('。'),
        u32::from('￥'),
        0x31350,
        0x33479,
    ] {
        assert!(excalidraw_render_assets_base::is_cjk_scalar_v1(scalar));
    }
    for scalar in [u32::from('A'), 0x2ff0, 0xd800, 0x10ffff] {
        assert!(!excalidraw_render_assets_base::is_cjk_scalar_v1(scalar));
    }
}

#[test]
fn svg_font_compatibility_artifacts_are_incorporated_into_revision_1() {
    let catalog = revision_1();
    let profile = catalog
        .incorporated_artifacts
        .iter()
        .find(|artifact| {
            artifact.repository_path
                == "tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json"
        })
        .expect("the SVG font subset profile is incorporated");
    let cjk = catalog
        .incorporated_artifacts
        .iter()
        .find(|artifact| {
            artifact.repository_path
                == "tests/oracle/compatibility-artifacts/revision-1-chromium-149-cjk-scalars.bin"
        })
        .expect("the Chromium CJK table is incorporated");

    assert_eq!(catalog.incorporated_artifacts.len(), 8);
    assert_eq!(cjk.bytes, CJK_SCALAR_TABLE_BYTES_V1);
    assert_eq!(cjk.sha256, CJK_SCALAR_TABLE_SHA256_V1);

    let profile: serde_json::Value =
        serde_json::from_slice(profile.bytes).expect("the subset profile is JSON");
    assert_eq!(
        profile["settingsPreimage"],
        std::str::from_utf8(FONT_SUBSET_PROFILE_SETTINGS_V1)
            .expect("the profile settings are UTF-8")
    );
    assert_eq!(
        profile["semanticDigest"],
        "sha256:49015dfb91f9e89fcf1e7f3e16fd892e4c1715f5d49b6488d67037b7aac21c23"
    );
    let rules = profile["tableRules"]
        .as_array()
        .expect("the table rules are an array");
    assert_eq!(rules.len(), FONT_SUBSET_TABLE_RULES_V1.len());
    for (actual, expected) in rules.iter().zip(FONT_SUBSET_TABLE_RULES_V1) {
        assert_eq!(actual["tag"], std::str::from_utf8(&expected.tag).unwrap());
        assert_eq!(actual["required"], expected.required);
        assert_eq!(
            actual["action"],
            match expected.action {
                FontSubsetTableAction::Subset => "subset",
                FontSubsetTableAction::Rewrite => "rewrite",
                FontSubsetTableAction::PassThrough => "pass_through",
                FontSubsetTableAction::Drop => "drop",
            }
        );
    }
}
