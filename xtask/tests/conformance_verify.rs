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
