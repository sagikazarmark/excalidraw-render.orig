use std::fs;
use std::path::{Path, PathBuf};

use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass,
};
use serde_json::Value;

const EVIDENCE_ROOT: &str = "tests/conformance/svg-viewer-qualification/rectangle-outline/evidence";

#[test]
fn approved_rectangle_qualifies_both_svg_viewers_and_the_comparator() {
    let root = repository_root();
    for (viewer, mode) in [
        ("chromium", ComparisonMode::Canonical),
        ("pure-rust", ComparisonMode::PureRustSvg),
    ] {
        let reference = canonical_raster(&root, viewer, "reference");
        let candidate = canonical_raster(&root, viewer, "candidate");

        let report = compare(
            &reference,
            &candidate,
            VisualClass::GeometryImage,
            &[],
            mode,
        )
        .unwrap();

        assert!(report.passed());
        assert_eq!(report.whole.width, 200);
        assert_eq!(report.whole.height, 120);
        assert_eq!(report.whole.changed_pixels, 0);
        assert_eq!(report.whole.ssim, SsimOutcome::Score(1.0));
        assert!(report.crops.is_empty());

        let expected: Value = serde_json::from_slice(
            &fs::read(
                root.join(EVIDENCE_ROOT)
                    .join(format!("{viewer}-comparison.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(expected["changed_pixels"], 0);
        assert_eq!(expected["height"], 120);
        assert_eq!(expected["mode"], viewer);
        assert_eq!(expected["passed"], true);
        assert_eq!(expected["ssim_bits"], "3ff0000000000000");
        assert_eq!(expected["width"], 200);
    }
}

fn canonical_raster(root: &Path, viewer: &str, role: &str) -> CanonicalRgba8 {
    let bytes = fs::read(
        root.join(EVIDENCE_ROOT)
            .join(format!("{viewer}-{role}.rgba")),
    )
    .unwrap();
    CanonicalRgba8::new(200, 120, bytes).unwrap()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}
