use std::process::Command;

#[test]
fn verifies_the_approved_rectangle_reference_case_offline() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["conformance", "verify", "focused.rectangle-outline.v1"])
        .output()
        .expect("run conformance verifier");

    assert!(
        output.status.success(),
        "verifier failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn approval_commands_are_retired() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["conformance", "approval-subject", "focused.frame-export.v1"])
        .output()
        .expect("compute frame conformance approval subject");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("approval commands are retired"));
    assert!(output.stdout.is_empty());
}
