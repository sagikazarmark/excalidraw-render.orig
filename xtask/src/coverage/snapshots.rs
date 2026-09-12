//! Validation of native public-facade output. Traces and WASM remain separate
//! collectors; absence of those results must never masquerade as full equality.
use super::*;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    path: String,
    sha256: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SnapshotOutcome {
    Artifact { bytes: Bytes },
    Failure { code: String, descriptor: Bytes },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SnapshotSample {
    outcome: SnapshotOutcome,
    report: Bytes,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SnapshotResult {
    key: CaseKey,
    samples: [SnapshotSample; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SnapshotRun {
    order: Order,
    process_id: String,
    results: Vec<SnapshotResult>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshots {
    schema: String,
    compatibility_revision: u64,
    target: String,
    approval: Value,
    commit: String,
    source_status: String,
    rustc: String,
    inventory: FileRecord,
    tool_manifest: FileRecord,
    dagger_lock: FileRecord,
    consumer_lock: FileRecord,
    consumer_manifest: FileRecord,
    consumer_config: FileRecord,
    collector: FileRecord,
    packages: Vec<FileRecord>,
    runs: [SnapshotRun; 2],
}

fn current_file(record: &FileRecord, path: &str) -> Result<(), String> {
    if record.path != path {
        return Err(format!("unexpected snapshot source {}", record.path));
    }
    let bytes = read(path)?;
    if record.sha256 != sha256(&bytes) || record.byte_length != bytes.len() as u64 {
        return Err(format!("stale snapshot source {path}"));
    }
    Ok(())
}

fn blob(root: &Path, record: &Bytes, referenced: &mut BTreeSet<String>) -> Result<Vec<u8>, String> {
    if !hex(&record.sha256, 64) || record.byte_length == 0 {
        return Err("invalid snapshot blob descriptor".into());
    }
    let relative = format!("blobs/{}", record.sha256);
    let bytes = read_repository_file(root, &relative)?;
    if sha256(&bytes) != record.sha256 || bytes.len() as u64 != record.byte_length {
        return Err(format!("snapshot blob bytes disagree: {relative}"));
    }
    referenced.insert(record.sha256.clone());
    Ok(bytes)
}

fn validate(root: &Path, inventory: &Inventory, bundle: &Snapshots) -> Result<(), String> {
    if bundle.schema != "excalidraw-render/native-snapshots/v1"
        || bundle.compatibility_revision != 1
        || bundle.target != "x86_64-unknown-linux-gnu"
        || !bundle.approval.is_null()
        || !hex(&bundle.commit, 40)
    {
        return Err(
            "invalid native snapshot identity (only unapproved candidates accepted)".into(),
        );
    }
    let expected: Vec<_> = inventory.executions.iter().map(|e| &e.key).collect();
    if expected.is_empty() {
        return Err("empty snapshot inventory".into());
    }
    let mut canonical = BTreeMap::new();
    let mut referenced = BTreeSet::new();
    let mut processes = BTreeSet::new();
    for (run, order) in bundle.runs.iter().zip([Order::Forward, Order::Reverse]) {
        if run.order != order
            || run.process_id.trim().is_empty()
            || !processes.insert(&run.process_id)
        {
            return Err("invalid snapshot process identities/orders".into());
        }
        let keys: Vec<_> = if order == Order::Forward {
            expected.clone()
        } else {
            expected.iter().rev().copied().collect()
        };
        if run.results.iter().map(|r| &r.key).collect::<Vec<_>>() != keys {
            return Err("snapshot keys missing, extra, stale, duplicated or out of order".into());
        }
        for result in &run.results {
            for sample in &result.samples {
                let report: Value = parse(&blob(root, &sample.report, &mut referenced)?)?;
                if report["schema_version"] != 1 || !report["diagnostics"].is_array() {
                    return Err("snapshot is not a complete serialized compatibility report".into());
                }
                validate_report(&report, &result.key.format)
                    .map_err(|e| format!("{:?}: {e}", result.key))?;
                match &sample.outcome {
                    SnapshotOutcome::Artifact { bytes } => {
                        blob(root, bytes, &mut referenced)?;
                    }
                    SnapshotOutcome::Failure { code, descriptor } => {
                        let descriptor: Value = parse(&blob(root, descriptor, &mut referenced)?)?;
                        validate_failure(&descriptor, code, &result.key.format)?;
                    }
                }
            }
            if result.samples[0] != result.samples[1] {
                return Err(format!("snapshot fresh-session mismatch: {:?}", result.key));
            }
            if let Some(previous) = canonical.insert(&result.key, &result.samples[0]) {
                if previous != &result.samples[0] {
                    return Err(format!("snapshot fresh-process mismatch: {:?}", result.key));
                }
            }
        }
    }
    let actual = fs::read_dir(root.join("blobs"))
        .map_err(|e| e.to_string())?
        .map(|e| {
            e.map_err(|e| e.to_string()).and_then(|e| {
                e.file_name()
                    .into_string()
                    .map_err(|_| "non-UTF8 blob name".into())
            })
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if actual != referenced {
        return Err("snapshot has unreferenced or missing blobs".into());
    }
    Ok(())
}

fn fields(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|o| {
        o.keys().map(String::as_str).collect::<BTreeSet<_>>() == expected.iter().copied().collect()
    })
}

fn validate_failure(descriptor: &Value, code: &str, format: &Format) -> Result<(), String> {
    if code.trim().is_empty()
        || descriptor["code"] != code
        || !fields(
            descriptor,
            &[
                "family",
                "category",
                "code",
                "message",
                "format",
                "element_id",
                "json_pointer",
                "details",
            ],
        )
        || descriptor["family"] != "render"
        || !descriptor["category"].is_string()
        || !descriptor["message"].is_string()
        || descriptor["format"] != serde_json::to_value(format).unwrap()
    {
        return Err("snapshot failure descriptor disagrees with code or format".into());
    }
    Ok(())
}

fn validate_report(report: &Value, format: &Format) -> Result<(), String> {
    if !fields(report, &["schema_version", "diagnostics"]) {
        return Err("invalid report fields".into());
    }
    for d in report["diagnostics"]
        .as_array()
        .ok_or("invalid diagnostics")?
    {
        if !fields(
            d,
            &[
                "code",
                "severity",
                "format",
                "effects",
                "element_id",
                "json_pointer",
                "message",
                "details",
            ],
        ) || !d["code"].as_str().is_some_and(|s| !s.is_empty())
            || !matches!(d["severity"].as_str(), Some("warning" | "info"))
            || d["format"] != serde_json::to_value(format).unwrap()
            || !d["effects"].as_array().is_some_and(|a| {
                !a.is_empty()
                    && a.iter()
                        .all(|v| matches!(v.as_str(), Some("visual" | "semantic" | "metadata")))
            })
            || !(d["element_id"].is_null() || d["element_id"].is_string())
            || !d["json_pointer"].is_string()
            || !d["message"].is_string()
            || !(d["details"].is_null() || d["details"].is_object())
        {
            return Err("incomplete or wrong-format diagnostic".into());
        }
    }
    Ok(())
}

pub fn verify_snapshots(directory: &str) -> Result<(), String> {
    let inventory = load_inventory()?;
    let root = Path::new(directory);
    let bundle: Snapshots =
        parse(&fs::read(root.join("snapshots.json")).map_err(|e| e.to_string())?)?;
    current_file(&bundle.inventory, INVENTORY)?;
    current_file(
        &bundle.tool_manifest,
        "tests/package-consumer/tool-manifest.json",
    )?;
    current_file(&bundle.dagger_lock, "dagger.lock")?;
    current_file(&bundle.consumer_lock, "tests/package-consumer/Cargo.lock")?;
    current_file(
        &bundle.consumer_manifest,
        "tests/package-consumer/Cargo.toml",
    )?;
    current_file(
        &bundle.consumer_config,
        "tests/package-consumer/.cargo/config.toml",
    )?;
    current_file(
        &bundle.collector,
        "tests/package-consumer/examples/native_snapshots.rs",
    )?;
    let tools: Value = parse(&read("tests/package-consumer/tool-manifest.json")?)?;
    if !bundle.rustc.starts_with(&format!(
        "rustc {} ",
        string(&tools["rust_toolchain"], "version")?
    )) {
        return Err("snapshot compiler is not pinned".into());
    }
    let mut packages = BTreeSet::new();
    for package in &bundle.packages {
        if !packages.insert(package.path.as_str())
            || !hex(&package.sha256, 64)
            || package.byte_length == 0
        {
            return Err("invalid snapshot package inventory".into());
        }
        let name = Path::new(&package.path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("invalid snapshot package name")?;
        let bytes = read_repository_file(root, &format!("packages/{name}"))?;
        if sha256(&bytes) != package.sha256 || bytes.len() as u64 != package.byte_length {
            return Err(format!("snapshot package bytes disagree: {name}"));
        }
    }
    let expected: [&str; 4] = [
        "target/package/excalidraw-render-0.1.0.crate",
        "target/package/excalidraw-render-assets-base-0.1.0.crate",
        "target/package/excalidraw-render-assets-data-000-0.1.0.crate",
        "target/package/excalidraw-render-assets-data-001-0.1.0.crate",
    ];
    if packages != expected.into_iter().collect() {
        return Err("incomplete snapshot package inventory".into());
    }
    validate(root, &inventory, &bundle)?;
    for (run, name) in bundle.runs.iter().zip(["forward.json", "reverse.json"]) {
        let recorded: SnapshotRun = parse(&read_repository_file(root, name)?)?;
        if recorded != *run {
            return Err(format!("snapshot worker file disagrees: {name}"));
        }
    }
    println!("verified {} native snapshot candidates (two sessions, forward/reverse processes); source {}; approval, WASM and traces pending",
        inventory.executions.len(),if bundle.source_status.is_empty(){"clean"}else{"modified"});
    Ok(())
}

mod baseline;
mod provenance;
pub(super) mod wasm;
pub use baseline::{
    baseline_subject, record_baseline_approval, replay_snapshots, stage_baseline, verify_baseline,
};
pub use wasm::{report_wasm_snapshots, stage_wasm_snapshots, verify_wasm_snapshots};

/// Compare test-build outcomes with packaged public measurements before attaching traces.
pub(super) fn join_public_results(
    directory: &Path,
    bundle: &mut ResultBundle,
) -> Result<Vec<Value>, String> {
    let native_root = directory.join("native-snapshots");
    verify_snapshots(
        native_root
            .to_str()
            .ok_or("non-UTF8 native snapshot path")?,
    )?;
    let native: Snapshots = parse(&read_repository_file(&native_root, "snapshots.json")?)?;
    let wasm_root = directory.join("wasm-snapshots");
    let wasm_commit = String::from_utf8(read_repository_file(&wasm_root, "commit.txt")?)
        .map_err(|e| e.to_string())?;
    if native.commit != bundle.commit || wasm_commit.trim() != bundle.commit {
        return Err("trace/public collectors disagree on commit".into());
    }
    let inventory = load_inventory()?;
    let input = sha256(&wasm::inputs()?);
    let mut disagreements = Vec::new();
    for run in &mut bundle.runs {
        let index = if run.order == Order::Forward { 0 } else { 1 };
        let name = if index == 0 { "forward" } else { "reverse" };
        let public = if run.target == "x86_64-unknown-linux-gnu" {
            native.runs[index].clone()
        } else {
            let log = String::from_utf8(read_repository_file(&wasm_root, &format!("{name}.log"))?)
                .map_err(|e| e.to_string())?;
            wasm::parse_run(&log, run.order.clone(), &input, &inventory)?
        };
        if run.results.len() != public.results.len() {
            return Err("trace/public execution counts disagree".into());
        }
        for (traced, actual) in run.results.iter_mut().zip(public.results) {
            if traced.key != actual.key {
                return Err("trace/public execution keys disagree".into());
            }
            for (sample, public) in traced.samples.iter_mut().zip(actual.samples) {
                let outcome = match public.outcome {
                    SnapshotOutcome::Artifact { bytes } => Outcome::Artifact { bytes },
                    SnapshotOutcome::Failure { code, .. } => Outcome::Failure { code },
                };
                if sample.outcome != outcome || sample.report != public.report {
                    disagreements.push(
                        json!({"target":run.target,"order":run.order,"key":traced.key,
                        "test_outcome":sample.outcome,"test_report":sample.report,
                        "public_outcome":outcome,"public_report":public.report}),
                    );
                }
                sample.outcome = outcome;
                sample.report = public.report;
            }
        }
    }
    Ok(disagreements)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: std::path::PathBuf,
        inventory: Inventory,
        bundle: Value,
    }
    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("native-snapshots-{name}-{}", std::process::id()));
            fs::create_dir_all(root.join("blobs")).unwrap();
            let mut inventory = derive_inventory().unwrap();
            inventory.executions.truncate(2);
            let artifact = b"<svg></svg>";
            let report = br#"{"schema_version":1,"diagnostics":[]}"#;
            let record = |bytes: &[u8]| {
                fs::write(root.join("blobs").join(sha256(bytes)), bytes).unwrap();
                json!({"sha256":sha256(bytes),"byte_length":bytes.len()})
            };
            let sample = json!({"outcome":{"kind":"artifact","bytes":record(artifact)},"report":record(report)});
            let results: Vec<_> = inventory
                .executions
                .iter()
                .map(|e| json!({"key":e.key,"samples":[sample,sample]}))
                .collect();
            let mut reverse = results.clone();
            reverse.reverse();
            let file = json!({"path":"fixture","sha256":"a".repeat(64),"byte_length":1});
            let bundle = json!({"schema":"excalidraw-render/native-snapshots/v1","compatibility_revision":1,
                "target":"x86_64-unknown-linux-gnu","approval":null,"commit":"1".repeat(40),"source_status":"modified",
                "rustc":"rustc 1.97.1 fixture","inventory":file,"tool_manifest":file,"dagger_lock":file,
                "consumer_lock":file,"consumer_manifest":file,"consumer_config":file,"collector":file,"packages":[],"runs":[
                    {"order":"forward","process_id":"first","results":results},
                    {"order":"reverse","process_id":"second","results":reverse}]});
            Self {
                root,
                inventory,
                bundle,
            }
        }
        fn validate(&self, bundle: &Value) -> Result<(), String> {
            let bundle: Snapshots =
                serde_json::from_value(bundle.clone()).map_err(|e| e.to_string())?;
            validate(&self.root, &self.inventory, &bundle)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn validates_repeated_bytes_and_rejects_altered_process_results() {
        let fixture = Fixture::new("results");
        fixture.validate(&fixture.bundle).unwrap();
        type Mutation = (&'static str, fn(&mut Value));
        let mutations: &[Mutation] = &[
            ("snapshot keys", |v| {
                v["runs"][0]["results"].as_array_mut().unwrap().pop();
            }),
            ("snapshot keys", |v| {
                v["runs"][1]["results"].as_array_mut().unwrap().reverse()
            }),
            ("process identities", |v| {
                v["runs"][1]["process_id"] = json!("first")
            }),
            ("snapshot keys", |v| {
                v["runs"][0]["results"][0]["key"]["inputs_sha256"] = json!("0".repeat(64))
            }),
            ("unknown field", |v| {
                v["runs"][0]["results"][0]["key"]["export"]["ordinal"] = json!(1)
            }),
            ("fresh-session mismatch", |v| {
                v["runs"][0]["results"][0]["samples"][1]["outcome"]["bytes"] =
                    v["runs"][0]["results"][0]["samples"][0]["report"].clone()
            }),
            ("fresh-process mismatch", |v| {
                let report = v["runs"][1]["results"][0]["samples"][0]["report"].clone();
                for sample in v["runs"][1]["results"][0]["samples"]
                    .as_array_mut()
                    .unwrap()
                {
                    sample["outcome"]["bytes"] = report.clone();
                }
            }),
        ];
        for (message, change) in mutations {
            let mut bundle = fixture.bundle.clone();
            change(&mut bundle);
            let error = fixture.validate(&bundle).unwrap_err();
            assert!(error.contains(message), "{message}: {error}");
        }
    }

    #[test]
    fn verifies_actual_blob_bytes_and_rejects_unreferenced_files() {
        let fixture = Fixture::new("blobs");
        fs::write(fixture.root.join("blobs/extra"), b"unused").unwrap();
        assert!(fixture
            .validate(&fixture.bundle)
            .unwrap_err()
            .contains("unreferenced"));
        fs::remove_file(fixture.root.join("blobs/extra")).unwrap();
        let sha = fixture.bundle["runs"][0]["results"][0]["samples"][0]["report"]["sha256"]
            .as_str()
            .unwrap();
        fs::write(fixture.root.join("blobs").join(sha), b"changed").unwrap();
        assert!(fixture
            .validate(&fixture.bundle)
            .unwrap_err()
            .contains("blob bytes disagree"));
    }

    #[test]
    fn rejects_incomplete_and_wrong_format_reports() {
        assert!(validate_report(
            &json!({"schema_version":1,"diagnostics":[null]}),
            &Format::Svg
        )
        .is_err());
        let diagnostic = json!({"code":"compatibility.unsupported-property","severity":"warning",
            "format":"png","effects":["semantic"],"element_id":null,"json_pointer":"/unknown",
            "message":"ignored","details":null});
        let report = json!({"schema_version":1,"diagnostics":[diagnostic]});
        assert!(validate_report(&report, &Format::Svg).is_err());
        validate_report(&report, &Format::Png).unwrap();
    }
}
