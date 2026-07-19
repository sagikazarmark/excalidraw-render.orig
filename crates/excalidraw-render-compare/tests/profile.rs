use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest, Sha256};

const PROFILE_PATH: &str = "tests/conformance/comparator-profiles/canonical-visual-v1.json";
const PROFILE_DIGEST_PATH: &str =
    "tests/conformance/comparator-profiles/canonical-visual-v1.sha256";

#[test]
fn canonical_comparator_profile_schema_and_all_bound_bytes_are_current() {
    let root = repository_root();
    let profile_bytes = fs::read(root.join(PROFILE_PATH)).unwrap();
    let recorded_digest = fs::read_to_string(root.join(PROFILE_DIGEST_PATH)).unwrap();
    assert_eq!(recorded_digest.trim(), sha256(&profile_bytes));

    let profile: Value = serde_json::from_slice(&profile_bytes).unwrap();
    let schema_bytes = verify_descriptor(&root, &profile["schema"]);
    let schema: Value = serde_json::from_slice(&schema_bytes).unwrap();
    jsonschema::draft202012::meta::validate(&schema).unwrap();
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .unwrap()
        .validate(&profile)
        .unwrap();

    verify_descriptor(&root, &profile["lockfile"]);
    for descriptor in profile["implementation"]["sources"].as_array().unwrap() {
        verify_descriptor(&root, descriptor);
    }
    for descriptor in profile["vectors"].as_array().unwrap() {
        verify_descriptor(&root, descriptor);
    }
    verify_descriptor(&root, &profile["license"]["upstream_notice"]);
    verify_upstream_fixture_manifest(&root);
}

fn verify_upstream_fixture_manifest(root: &Path) {
    let relative =
        Path::new("tests/conformance/comparator-vectors/pixelmatch-v7.2.0/manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(root.join(relative)).unwrap()).unwrap();
    assert_eq!(
        manifest["source"]["commit"],
        "9faed09302aaecec130b4ce0e8505d5ed5221393"
    );
    assert_eq!(
        manifest["source"]["test_blob"],
        "1217e17f38c777d98c187f7d702f26a54a9479fb"
    );

    let fixture_root = root.join(relative.parent().unwrap());
    let license = &manifest["license"];
    let license_bytes = fs::read(root.join(license["path"].as_str().unwrap())).unwrap();
    assert_eq!(license["sha256"], sha256(&license_bytes));
    assert_eq!(license["git_blob"], git_blob_sha1(&license_bytes));

    let fixtures = manifest["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 27);
    let mut paths = BTreeSet::new();
    for fixture in fixtures {
        let path = fixture["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert_safe_relative_path(path);
        let bytes = fs::read(fixture_root.join(path)).unwrap();
        assert_eq!(fixture["sha256"], sha256(&bytes));
        assert_eq!(fixture["git_blob"], git_blob_sha1(&bytes));
    }
}

fn verify_descriptor(root: &Path, descriptor: &Value) -> Vec<u8> {
    let path = descriptor["path"].as_str().unwrap();
    assert_safe_relative_path(path);
    let bytes = fs::read(root.join(path)).unwrap();
    assert_eq!(descriptor["sha256"], sha256(&bytes));
    bytes
}

fn assert_safe_relative_path(path: &str) {
    assert!(!path.is_empty());
    assert!(Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir)));
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git_blob_sha1(bytes: &[u8]) -> String {
    let mut hash = Sha1::new();
    hash.update(format!("blob {}\0", bytes.len()));
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}
