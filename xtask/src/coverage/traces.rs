//! Test-build trace instrumentation is staged only in Dagger's source snapshot.
use super::*;
use std::path::Path;

fn trace_inputs() -> Result<Vec<u8>, String> {
    let mut input: Value = parse(&snapshots::wasm::inputs()?)?;
    input["trace_template_sha256"] =
        json!(sha256(&read("tests/package-consumer/trace-snapshots.inc")?));
    input["trace_runner_sha256"] = json!(sha256(&read("tools/ci/trace-snapshots.sh")?));
    input["trace_stager_sha256"] = json!(sha256(&read("xtask/src/coverage/traces.rs")?));
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("cannot resolve trace input commit".into());
    }
    input["commit"] = json!(String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())?
        .trim());
    serde_json::to_vec(&input).map_err(|e| e.to_string())
}

pub fn stage_traces() -> Result<(), String> {
    verify_inventory()?;
    let input = trace_inputs()?;
    let native = String::from_utf8(read("tests/package-consumer/examples/native_snapshots.rs")?)
        .map_err(|e| e.to_string())?;
    let generated = format!(
        "#[allow(dead_code)]\nmod inventory_traces {{\n{}\n{}\n}}\n",
        native
            .replacen("//!", "//", 1)
            .replacen("use excalidraw_render::{", "use crate::{", 1),
        String::from_utf8(read("tests/package-consumer/trace-snapshots.inc")?)
            .map_err(|e| e.to_string())?
    );
    let root = repo_root().join("crates/excalidraw-render/src/render");
    let path = root.join("test_support.rs");
    let mut support = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if support.contains("include!(\"inventory_traces.rs\")") {
        return Err("trace instrumentation already staged".into());
    }
    support.push_str("\ninclude!(\"inventory_traces.rs\");\n");
    fs::write(path, support).map_err(|e| e.to_string())?;
    fs::write(root.join("inventory_traces.rs"), generated).map_err(|e| e.to_string())?;
    fs::write(root.join("snapshot-inputs.json"), input).map_err(|e| e.to_string())?;
    Ok(())
}

fn run(
    log: &str,
    target: &str,
    order: Order,
    input: &str,
    inventory: &Inventory,
) -> Result<ProcessRun, String> {
    let mut results = Vec::new();
    let mut completed = false;
    let name = if order == Order::Forward {
        "forward"
    } else {
        "reverse"
    };
    for line in log.lines() {
        if let Some((_, v)) = line.split_once("TRACE_SNAPSHOT_RESULT=") {
            if completed {
                return Err("trace result after completion".into());
            }
            results.push(parse::<CaseResult>(v.as_bytes())?);
        }
        if let Some((_, v)) = line.split_once("TRACE_SNAPSHOT_COMPLETE=") {
            let value: Value = parse(v.as_bytes())?;
            if completed || value != json!({"order":name,"target":target,"input_sha256":input}) {
                return Err("invalid trace completion binding".into());
            }
            completed = true;
        }
    }
    if !completed {
        return Err("missing trace completion".into());
    }
    let mut keys: Vec<_> = inventory.executions.iter().map(|e| &e.key).collect();
    if order == Order::Reverse {
        keys.reverse();
    }
    if results.iter().map(|e| &e.key).collect::<Vec<_>>() != keys {
        return Err("incomplete or reordered trace keys".into());
    }
    for result in &results {
        for sample in &result.samples {
            let mut records = vec![&sample.trace, &sample.report];
            match &sample.outcome {
                Outcome::Artifact { bytes } => records.push(bytes),
                Outcome::Failure { code } if code.trim().is_empty() => {
                    return Err("empty trace outcome error".into())
                }
                Outcome::Failure { .. } => {}
            }
            if records
                .iter()
                .any(|r| !hex(&r.sha256, 64) || r.byte_length == 0)
            {
                return Err("invalid trace sample descriptor".into());
            }
        }
    }
    Ok(ProcessRun {
        target: target.into(),
        order,
        process_id: format!("{target}-{name}"),
        results,
    })
}

pub fn aggregate_traces(directory: &str) -> Result<(), String> {
    aggregate(directory, false)
}

pub fn join_trace_results(directory: &str) -> Result<(), String> {
    aggregate(directory, true)
}

pub fn verify_joined_results(directory: &str) -> Result<(), String> {
    aggregate(directory, true)?;
    let report: Value =
        parse(&fs::read(Path::new(directory).join("comparison.json")).map_err(|e| e.to_string())?)?;
    if report["combined_decision"] != "pass" {
        return Err("combined trace/public equality failed; see comparison.json".into());
    }
    Ok(())
}

fn aggregate(directory: &str, join: bool) -> Result<(), String> {
    let inventory = load_inventory()?;
    let root = Path::new(directory);
    let mut runs = Vec::new();
    let input = sha256(&trace_inputs()?);
    for (dir, target) in [
        ("native", "x86_64-unknown-linux-gnu"),
        ("wasm", "wasm32-unknown-unknown"),
    ] {
        for (name, order) in [("forward", Order::Forward), ("reverse", Order::Reverse)] {
            let log = String::from_utf8(read_repository_file(root, &format!("{dir}/{name}.log"))?)
                .map_err(|e| e.to_string())?;
            runs.push(run(&log, target, order, &input, &inventory)?);
        }
    }
    let mut differences = Vec::new();
    let mut first: BTreeMap<&CaseKey, &Sample> = BTreeMap::new();
    for run in &runs {
        for result in &run.results {
            for sample in &result.samples {
                if let Some(previous) = first.get(&result.key) {
                    if previous.trace != sample.trace {
                        differences.push(json!({"key":result.key,"target":run.target,"expected":previous.trace,"actual":sample.trace}));
                    }
                } else {
                    first.insert(&result.key, sample);
                }
            }
        }
    }
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("cannot resolve trace commit".into());
    }
    let mut bundle = ResultBundle {
        schema: RESULT_SCHEMA.into(),
        compatibility_revision: 1,
        commit: String::from_utf8(output.stdout)
            .map_err(|e| e.to_string())?
            .trim()
            .into(),
        inventory_sha256: digest(&inventory)?,
        tool_manifest_sha256: None,
        dagger_lock_sha256: None,
        runs,
    };
    let public_disagreements = if join {
        snapshots::join_public_results(root, &mut bundle)?
    } else {
        Vec::new()
    };
    let equality = validate_results(&inventory, &bundle);
    let mut public_differences = Vec::new();
    let mut public_first: BTreeMap<&CaseKey, &Sample> = BTreeMap::new();
    for run in &bundle.runs {
        for result in &run.results {
            for sample in &result.samples {
                if let Some(previous) = public_first.get(&result.key) {
                    if previous.outcome != sample.outcome || previous.report != sample.report {
                        public_differences.push(
                            json!({"target":run.target,"order":run.order,"key":result.key,
                    "expected_outcome":previous.outcome,"expected_report":previous.report,
                    "actual_outcome":sample.outcome,"actual_report":sample.report}),
                        );
                    }
                } else {
                    public_first.insert(&result.key, sample);
                }
            }
        }
    }
    // A measured collection is not an independently validated coverage-results
    // bundle: consumers must recheck its raw logs and public join with the
    // dedicated command, not bypass join failures via verify-results.
    let mut exported = serde_json::to_value(&bundle).map_err(|e| e.to_string())?;
    exported["schema"] = json!("excalidraw-render/trace-collection/v1");
    exported["scope"] = json!(if join { "joined" } else { "test-build" });
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&exported).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(root.join("comparison.json"),serde_json::to_vec_pretty(&json!({"trace_differences":differences,
        "trace_decision":if differences.is_empty(){"pass"}else{"fail"},"combined_decision":if equality.is_ok() && public_disagreements.is_empty(){"pass"}else{"fail"},
        "combined_error":equality.err(),"public_trace_disagreements":public_disagreements,"artifact_report_differences":public_differences,
        "scope":if join {"packaged public-facade outcomes joined to test-only traces"}else{"test-build outcomes and traces; packaged facade join pending"}})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    println!(
        "materialized trace comparison; {} trace differences (combined gate via verify-joined-results)",
        differences.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_logs_require_exact_keys_target_order_and_input() {
        let mut inventory = derive_inventory().unwrap();
        inventory.executions.truncate(1);
        let bytes = json!({"sha256":"a".repeat(64),"byte_length":1});
        let sample =
            json!({"outcome":{"kind":"artifact","bytes":bytes},"report":bytes,"trace":bytes});
        let result = json!({"key":inventory.executions[0].key,"samples":[sample,sample]});
        let input = "b".repeat(64);
        let target = "x86_64-unknown-linux-gnu";
        let record = format!("TRACE_SNAPSHOT_RESULT={result}\n");
        let end = format!(
            "TRACE_SNAPSHOT_COMPLETE={}\n",
            json!({"order":"forward","target":target,"input_sha256":input})
        );
        let valid = format!("{record}{end}");
        run(&valid, target, Order::Forward, &input, &inventory).unwrap();
        for log in [
            &record,
            &end,
            &format!("{record}{record}{end}"),
            &format!("{valid}{end}"),
            &format!("{end}{record}"),
        ] {
            assert!(run(log, target, Order::Forward, &input, &inventory).is_err());
        }
        assert!(run(
            &valid,
            "wasm32-unknown-unknown",
            Order::Forward,
            &input,
            &inventory
        )
        .is_err());
        assert!(run(&valid, target, Order::Reverse, &input, &inventory).is_err());
        assert!(run(&valid, target, Order::Forward, &"c".repeat(64), &inventory).is_err());
        assert!(run(
            &valid.replace(&"a".repeat(64), "invalid"),
            target,
            Order::Forward,
            &input,
            &inventory
        )
        .is_err());
    }
}
