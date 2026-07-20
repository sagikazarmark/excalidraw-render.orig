use std::fs;
use std::path::{Path, PathBuf};

use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use serde_json::Value;

const REFERENCE: &str =
    "tests/conformance/svg-viewer-qualification/rectangle-outline/evidence/chromium-reference.rgba";
const EVIDENCE_ROOT: &str = "tests/conformance/pdf-viewer-qualification/rectangle-outline/evidence";

#[test]
fn pdf_viewers_and_comparator_keep_visual_and_structural_gates_independent() {
    let root = repository_root();
    let reference = canonical_raster(&root, REFERENCE);
    for (viewer, mode) in [
        ("pdfium", ComparisonMode::Canonical),
        ("pdfjs", ComparisonMode::PdfJs),
    ] {
        for (role, expected_pass, expected_changed) in [
            ("valid", true, 0),
            ("visual-fail", false, 1_600),
            ("structure-fail", true, 0),
        ] {
            let candidate =
                canonical_raster(&root, &format!("{EVIDENCE_ROOT}/{viewer}-{role}.rgba"));
            let report = compare(
                &reference,
                &candidate,
                VisualClass::GeometryImage,
                &[],
                mode,
            )
            .unwrap();

            assert_eq!(report.passed(), expected_pass, "{viewer} {role}");
            assert_eq!(report.whole.changed_pixels, expected_changed);
            assert_eq!(report.whole.width, 200);
            assert_eq!(report.whole.height, 120);

            let checked: Value = serde_json::from_slice(
                &fs::read(
                    root.join(EVIDENCE_ROOT)
                        .join(format!("{viewer}-{role}-comparison.json")),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(checked["mode"], viewer);
            assert_eq!(checked["passed"], expected_pass);
            assert_eq!(checked["changed_pixels"], expected_changed);
        }
    }
}

fn canonical_raster(root: &Path, relative: &str) -> CanonicalRgba8 {
    let bytes = fs::read(root.join(relative)).unwrap();
    CanonicalRgba8::new(200, 120, bytes).unwrap()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}
