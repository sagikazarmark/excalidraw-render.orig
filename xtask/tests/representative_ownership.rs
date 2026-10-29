//! Negative public contracts: never assign the interaction from a pending case,
//! a dev-profile historical capture, or an unreviewed authority digest.
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
