//! Exercise the process boundary used by renderer tests while Cargo holds the
//! parent target lock. The Rust inspector's own tests cover its parsing logic.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = root().join(format!(
            "target/pdf-wrapper-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn prebuilt_wrappers_reproduce_all_six_fixtures_and_structural_reports() {
    let scratch = Scratch::new();
    let case = root().join("tests/conformance/pdf-viewer-qualification/rectangle-outline");
    let output = Command::new("bash")
        .arg(root().join("tools/pdf-harness/run-xtask.sh"))
        .arg("generate-fixtures")
        .arg(&scratch.0)
        .env("XTASK_PDF_HARNESS", env!("CARGO_BIN_EXE_xtask"))
        .current_dir(&scratch.0)
        .output()
        .unwrap();
    success(&output);
    for (role, pdf, expectation, expected) in [
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
        assert_eq!(
            fs::read(scratch.0.join(pdf)).unwrap(),
            fs::read(case.join(pdf)).unwrap()
        );
        let report = scratch.0.join(format!("{role}.json"));
        let output = Command::new("bash")
            .arg(root().join("tools/pdf-harness/run-structure.sh"))
            .arg("inspect")
            .arg(scratch.0.join(pdf))
            .arg(case.join(expectation))
            .arg(expected)
            .arg(&report)
            .env("XTASK_PDF_HARNESS", env!("CARGO_BIN_EXE_xtask"))
            // The wrapper must select the pinned parser, not a caller override.
            .env("QPDF", "/not/the/pinned/qpdf")
            .current_dir(&scratch.0)
            .output()
            .unwrap();
        success(&output);
        assert_eq!(
            fs::read(report).unwrap(),
            fs::read(case.join(format!("evidence/{role}-structure.json"))).unwrap(),
            "{role}"
        );
    }
}

#[test]
fn fallback_uses_locked_absolute_manifest_and_isolated_target_even_with_parent_override() {
    let scratch = Scratch::new();
    // Inspect the actual Cargo invocation without rebuilding the workspace in a
    // second target tree on every test run. Then execute the already-built xtask.
    let cargo = scratch.0.join("cargo");
    fs::write(&cargo, b"#!/usr/bin/env bash\nset -euo pipefail\n[[ $# -eq 12 ]]\n[[ $1 == run && $2 == --quiet && $3 == --locked ]]\n[[ $4 == --manifest-path && $5 == \"$ROOT/xtask/Cargo.toml\" ]]\n[[ $6 == --target-dir && $7 == \"$ROOT/target/pdf-harness/xtask\" ]]\n[[ $8 == --bin && $9 == xtask && ${10} == -- ]]\n[[ ${11} == pdf-harness && ${12} == invalid-command ]]\nexec \"$PREBUILT\" \"${@:11}\"\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new("bash")
        .arg(root().join("tools/pdf-harness/run-xtask.sh"))
        .arg("invalid-command")
        .env_remove("XTASK_PDF_HARNESS")
        .env("CARGO_TARGET_DIR", root().join("target"))
        .env("ROOT", root())
        .env("PREBUILT", env!("CARGO_BIN_EXE_xtask"))
        .env(
            "PATH",
            format!("{}:{}", scratch.0.display(), std::env::var("PATH").unwrap()),
        )
        .current_dir(&scratch.0)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("unrecognized subcommand 'invalid-command'"));
}

#[test]
fn structural_wrapper_rejects_output_symlinks_before_launching_xtask() {
    let scratch = Scratch::new();
    let target = scratch.0.join("keep.json");
    let link = scratch.0.join("report.json");
    fs::write(&target, b"keep").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let case = root().join("tests/conformance/pdf-viewer-qualification/rectangle-outline");
    let output = Command::new("bash")
        .arg(root().join("tools/pdf-harness/run-structure.sh"))
        .arg("inspect")
        .arg(case.join("candidate-control.pdf"))
        .arg(case.join("expected-structure.json"))
        .arg("pass")
        .arg(link)
        .env("XTASK_PDF_HARNESS", "/must/not/be/launched")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output path must not be a symlink"));
    assert_eq!(fs::read(target).unwrap(), b"keep");
}
