//! The approved Excalifont text PDF through both canonical PDF viewers and the
//! structural inspector, judged against the official-SVG evidence under the
//! typography budget and the approved PDFium tolerance exception (issue #73).

use std::fs;

use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{
    compare, compare_with_limits, CanonicalRgba8, ComparisonLimits, ComparisonMode, VisualClass,
};
use sha2::{Digest, Sha256};

use crate::common::pdf_harness::{assert_structure, render_viewer};
use crate::common::repository_root;
use crate::common::scratch::ScratchDir;

#[test]
#[ignore = "requires the exact-pinned PDFium, PDF.js, and qpdf harnesses"]
fn approved_excalifont_text_passes_both_canonical_pdf_gates() {
    let root = repository_root();
    let case = root.join("tests/conformance/cases/focused/excalifont-text");
    let scene_bytes = fs::read(case.join("scene.excalidraw")).expect("approved scene is present");
    let scene = SceneDocument::parse(&scene_bytes).expect("approved scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("default PDF request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("approved text renders directly to PDF");
    let exception_bytes =
        fs::read(case.join("pdf-visual-exception.json")).expect("PDF exception is present");
    assert_eq!(
        format!("{:x}", Sha256::digest(&exception_bytes)),
        "57906d0d050b20c68990c07417a66574e6685f1b8783fadcff0637e14500a0f3"
    );
    let exception: serde_json::Value =
        serde_json::from_slice(&exception_bytes).expect("PDF exception is valid JSON");
    assert_eq!(exception["approved_issue"], 73);
    assert_eq!(exception["viewer_profile"], "canonical-pdf.pdfium.v1");
    let reference = CanonicalRgba8::new(
        110,
        92,
        fs::read(case.join("evidence/official-svg.rgba"))
            .expect("approved official-SVG evidence is present"),
    )
    .expect("approved evidence is canonical RGBA8");
    let scratch = ScratchDir::new("excalifont-pdf-candidate");
    let candidate = scratch.write("candidate.pdf", outcome.artifact().bytes());

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
        let candidate = CanonicalRgba8::new(
            110,
            92,
            render_viewer(&root, viewer, script, &candidate, &output, 110, 92),
        )
        .expect("canonical PDF viewer output is valid");
        let report = if viewer == "pdfium" {
            let minimum_bits =
                u64::from_str_radix(exception["minimum_ssim_bits"].as_str().unwrap(), 16)
                    .expect("PDFium exception contains valid f64 bits");
            let limits = ComparisonLimits::tolerance_exception(
                exception["max_changed_pixels"].as_u64().unwrap(),
                f64::from_bits(minimum_bits),
            )
            .expect("PDFium exception limits are finite");
            compare_with_limits(&reference, &candidate, limits, &[], mode)
        } else {
            compare(&reference, &candidate, VisualClass::Typography, &[], mode)
        }
        .expect("candidate and reference dimensions agree");
        assert!(
            report.passed(),
            "{viewer} typography budget failed: changed_pixels={}, ssim={:?}",
            report.whole.changed_pixels,
            report.whole.ssim
        );
    }

    let structure_output = scratch.relative("structure.json");
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(
                fs::read(case.join("expected-pdf-structure.json"))
                    .expect("PDF structure expectation is present")
            )
        ),
        "016ba9734a8e77425cb0728028e9c87e5d7a799caa3ea12a6efe233122f7f12f"
    );
    assert_structure(
        &root,
        &candidate,
        "tests/conformance/cases/focused/excalifont-text/expected-pdf-structure.json",
        &structure_output,
    );
}
