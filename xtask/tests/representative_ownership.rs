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
fn representative_owner_cli_refuses_unknown_and_broadened_assignments() {
    let ledger = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&ledger).unwrap();
    let registry: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join("tests/conformance/candidates/registry.json")).unwrap(),
    )
    .unwrap();
    let row = &registry["cases"][0];
    let case = row["id"].as_str().unwrap();
    let unknown = format!("{case}-unregistered");
    let attempts = vec![
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
    // A candidate whose native capture passes the source, request and schema
    // checks but records the dev-profile build the superseded V6 collection
    // made (the tree at 83960b50 has it). The check must stop at the build
    // identity, before it compiles the release CLI.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = format!(
        "target/dev-capture-{}-{nonce}/stage4-frames-text-assets-v6",
        std::process::id()
    );
    let scratch = root().join(&directory);
    fs::create_dir_all(&scratch).unwrap();
    let live = root().join("tests/conformance/candidates/stage4-frames-text-assets-v28");
    fs::copy(live.join("scene.json"), scratch.join("scene.json")).unwrap();
    let mut capture: serde_json::Value =
        serde_json::from_slice(&fs::read(live.join("native-capture.json")).unwrap()).unwrap();
    capture["build"] = serde_json::json!({
        "cargo": "cargo 1.97.1 (c980f4866 2026-06-30)",
        "debug": 0,
        "executable_sha256": "a75c02357470eed503db7d54573d08257e7a996166409d450c9a808bb22d0289",
        "incremental": false,
        "profile": "dev",
        "rustc": "rustc 1.97.1 (8bab26f4f 2026-07-14)\nbinary: rustc\ncommit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452\ncommit-date: 2026-07-14\nhost: x86_64-unknown-linux-gnu\nrelease: 1.97.1\nLLVM version: 22.1.6",
        "rustflags": []
    });
    fs::write(
        scratch.join("native-capture.json"),
        serde_json::to_vec(&capture).unwrap(),
    )
    .unwrap();
    let out = run(&["conformance", "candidate-native-check", &directory]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("release --locked"));
    let _ = fs::remove_dir_all(scratch.parent().unwrap());
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
    let hash = lower_hex(&Sha256::digest(
        fs::read(root().join("docs/renderer-spec-v1.md")).unwrap(),
    ));
    let output = run(&[
        "conformance",
        "ownership-authority-migration",
        "629930b",
        &hash,
        "docs/research/stage4-png-device-phase-assessment.md",
    ]);
    if !output.status.success() {
        // Once a reviewed rebind is applied, the old base no longer authenticates
        // the ledger, and once the spec moves past the reviewed amendment the
        // transition is no longer confined to it. The planner must refuse both.
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("previous catalog is not authenticated")
                || stderr.contains("authority migration must be confined"),
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

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
