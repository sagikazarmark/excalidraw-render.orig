use std::fs;
use std::path::Path;
use std::process::Command;

pub fn render_viewer(
    root: &Path,
    viewer: &str,
    script: &str,
    candidate: &str,
    output: &str,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let status = Command::new("bash")
        .current_dir(root)
        .args([
            script,
            "render",
            candidate,
            output,
            &width.to_string(),
            &height.to_string(),
        ])
        .status()
        .unwrap_or_else(|error| panic!("launch canonical {viewer} viewer: {error}"));
    assert!(status.success(), "canonical {viewer} viewer failed");
    fs::read(root.join(output)).expect("canonical PDF viewer output is present")
}

pub fn assert_structure(root: &Path, candidate: &str, expectation: &str, output: &str) {
    let status = Command::new("bash")
        .current_dir(root)
        .args([
            "tools/pdf-harness/run-structure.sh",
            "inspect",
            candidate,
            expectation,
            "pass",
            output,
        ])
        .status()
        .expect("launch canonical PDF structural inspector");
    assert!(status.success(), "PDF structural gate failed");
}
