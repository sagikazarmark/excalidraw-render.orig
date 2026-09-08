//! The approved rectangle-outline PDF through both canonical PDF viewers and
//! the structural inspector, judged against the Chromium official-SVG evidence
//! and the approved PDFium tolerance exception (issue #44).

use std::collections::BTreeMap;
use std::fs;

use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass,
};

use crate::common::approved_scenes::render_rectangle_outline_pdf;
use crate::common::pdf_harness::{assert_structure, render_viewer};
use crate::common::repository_root;

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harness"]
fn approved_rectangle_passes_both_canonical_pdf_visual_gates() {
    let root = repository_root();
    let case = root.join("tests/conformance/cases/focused/rectangle-outline");
    let exception: serde_json::Value = serde_json::from_slice(
        &fs::read(case.join("pdf-visual-exception.json")).expect("PDF exception is present"),
    )
    .expect("PDF exception is valid JSON");
    assert_eq!(exception["approved_issue"], 44);
    assert_eq!(exception["viewer_profile"], "canonical-pdf.pdfium.v1");
    let stem = format!("rectangle-pdf-candidate-{}", std::process::id());
    let candidate = format!("target/{stem}.pdf");
    let reference = CanonicalRgba8::new(
        200,
        120,
        fs::read(root.join(concat!(
            "tests/conformance/svg-viewer-qualification/rectangle-outline/",
            "evidence/chromium-reference.rgba"
        )))
        .unwrap(),
    )
    .expect("canonical Chromium official-SVG evidence is valid");
    fs::write(
        root.join(&candidate),
        render_rectangle_outline_pdf().artifact().bytes(),
    )
    .expect("write renderer-owned candidate PDF under target");

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
        let output = format!("target/{stem}-{viewer}.rgba");
        let candidate = CanonicalRgba8::new(
            200,
            120,
            render_viewer(&root, viewer, script, &candidate, &output, 200, 120),
        )
        .expect("canonical PDF viewer output is valid");
        let report = compare(
            &reference,
            &candidate,
            VisualClass::GeometryImage,
            &[],
            mode,
        )
        .expect("candidate and reference dimensions agree");
        if report.passed() {
            continue;
        }
        assert_eq!(
            viewer, "pdfium",
            "only the approved PDFium exception applies"
        );
        assert_eq!(
            report.whole.changed_pixels,
            exception["max_changed_pixels"].as_u64().unwrap()
        );
        let SsimOutcome::Score(ssim) = report.whole.ssim else {
            panic!("PDFium exception requires a finite SSIM score");
        };
        let minimum_bits =
            u64::from_str_radix(exception["minimum_ssim_bits"].as_str().unwrap(), 16)
                .expect("PDFium exception contains valid f64 bits");
        assert!(
            ssim >= f64::from_bits(minimum_bits),
            "PDFium exception failed: changed_pixels={}, ssim={ssim:?}, deltas={:?}",
            report.whole.changed_pixels,
            pixel_delta_summary(reference.rgba(), candidate.rgba())
        );
    }

    let structure_output = format!("target/{stem}-structure.json");
    assert_structure(
        &root,
        &candidate,
        "tests/conformance/cases/focused/rectangle-outline/expected-pdf-structure.json",
        &structure_output,
    );
}

fn pixel_delta_summary(reference: &[u8], candidate: &[u8]) -> BTreeMap<i16, u64> {
    let mut summary = BTreeMap::new();
    for (reference, candidate) in reference.iter().zip(candidate) {
        if reference != candidate {
            *summary
                .entry(i16::from(*candidate) - i16::from(*reference))
                .or_insert(0) += 1;
        }
    }
    summary
}
