use std::collections::BTreeMap;

use excalidraw_render_assets_base::{load_asset, revision_1, RAW_SHARD_LIMIT};

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
