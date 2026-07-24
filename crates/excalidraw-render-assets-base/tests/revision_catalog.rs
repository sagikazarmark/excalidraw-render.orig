use std::collections::BTreeMap;

use excalidraw_render_assets_base::{
    excalifont_basic_latin_sfnt_transformation, load_asset, load_excalifont_basic_latin_sfnt,
    revision_1, RAW_SHARD_LIMIT,
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
