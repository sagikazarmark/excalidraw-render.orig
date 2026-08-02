use crate::conformance::repo_root;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Component, Path};
use std::process::Command;

const REPORT_SCHEMA: &str = "excalidraw-render/svg-font-subset-target-evidence/v2";
const AGGREGATE_SCHEMA: &str = "excalidraw-render/svg-font-subset-target-aggregate/v2";
const MEASUREMENT_PREFIX: &str = "SVG_FONT_SUBSET_TARGET_MEASUREMENT=";
const TARGETS: [&str; 5] = [
    "aarch64-apple-darwin",
    "wasm32-unknown-unknown",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
];

pub(crate) fn materialize(target: &str, test_log: &str, output: &str) -> Result<(), String> {
    if !TARGETS.contains(&target) {
        return Err(format!("unsupported target {target:?}"));
    }
    let commit = tested_commit()?;
    let log = fs::read_to_string(test_log)
        .map_err(|error| format!("failed to read qualification log {test_log}: {error}"))?;
    let measurements = log
        .lines()
        .filter_map(|line| line.split_once(MEASUREMENT_PREFIX).map(|(_, value)| value))
        .collect::<Vec<_>>();
    let [measurement] = measurements.as_slice() else {
        return Err(format!(
            "expected exactly one qualification measurement in {test_log}, got {}",
            measurements.len()
        ));
    };
    let measurement: Value = serde_json::from_str(measurement)
        .map_err(|error| format!("qualification measurement is invalid JSON: {error}"))?;
    validate_measurement(&measurement)?;

    let root = repo_root();
    let corpus = root.join(
        "tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-qualification-corpus.json",
    );
    let profile = read_json(
        &root.join("tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json"),
    )?;
    let viewers = read_json(
        &root.join("tests/conformance/svg-font-subset-viewer-qualification/revision-1.json"),
    )?;
    let report = json!({
        "schema": REPORT_SCHEMA,
        "commit": commit,
        "compatibility_revision": 1,
        "target": target,
        "corpus_sha256": sha256_file(&corpus)?,
        "subset_profile_sha256": required_string(&profile, "semanticDigest")?
            .strip_prefix("sha256:")
            .ok_or_else(|| "subset profile semantic digest lacks sha256 prefix".to_owned())?,
        "viewer_approval_subject": required_path_string(
            &viewers,
            &["approval", "approval_subject_sha256"],
        )?,
        "measurement": measurement,
        "status": "pass",
    });
    write_json(Path::new(output), &report)
}

fn tested_commit() -> Result<String, String> {
    if let Ok(commit) = env::var("GITHUB_SHA") {
        require_digest(&commit, 40, "GITHUB_SHA")?;
        return Ok(commit);
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root())
        .output()
        .map_err(|error| format!("failed to resolve tested commit: {error}"))?;
    if !output.status.success() {
        return Err(format!("git rev-parse HEAD exited with {}", output.status));
    }
    let commit = String::from_utf8(output.stdout)
        .map_err(|error| format!("tested commit is not UTF-8: {error}"))?
        .trim()
        .to_owned();
    require_digest(&commit, 40, "tested commit")?;
    Ok(commit)
}

pub(crate) fn aggregate(input: &str, output: &str) -> Result<(), String> {
    let mut paths = fs::read_dir(input)
        .map_err(|error| format!("failed to read target report directory {input}: {error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate target reports: {error}"))?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    if paths.len() != TARGETS.len() {
        return Err(format!(
            "expected {} target reports, got {}",
            TARGETS.len(),
            paths.len()
        ));
    }
    let mut reports = paths
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<_>, _>>()?;
    for report in &reports {
        validate_report(report)?;
    }
    reports.sort_by_key(|report| {
        required_string(report, "target")
            .unwrap_or_default()
            .to_owned()
    });
    let actual_targets = reports
        .iter()
        .map(|report| required_string(report, "target"))
        .collect::<Result<Vec<_>, _>>()?;
    if actual_targets != TARGETS {
        return Err(format!(
            "target report set is incomplete: {actual_targets:?}"
        ));
    }
    for path in [
        &["commit"][..],
        &["corpus_sha256"][..],
        &["subset_profile_sha256"][..],
        &["viewer_approval_subject"][..],
        &["measurement", "generatedBytesSha256"][..],
        &["measurement", "resourceTracesSha256"][..],
        &["measurement", "successfulCells"][..],
        &["measurement", "tracedCells"][..],
    ] {
        let first = required_path(&reports[0], path)?;
        if reports.iter().skip(1).any(|report| {
            required_path(report, path).is_err() || required_path(report, path).ok() != Some(first)
        }) {
            return Err(format!("target reports disagree at {}", path.join(".")));
        }
    }
    let aggregate = json!({
        "schema": AGGREGATE_SCHEMA,
        "commit": required_string(&reports[0], "commit")?,
        "compatibility_revision": 1,
        "corpus_sha256": required_string(&reports[0], "corpus_sha256")?,
        "subset_profile_sha256": required_string(&reports[0], "subset_profile_sha256")?,
        "viewer_approval_subject": required_string(&reports[0], "viewer_approval_subject")?,
        "measurement": required_path(&reports[0], &["measurement"])?,
        "targets": reports,
        "decision": "pass",
    });
    write_json(Path::new(output), &aggregate)
}

pub(crate) fn verify_release() -> Result<(), String> {
    let root = repo_root();
    let manifest_path =
        root.join("tests/conformance/svg-font-subset-release-evidence/revision-1.json");
    let manifest = read_json(&manifest_path)?;
    if required_string(&manifest, "id")? != "svg-font-subsets.release-evidence.revision-1.v1"
        || required_path(&manifest, &["compatibilityRevision"]) != Ok(&json!(1))
        || required_path(&manifest, &["corpus", "successfulCells"]) != Ok(&json!(971))
        || required_path(&manifest, &["licensing", "qualifiedSources"]) != Ok(&json!(230))
        || required_path(&manifest, &["viewerEvidence", "executions"]) != Ok(&json!(851))
        || required_path_string(&manifest, &["viewerEvidence", "status"])? != "approved"
        || required_path_string(&manifest, &["targetEvidence", "status"])? != "pending_ci"
        || required_string(&manifest, "decision")? != "pending_target_aggregation"
    {
        return Err("SVG font subset release evidence state is stale".to_owned());
    }
    let viewer_path = required_path_string(&manifest, &["viewerEvidence", "path"])?;
    let viewer: Value = serde_json::from_slice(&repository_file(&root, viewer_path)?)
        .map_err(|error| format!("invalid viewer evidence {viewer_path}: {error}"))?;
    if required_path_string(&manifest, &["viewerEvidence", "approvalSubject"])?
        != required_path_string(&viewer, &["approval", "approval_subject_sha256"])?
    {
        return Err("release evidence and viewer approval subjects disagree".to_owned());
    }
    verify_descriptors(&root, &manifest)?;
    Ok(())
}

fn validate_report(report: &Value) -> Result<(), String> {
    if required_string(report, "schema")? != REPORT_SCHEMA
        || required_path(report, &["compatibility_revision"]) != Ok(&json!(1))
        || required_string(report, "status")? != "pass"
    {
        return Err("target report identity or status is invalid".to_owned());
    }
    let target = required_string(report, "target")?;
    if !TARGETS.contains(&target) {
        return Err(format!("unsupported target report {target:?}"));
    }
    require_digest(required_string(report, "commit")?, 40, "commit")?;
    require_digest(
        required_string(report, "corpus_sha256")?,
        64,
        "corpus digest",
    )?;
    require_digest(
        required_string(report, "subset_profile_sha256")?,
        64,
        "profile digest",
    )?;
    let viewer = required_string(report, "viewer_approval_subject")?;
    require_digest(
        viewer.strip_prefix("sha256:").unwrap_or_default(),
        64,
        "viewer approval subject",
    )?;
    validate_measurement(required_path(report, &["measurement"])?)
}

fn validate_measurement(measurement: &Value) -> Result<(), String> {
    let object = measurement
        .as_object()
        .ok_or_else(|| "qualification measurement is not an object".to_owned())?;
    let expected = [
        "generatedBytesSha256",
        "resourceTracesSha256",
        "successfulCells",
        "tracedCells",
    ];
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("qualification measurement fields are incomplete".to_owned());
    }
    require_digest(
        required_string(measurement, "generatedBytesSha256")?,
        64,
        "generated-byte measurement",
    )?;
    require_digest(
        required_string(measurement, "resourceTracesSha256")?,
        64,
        "resource-trace measurement",
    )?;
    if required_path(measurement, &["successfulCells"]) != Ok(&json!(971))
        || required_path(measurement, &["tracedCells"]) != Ok(&json!(1198))
    {
        return Err("qualification measurement cell counts are stale".to_owned());
    }
    Ok(())
}

fn verify_descriptors(root: &Path, value: &Value) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if let (Some(Value::String(path)), Some(Value::String(expected))) =
                (object.get("path"), object.get("sha256"))
            {
                let actual = format!("{:x}", Sha256::digest(repository_file(root, path)?));
                if actual != *expected {
                    return Err(format!("stale SVG font subset release descriptor: {path}"));
                }
            }
            for child in object.values() {
                verify_descriptors(root, child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                verify_descriptors(root, child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    required_path_string(value, &[key])
}

fn required_path_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    required_path(value, path)?
        .as_str()
        .ok_or_else(|| format!("{} is not a string", path.join(".")))
}

fn required_path<'a>(mut value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    for key in path {
        value = value
            .get(*key)
            .ok_or_else(|| format!("missing {}", path.join(".")))?;
    }
    Ok(value)
}

fn require_digest(value: &str, length: usize, name: &str) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{name} is not lowercase hexadecimal"));
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn repository_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    let root = fs::canonicalize(root)
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let resolved = fs::canonicalize(root.join(path))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("path escapes repository: {relative}"));
    }
    fs::read(resolved).map_err(|error| format!("failed to read {relative}: {error}"))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    bytes.push(b'\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}
