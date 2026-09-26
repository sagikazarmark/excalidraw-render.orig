//! Registered representative contracts through the Rust command boundary.
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
const CASE: &str = "tests/conformance/cases/representative/excali-local-slides";

struct Copy(PathBuf);
impl Copy {
    fn new() -> Self {
        fn copy(from: &Path, to: &Path) {
            fs::create_dir(to).unwrap();
            for entry in fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &to.join(entry.file_name()));
                } else {
                    fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
                }
            }
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "representative-case-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        copy(&root().join(CASE), &path);
        Self(path)
    }
    fn record(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("case.json")).unwrap()).unwrap()
    }
    fn set(&self, value: &Value) {
        fs::write(self.0.join("case.json"), serde_json::to_vec(value).unwrap()).unwrap();
    }
    fn run(&self, options: &[&str], success: bool) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["representative", "case", "verify", self.0.to_str().unwrap()])
            .args(options)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(if success {
            output.stdout
        } else {
            output.stderr
        })
        .unwrap()
    }
}
impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn offline_registry_verifies_every_registered_case_without_an_approval_requirement() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["representative", "case", "verify-registry"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect::<Vec<_>>();
    let registry: Value = serde_json::from_slice(
        &fs::read(root().join("tests/conformance/cases/representative/registry.json")).unwrap(),
    )
    .unwrap();
    let expected = registry["cases"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len());
    for (row, entry) in rows.iter().zip(expected) {
        assert_eq!(row["case_id"], entry["case_id"]);
        assert!(row["approved"].is_boolean() && row.get("native").is_none());
    }
}

#[test]
fn retained_bytes_are_checked_without_acceptance_hashes() {
    let case = Copy::new();
    case.run(&[], true);
    let mut record = case.record();
    record["approval"] = Value::Null;
    case.set(&record);
    case.run(&[], true);
    fs::write(case.0.join("expected-report-svg.json"), b"{}").unwrap();
    assert!(case.run(&[], false).contains("retained case files changed"));
    let case = Copy::new();
    let mut record = case.record();
    record["approval"]["approval_subject_sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    case.set(&record);
    case.run(&[], true);
}

#[test]
#[cfg(unix)]
fn retained_file_inventory_rejects_backslash_aliases() {
    let case = Copy::new();
    fs::copy(
        case.0.join("reference/light/official.svg"),
        case.0.join("reference\\light\\official.svg"),
    )
    .unwrap();
    assert!(case.run(&[], false).contains("backslash"));
}

#[test]
fn invalid_resource_declarations_and_viewers_without_renderer_are_rejected() {
    let case = Copy::new();
    let mut record = case.record();
    record["approval"] = Value::Null;
    record["resource_outcomes"] = json!([{"id":"light-png", "variant":"light", "format":"png", "overrides":{"scale":1}, "expected_report":{"status":"success","artifact":null}}]);
    case.set(&record);
    assert!(case.run(&[], false).contains("expects a failed render"));
    record["resource_outcomes"][0]["expected_report"]["status"] = json!("failure");
    record["resource_outcomes"][0]["id"] = json!("../outside");
    case.set(&record);
    assert!(case.run(&[], false).contains("resource outcome identity"));
    assert!(case
        .run(&["--viewers"], false)
        .contains("requires --renderer"));
}

#[test]
fn invalid_case_inventory_options_and_duplicate_members_fail_offline() {
    for (field, value) in [
        ("formats", json!(["svg", "svg"])),
        ("variants", json!(["missing"])),
        ("comparisons", json!([])),
    ] {
        let case = Copy::new();
        let mut record = case.record();
        record["approval"] = Value::Null;
        record[field] = value;
        case.set(&record);
        case.run(&[], false);
    }
    let case = Copy::new();
    let raw = fs::read_to_string(case.0.join("case.json")).unwrap();
    fs::write(
        case.0.join("case.json"),
        raw.replacen('{', "{\"schema\":\"duplicate\",", 1),
    )
    .unwrap();
    assert!(case.run(&[], false).contains("duplicate JSON member"));
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn registered_cases_replay_with_rust_v2_capture() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args([
            "representative",
            "case",
            "verify-registry",
            "--renderer",
            &renderer,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = String::from_utf8(output.stdout).unwrap();
    let registry: Value = serde_json::from_slice(
        &fs::read(root().join("tests/conformance/cases/representative/registry.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rows.lines().count(),
        registry["cases"].as_array().unwrap().len()
    );
    assert!(rows
        .lines()
        .all(|row| serde_json::from_str::<Value>(row).unwrap()["native"].is_string()));
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn false_semantic_and_resource_expectations_fail_fresh_replay() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let case = Copy::new();
    let mut record = case.record();
    record["approval"] = Value::Null;
    record["semantic_expectations"]["light"]["svg_size"] = json!(["1", "1"]);
    case.set(&record);
    assert!(case
        .run(&["--renderer", &renderer], false)
        .contains("SVG size differs"));
    let case = Copy::new();
    let mut record = case.record();
    record["approval"] = Value::Null;
    record["resource_outcomes"] = json!([{"id":"light-png", "variant":"light", "format":"png", "overrides":{"scale":1}, "expected_report":{"status":"failure","artifact":null,"error":{"code":"resource.raster-surface-exceeded"}}}]);
    case.set(&record);
    assert!(case
        .run(&["--renderer", &renderer], false)
        .contains("light-png: render succeeded"));
}

fn reject_semantic_expectation(
    format: &str,
    variant: &str,
    field: &str,
    value: Value,
    error: &str,
) {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let case = Copy::new();
    let mut record = case.record();
    record["approval"] = Value::Null;
    // Isolate the format under test, particularly PDF notices: an SVG notice
    // mismatch must not hide a missing PDF check of the shared expectation.
    record["formats"] = json!([format]);
    record["comparisons"] = match format {
        "svg" => json!(["svg-chromium", "svg-rust"]),
        "png" => json!(["png-canvas"]),
        "pdf" => json!(["pdf-pdfium", "pdf-pdfjs"]),
        _ => panic!("unsupported test format"),
    };
    record["semantic_expectations"][variant][field] = value;
    case.set(&record);
    let output = case.run(&["--renderer", &renderer], false);
    assert!(
        output.contains(&format!("{variant}-{format}: {error}")),
        "{output}"
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn svg_mandatory_notice_mismatch_fails_fresh_replay() {
    reject_semantic_expectation(
        "svg",
        "light",
        "notices",
        json!([]),
        "mandatory notices differ",
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn pdf_mandatory_notice_mismatch_fails_fresh_replay() {
    reject_semantic_expectation(
        "pdf",
        "light",
        "notices",
        json!([]),
        "mandatory notices differ",
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn png_dimension_mismatch_fails_fresh_replay() {
    reject_semantic_expectation(
        "png",
        "light",
        "png_size",
        json!([1, 1]),
        "PNG size differs",
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn pdf_dimension_mismatch_fails_fresh_replay() {
    reject_semantic_expectation(
        "pdf",
        "light",
        "pdf_media_box",
        json!(["1", "1"]),
        "PDF MediaBox differs",
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn insufficient_selectable_text_fails_fresh_replay() {
    reject_semantic_expectation(
        "svg",
        "light",
        "min_text_elements",
        json!(1_000_000),
        "selectable text differs",
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn link_count_mismatch_fails_fresh_replay() {
    reject_semantic_expectation("svg", "light", "links", json!(1), "links differ");
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn dark_root_filter_mismatch_fails_fresh_replay() {
    reject_semantic_expectation(
        "svg",
        "dark",
        "root_theme_filter",
        json!(false),
        "root theme filter differs",
    );
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn embedded_scene_mismatch_fails_fresh_replay() {
    reject_semantic_expectation(
        "svg",
        "light",
        "embedded_scene",
        json!(false),
        "embedded scene differs",
    );
}

fn resource_failure_case() -> Copy {
    let case = Copy::new();
    let mut record = case.record();
    record["approval"] = Value::Null;
    record["resource_outcomes"] = json!([{
        "id": "oversized-png",
        "variant": "light",
        "format": "png",
        "overrides": {"scale": 1000},
        "expected_report": {
            "status": "failure",
            "artifact": null,
            "error": {"code": "resource.raster-surface-exceeded"}
        }
    }]);
    case.set(&record);
    case
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn declared_resource_failure_passes_fresh_replay() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let case = resource_failure_case();
    let summary: Value = serde_json::from_str(&case.run(&["--renderer", &renderer], true)).unwrap();
    assert_eq!(summary["resource_outcomes"], 1);
    assert!(summary["native"].is_string());
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn incorrect_resource_error_fails_fresh_replay() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let case = resource_failure_case();
    let mut record = case.record();
    record["resource_outcomes"][0]["expected_report"]["error"]["code"] =
        json!("resource.not-the-raster-limit");
    case.set(&record);
    let output = case.run(&["--renderer", &renderer], false);
    assert!(
        output.contains("oversized-png: error.code differs from expectation"),
        "{output}"
    );
}

#[cfg(unix)]
fn reject_resource_output_fault(fault: &str, error: &str, failures: usize) {
    use std::os::unix::fs::PermissionsExt;
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let case = resource_failure_case();
    // Keep the wrapper and its observations outside the verified case inventory.
    let wrapper_files = Copy::new();
    let wrapper = wrapper_files.0.join("renderer");
    let calls = wrapper_files.0.join("failures");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let script = format!(
        "#!/bin/sh\nprevious=''\nfor arg do\n if [ \"$previous\" = '--output' ]; then artifact=$arg; fi\n if [ \"$previous\" = '--report-output' ]; then report=$arg; fi\n previous=$arg\ndone\n{} \"$@\"\nstatus=$?\nif [ \"$status\" -ne 0 ]; then\n calls={}\n {fault}\n printf '%s\\n' \"$status\" >> \"$calls\"\nfi\nexit \"$status\"\n",
        quote(&renderer), quote(calls.to_str().unwrap()),
    );
    fs::write(&wrapper, script).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let output = case.run(&["--renderer", wrapper.to_str().unwrap()], false);
    assert!(
        output.contains(&format!("oversized-png: {error}")),
        "{output}"
    );
    assert_eq!(fs::read_to_string(calls).unwrap().lines().count(), failures);
}

#[cfg(unix)]
#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn resource_failure_with_artifact_fails_fresh_replay() {
    reject_resource_output_fault(
        "printf 'unexpected artifact' > \"$artifact\"",
        "a failed render wrote an artifact",
        1,
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to freshly built CLI"]
fn resource_failure_report_byte_drift_fails_fresh_replay() {
    // Whitespace preserves the report's meaning but violates exact repeat bytes.
    reject_resource_output_fault(
        "if [ -f \"$calls\" ]; then printf ' ' >> \"$report\"; fi",
        "report differs between fresh processes",
        2,
    );
}
