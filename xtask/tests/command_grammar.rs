//! Command-boundary grammar: malformed invocations fail with exit code 1,
//! write nothing to stdout, and name the offending argument before any
//! command runs. Paths below never exist, so a parser that let an invocation
//! through would fail differently.
use std::{path::Path, process::Command};

fn xtask(args: &[&str]) -> (Option<i32>, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[track_caller]
fn rejects(args: &[&str], needle: &str) {
    let (code, stdout, stderr) = xtask(args);
    assert_eq!(code, Some(1), "{args:?}: {stderr}");
    assert!(stdout.is_empty(), "{args:?} wrote stdout: {stdout}");
    assert!(
        stderr.to_lowercase().contains(&needle.to_lowercase()),
        "{args:?}: expected {needle:?} in {stderr}"
    );
}

const MISSING: &str = "/nonexistent/xtask-grammar";

#[test]
fn unknown_or_missing_commands_fail_with_usage() {
    rejects(&[], "usage");
    rejects(&["bogus"], "usage");
    rejects(&["catalog"], "usage");
    rejects(&["catalog", "bogus"], "usage");
    rejects(&["conformance"], "usage");
    rejects(&["conformance", "bogus"], "usage");
    rejects(&["representative", "bogus"], "usage");
    rejects(&["representative", "case", "bogus"], "usage");
    rejects(&["pdf-harness", "bogus"], "usage");
}

#[test]
fn missing_arguments_are_rejected() {
    rejects(&["catalog", "import"], "usage");
    rejects(&["conformance", "verify"], "case");
    rejects(
        &["conformance", "record-closure-owner", "a", "b"],
        "rationale",
    );
    rejects(
        &["conformance", "stage-native-baseline", MISSING],
        "baseline",
    );
    rejects(
        &["conformance", "record-test-owner", "--package"],
        "package",
    );
    rejects(&["representative", "prepare", MISSING, MISSING], "usage");
    rejects(&["representative", "evidence", "collect", MISSING], "usage");
    rejects(&["representative", "viewer-assets", "collect"], "usage");
    rejects(&["representative", "compact", "retain", MISSING], "usage");
    rejects(&["representative", "controls", MISSING, "--tiles"], "value");
    rejects(&["pdf-harness", "inspect", MISSING], "usage");
}

#[test]
fn extra_arguments_are_rejected() {
    rejects(&["catalog", "verify", "extra"], "unexpected argument");
    rejects(
        &["conformance", "inventory", "extra"],
        "unexpected argument",
    );
    rejects(
        &["conformance", "verify", "case", "extra"],
        "unexpected argument",
    );
    rejects(
        &["conformance", "verify-layers", "--bogus"],
        "unexpected argument",
    );
    rejects(
        &["conformance", "verify-layers", "--strict", "extra"],
        "unexpected argument",
    );
    rejects(&["representative", "verify", MISSING, "extra"], "usage");
    rejects(
        &["representative", "viewer-assets", "collect", MISSING, "x"],
        "usage",
    );
    rejects(
        &["pdf-harness", "generate-fixtures", MISSING, "extra"],
        "usage",
    );
}

#[test]
fn invalid_values_are_rejected_before_running() {
    rejects(
        &["conformance", "extend-ownership", MISSING, "x", "gap"],
        "issue",
    );
    rejects(
        &[
            "representative",
            "capture",
            MISSING,
            MISSING,
            MISSING,
            "--timeout",
            "x",
        ],
        "timeout",
    );
    rejects(
        &["representative", "controls", MISSING, "--tiles", "x"],
        "tile",
    );
    rejects(
        &[
            "representative",
            "evidence",
            "bogus",
            MISSING,
            MISSING,
            MISSING,
            MISSING,
        ],
        "evidence",
    );
    rejects(
        &["representative", "viewer-assets", "bogus", MISSING],
        "viewer-assets",
    );
    rejects(
        &["pdf-harness", "inspect", MISSING, MISSING, "maybe", MISSING],
        "pass",
    );
}

#[test]
fn repeated_conflicting_and_dependent_options_are_rejected() {
    rejects(
        &[
            "representative",
            "controls",
            MISSING,
            "--output",
            "a",
            "--output",
            "b",
        ],
        "--output",
    );
    rejects(
        &[
            "representative",
            "controls",
            MISSING,
            "--tiles",
            "8",
            "--tiles",
            "8",
        ],
        "--tiles",
    );
    rejects(
        &[
            "representative",
            "case",
            "materialize",
            MISSING,
            "--intake",
            "a",
            "--intake",
            "b",
        ],
        "materialize",
    );
    rejects(
        &["representative", "case", "materialize", MISSING, "--intake"],
        "value",
    );
    rejects(
        &[
            "representative",
            "case",
            "materialize",
            MISSING,
            "--bogus",
            "x",
        ],
        "materialize",
    );
    rejects(
        &["representative", "case", "verify", MISSING, "--viewers"],
        "--renderer",
    );
    rejects(
        &["representative", "case", "verify-registry", "--viewers"],
        "--renderer",
    );
    rejects(
        &["representative", "case", "verify-registry", "--bogus"],
        "unexpected",
    );
}

#[test]
fn help_is_derived_from_the_grammar_and_succeeds() {
    let (code, stdout, stderr) = xtask(&["conformance", "--help"]);
    assert_eq!(code, Some(0), "{stderr}");
    for command in ["candidate-verify", "record-test-owner", "verify-ownership"] {
        assert!(stdout.contains(command), "{command} missing from {stdout}");
    }
    let (code, stdout, _) = xtask(&["representative", "case", "verify", "--help"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("--renderer <RENDERER>") && stdout.contains("--viewers"));
}
