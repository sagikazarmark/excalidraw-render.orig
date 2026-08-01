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

#[test]
fn verifies_every_svg_font_subset_in_both_canonical_viewers_offline() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["conformance", "verify-svg-font-subsets"])
        .output()
        .expect("run SVG font subset viewer qualification verifier");

    assert!(
        output.status.success(),
        "verifier failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
