//! The PDF backend qualification gates: the three operation matrices through
//! the canonical qpdf structural inspector, and the isolated raster fallback
//! through both canonical PDF viewers under its approved tolerance exception.

use std::fs;

use excalidraw_render::RenderOutcome;
use excalidraw_render_compare::{
    compare_with_limits, CanonicalRgba8, ComparisonLimits, ComparisonMode,
};
use sha2::{Digest, Sha256};

use crate::common::approved_scenes::{
    render_isolated_fallback, render_nested_native_operations, render_project_operations,
};
use crate::common::lower_hex;
use crate::common::pdf_harness::{assert_structure, render_viewer};
use crate::common::repository_root;
use crate::common::scratch::ScratchDir;

#[test]
#[ignore = "requires the exact-pinned qpdf structural harness"]
fn complete_project_operation_matrix_passes_the_canonical_structural_gate() {
    assert_structural_gate(
        "pdf-backend-qualification",
        render_project_operations(),
        "tests/conformance/pdf-backend-qualification/project-operations-structure.json",
    );
}

#[test]
#[ignore = "requires the exact-pinned qpdf structural harness"]
fn isolated_fallback_passes_the_canonical_structural_gate() {
    assert_structural_gate(
        "pdf-fallback-qualification",
        render_isolated_fallback(),
        "tests/conformance/pdf-backend-qualification/isolated-fallback-structure.json",
    );
}

#[test]
#[ignore = "requires the exact-pinned qpdf structural harness"]
fn complete_nested_svg_native_matrix_passes_the_canonical_structural_gate() {
    assert_structural_gate(
        "pdf-nested-native-qualification",
        render_nested_native_operations(),
        "tests/conformance/pdf-backend-qualification/nested-native-operations-structure.json",
    );
}

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harnesses"]
fn isolated_fallback_passes_both_canonical_pdf_visual_gates() {
    let root = repository_root();
    let evidence = root.join("tests/conformance/cases/focused/pdf-isolated-fallback");
    assert_eq!(
        lower_hex(&Sha256::digest(
            fs::read(evidence.join("scene.excalidraw")).unwrap()
        )),
        "fda8b589d6010f75bc73b5694c8832d9400e401d61c634442f0faea4f4c3eaf0"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(
            fs::read(evidence.join("evidence/official.svg")).unwrap()
        )),
        "8efab916e92d110a76ac8ecb692027895a6b021d3bf594ca747f742ac600c363"
    );
    let reference_bytes = fs::read(evidence.join("evidence/official-svg.rgba"))
        .expect("approved official-SVG fallback evidence is present");
    assert_eq!(
        lower_hex(&Sha256::digest(&reference_bytes)),
        "e08641597e5c3a4c2e222153a196f591639d97355552a6e25e3b007dae3821d6"
    );
    let reference = CanonicalRgba8::new(40, 30, reference_bytes)
        .expect("official-SVG fallback evidence is canonical RGBA8");
    let exception_bytes = fs::read(evidence.join("pdf-visual-exception.json"))
        .expect("approved fallback visual exception is present");
    assert_eq!(
        lower_hex(&Sha256::digest(&exception_bytes)),
        "e79c29f3849c480dcce62fa6169c01a7f11a399dd241e37fc0ee08780669e6cc"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(
            fs::read(root.join("tools/reference-oracle/scripts/pdf-fallback-evidence.mjs"))
                .expect("fallback evidence generator is present")
        )),
        "1d49b89a04fd7a05a634fa4906d611f71c6f72379306bb6d22a684494d0ca656"
    );
    let exception: serde_json::Value =
        serde_json::from_slice(&exception_bytes).expect("fallback visual exception is valid JSON");
    let minimum_bits = u64::from_str_radix(exception["minimum_ssim_bits"].as_str().unwrap(), 16)
        .expect("fallback exception contains valid f64 bits");
    let limits = ComparisonLimits::tolerance_exception(
        exception["max_changed_pixels"].as_u64().unwrap(),
        f64::from_bits(minimum_bits),
    )
    .expect("fallback exception limits are finite");
    let scratch = ScratchDir::new("pdf-fallback-viewers");
    let candidate = scratch.write(
        "candidate.pdf",
        render_isolated_fallback().artifact().bytes(),
    );

    for (viewer, script, mode) in [
        (
            "pdfjs",
            "tools/pdf-harness/run-pdfjs.sh",
            ComparisonMode::PdfJs,
        ),
        (
            "pdfium",
            "tools/pdf-harness/run-pdfium.sh",
            ComparisonMode::Canonical,
        ),
    ] {
        let output = scratch.relative(&format!("{viewer}.rgba"));
        let rgba = render_viewer(&root, viewer, script, &candidate, &output, 40, 30);
        let candidate =
            CanonicalRgba8::new(40, 30, rgba).expect("canonical PDF viewer output is valid");
        let report = compare_with_limits(&reference, &candidate, limits, &[], mode)
            .expect("candidate and official fallback dimensions agree");
        assert!(
            report.passed(),
            "{viewer} fallback budget failed: changed_pixels={}, ssim={:?}",
            report.whole.changed_pixels,
            report.whole.ssim
        );
    }
}

fn assert_structural_gate(scratch_name: &str, outcome: RenderOutcome, expectation: &str) {
    let root = repository_root();
    let scratch = ScratchDir::new(scratch_name);
    let candidate = scratch.write("candidate.pdf", outcome.artifact().bytes());
    let output = scratch.relative("structure.json");

    assert_structure(&root, &candidate, expectation, &output);
}
