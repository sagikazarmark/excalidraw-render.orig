//! Checked-in native expectations, separate from run-specific provenance.
//! The legacy subject encoding remains readable for historical snapshot receipts.
use super::*;
use std::path::Component;
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
) -> Result<(), String> {
    if baseline.schema != SCHEMA
        || baseline.compatibility_revision != 1
        || baseline.target != "x86_64-unknown-linux-gnu"
        || !hex(&baseline.provenance.candidate_sha256, 64)
    {
        return Err("invalid native baseline identity".into());
    }
    // The inventory also contains ordinary source-anchor bookkeeping. Preserve
    // its generating-run identity as provenance; exact current execution keys
    // below bind all scene/profile/options inputs without making an unrelated
    // test-source edit invalidate otherwise identical approved expectations.
    if baseline.inventory.path != INVENTORY
        || !hex(&baseline.inventory.sha256, 64)
        || baseline.inventory.byte_length == 0
    {
        return Err("invalid historical baseline inventory descriptor".into());
    }
    let provenance = &baseline.provenance;
    if !hex(&provenance.commit, 40) || provenance.rustc.trim().is_empty() {
        return Err("invalid baseline provenance".into());
    }
    // These descriptors identify the generating run, not today's source tree.
    // Fresh snapshot candidates still bind their current collector and inputs;
    // the baseline is checked by execution keys and exact output bytes below.
    for record in [
        &baseline.implementation,
        &provenance.tool_manifest,
        &provenance.dagger_lock,
        &provenance.consumer_lock,
        &provenance.consumer_manifest,
        &provenance.consumer_config,
        &provenance.collector,
    ] {
        if !hex(&record.sha256, 64)
            || record.byte_length == 0
            || record.path.is_empty()
            || Path::new(&record.path)
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("invalid baseline provenance descriptor".into());
        }
    }
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
    validate_baseline(output, &inventory, &baseline)?;
    write(output, &baseline)?;
    println!(
        "staged {} native expectations; review the output diff",
        baseline.expectations.len()
    );
    Ok(())
}

pub fn baseline_subject() -> Result<String, String> {
    let root = repo_root().join(BASELINE);
    let baseline = load(&root)?;
    validate_baseline(&root, &load_inventory()?, &baseline)?;
    subject(&baseline)
}

pub fn verify_baseline() -> Result<(), String> {
    let root = repo_root().join(BASELINE);
    let baseline = load(&root)?;
    validate_baseline(&root, &load_inventory()?, &baseline)?;
    println!("verified native snapshot baseline; all {} actual reports equal their expected reports (PDF from pdf-judgments.json); global closure, WASM and traces remain separate", baseline.expectations.len());
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

/// Prepare only new execution keys. Historical run provenance and every accepted
/// artifact/report descriptor are preserved; both physical blob copies are read.
pub(in crate::coverage) fn storage_rebind(
    old: &Inventory,
    current: &Inventory,
    candidate: &Path,
) -> Result<(Vec<u8>, Vec<u8>, Value), String> {
    let root = repo_root().join(BASELINE);
    let mut baseline = load(&root)?;
    if baseline
        .expectations
        .iter()
        .map(|e| &e.key)
        .collect::<Vec<_>>()
        != old.executions.iter().map(|e| &e.key).collect::<Vec<_>>()
    {
        return Err("storage migration requires the exact previous baseline keys".into());
    }
    let bundle = verified_bundle(candidate, current)?;
    let mut referenced = BTreeSet::new();
    for ((entry, execution), result) in baseline
        .expectations
        .iter_mut()
        .zip(&current.executions)
        .zip(&bundle.runs[0].results)
    {
        if entry.sample != result.samples[0] {
            return Err(format!("storage migration output drift: {:?}", entry.key));
        }
        let outcome = match &entry.sample.outcome {
            SnapshotOutcome::Artifact { bytes } => bytes,
            SnapshotOutcome::Failure { descriptor, .. } => descriptor,
        };
        for bytes in [&entry.sample.report, outcome] {
            if blob(&root, bytes, &mut referenced)? != blob(candidate, bytes, &mut BTreeSet::new())?
            {
                return Err("storage migration blob bytes differ".into());
            }
        }
        entry.key = execution.key.clone();
    }
    compare(&baseline, &bundle)?;
    let bytes = json_bytes(&baseline)?;
    let receipt = json!({
        "candidate_sha256": sha256(&read_repository_file(candidate, "snapshots.json")?),
        "rustc": bundle.rustc, "source_status": bundle.source_status,
        "packages": bundle.packages,
        "executions": baseline.expectations.len(), "sessions_per_process": 2,
        "processes": bundle.runs.iter().map(|r| &r.process_id).collect::<Vec<_>>(),
        "unchanged_samples_sha256": digest(&baseline.expectations.iter().map(|e| &e.sample).collect::<Vec<_>>())?,
        "artifact_and_report_bytes_equal": true
    });
    Ok((
        [&bytes[..], b"\n"].concat(),
        format!("{}\n", sha256(&bytes)).into_bytes(),
        receipt,
    ))
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
        "native replay matches all {} checked-in artifact/report expectations",
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

    fn approve_in_memory(baseline: &mut Baseline) {
        // Synthetic acceptance for isolated verifier tests; never written out.
        baseline.approval = json!({
            "approved_by": "snapshot-test",
            "approved_at": "2026-09-25",
            "approval_url": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1",
            "approval_subject_sha256": format!("sha256:{}", subject(baseline).unwrap()),
        });
    }

    #[test]
    fn historical_tool_sources_need_not_match_the_current_checkout() {
        let root = repo_root().join(BASELINE);
        let inventory = load_inventory().unwrap();
        for tool in ["generator", "collector"] {
            let mut historical = load(&root).unwrap();
            let source = if tool == "generator" {
                &mut historical.implementation
            } else {
                &mut historical.provenance.collector
            };
            // A historical run can have different source bytes, lengths and a
            // path that disappeared in a refactor. Its expectations still apply.
            source.path = format!("retired/{tool}.rs");
            source.sha256 = "0".repeat(64);
            source.byte_length = 1;
            approve_in_memory(&mut historical);
            validate_baseline(&root, &inventory, &historical).unwrap();
        }
    }

    #[test]
    fn historical_tool_records_do_not_gate_current_expectations() {
        let root = repo_root().join(BASELINE);
        let inventory = load_inventory().unwrap();
        for collector in [false, true] {
            let mut changed = load(&root).unwrap();
            let source = if collector {
                &mut changed.provenance.collector
            } else {
                &mut changed.implementation
            };
            source.sha256 = "0".repeat(64);
            changed.approval = Value::Null;
            validate_baseline(&root, &inventory, &changed).unwrap();
        }
    }

    #[test]
    fn historical_tool_records_require_valid_relative_paths_hashes_and_lengths() {
        let root = repo_root().join(BASELINE);
        let inventory = load_inventory().unwrap();
        type Mutation = fn(&mut FileRecord);
        let mutations: &[Mutation] = &[
            |r| r.path.clear(),
            |r| r.path = "../outside.rs".into(),
            |r| r.path = "/absolute.rs".into(),
            |r| r.sha256 = "not-a-digest".into(),
            |r| r.byte_length = 0,
        ];
        for collector in [false, true] {
            for mutate in mutations {
                let mut changed = load(&root).unwrap();
                let source = if collector {
                    &mut changed.provenance.collector
                } else {
                    &mut changed.implementation
                };
                mutate(source);
                approve_in_memory(&mut changed);
                assert!(validate_baseline(&root, &inventory, &changed)
                    .unwrap_err()
                    .contains("invalid baseline provenance descriptor"));
            }
        }
    }

    #[test]
    fn inventory_bookkeeping_can_move_but_render_inputs_cannot() {
        let root = repo_root().join(BASELINE);
        let baseline = load(&root).unwrap();
        let mut inventory = load_inventory().unwrap();
        inventory.requirements[0]
            .remaining
            .push_str(" (bookkeeping-only test)");
        validate_baseline(&root, &inventory, &baseline).unwrap();
        inventory.executions[0].key.inputs_sha256 = "0".repeat(64);
        assert!(validate_baseline(&root, &inventory, &baseline)
            .unwrap_err()
            .contains("execution keys"));
        let mut changed_record = baseline.clone();
        changed_record.inventory.sha256 = "0".repeat(64);
        validate_baseline(&root, &load_inventory().unwrap(), &changed_record).unwrap();
    }
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
