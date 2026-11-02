use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest, Sha256};

const PROFILE_PATH: &str = "tests/conformance/comparator-profiles/canonical-visual-v1.json";
const PROFILE_DIGEST_PATH: &str =
    "tests/conformance/comparator-profiles/canonical-visual-v1.sha256";
const CRATE_DIR: &str = "crates/excalidraw-render-compare/";
const WORKSPACE_MANIFEST_PATH: &str = "Cargo.toml";

/// The profile is sealed by its sidecar and binds, live, what defines a
/// measurement: its schema, the adversarial vectors and the imported upstream
/// fixtures, which the comparator's own tests execute. Its
/// `implementation.sources` digests are historical (ADR-0005, amended
/// 2026-11-01): they record the sources the profile was cut from and are
/// checked for shape only, so a comment edit under `src/` or a dev-dependency
/// bump moves nothing here. A source change that moves a measurement fails the
/// vectors instead.
#[test]
fn canonical_comparator_profile_is_sealed_and_its_measurement_bindings_are_current() {
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

    let implementation = &profile["implementation"];
    for descriptor in implementation["sources"].as_array().unwrap() {
        verify_historical_descriptor(descriptor);
    }
    verify_workspace_inheritance(&root, implementation);
    for descriptor in profile["vectors"].as_array().unwrap() {
        verify_descriptor(&root, descriptor);
    }
    verify_descriptor(&root, &profile["license"]["upstream_notice"]);
    verify_upstream_fixture_manifest(&root);
}

/// A historical source record: a safe path under the comparator's crate and a
/// SHA-256 digest, not compared with today's file.
fn verify_historical_descriptor(descriptor: &Value) {
    let path = descriptor["path"].as_str().unwrap();
    assert_safe_relative_path(path);
    assert!(
        path.starts_with(CRATE_DIR),
        "{path} is not a comparator source"
    );
    let digest = descriptor["sha256"].as_str().unwrap();
    assert!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "{path} records no SHA-256 digest"
    );
}

/// The comparator inherits its edition and its `unsafe_code` lint level from the
/// workspace manifest, which the profile no longer pins by digest; the profile states
/// those two facts and this checks them against both manifests.
fn verify_workspace_inheritance(root: &Path, implementation: &Value) {
    let workspace = fs::read_to_string(root.join(WORKSPACE_MANIFEST_PATH)).unwrap();
    let package = fs::read_to_string(root.join(CRATE_DIR).join("Cargo.toml")).unwrap();

    assert_eq!(toml_value(&package, "package", "edition.workspace"), "true");
    assert_eq!(
        implementation["edition"],
        toml_value(&workspace, "workspace.package", "edition")
    );

    assert_eq!(toml_value(&package, "lints", "workspace"), "true");
    assert_eq!(
        implementation["lints"]["rust"]["unsafe_code"],
        toml_value(&workspace, "workspace.lints.rust", "unsafe_code")
    );
}

/// The raw right-hand side of `key` inside `[table]` of a Cargo manifest, unquoted.
/// Enough for the flat, machine-regular tables this test reads; not a TOML parser.
fn toml_value(manifest: &str, table: &str, key: &str) -> String {
    let header = format!("[{table}]");
    let mut in_table = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == header;
            continue;
        }
        if !in_table {
            continue;
        }
        if let Some((candidate, value)) = line.split_once('=') {
            if candidate.trim() == key {
                let value = value.trim();
                return strip_quotes(value).unwrap_or(value).to_owned();
            }
        }
    }
    panic!("{table}.{key} is not set in the manifest");
}

fn strip_quotes(value: &str) -> Option<&str> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
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
    lower_hex(&Sha256::digest(bytes))
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn git_blob_sha1(bytes: &[u8]) -> String {
    let mut hash = Sha1::new();
    hash.update(format!("blob {}\0", bytes.len()));
    hash.update(bytes);
    lower_hex(&hash.finalize())
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}
