use std::collections::{BTreeMap, BTreeSet};

use excalidraw_render_fonts::{
    excalifont_basic_latin_sfnt_transformation, find_asset, load_asset,
    load_excalifont_basic_latin_sfnt, revision_1, verify_reviewed_decode, FontLicense,
    FontSubsetTableAction, CJK_SCALAR_COUNT_V1, CJK_SCALAR_RANGES_V1, CJK_SCALAR_TABLE_BYTES_V1,
    CJK_SCALAR_TABLE_SHA256_V1, FONT_SUBSET_PROFILE_SETTINGS_V1, FONT_SUBSET_TABLE_RULES_V1,
    NOTO_EMOJI_FAMILY, NOTO_EMOJI_IDENTITY,
};
use sha2::{Digest, Sha256};

/// The `+excalidraw.x.y.z` build metadata names the release the bundled fonts
/// come from, so the label cannot drift from the catalog.
#[test]
fn package_version_names_the_catalog_excalidraw_release() {
    let (_, metadata) = env!("CARGO_PKG_VERSION")
        .split_once('+')
        .expect("the package version carries build metadata");
    let release = metadata
        .strip_prefix("excalidraw.")
        .expect("the build metadata names the Excalidraw release");
    assert_eq!(format!("v{release}"), revision_1().excalidraw_release);
}

#[test]
fn payloads_keep_whole_chunks_in_the_approved_family_and_declaration_order() {
    let mut payloads = BTreeMap::<&str, Vec<_>>::new();
    for asset in revision_1().assets {
        assert_eq!(asset.segments.len(), 1, "source chunks remain whole");
        let segment = &asset.segments[0];
        assert_eq!(segment.length, asset.complete_length);
        assert_eq!(segment.sha256, asset.complete_sha256);
        if asset.family == "Xiaolai" {
            assert!(matches!(segment.shard, "revision-1-001" | "revision-1-002"));
        } else {
            assert_eq!(segment.shard, "revision-1-000");
        }
        payloads
            .entry(segment.shard)
            .or_default()
            .push((segment.offset, asset));
    }
    assert_eq!(payloads.len(), 3);
    for ((_, assets), (count, length)) in
        payloads
            .iter_mut()
            .zip([(21, 358_432), (165, 9_596_296), (44, 3_071_196)])
    {
        assets.sort_by_key(|(offset, _)| *offset);
        assert_eq!(assets.len(), count);
        let mut end = 0;
        for &(offset, asset) in assets.iter() {
            assert_eq!(offset, end);
            end += asset.complete_length;
        }
        assert_eq!(end, length);
        if assets[0].1.family == "Xiaolai" {
            for pair in assets.windows(2) {
                let order = |asset: &excalidraw_render_fonts::AssetRecord| {
                    excalidraw_render_fonts::font_face_order(asset.family, asset.upstream_path)
                        .unwrap()
                };
                assert!(order(pair[0].1) < order(pair[1].1));
            }
        }
    }
}

#[test]
fn xiaolai_groups_match_published_path_and_content_fingerprints() {
    // Published excalidraw-image-fonts-cjk{,-extra} 1.0.0 fingerprints. This
    // remains testable without CJK bytes; the available-byte test below binds
    // the catalog source hashes to actual bytes when the feature is enabled.
    for (shard, count, fingerprint) in [
        (
            "revision-1-001",
            165,
            "630cb497d7b7de4fea58a890b78a2de5ea0e74221d8b876156de4993698e24ad",
        ),
        (
            "revision-1-002",
            44,
            "023c933b9a01e3b09cc6d8f58dbb7c9aaf6376086290338814ffa1e51c1a8a61",
        ),
    ] {
        let mut lines = revision_1()
            .assets
            .iter()
            .filter(|asset| asset.segments[0].shard == shard)
            .map(|asset| {
                format!(
                    "{}:{}",
                    asset.upstream_path.strip_prefix("fonts/").unwrap(),
                    asset.complete_sha256
                )
            })
            .collect::<Vec<_>>();
        lines.sort();
        assert_eq!(lines.len(), count);
        assert_eq!(lower_hex(&Sha256::digest(lines.join("\n"))), fingerprint);
    }
}

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
        assert_eq!(
            excalidraw_render_fonts::font_face_decoded_length(asset.upstream_path),
            Some(asset.decoded_length)
        );
        assert_eq!(
            asset.is_available(),
            asset.family != "Xiaolai" || cfg!(feature = "cjk")
        );
        if !asset.is_available() {
            let error = load_asset(asset.upstream_path).unwrap_err();
            assert!(error.is_unavailable());
            assert_eq!(error.asset_identity(), asset.identity);
            assert_eq!(error.segment_identity(), None);
            continue;
        }
        let verified = load_asset(asset.upstream_path)
            .expect("checked-in segments retain their digests")
            .expect("every catalog path resolves");
        assert_eq!(verified.bytes().len(), asset.complete_length);
        assert_eq!(&verified.bytes()[..4], b"wOF2");
        assert!(
            excalidraw_render_fonts::font_face_decoded_length(asset.upstream_path)
                .is_some_and(|length| length > 0)
        );
        assert_eq!(asset.bytes.unwrap(), verified.bytes());
        for segment in asset.segments {
            shard_extents.entry(segment.shard).or_default().push((
                segment.offset,
                segment.offset + segment.length,
                match segment.shard {
                    "revision-1-000" => 358_432,
                    "revision-1-001" => 9_596_296,
                    "revision-1-002" => 3_071_196,
                    other => panic!("unknown historical packing group: {other}"),
                },
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

/// Spec 8.6: every official font chunk's reviewed decode is what the pinned
/// decoder produces for its exact manifested bytes, and the catalog records it
/// by length and digest. The renderer may embed a decode only after matching
/// it, so a decode that drifts by one byte is an integrity failure, not a font.
#[test]
fn every_official_font_chunk_carries_a_reviewed_decode_the_catalog_verifies() {
    let catalog = revision_1();
    for asset in catalog.assets {
        if !asset.is_available() {
            continue;
        }
        let source = load_asset(asset.upstream_path)
            .expect("source integrity")
            .expect("source is manifested");
        let decoded = wuff::decompress_woff2(source.bytes()).expect("qualified chunk decodes");
        assert_eq!(
            u32::from_be_bytes(source.bytes()[16..20].try_into().unwrap()) as usize,
            asset.decoded_length,
            "reviewed length equals the source WOFF2 header"
        );
        assert_eq!(
            decoded.len(),
            asset.decoded_length,
            "{} decodes to its recorded length",
            asset.upstream_path
        );
        assert_eq!(
            lower_hex(&Sha256::digest(&decoded)),
            asset.decoded_sha256,
            "{} decodes to its reviewed digest",
            asset.upstream_path
        );
        let verified = verify_reviewed_decode(asset, decoded.clone())
            .expect("the pinned decoder's output is the reviewed decode");
        assert_eq!(verified.bytes(), decoded.as_slice());
        assert_eq!(verified.into_bytes(), decoded);
    }

    let liberation = find_asset("fonts/Liberation/LiberationSans-Regular.woff2")
        .expect("the Liberation Sans replacement is an official chunk");
    let mut drifted = wuff::decompress_woff2(
        load_asset(liberation.upstream_path)
            .expect("source integrity")
            .expect("source is manifested")
            .bytes(),
    )
    .expect("qualified chunk decodes");
    let middle = drifted.len() / 2;
    drifted[middle] ^= 0x01;
    let error = verify_reviewed_decode(liberation, drifted)
        .expect_err("a decode that is not the reviewed one is rejected");
    assert_eq!(error.asset_identity(), liberation.identity);
    assert!(!error.is_unavailable());
    assert_eq!(error.segment_identity(), None);
    assert!(error.to_string().contains("reviewed decode"));
    assert!(find_asset("fonts/Assistant/Assistant-Regular.woff2").is_none());
}

/// The one chunk whose reviewed decode is also checked in byte for byte (#58)
/// records the same decode as the catalog, so the two paths the renderer may
/// take to the Excalifont basic-Latin sfnt cannot disagree.
#[test]
fn the_checked_in_excalifont_decode_is_the_catalog_reviewed_decode() {
    let asset =
        find_asset("fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2")
            .expect("the Excalifont basic-Latin chunk is an official chunk");
    let sfnt = load_excalifont_basic_latin_sfnt().expect("predecoded sfnt integrity");
    assert_eq!(sfnt.bytes().len(), asset.decoded_length);
    assert_eq!(
        lower_hex(&Sha256::digest(sfnt.bytes())),
        asset.decoded_sha256
    );
    assert_eq!(
        asset.decoded_sha256,
        excalifont_basic_latin_sfnt_transformation().output_sha256
    );
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
        lower_hex(&Sha256::digest(source.bytes())),
        transformation.source_sha256
    );
    assert_eq!(
        lower_hex(&Sha256::digest(sfnt.bytes())),
        transformation.output_sha256
    );
    assert_eq!(
        wuff::decompress_woff2(source.bytes()).expect("source decodes"),
        sfnt.bytes()
    );
    let face = ttf_parser::Face::parse(sfnt.bytes(), 0).expect("output is a valid sfnt");
    assert!(face.glyph_index('A').is_some());
    assert_eq!(transformation.tool, "wuff 0.2.9");
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
    let asset = excalidraw_render_fonts::load_noto_emoji_sfnt()
        .expect("approved Noto Emoji fallback verifies");
    assert_eq!(asset.bytes().len(), 862_680);
    let face = ttf_parser::Face::parse(asset.bytes(), 0).expect("Noto Emoji is valid SFNT");
    assert_eq!(face.units_per_em(), 1_000);
    assert!(face.glyph_index('\u{1f600}').is_some());
    assert!(face.glyph_index('A').is_none());

    let metadata = include_str!("../catalog/fallback/NotoEmoji-Regular.ttf.json");
    assert!(metadata.contains("Version 3.003"));
    assert!(metadata.contains("0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"));
}

#[test]
fn every_official_face_has_a_unique_pinned_family_order() {
    let catalog = excalidraw_render_fonts::revision_1();
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
            let order = excalidraw_render_fonts::font_face_order(family, asset.upstream_path)
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
    let catalog = excalidraw_render_fonts::revision_1();
    for asset in catalog.assets {
        let ranges =
            excalidraw_render_fonts::font_face_unicode_ranges(asset.family, asset.upstream_path)
                .unwrap_or_else(|| panic!("{} has declared ranges", asset.upstream_path));
        assert!(!ranges.is_empty());
        assert!(ranges.iter().all(|(start, end)| start <= end));
    }

    let latin = excalidraw_render_fonts::font_face_unicode_ranges(
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

/// Pins concrete values transcribed from `catalog/order/Xiaolai-index.ts`: the
/// first, third (single-line descriptor), and last of its 209 declarations.
#[test]
fn xiaolai_chunks_follow_the_pinned_face_order_and_declared_ranges_of_the_index() {
    let face = |file_name: &str| {
        let upstream_path = format!("fonts/Xiaolai/{file_name}");
        let order = excalidraw_render_fonts::font_face_order("Xiaolai", &upstream_path)
            .unwrap_or_else(|| panic!("{file_name} has a pinned order"));
        let ranges = excalidraw_render_fonts::font_face_unicode_ranges("Xiaolai", &upstream_path)
            .unwrap_or_else(|| panic!("{file_name} has declared ranges"));
        (order, ranges)
    };

    let (order, ranges) = face("Xiaolai-Regular-09850c4077f3fffe707905872e0e2460.woff2");
    assert_eq!(order, 0);
    assert_eq!(
        ranges,
        [
            (0xf9b8, 0xfa6d),
            (0xfe32, 0xfe32),
            (0xfe45, 0xfe4f),
            (0xff02, 0xff0b),
            (0xff0d, 0xff1e),
            (0xff20, 0xff2a),
        ]
    );

    let (order, ranges) = face("Xiaolai-Regular-60a3089806700d379f11827ee9843b6b.woff2");
    assert_eq!(order, 2);
    assert_eq!(ranges, [(0xd7eb, 0xd7fb), (0xf900, 0xf9b7)]);

    let (order, ranges) = face("Xiaolai-Regular-2b7441d46298788ac94e610ffcc709b6.woff2");
    assert_eq!(order, 208);
    assert_eq!(ranges.len(), 19);
    assert_eq!(ranges.first(), Some(&(0x7e2b, 0x7e3a)));
    assert_eq!(ranges.last(), Some(&(0x7f3b, 0x7f3b)));

    assert_eq!(
        excalidraw_render_fonts::font_face_order(
            "Xiaolai",
            "fonts/Xiaolai/Xiaolai-Regular-0000000000000000000000000000000.woff2",
        ),
        None
    );
}

/// Pins the upstream Google Fonts `latin` subset declaration Nunito's fifth chunk
/// carries: 20 ranges from `U+0000-00FF` to `U+FFFD`.
#[test]
fn nunito_latin_chunk_declares_the_upstream_google_fonts_latin_subset() {
    let upstream_path =
        "fonts/Nunito/Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTQ3j6zbXWjgeg.woff2";
    assert_eq!(
        excalidraw_render_fonts::font_face_order("Nunito", upstream_path),
        Some(4)
    );
    let ranges = excalidraw_render_fonts::font_face_unicode_ranges("Nunito", upstream_path)
        .expect("Nunito latin ranges");
    assert_eq!(ranges.len(), 20);
    assert_eq!(ranges.first(), Some(&(0x0000, 0x00ff)));
    assert_eq!(ranges.get(3), Some(&(0x02bb, 0x02bc)));
    assert_eq!(ranges.get(10), Some(&(0x2000, 0x206f)));
    assert_eq!(ranges.last(), Some(&(0xfffd, 0xfffd)));
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
    let digest = excalidraw_render_fonts::font_subset_profile_digest_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        digest,
        "522b5b9a6f1ae71af1ce72ddda9db2c6aca6607fc7567b1362ec449c2339abd0"
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
        lower_hex(&Sha256::digest(CJK_SCALAR_TABLE_BYTES_V1)),
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
        assert!(excalidraw_render_fonts::is_cjk_scalar_v1(scalar));
    }
    for scalar in [u32::from('A'), 0x2ff0, 0xd800, 0x10ffff] {
        assert!(!excalidraw_render_fonts::is_cjk_scalar_v1(scalar));
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
        "sha256:522b5b9a6f1ae71af1ce72ddda9db2c6aca6607fc7567b1362ec449c2339abd0"
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

/// Artifacts attribute each official font through its catalog record's
/// family: the notice names the record's license and carries the bundle's
/// provenance line without its license sentence, plus the license terms. The
/// Noto Emoji fallback, which is no asset record but is subset for direct SVG
/// like the chunks (#150), has the ninth notice, with the copyright statement
/// its sealed `noto-emoji.ofl-full` notice opens with.
#[test]
fn every_official_font_resolves_one_notice_by_asset_identity_that_agrees_with_its_record() {
    let catalog = revision_1();
    assert_eq!(catalog.font_notices.len(), 9);
    let asset_families = catalog
        .assets
        .iter()
        .map(|asset| asset.family)
        .collect::<BTreeSet<_>>();
    assert_eq!(asset_families.len(), 8);
    assert!(!asset_families.contains(NOTO_EMOJI_FAMILY));
    for notice in catalog.font_notices {
        assert!(
            asset_families.contains(notice.family) || notice.family == NOTO_EMOJI_FAMILY,
            "{} covers no asset family and is not the fallback face",
            notice.family
        );
    }
    let noto_emoji = catalog
        .font_notice(NOTO_EMOJI_FAMILY)
        .expect("the Noto Emoji fallback has a notice");
    assert_eq!(noto_emoji.license_identity, "font.noto-emoji.ofl-1.1");
    assert_eq!(noto_emoji.license, FontLicense::Ofl11);
    assert_eq!(noto_emoji.attribution, "Copyright 2013 Google LLC.");
    assert_eq!(noto_emoji.terms_identity, "license.ofl-1.1");
    assert!(noto_emoji
        .terms
        .starts_with("SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007"));
    assert!(
        catalog.asset(NOTO_EMOJI_IDENTITY).is_none(),
        "the fallback face is not a catalog asset record"
    );
    // The OFL body is the bundle's copy of the license and is shared by every
    // OFL family (#151): one `license.ofl-1.1` notice per artifact, referred
    // to by each family's attribution notice.
    let bundle = include_str!("../LICENSE-BUNDLE.txt");
    let ofl_part = bundle
        .split_once("SIL Open Font License 1.1\n==============================================================================\n\n")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split("\n\n==============================================================================\n").next())
        .expect("the bundle prints the OFL 1.1 text");
    assert_eq!(noto_emoji.terms, ofl_part);
    assert_eq!(
        noto_emoji.attribution_notice().unwrap(),
        "Copyright 2013 Google LLC. Licensed under the SIL Open Font License, Version 1.1; the license text is this manifest's license.ofl-1.1 notice."
    );

    for asset in catalog.assets {
        let record = catalog
            .asset(asset.identity)
            .expect("every asset identity resolves to its record");
        assert!(std::ptr::eq(record, asset));
        let notice = catalog
            .font_notice(asset.family)
            .expect("every asset family has a notice");
        assert_eq!(notice.license_identity, asset.license_identity);
        assert!(!notice.attribution.contains("Licensed under"));
        assert!(!notice.terms.ends_with('\n'));
        assert_eq!(
            notice.attribution_notice().unwrap(),
            format!(
                "{} {}; the license text is this manifest's {} notice.",
                notice.attribution,
                notice.license.sentence(),
                notice.terms_identity
            )
        );
        match notice.license {
            FontLicense::Ofl11 => {
                assert!(notice.license_identity.ends_with(".ofl-1.1"));
                assert_eq!(notice.terms_identity, "license.ofl-1.1");
                assert!(std::ptr::eq(notice.terms, noto_emoji.terms));
            }
            FontLicense::Mit => {
                assert_eq!(notice.license_identity, "font.comic-shanns.mit");
                assert_eq!(notice.terms_identity, "license.comic-shanns.mit");
                assert!(notice
                    .terms
                    .starts_with("MIT License\n\nCopyright (c) 2018 Shannon Miwa"));
            }
        }
    }

    let cascadia = catalog
        .font_notice("Cascadia")
        .expect("Cascadia has a notice");
    assert_eq!(
        cascadia.attribution,
        "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code."
    );
    assert!(catalog
        .asset("fonts/Assistant/Assistant-Regular.woff2")
        .is_none());
    assert!(catalog.font_notice("Assistant").is_none());
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
