//! The external contract consumed by future native/WASM snapshot collectors.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn inventory() -> Value {
    serde_json::from_slice(
        &fs::read(root().join("tests/conformance/coverage/inventory.json")).unwrap(),
    )
    .unwrap()
}

/// Use the contract's CLI digest rather than a second JSON serializer.
fn valid_bundle() -> Value {
    let inventory = inventory();
    let identity = command(&["conformance", "inventory-digest"]);
    assert!(identity.status.success());
    let identity = String::from_utf8(identity.stdout)
        .unwrap()
        .trim()
        .to_owned();
    let bytes = json!({"sha256":"a".repeat(64),"byte_length":42});
    let sample = json!({"outcome":{"kind":"artifact","bytes":bytes},"report":bytes,"trace":bytes});
    let results: Vec<_> = inventory["executions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!({"key":e["key"],"samples":[sample,sample]}))
        .collect();
    let mut reverse = results.clone();
    reverse.reverse();
    let mut runs = Vec::new();
    for target in ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"] {
        for (order, values) in [("forward", &results), ("reverse", &reverse)] {
            runs.push(json!({"target":target,"order":order,"process_id":format!("{target}-{order}"),"results":values}));
        }
    }
    json!({"schema":"excalidraw-render/coverage-results/v1","compatibility_revision":1,
        "commit":"1".repeat(40),"inventory_sha256":identity,
        "tool_manifest_sha256":hash(&fs::read(root().join("tests/package-consumer/tool-manifest.json")).unwrap()),
        "dagger_lock_sha256":hash(&fs::read(root().join("dagger.lock")).unwrap()),"runs":runs})
}

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("coverage-{name}-{}.json", std::process::id()));
        Self(path)
    }
    fn verify(&self, value: &Value) -> Output {
        fs::write(&self.0, serde_json::to_vec(value).unwrap()).unwrap();
        command(&["conformance", "verify-results", self.0.to_str().unwrap()])
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn ownership_recording_rejects_unknown_obligations_without_changing_the_ledger() {
    let path = root().join("tests/conformance/coverage/ownership.json");
    let before = fs::read(&path).unwrap();
    let output = command(&[
        "conformance",
        "record-test-owner",
        "surface_metadata_contract",
        "editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports",
        "Explicit reviewed editor-only surface contract",
        "surface:unknown:/unknown:svg",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown ownership identity"));
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
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
fn inventory_is_current_and_release_closure_does_not_pass_for_a_draft() {
    assert_success(command(&["conformance", "verify-inventory"]));
    let output = command(&["conformance", "verify-closure"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("draft and unapproved"));
    let inventory = inventory();
    assert_eq!(inventory["executions"].as_array().unwrap().len(), 168);
    assert!(inventory["coverage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["candidates"].as_array().unwrap().len() > 1));
}

#[test]
fn compares_both_targets_both_orders_and_both_sessions_including_failures() {
    let scratch = Scratch::new("pass");
    let mut bundle = valid_bundle();
    // A typed failure is also a deterministic outcome with a report and trace.
    for run in bundle["runs"].as_array_mut().unwrap() {
        for result in run["results"].as_array_mut().unwrap() {
            for sample in result["samples"].as_array_mut().unwrap() {
                sample["outcome"] =
                    json!({"kind":"failure","code":"resource.output-bytes-exceeded"});
            }
        }
    }
    assert_success(scratch.verify(&bundle));
    assert_success(scratch.verify(&valid_bundle()));
}

#[test]
fn rejects_incomplete_stale_or_divergent_collector_results() {
    let scratch = Scratch::new("reject");
    let valid = valid_bundle();
    type Mutation = (&'static str, fn(&mut Value));
    let mutations: &[Mutation] = &[
        ("unknown field", |v| {
            v["runs"][0]["results"][0]["key"]["export"]["ordinal"] = json!(1);
        }),
        ("unknown field", |v| {
            v["runs"][0]["results"][0]["samples"][0]["outcome"]["extra"] = json!(true);
        }),
        ("missing supported target", |v| {
            v["runs"].as_array_mut().unwrap().pop();
        }),
        ("unsupported target", |v| {
            v["runs"][0]["target"] = json!("aarch64-apple-darwin")
        }),
        ("duplicate process order", |v| {
            v["runs"][1]["order"] = json!("forward")
        }),
        ("process identities", |v| {
            v["runs"][1]["process_id"] = v["runs"][0]["process_id"].clone()
        }),
        ("result keys", |v| {
            v["runs"][0]["results"].as_array_mut().unwrap().pop();
        }),
        ("result keys", |v| {
            v["runs"][0]["results"].as_array_mut().unwrap().swap(0, 1)
        }),
        ("result keys", |v| {
            v["runs"][0]["results"][0]["key"]["inputs_sha256"] = json!("0".repeat(64))
        }),
        ("inventory binding", |v| {
            v["inventory_sha256"] = json!("0".repeat(64))
        }),
        ("tooling differs", |v| {
            v["tool_manifest_sha256"] = json!("0".repeat(64))
        }),
        ("fresh-session mismatch", |v| {
            v["runs"][0]["results"][0]["samples"][1]["report"]["sha256"] = json!("b".repeat(64))
        }),
        ("cross-target/process mismatch", |v| {
            for s in v["runs"][2]["results"][0]["samples"]
                .as_array_mut()
                .unwrap()
            {
                s["trace"]["sha256"] = json!("c".repeat(64));
            }
        }),
        ("invalid artifact/report/trace", |v| {
            v["runs"][0]["results"][0]["samples"][0]["trace"]["sha256"] = json!("bad")
        }),
        ("unknown field", |v| v["claimed_pass"] = json!(true)),
    ];
    for (message, mutate) in mutations {
        let mut value = valid.clone();
        mutate(&mut value);
        let result = scratch.verify(&value);
        assert!(!result.status.success(), "accepted {message}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(message),
            "{message}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn layer_reporting_is_read_only_and_maintenance_commands_are_retired() {
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
    for retired in [
        "layers-generate",
        "layers-approval-subject",
        "record-layers-approval",
    ] {
        let output = command(&["conformance", retired]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("retired"));
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
