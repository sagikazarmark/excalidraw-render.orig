use excalidraw_render_fonts::fonts::{find_face, Container, FamilyId};
use excalidraw_render_fonts::{font_face_order, font_face_unicode_ranges, revision_1};

#[test]
fn ordered_families_cover_the_catalog_without_changing_declarations() {
    let mut identities = std::collections::BTreeSet::new();
    for family in FamilyId::ALL {
        assert_eq!(FamilyId::from_name(family.catalog_name()), Some(family));
        assert_eq!(FamilyId::from_name(family.display_name()), Some(family));
        assert_eq!(family.notice().unwrap().family, family.catalog_name());
        assert!(!family.faces().is_empty());
        for (order, face) in family.faces().iter().enumerate() {
            assert!(
                identities.insert(face.identity()),
                "face belongs to exactly one family"
            );
            assert_eq!(face.family(), family);
            assert!(std::ptr::eq(face, face.id().face()));
            if let Some(record) = revision_1().asset(face.identity()) {
                assert!(face.is_official_chunk());
                assert!(std::ptr::eq(face, find_face(record.upstream_path).unwrap()));
                assert_eq!(
                    font_face_order(record.family, record.upstream_path),
                    Some(order)
                );
                assert_eq!(
                    font_face_unicode_ranges(record.family, record.upstream_path),
                    Some(face.declared_ranges())
                );
                assert_eq!(face.container(), Container::Woff2);
                assert_eq!(face.is_available(), record.is_available());
                assert_eq!(face.source_sha256(), record.complete_sha256);
                assert_eq!(face.source_length(), record.complete_length);
                assert_eq!(face.decoded_length(), record.decoded_length);
            } else {
                assert!(!face.is_official_chunk());
                assert_eq!(family, FamilyId::NotoEmoji);
                assert_eq!(face.container(), Container::Sfnt);
                assert!(face.is_available());
                assert_eq!(face.declared_ranges(), &[(0, 0x10ffff)]);
            }
        }
    }
    assert_eq!(identities.len(), revision_1().assets.len() + 1);
    assert!(FamilyId::from_name("not a family").is_none());
    assert!(find_face("not a path").is_none());
}

#[test]
fn font_source_loading_preserves_manifested_bytes_and_optional_payloads() {
    use sha2::{Digest, Sha256};
    for face in FamilyId::ALL.into_iter().flat_map(FamilyId::faces) {
        if face.is_available() {
            let source = face.load_source().unwrap();
            if let Some(record) = revision_1().asset(face.identity()) {
                assert!(
                    std::ptr::eq(source.bytes(), record.bytes.unwrap()),
                    "verified source borrows the individual embedded file"
                );
            }
            assert_eq!(source.bytes().len(), face.source_length());
            assert_eq!(
                lower_hex(&Sha256::digest(source.bytes())),
                face.source_sha256()
            );
        } else {
            let error = face.load_source().unwrap_err();
            assert!(error.is_unavailable());
            assert_eq!(error.asset_identity(), face.identity());
        }
    }
}

#[test]
fn reviewed_program_validation_rejects_different_bytes_for_chunks_and_complete_faces() {
    for face in [
        &FamilyId::Liberation.faces()[0],
        &FamilyId::NotoEmoji.faces()[0],
    ] {
        let source = face.load_source().unwrap();
        let mut sfnt = match face.container() {
            Container::Woff2 => wuff::decompress_woff2(source.bytes()).unwrap(),
            Container::Sfnt => source.into_bytes(),
        };
        assert_eq!(face.verify_decode(sfnt.clone()).unwrap(), sfnt);
        sfnt[0] ^= 1;
        let error = face.verify_decode(sfnt).unwrap_err();
        assert_eq!(error.asset_identity(), face.identity());
        assert!(!error.is_unavailable());
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
