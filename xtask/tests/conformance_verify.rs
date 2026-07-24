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
fn computes_the_unapproved_frame_reference_subject_offline() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["conformance", "approval-subject", "focused.frame-export.v1"])
        .output()
        .expect("compute frame conformance approval subject");

    assert!(
        output.status.success(),
        "approval subject failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "sha256:5f19dfd6b50ef8f138eef0ab57dd923ee7b37b0f1fa8f1af9f8fb67b1a0db285\n"
    );
}
