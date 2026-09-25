//! Retired PDF workflows fail with guidance and preserve qualification evidence.
use std::{fs, path::Path, process::Command};

#[test]
fn retired_pdf_commands_preserve_qualification_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let manifest =
        root.join("tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.json");
    let before = fs::read(&manifest).unwrap();
    for (command, guidance) in [
        (
            "candidate-pdf-requalify",
            "use current PDF harness and native snapshot checks",
        ),
        (
            "pdf-requalification-subject",
            "retained evidence remains available",
        ),
        (
            "refresh-pdf-viewer-dependencies",
            "use conformance verify-pdf-viewers",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(root)
            .args(["conformance", command])
            .output()
            .unwrap();
        assert_eq!(fs::read(&manifest).unwrap(), before, "{command}");
        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        assert!(output.stdout.is_empty(), "{command} wrote success output");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("retired"), "{command}: {stderr}");
        assert!(stderr.contains(guidance), "{command}: {stderr}");
    }
}
