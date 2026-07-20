use std::process::Command;

#[test]
fn verifies_canonical_pdf_viewer_profiles_and_structural_gates_offline() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["conformance", "verify-pdf-viewers"])
        .output()
        .expect("run PDF viewer qualification verifier");

    assert!(
        output.status.success(),
        "verifier failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
