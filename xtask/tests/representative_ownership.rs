//! Negative public contracts: never assign the interaction from a pending case,
//! a dev-profile historical capture, or an unreviewed authority digest.
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(root())
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn representative_owner_cli_refuses_unknown_unapproved_and_broadened_assignments() {
    let ledger = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&ledger).unwrap();
    let registry: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join("tests/conformance/candidates/registry.json")).unwrap(),
    )
    .unwrap();
    let row = &registry["cases"][0];
    let case = row["id"].as_str().unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(
            root()
                .join(row["directory"].as_str().unwrap())
                .join("manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let unknown = format!("{case}-unregistered");
    let mut attempts = vec![
        (
            vec!["conformance", "verify-representative-owner", &unknown],
            "unknown/unregistered",
        ),
        (
            vec![
                "conformance",
                "record-representative-owner",
                case,
                "reviewed",
                "codec:png:limits",
            ],
            "unexpected",
        ),
    ];
    // Never let a negative test assign a future genuinely approved candidate.
    // The in-memory unit fixture always exercises null-approval rejection too.
    if manifest["approval"].is_null() {
        attempts.push((
            vec![
                "conformance",
                "record-representative-owner",
                case,
                "reviewed representative interaction",
            ],
            "approval must be an object",
        ));
    }
    for (args, needle) in attempts {
        let out = run(&args);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(fs::read(&ledger).unwrap(), before);
    }
}
#[test]
fn release_owner_cli_rejects_dev_capture_before_attempting_native_build() {
    let out = run(&[
        "conformance",
        "candidate-native-check",
        "tests/conformance/candidates/stage4-frames-text-assets-v6",
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("release --locked"));
}
#[test]
fn authority_migration_cli_refuses_unreviewed_spec_digest_without_writing() {
    let path = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&path).unwrap();
    let out = run(&[
        "conformance",
        "ownership-authority-migration",
        "629930b",
        &"0".repeat(64),
        "docs/research/png-stage4-v6-collection.md",
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("reviewed new spec digest"));
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn authority_migration_cli_only_previews_authenticated_metadata_and_preserves_live_gates() {
    let path = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&path).unwrap();
    let ledger: serde_json::Value = serde_json::from_slice(&before).unwrap();
    let hash = format!(
        "{:x}",
        Sha256::digest(fs::read(root().join("docs/renderer-spec-v1.md")).unwrap())
    );
    let output = run(&[
        "conformance",
        "ownership-authority-migration",
        "629930b",
        &hash,
        "docs/research/stage4-png-device-phase-assessment.md",
    ]);
    if !output.status.success() {
        // Once a reviewed rebind is applied, the old base no longer authenticates
        // the ledger. The planner must refuse that stale transition as well.
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("previous catalog is not authenticated"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(path).unwrap(), before);
        return;
    }
    let proposal: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(proposal["status"], "review-required-not-applied");
    assert!(proposal["approval"].is_null());
    assert_eq!(proposal["old_catalog_sha256"], ledger["catalog_sha256"]);
    assert_eq!(
        proposal["preserved_exclusions"].as_array().unwrap().len(),
        3
    );
    if proposal["live_inventory_validation"]["passed"] == false {
        let normal = run(&["conformance", "verify-inventory"]);
        assert!(
            !normal.status.success(),
            "planning may not bypass the live inventory gate"
        );
        assert!(String::from_utf8_lossy(&normal.stderr).contains(
            proposal["live_inventory_validation"]["error"]
                .as_str()
                .unwrap()
        ));
    }
    assert_eq!(fs::read(path).unwrap(), before);
}
