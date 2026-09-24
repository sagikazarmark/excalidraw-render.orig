//! Approved canonical native expectations, separate from run-specific provenance.
//! The subject contains only strings, integers and null: no draft option-number
//! serialization is promoted to an RFC8785 approval subject.
use super::*;
use crate::conformance::verify_approval_for_issue;
use std::process::Command;

const BASELINE: &str = "tests/conformance/coverage/native-baseline";
const SCHEMA: &str = "excalidraw-render/native-snapshot-baseline/v1";
const SUBJECT: &str = "excalidraw-render/native-snapshot-approval-subject/v1";
const IMPLEMENTATION: &str = "xtask/src/coverage/snapshots/baseline/producer.rs";
mod producer;
use producer::{build_baseline, subject};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Expectation {
    key: CaseKey,
    sample: SnapshotSample,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    commit: String,
    rustc: String,
    candidate_sha256: String,
    tool_manifest: FileRecord,
    dagger_lock: FileRecord,
    consumer_lock: FileRecord,
    consumer_manifest: FileRecord,
    consumer_config: FileRecord,
    collector: FileRecord,
    packages: Vec<FileRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    schema: String,
    compatibility_revision: u64,
    target: String,
    inventory: FileRecord,
    implementation: FileRecord,
    provenance: Provenance,
    expectations: Vec<Expectation>,
    approval: Value,
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    canonical_bytes(&serde_json::to_value(value).map_err(|e| e.to_string())?)
}

fn write(root: &Path, baseline: &Baseline) -> Result<(), String> {
    let bytes = json_bytes(baseline)?;
    fs::write(root.join("revision-1.json"), [&bytes[..], b"\n"].concat())
        .map_err(|e| e.to_string())?;
    fs::write(
        root.join("revision-1.sha256"),
        format!("{}\n", sha256(&bytes)),
    )
    .map_err(|e| e.to_string())
}

fn load(root: &Path) -> Result<Baseline, String> {
    let bytes = read_repository_file(root, "revision-1.json")?;
    let baseline: Baseline = parse(&bytes)?;
    let canonical = json_bytes(&baseline)?;
    if bytes != [&canonical[..], b"\n"].concat()
        || read_repository_file(root, "revision-1.sha256")?
            != format!("{}\n", sha256(&canonical)).as_bytes()
    {
        return Err("native baseline canonical bytes or sidecar is stale".into());
    }
    Ok(baseline)
}

fn ensure_ancestor(commit: &str) -> Result<(), String> {
    if !hex(commit, 40) {
        return Err("invalid baseline commit".into());
    }
    let result = Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, "HEAD"])
        .current_dir(repo_root())
        .status()
        .map_err(|e| e.to_string())?;
    if !result.success() {
        return Err("baseline commit is not an ancestor of HEAD".into());
    }
    Ok(())
}

fn validate_baseline(
    root: &Path,
    inventory: &Inventory,
    baseline: &Baseline,
    approval: bool,
) -> Result<(), String> {
    if baseline.schema != SCHEMA
        || baseline.compatibility_revision != 1
        || baseline.target != "x86_64-unknown-linux-gnu"
        || !hex(&baseline.provenance.candidate_sha256, 64)
    {
        return Err("invalid native baseline identity".into());
    }
    current_file(&baseline.inventory, INVENTORY)?;
    current_file(&baseline.implementation, IMPLEMENTATION)?;
    let provenance = &baseline.provenance;
    if !hex(&provenance.commit, 40) || provenance.rustc.trim().is_empty() {
        return Err("invalid baseline provenance".into());
    }
    for record in [
        &provenance.tool_manifest,
        &provenance.dagger_lock,
        &provenance.consumer_lock,
        &provenance.consumer_manifest,
        &provenance.consumer_config,
        &provenance.collector,
    ] {
        if !hex(&record.sha256, 64) || record.byte_length == 0 || record.path.is_empty() {
            return Err("invalid baseline provenance descriptor".into());
        }
    }
    // Collector/request mapping changes require reapproval; a renderer bug fix
    // is detected by replay, not by a current-package-provenance mismatch.
    current_file(
        &baseline.provenance.collector,
        "tests/package-consumer/examples/native_snapshots.rs",
    )?;
    let expected: Vec<_> = inventory.executions.iter().map(|e| &e.key).collect();
    if expected.is_empty()
        || baseline
            .expectations
            .iter()
            .map(|e| &e.key)
            .collect::<Vec<_>>()
            != expected
    {
        return Err("baseline execution keys are missing, extra, duplicated or stale".into());
    }
    let mut referenced = BTreeSet::new();
    for entry in &baseline.expectations {
        let report: Value = parse(&blob(root, &entry.sample.report, &mut referenced)?)?;
        if report["schema_version"] != 1 {
            return Err("invalid baseline report schema".into());
        }
        validate_report(&report, &entry.key.format)?;
        match &entry.sample.outcome {
            SnapshotOutcome::Artifact { bytes } => {
                blob(root, bytes, &mut referenced)?;
            }
            SnapshotOutcome::Failure { code, descriptor } => {
                let error: Value = parse(&blob(root, descriptor, &mut referenced)?)?;
                validate_failure(&error, code, &entry.key.format)?;
            }
        }
    }
    let actual = fs::read_dir(root.join("blobs"))
        .map_err(|e| e.to_string())?
        .map(|e| {
            e.map_err(|e| e.to_string()).and_then(|e| {
                e.file_name()
                    .into_string()
                    .map_err(|_| "invalid blob name".into())
            })
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if actual != referenced {
        return Err("baseline contains extra or missing blobs".into());
    }
    expected_reports(
        root,
        baseline
            .expectations
            .iter()
            .map(|e| (&e.key, &e.sample.report)),
    )?;
    if approval || !baseline.approval.is_null() {
        if baseline.approval.is_null() {
            return Err("native snapshot baseline awaits explicit #85 approval".into());
        }
        verify_approval_for_issue(&baseline.approval, &subject(baseline)?, "85")?;
    }
    Ok(())
}

pub fn stage_baseline(candidate: &str, output: &str) -> Result<(), String> {
    verify_snapshots(candidate)?;
    let inventory = load_inventory()?;
    let candidate = Path::new(candidate);
    let raw = read_repository_file(candidate, "snapshots.json")?;
    let bundle: Snapshots = parse(&raw)?;
    if !bundle.source_status.is_empty() {
        return Err("baseline staging requires clean-commit snapshot candidates".into());
    }
    ensure_ancestor(&bundle.commit)?;
    super::provenance::verify(candidate, &bundle)?;
    let baseline = build_baseline(bundle, &raw)?;
    let output = Path::new(output);
    fs::create_dir(output).map_err(|e| e.to_string())?;
    fs::create_dir(output.join("blobs")).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(candidate.join("blobs")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "invalid blob name")?;
        let bytes = read_repository_file(candidate, &format!("blobs/{name}"))?;
        fs::write(output.join("blobs").join(name), bytes).map_err(|e| e.to_string())?;
    }
    validate_baseline(output, &inventory, &baseline, false)?;
    write(output, &baseline)?;
    println!(
        "staged {} native expectations; approval subject sha256:{}",
        baseline.expectations.len(),
        subject(&baseline)?
    );
    Ok(())
}

pub fn baseline_subject() -> Result<String, String> {
    let root = repo_root().join(BASELINE);
    let baseline = load(&root)?;
    validate_baseline(&root, &load_inventory()?, &baseline, false)?;
    subject(&baseline)
}

pub fn record_baseline_approval(
    login: &str,
    date: &str,
    url: &str,
    expected: &str,
) -> Result<(), String> {
    let root = repo_root().join(BASELINE);
    let mut baseline = load(&root)?;
    validate_baseline(&root, &load_inventory()?, &baseline, false)?;
    ensure_ancestor(&baseline.provenance.commit)?;
    if !baseline.approval.is_null() {
        return Err("native baseline is already approved".into());
    }
    let subject = subject(&baseline)?;
    if expected.strip_prefix("sha256:").unwrap_or(expected) != subject {
        return Err("native baseline approval subject mismatch".into());
    }
    baseline.approval = json!({"approved_by":login,"approved_at":date,"approval_url":url,"approval_subject_sha256":format!("sha256:{subject}")});
    verify_approval_for_issue(&baseline.approval, &subject, "85")?;
    write(&root, &baseline)
}

pub fn verify_baseline() -> Result<(), String> {
    let root = repo_root().join(BASELINE);
    let baseline = load(&root)?;
    validate_baseline(&root, &load_inventory()?, &baseline, true)?;
    println!("verified approved native snapshot baseline; all {} actual reports equal their expected reports (PDF from pdf-judgments.json); global closure, WASM and traces remain separate", baseline.expectations.len());
    Ok(())
}

fn compare(baseline: &Baseline, candidate: &Snapshots) -> Result<(), String> {
    let actual: Vec<_> = candidate.runs[0]
        .results
        .iter()
        .map(|r| Expectation {
            key: r.key.clone(),
            sample: r.samples[0].clone(),
        })
        .collect();
    if actual.len() != baseline.expectations.len() {
        return Err("native replay execution count changed".into());
    }
    for (expected, actual) in baseline.expectations.iter().zip(actual) {
        if *expected != actual {
            return Err(format!(
                "native replay drift for {:?}: expected {:?}, actual {:?}",
                expected.key, expected.sample, actual.sample
            ));
        }
    }
    Ok(())
}

pub fn replay_snapshots(candidate: &str) -> Result<(), String> {
    verify_baseline()?;
    verify_snapshots(candidate)?;
    let bundle: Snapshots = parse(&read_repository_file(
        Path::new(candidate),
        "snapshots.json",
    )?)?;
    compare(&load(&repo_root().join(BASELINE))?, &bundle)?;
    println!(
        "native replay matches all {} approved artifact/report expectations",
        bundle.runs[0].results.len()
    );
    Ok(())
}

pub(super) fn compare_results(
    results: &[SnapshotResult],
    reverse: bool,
) -> Result<Vec<Value>, String> {
    let baseline = load(&repo_root().join(BASELINE))?;
    let mut actual: Vec<_> = results.iter().collect();
    if reverse {
        actual.reverse();
    }
    if actual.len() != baseline.expectations.len() {
        return Err("WASM replay count differs from native baseline".into());
    }
    let mut differences = Vec::new();
    for (expected, actual) in baseline.expectations.iter().zip(actual) {
        if expected.key != actual.key || actual.samples.iter().any(|s| s != &expected.sample) {
            differences.push(
                json!({"key":expected.key,"expected":expected.sample,"actual":actual.samples}),
            );
        }
    }
    Ok(differences)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn baseline() -> Baseline {
        let file = FileRecord {
            path: "fixture".into(),
            sha256: "a".repeat(64),
            byte_length: 1,
        };
        Baseline {
            schema: SCHEMA.into(),
            compatibility_revision: 1,
            target: "x86_64-unknown-linux-gnu".into(),
            inventory: file.clone(),
            implementation: file.clone(),
            provenance: Provenance {
                commit: "1".repeat(40),
                rustc: "rustc 1.97.1".into(),
                candidate_sha256: "b".repeat(64),
                tool_manifest: file.clone(),
                dagger_lock: file.clone(),
                consumer_lock: file.clone(),
                consumer_manifest: file.clone(),
                consumer_config: file.clone(),
                collector: file,
                packages: vec![],
            },
            expectations: vec![],
            approval: Value::Null,
        }
    }
    #[test]
    fn approval_binds_provenance_and_expectations_but_not_itself() {
        let mut baseline = baseline();
        let expected = subject(&baseline).unwrap();
        baseline.approval = json!({"approved_by":"sagikazarmark","approved_at":"2026-09-11",
            "approval_url":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1",
            "approval_subject_sha256":format!("sha256:{expected}")});
        assert_eq!(subject(&baseline).unwrap(), expected);
        verify_approval_for_issue(&baseline.approval, &expected, "85").unwrap();
        baseline.provenance.commit = "2".repeat(40);
        assert_ne!(subject(&baseline).unwrap(), expected);
        assert!(
            verify_approval_for_issue(&baseline.approval, &subject(&baseline).unwrap(), "85")
                .is_err()
        );
        assert!(verify_approval_for_issue(&Value::Null, &expected, "85").is_err());
    }

    #[test]
    fn replay_ignores_run_provenance_but_rejects_artifact_report_and_failure_drift() {
        let mut baseline = baseline();
        let key = CaseKey {
            case: "fixture".into(),
            variant: "default".into(),
            export: ExportTarget::Scene {},
            format: Format::Svg,
            inputs_sha256: "c".repeat(64),
        };
        let bytes = Bytes {
            sha256: "d".repeat(64),
            byte_length: 100,
        };
        let sample = SnapshotSample {
            outcome: SnapshotOutcome::Artifact {
                bytes: bytes.clone(),
            },
            report: bytes.clone(),
        };
        baseline.expectations.push(Expectation {
            key: key.clone(),
            sample: sample.clone(),
        });
        let f = json!({"path":"fixture","sha256":"e".repeat(64),"byte_length":1});
        let value = json!({"schema":"excalidraw-render/native-snapshots/v1","compatibility_revision":1,"target":"x86_64-unknown-linux-gnu",
            "approval":null,"commit":"9".repeat(40),"source_status":"WIP","rustc":"fixture",
            "inventory":f,"tool_manifest":f,"dagger_lock":f,"consumer_lock":f,"consumer_manifest":f,"consumer_config":f,"collector":f,
            "packages":[],"runs":[{"order":"forward","process_id":"new-a","results":[{"key":key,"samples":[sample,sample]}]},
            {"order":"reverse","process_id":"new-b","results":[]}]});
        let candidate: Snapshots = serde_json::from_value(value).unwrap();
        compare(&baseline, &candidate).unwrap();
        for sample in [
            SnapshotSample {
                report: Bytes {
                    sha256: "f".repeat(64),
                    ..bytes.clone()
                },
                ..sample.clone()
            },
            SnapshotSample {
                outcome: SnapshotOutcome::Artifact {
                    bytes: Bytes {
                        byte_length: 101,
                        ..bytes.clone()
                    },
                },
                ..sample.clone()
            },
            SnapshotSample {
                outcome: SnapshotOutcome::Failure {
                    code: "render.failed".into(),
                    descriptor: bytes,
                },
                ..sample
            },
        ] {
            let mut changed = candidate.clone();
            changed.runs[0].results[0].samples[0] = sample;
            assert!(compare(&baseline, &changed)
                .unwrap_err()
                .contains("native replay drift"));
        }
        let mut missing = candidate;
        missing.runs[0].results.clear();
        assert!(compare(&baseline, &missing).is_err());
    }
}
