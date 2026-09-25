//! Retained WebP viewer observations and adversarial comparison controls.
use std::{path::Path, process::Command};

#[test]
fn retained_webp_viewer_boundary_rejects_pixel_and_semantic_drift() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Command::new("node")
        .current_dir(root)
        .arg("tools/pdf-harness/diagnostics/webp191-verify-boundary.mjs")
        .output()
        .expect("Node is required for evidence checks");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
