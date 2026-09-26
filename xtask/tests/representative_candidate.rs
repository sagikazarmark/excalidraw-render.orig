//! The supplemental corpus's real consumer requires fresh three-format results.
use serde_json::Value;
use std::{fs, path::Path, process::Command};

#[test]
fn representative_registry_is_consumed_through_native_execution() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("tests/conformance/candidates/registry.json")).unwrap(),
    )
    .unwrap();
    let rows = registry["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let directory = rows[0]["directory"].as_str().unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join(directory).join("manifest.json")).unwrap())
            .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(root)
        .args(["conformance", "candidate-executions"])
        .output()
        .unwrap();
    {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        let executions = report["cases"][0]["executions"].as_array().unwrap();
        assert_eq!(executions.len(), 3);
        for (row, format) in executions.iter().zip(["svg", "png", "pdf"]) {
            assert_eq!(row["native"]["format"], format);
            assert_eq!(row["native"]["fresh_process_runs"], 2);
            assert_eq!(
                row["execution_key"],
                format!("{}:default:{format}", manifest["case_id"].as_str().unwrap())
            );
        }
    }
}
