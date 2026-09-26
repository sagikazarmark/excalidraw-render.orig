//! Black-box coverage of the SVG font subset target-evidence commands: the
//! per-target report generator, the five-target aggregator, and the release
//! evidence verifier. These are the commands the packaged-public-facade
//! workflow runs; they are exercised here against the committed evidence so a
//! regression is caught before CI has to assemble five targets to find it.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MEASUREMENT_PREFIX: &str = "SVG_FONT_SUBSET_TARGET_MEASUREMENT=";
const TESTED_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace-root child")
        .to_owned()
}

fn temp_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "excalidraw-render-xtask-test-{}-{name}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path).expect("remove stale test temp directory");
    }
    fs::create_dir_all(&path).expect("create test temp directory");
    path
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON fixture")).expect("parse JSON fixture")
}

fn xtask(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .env("GITHUB_SHA", TESTED_COMMIT)
        .output()
        .expect("run xtask")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "xtask failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_rejected(output: &Output, rule: &str) {
    assert!(
        !output.status.success(),
        "xtask unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(rule),
        "expected a rejection mentioning {rule:?}, got {stderr:?}"
    );
}

/// The measurement every supported target has agreed on so far, read from the
/// committed cross-target evidence that every aggregation re-asserts.
fn committed_measurement() -> Value {
    read_json(
        &repo_root()
            .join("tests/conformance/svg-font-subset-cross-target-measurement/revision-1.json"),
    )["measurement"]
        .clone()
}

#[test]
fn materializes_a_target_report_from_one_qualification_measurement_line() {
    let directory = temp_directory("materialize-target");
    let log = directory.join("qualification.log");
    let report = directory.join("report.json");
    let measurement = committed_measurement();
    fs::write(
        &log,
        format!(
            "running 3 tests\ntest render::font_subset::... ok\n{MEASUREMENT_PREFIX}{measurement}\ntest result: ok.\n"
        ),
    )
    .expect("write qualification log");

    assert_success(&xtask(&[
        "conformance",
        "materialize-svg-font-subset-target",
        "x86_64-unknown-linux-gnu",
        log.to_str().unwrap(),
        report.to_str().unwrap(),
    ]));

    let report = read_json(&report);
    let root = repo_root();
    let release =
        read_json(&root.join("tests/conformance/svg-font-subset-release-evidence/revision-1.json"));
    assert_eq!(
        report["schema"],
        json!("excalidraw-render/svg-font-subset-target-evidence/v4")
    );
    assert_eq!(report["target"], json!("x86_64-unknown-linux-gnu"));
    assert_eq!(report["commit"], json!(TESTED_COMMIT));
    assert_eq!(report["compatibility_revision"], json!(1));
    assert_eq!(report["status"], json!("pass"));
    assert_eq!(report["measurement"], measurement);
    assert_eq!(report["corpus_sha256"], release["corpus"]["sha256"]);
    assert!(report.get("viewer_approval_subject").is_none());

    let schema =
        read_json(&root.join("tests/conformance/svg-font-subset-target-evidence-v4.schema.json"));
    let validator = jsonschema::validator_for(&schema).expect("target report schema compiles");
    let violations = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        violations.is_empty(),
        "target report violates its schema: {violations:?}"
    );

    fs::remove_dir_all(&directory).expect("remove test temp directory");
}

/// The supported targets (spec §10, ADR-0006), in canonical order.
const TARGETS: [&str; 2] = ["wasm32-unknown-unknown", "x86_64-unknown-linux-gnu"];

/// Materializes one report per supported target from the same qualification
/// log, the way the workflow's target jobs do, into `directory/reports`.
fn materialize_all_targets(directory: &Path, measurement: &Value) -> PathBuf {
    let log = directory.join("qualification.log");
    fs::write(&log, format!("{MEASUREMENT_PREFIX}{measurement}\n"))
        .expect("write qualification log");
    let reports = directory.join("reports");
    fs::create_dir_all(&reports).expect("create report directory");
    for target in TARGETS {
        assert_success(&xtask(&[
            "conformance",
            "materialize-svg-font-subset-target",
            target,
            log.to_str().unwrap(),
            reports.join(format!("{target}.json")).to_str().unwrap(),
        ]));
    }
    reports
}

#[test]
fn aggregates_every_supported_target_report_into_a_passing_aggregate() {
    let directory = temp_directory("aggregate-pass");
    let measurement = committed_measurement();
    let reports = materialize_all_targets(&directory, &measurement);
    let aggregate = directory.join("aggregate.json");

    assert_success(&xtask(&[
        "conformance",
        "aggregate-svg-font-subset-targets",
        reports.to_str().unwrap(),
        aggregate.to_str().unwrap(),
    ]));

    let aggregate = read_json(&aggregate);
    assert_eq!(
        aggregate["schema"],
        json!("excalidraw-render/svg-font-subset-target-aggregate/v4")
    );
    assert_eq!(aggregate["decision"], json!("pass"));
    assert_eq!(aggregate["commit"], json!(TESTED_COMMIT));
    assert_eq!(aggregate["compatibility_revision"], json!(1));
    assert_eq!(aggregate["measurement"], measurement);
    let targets = aggregate["targets"]
        .as_array()
        .expect("aggregate lists its target reports")
        .iter()
        .map(|report| report["target"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(targets, TARGETS);

    fs::remove_dir_all(&directory).expect("remove test temp directory");
}

#[test]
fn rejects_an_incomplete_target_report_set() {
    let directory = temp_directory("aggregate-incomplete");
    let reports = materialize_all_targets(&directory, &committed_measurement());
    fs::remove_file(reports.join("wasm32-unknown-unknown.json")).expect("drop the WASM report");

    assert_rejected(
        &xtask(&[
            "conformance",
            "aggregate-svg-font-subset-targets",
            reports.to_str().unwrap(),
            directory.join("aggregate.json").to_str().unwrap(),
        ]),
        "expected 2 target reports, got 1",
    );

    fs::remove_dir_all(&directory).expect("remove test temp directory");
}

#[test]
fn rejects_target_reports_that_disagree_on_the_tested_commit() {
    let directory = temp_directory("aggregate-disagree");
    let reports = materialize_all_targets(&directory, &committed_measurement());
    let wasm = reports.join("wasm32-unknown-unknown.json");
    let mut report = read_json(&wasm);
    report["commit"] = json!("fedcba9876543210fedcba9876543210fedcba98");
    fs::write(&wasm, serde_json::to_vec_pretty(&report).unwrap()).expect("rewrite report");

    assert_rejected(
        &xtask(&[
            "conformance",
            "aggregate-svg-font-subset-targets",
            reports.to_str().unwrap(),
            directory.join("aggregate.json").to_str().unwrap(),
        ]),
        "target reports disagree at commit",
    );

    fs::remove_dir_all(&directory).expect("remove test temp directory");
}

/// ADR-0006: a target outside the supported set has no report to give.
#[test]
fn rejects_a_report_for_a_target_outside_the_supported_set() {
    let directory = temp_directory("materialize-unsupported");
    let log = directory.join("qualification.log");
    fs::write(
        &log,
        format!("{MEASUREMENT_PREFIX}{}\n", committed_measurement()),
    )
    .expect("write qualification log");

    assert_rejected(
        &xtask(&[
            "conformance",
            "materialize-svg-font-subset-target",
            "x86_64-apple-darwin",
            log.to_str().unwrap(),
            directory.join("report.json").to_str().unwrap(),
        ]),
        "unsupported target",
    );

    fs::remove_dir_all(&directory).expect("remove test temp directory");
}

#[test]
fn verifies_the_committed_svg_font_subset_release_evidence_offline() {
    let output = xtask(&["conformance", "verify-svg-font-subset-release"]);
    assert_success(&output);
    let release = read_json(
        &repo_root().join("tests/conformance/svg-font-subset-release-evidence/revision-1.json"),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let status = release["targetEvidence"]["status"]
        .as_str()
        .expect("release evidence names its target-evidence status");
    assert!(
        stdout.contains(status),
        "verifier must report the target-evidence state it verified ({status}); got {stdout:?}"
    );
}

#[test]
fn rejects_a_qualification_log_without_a_measurement() {
    let directory = temp_directory("materialize-no-measurement");
    let log = directory.join("qualification.log");
    fs::write(&log, "running 1 test\ntest result: ok.\n").expect("write qualification log");

    assert_rejected(
        &xtask(&[
            "conformance",
            "materialize-svg-font-subset-target",
            "x86_64-unknown-linux-gnu",
            log.to_str().unwrap(),
            directory.join("report.json").to_str().unwrap(),
        ]),
        "exactly one qualification measurement",
    );

    fs::remove_dir_all(&directory).expect("remove test temp directory");
}
