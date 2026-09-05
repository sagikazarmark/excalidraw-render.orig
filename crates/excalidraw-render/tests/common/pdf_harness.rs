//! Shared plumbing for the harness-gated PDF tests: running the canonical PDF
//! viewer adapters and the structural inspector through their process
//! contracts.

// Each harness binary includes this module by path and uses the part it needs.
#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;

/// One successful canonical viewer render: the RGBA bytes the adapter wrote and
/// the JSON summary it printed.
pub struct ViewerRender {
    pub rgba: Vec<u8>,
    pub summary: serde_json::Value,
}

/// Runs a canonical PDF viewer adapter through its process contract
/// `render <input.pdf> <output.rgba> <width> <height>`; a failed render yields
/// the adapter's diagnostic output instead of panicking.
pub fn try_render_viewer(
    root: &Path,
    viewer: &str,
    script: &str,
    candidate: &str,
    output: &str,
    width: u32,
    height: u32,
) -> Result<ViewerRender, String> {
    let result = Command::new("bash")
        .current_dir(root)
        .args([
            script,
            "render",
            candidate,
            output,
            &width.to_string(),
            &height.to_string(),
        ])
        .output()
        .unwrap_or_else(|error| panic!("launch canonical {viewer} viewer: {error}"));
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    if !result.status.success() {
        return Err(format!("{stderr}{stdout}"));
    }
    print!("{stdout}");
    let summary = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|error| panic!("canonical {viewer} viewer summary is JSON: {error}"));
    let rgba = fs::read(root.join(output)).expect("canonical PDF viewer output is present");
    Ok(ViewerRender { rgba, summary })
}

pub fn render_viewer(
    root: &Path,
    viewer: &str,
    script: &str,
    candidate: &str,
    output: &str,
    width: u32,
    height: u32,
) -> Vec<u8> {
    try_render_viewer(root, viewer, script, candidate, output, width, height)
        .unwrap_or_else(|diagnostics| panic!("canonical {viewer} viewer failed:\n{diagnostics}"))
        .rgba
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
