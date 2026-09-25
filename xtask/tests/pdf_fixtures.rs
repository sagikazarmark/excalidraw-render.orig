//! Byte parity against hand-authored, tracked qualification fixtures. All output
//! goes to scratch directories; the canonical fixtures and raster are read-only.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const NAMES: [&str; 6] = [
    "candidate-control.pdf",
    "visual-fail-structure-pass.pdf",
    "visual-pass-structure-fail.pdf",
    "structure-coverage.pdf",
    "active-action.pdf",
    "parser-warning.pdf",
];

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pdf-fixtures-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn generate(&self, output: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["pdf-harness", "generate-fixtures"])
            .arg(output)
            .current_dir(&self.0)
            .output()
            .expect("run xtask pdf-harness generate-fixtures")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_pdfs_and_report_match_tracked_qualification_fixtures() {
    let scratch = Scratch::new();
    // A relative, nested output path also checks creation and cwd independence
    // when locating the canonical Chromium reference raster.
    let relative_output = Path::new("nested/generated");
    let output = scratch.generate(relative_output);
    assert_success(&output);
    let generated = scratch.0.join(relative_output);
    let canonical =
        repo_root().join("tests/conformance/pdf-viewer-qualification/rectangle-outline");
    let mut expected_stdout = String::new();
    for name in NAMES {
        let reference_path = canonical.join(name);
        let expected = fs::read(&reference_path).unwrap();
        let actual = fs::read(generated.join(name)).unwrap();
        assert!(
            actual == expected,
            "{name} differs from {} (generated {} bytes, reference {} bytes)",
            reference_path.display(),
            actual.len(),
            expected.len()
        );
        expected_stdout.push_str(&format!(
            "{name} {} {:x}\n",
            expected.len(),
            Sha256::digest(&expected)
        ));
    }
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected_stdout);
    let mut actual_names: Vec<_> = fs::read_dir(&generated)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    actual_names.sort();
    let mut expected_names = NAMES.to_vec();
    expected_names.sort();
    assert_eq!(actual_names, expected_names);

    // Existing output directories are supported and regeneration is stable.
    let repeated = scratch.generate(relative_output);
    assert_success(&repeated);
    assert_eq!(String::from_utf8(repeated.stdout).unwrap(), expected_stdout);
}

#[test]
fn output_directory_errors_are_reported() {
    let scratch = Scratch::new();
    let path = scratch.0.join("not-a-directory");
    fs::write(&path, b"keep this file").unwrap();
    let output = scratch.generate(&path);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not-a-directory"));
    assert_eq!(fs::read(path).unwrap(), b"keep this file");
}
