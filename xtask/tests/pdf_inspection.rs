use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
#[test]
fn inspector_matches_all_retained_calibration_reports() {
    let root = root();
    let case = root.join("tests/conformance/pdf-viewer-qualification/rectangle-outline");
    let temp = std::env::temp_dir().join(format!("pdf-inspection-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    for (role, pdf, expectation, pass) in [
        (
            "valid",
            "candidate-control.pdf",
            "expected-structure.json",
            "pass",
        ),
        (
            "visual-fail",
            "visual-fail-structure-pass.pdf",
            "expected-visual-fail-structure.json",
            "pass",
        ),
        (
            "structure-fail",
            "visual-pass-structure-fail.pdf",
            "expected-structure.json",
            "fail",
        ),
        (
            "coverage",
            "structure-coverage.pdf",
            "expected-coverage-structure.json",
            "pass",
        ),
        (
            "active-action",
            "active-action.pdf",
            "expected-structure.json",
            "fail",
        ),
        (
            "parser-warning",
            "parser-warning.pdf",
            "expected-structure.json",
            "fail",
        ),
    ] {
        let output = temp.join(format!("{role}.json"));
        let result = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["pdf-harness", "inspect"])
            .arg(case.join(pdf))
            .arg(case.join(expectation))
            .arg(pass)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{role}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let actual = fs::read(&output).unwrap();
        let retained = fs::read(case.join(format!("evidence/{role}-structure.json"))).unwrap();
        let a: Value = serde_json::from_slice(&actual).unwrap();
        let b: Value = serde_json::from_slice(&retained).unwrap();
        assert_eq!(a, b, "{role}");
        assert_eq!(actual, retained, "{role} canonical report bytes");
    }
    fs::remove_dir_all(temp).unwrap();
}
