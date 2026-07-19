use std::process::Command;

#[test]
fn verifies_canonical_svg_viewer_profiles_and_qualification_offline() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["conformance", "verify-svg-viewers"])
        .output()
        .expect("run SVG viewer qualification verifier");

    assert!(
        output.status.success(),
        "verifier failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
