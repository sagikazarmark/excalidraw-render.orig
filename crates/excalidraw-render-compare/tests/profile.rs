use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const PROFILE_PATH: &str = "tests/conformance/comparator-profiles/canonical-visual-v1.json";
const PROFILE_DIGEST_PATH: &str =
    "tests/conformance/comparator-profiles/canonical-visual-v1.sha256";
const CLOSURE_PATH: &str = "tests/conformance/comparator-profiles/canonical-visual-v1.closure.json";
const CLOSURE_SCHEMA: &str = "excalidraw-render/conformance-comparator-closure/v1";
const CLOSURE_HEADER_KEYS: [&str; 3] = ["schema", "lockfile", "root"];
const PACKAGE_KEYS: [&str; 4] = ["name", "version", "source", "checksum"];
const CRATE_NAME: &str = "excalidraw-render-compare";
const CRATE_DIR: &str = "crates/excalidraw-render-compare/";
const LOCKFILE_PATH: &str = "Cargo.lock";
const WORKSPACE_MANIFEST_PATH: &str = "Cargo.toml";

// The manifest and lockfile readers below are hand-rolled on purpose: a `toml` or
// `cargo-lock` dev-dependency would enter the very closure this test pins, so every
// upstream release of the reader would itself move the profile.

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

    let implementation = &profile["implementation"];
    for descriptor in implementation["sources"].as_array().unwrap() {
        verify_descriptor(&root, descriptor);
    }
    verify_workspace_inheritance(&root, implementation);
    verify_closure(&root, implementation);
    for descriptor in profile["vectors"].as_array().unwrap() {
        verify_descriptor(&root, descriptor);
    }
    verify_descriptor(&root, &profile["license"]["upstream_notice"]);
    verify_upstream_fixture_manifest(&root);
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

/// The committed closure is exactly the comparator's reachable subgraph of the live
/// workspace lock, so a renderer dependency change moves the profile only when it also
/// moves a package the comparator builds against.
fn verify_closure(root: &Path, implementation: &Value) {
    let committed: Value =
        serde_json::from_slice(&verify_descriptor(root, &implementation["closure"])).unwrap();
    let current = current_closure(root);
    for key in CLOSURE_HEADER_KEYS {
        assert_eq!(committed[key], current[key], "closure {key}");
    }

    let committed = package_records(&committed);
    let current = package_records(&current);
    let stale: Vec<_> = committed.difference(&current).collect();
    let missing: Vec<_> = current.difference(&committed).collect();
    assert!(
        stale.is_empty() && missing.is_empty(),
        "committed comparator closure differs from {LOCKFILE_PATH}\n\
         no longer locked: {stale:#?}\n\
         not committed: {missing:#?}\n\
         refresh with: cargo test --locked -p {CRATE_NAME} --test profile -- --ignored materialize \
         --nocapture, then copy the staged closure it names over {CLOSURE_PATH}"
    );
}

/// Each package of a closure as one canonical JSON record, so set differences name
/// whole packages.
fn package_records(closure: &Value) -> BTreeSet<String> {
    closure["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::to_string)
        .collect()
}

fn current_closure(root: &Path) -> Value {
    let lockfile = fs::read_to_string(root.join(LOCKFILE_PATH)).unwrap();
    json!({
        "schema": CLOSURE_SCHEMA,
        "lockfile": LOCKFILE_PATH,
        "root": CRATE_NAME,
        "packages": lock_closure(&lockfile, CRATE_NAME),
    })
}

/// Stages the current closure under Cargo's scratch directory and names the copy
/// that refreshes the committed file, so a run of every `--ignored` test leaves
/// the source tree clean.
#[test]
#[ignore = "explicitly refreshes the comparator's committed dependency closure"]
fn materialize_canonical_comparator_closure() {
    let root = repository_root();
    let committed = root.join(CLOSURE_PATH);
    let staged = Path::new(env!("CARGO_TARGET_TMPDIR")).join(
        committed
            .file_name()
            .expect("the committed closure has a file name"),
    );
    fs::write(&staged, format_closure(&current_closure(&root))).unwrap();
    println!(
        "staged the current comparator closure; refresh the committed one with:\n  cp {} {}",
        staged.display(),
        committed.display()
    );
}

/// One package per line, so a closure diff names exactly the packages that moved.
fn format_closure(closure: &Value) -> String {
    let mut out = String::from("{\n");
    for key in CLOSURE_HEADER_KEYS {
        out.push_str(&format!("  {:?}: {},\n", key, closure[key]));
    }
    out.push_str("  \"packages\": [\n");
    let packages = closure["packages"].as_array().unwrap();
    for (index, package) in packages.iter().enumerate() {
        out.push_str("    { ");
        let mut fields = Vec::new();
        for key in PACKAGE_KEYS {
            if let Some(value) = package.get(key) {
                fields.push(format!("{key:?}: {value}"));
            }
        }
        out.push_str(&fields.join(", "));
        out.push_str(" }");
        if index + 1 < packages.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}\n");
    out
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

#[test]
fn lock_closure_follows_versioned_dependency_specs_and_drops_unreachable_packages() {
    let lock = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "alpha"
version = "0.1.0"
dependencies = [
 "beta 2.0.0",
 "epsilon 1.0.0 (git+https://example.invalid/epsilon#0123abcd)",
 "gamma",
]

[[package]]
name = "beta"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "1111111111111111111111111111111111111111111111111111111111111111"
dependencies = [
 "delta",
]

[[package]]
name = "beta"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "2222222222222222222222222222222222222222222222222222222222222222"

[[package]]
name = "delta"
version = "0.9.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "4444444444444444444444444444444444444444444444444444444444444444"

[[package]]
name = "epsilon"
version = "1.0.0"
source = "git+https://example.invalid/epsilon#0123abcd"

[[package]]
name = "epsilon"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "6666666666666666666666666666666666666666666666666666666666666666"

[[package]]
name = "gamma"
version = "0.3.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3333333333333333333333333333333333333333333333333333333333333333"
dependencies = [
 "beta 2.0.0",
]

[[package]]
name = "omega"
version = "5.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "5555555555555555555555555555555555555555555555555555555555555555"
dependencies = [
 "alpha",
 "beta 1.0.0",
]

[[patch.unused]]
name = "gamma"
version = "0.4.0"
source = "git+https://example.invalid/gamma#89abcdef"
"#;

    assert_eq!(
        lock_closure(lock, "alpha"),
        json!([
            { "name": "alpha", "version": "0.1.0" },
            {
                "name": "beta",
                "version": "2.0.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "checksum": "2222222222222222222222222222222222222222222222222222222222222222"
            },
            {
                "name": "epsilon",
                "version": "1.0.0",
                "source": "git+https://example.invalid/epsilon#0123abcd"
            },
            {
                "name": "gamma",
                "version": "0.3.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "checksum": "3333333333333333333333333333333333333333333333333333333333333333"
            },
        ])
    );
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

/// One `[[package]]` entry of a Cargo lockfile.
#[derive(Debug, Default)]
struct LockedPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
    dependencies: Vec<String>,
}

impl LockedPackage {
    /// Whether this entry is the one a lockfile dependency spec names. Cargo writes
    /// `name`, `name version` when several versions are locked, and
    /// `name version (source)` when the same version comes from several sources.
    fn matches(&self, spec: &str) -> bool {
        let mut parts = spec.split(' ');
        parts.next() == Some(self.name.as_str())
            && parts.next().is_none_or(|version| version == self.version)
            && parts.next().is_none_or(|source| {
                self.source
                    .as_deref()
                    .is_some_and(|own| format!("({own})") == source)
            })
    }

    fn describe(&self) -> Value {
        let mut package = json!({ "name": self.name, "version": self.version });
        if let Some(source) = &self.source {
            package["source"] = json!(source);
        }
        if let Some(checksum) = &self.checksum {
            package["checksum"] = json!(checksum);
        }
        package
    }
}

/// Every `[[package]]` entry of a Cargo lockfile. Any other table (`[metadata]` in old
/// lockfiles, `[[patch.unused]]`) is skipped rather than folded into the last package.
fn locked_packages(lockfile: &str) -> Vec<LockedPackage> {
    let mut packages: Vec<LockedPackage> = Vec::new();
    let mut in_package = false;
    let mut lines = lockfile.lines();
    while let Some(line) = lines.next() {
        if line.starts_with('[') {
            in_package = line == "[[package]]";
            if in_package {
                packages.push(LockedPackage::default());
            }
            continue;
        }
        if !in_package {
            continue;
        }
        let package = packages.last_mut().unwrap();
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        match key {
            "name" => package.name = unquote(value),
            "version" => package.version = unquote(value),
            "source" => package.source = Some(unquote(value)),
            "checksum" => package.checksum = Some(unquote(value)),
            "dependencies" => {
                assert_eq!(value, "[");
                package.dependencies = lines
                    .by_ref()
                    .take_while(|line| *line != "]")
                    .map(|line| unquote(line.trim().trim_end_matches(',')))
                    .collect();
            }
            _ => {}
        }
    }
    packages
}

fn unquote(value: &str) -> String {
    strip_quotes(value)
        .unwrap_or_else(|| panic!("lockfile value {value:?} is not a quoted string"))
        .to_owned()
}

/// The packages reachable from `root` in `lockfile`, sorted by name and version.
fn lock_closure(lockfile: &str, root: &str) -> Value {
    let packages = locked_packages(lockfile);
    let resolve = |spec: &str| -> usize {
        let mut matches = packages
            .iter()
            .enumerate()
            .filter(|(_, package)| package.matches(spec))
            .map(|(index, _)| index);
        let index = matches
            .next()
            .unwrap_or_else(|| panic!("lockfile has no package matching {spec:?}"));
        assert!(
            matches.next().is_none(),
            "lockfile dependency spec {spec:?} is ambiguous"
        );
        index
    };

    let mut reached = BTreeSet::new();
    let mut pending = vec![resolve(root)];
    while let Some(index) = pending.pop() {
        if reached.insert(index) {
            pending.extend(
                packages[index]
                    .dependencies
                    .iter()
                    .map(|spec| resolve(spec)),
            );
        }
    }

    let mut closure: Vec<&LockedPackage> =
        reached.into_iter().map(|index| &packages[index]).collect();
    closure.sort_by(|left, right| (&left.name, &left.version).cmp(&(&right.name, &right.version)));
    Value::Array(closure.into_iter().map(LockedPackage::describe).collect())
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
