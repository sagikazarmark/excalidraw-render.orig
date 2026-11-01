//! The coverage inventory, the ownership ledger recorders and the layer report.
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

fn command(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .output()
        .unwrap()
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn inventory() -> Value {
    serde_json::from_slice(
        &fs::read(root().join("tests/conformance/coverage/inventory.json")).unwrap(),
    )
    .unwrap()
}

#[test]
#[ignore = "host tools: Node, through the representative owner's candidate gate (just test-release)"]
fn ownership_recording_rejects_unknown_obligations_without_changing_the_ledger() {
    let path = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&path).unwrap();
    let output = command(&[
        "conformance",
        "record-test-owner",
        "surface_metadata_contract",
        "editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports_svg",
        "Explicit reviewed editor-only surface contract",
        "surface:unknown:/unknown:svg",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown ownership identity"));
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
#[ignore = "host tools: Node, through the representative owner's candidate gate (just test-release)"]
fn ownership_recording_validates_the_entire_batch_before_writing() {
    let path = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&path).unwrap();
    let ledger: Value = serde_json::from_slice(&before).unwrap();
    let assignments = ledger["assignments"].as_array().unwrap();
    let gap = assignments
        .iter()
        .find(|row| {
            row["disposition"]["status"] == "gap"
                && !row["id"].as_str().unwrap().starts_with("closure:")
        })
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let owned = assignments
        .iter()
        .find(|row| {
            row["disposition"]["status"] == "owned"
                && !row["id"].as_str().unwrap().starts_with("closure:")
        })
        .unwrap()["id"]
        .as_str()
        .unwrap();
    for (function, rationale, ids, expected) in [
        (
            "scene_metadata_is_preserved_without_changing_static_exports",
            "reviewed",
            vec![gap, "unknown"],
            "unknown ownership",
        ),
        (
            "scene_metadata_is_preserved_without_changing_static_exports",
            "reviewed",
            vec![gap, gap],
            "unique ownership",
        ),
        (
            "scene_metadata_is_preserved_without_changing_static_exports",
            "reviewed",
            vec![gap, owned],
            "already assigned",
        ),
        ("no_such_test", "reviewed", vec![gap], "unconditional test"),
        (
            "scene_metadata_is_preserved_without_changing_static_exports",
            " ",
            vec![gap],
            "rationale is empty",
        ),
        (
            "scene_metadata_is_preserved_without_changing_static_exports",
            "reviewed",
            vec![],
            "nonempty",
        ),
    ] {
        let mut args = vec![
            "conformance",
            "record-test-owner",
            "surface_metadata_contract",
            function,
            rationale,
        ];
        args.extend(ids);
        let output = command(&args);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
#[ignore = "release tier: judges the live inventory (just test-release)"]
fn inventory_is_current() {
    assert_success(command(&["conformance", "verify-inventory"]));
    let inventory = inventory();
    assert_eq!(inventory["executions"].as_array().unwrap().len(), 168);
    assert!(inventory["coverage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["candidates"].as_array().unwrap().len() > 1));
}

#[test]
#[ignore = "release tier: builds the release CLI (just test-release)"]
fn release_closure_does_not_pass_for_a_draft() {
    let output = command(&["conformance", "verify-closure"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("release coverage is incomplete"));
}

#[test]
fn layer_reporting_is_read_only() {
    let path = root().join("tests/conformance/coverage/layers.json");
    let before = fs::read(&path).unwrap();
    let output = command(&["conformance", "layers-report"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "excalidraw-render/five-layers-report/v1");
    assert_eq!(report["advisory"], true);
    assert!(report["findings"].is_array());
    let alias = command(&["conformance", "verify-layers"]);
    assert!(alias.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&alias.stdout).unwrap(),
        report
    );
    let strict = command(&["conformance", "verify-layers", "--strict"]);
    // Historical records can be stale: strict audit must either verify or fail
    // on evidence, rather than rejecting the newly named option.
    assert!(!String::from_utf8_lossy(&strict.stderr).contains("unexpected argument"));
    for verifier in ["verify-layers", "verify-pdf-judgments"] {
        let obsolete = command(&["conformance", verifier, "--require-approval"]);
        assert!(!obsolete.status.success());
        assert!(String::from_utf8_lossy(&obsolete.stderr).contains("unexpected argument"));
    }
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn closure_record_and_ledger_approval_recorders_refuse_without_changing_the_ledger() {
    let path = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&path).unwrap();
    for (args, expected) in [
        (
            vec![
                "record-closure-owner",
                "codec:png:alpha",
                "pdf-judgments",
                "reviewed",
            ],
            "only for closure obligations",
        ),
        (
            vec![
                "record-closure-owner",
                "closure:pdf-judgments",
                "sh -c true",
                "reviewed",
            ],
            "unknown record verifier",
        ),
        (
            vec![
                "record-closure-owner",
                "closure:all-five-layers",
                "pdf-judgments",
                "reviewed",
            ],
            "stale or unbound record owner",
        ),
        (
            vec![
                "record-ownership-approval",
                "sagikazarmark",
                "2026-09-24",
                "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1",
                "sha256:0",
            ],
            "",
        ),
    ] {
        let mut full = vec!["conformance"];
        full.extend(args);
        let output = command(&full);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
