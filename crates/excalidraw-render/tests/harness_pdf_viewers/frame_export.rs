//! Every frame export of `focused.frame-export` as PDF through both canonical
//! PDF viewers, judged against the official-SVG raster evidence under the
//! case's approved PDF tolerance exceptions.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use excalidraw_render::{FrameRenderRequest, OutputFormat, Renderer, SceneDocument};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use sha2::{Digest, Sha256};

use crate::common::lower_hex;
use crate::common::pdf_harness::render_viewer;
use crate::common::registered_cases::{divergence, exception_limits, visual_class};
use crate::common::repository_root;
use crate::common::scratch::ScratchDir;

/// The approved PDF tolerance exceptions of `focused.frame-export`
/// (`pdf-tolerance-exceptions.json`), one record per frame target and canonical
/// viewer, pinned by digest so a record cannot move without a reviewed re-pin.
/// Each record names the target's `frame_ordinal` in place of a scene variant;
/// the rest of the shape is the `excalidraw-render/pdf-compatibility-exception/v1`
/// record the other PDF viewer gates consume.
const PDF_TOLERANCE_EXCEPTIONS_SHA256: &str =
    "8bd758cce65d803ef2f6adc5b1b9af6c2f10b8c424e38282ab0a32cb936e511c";

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harnesses"]
fn frame_pdfs_pass_both_canonical_pdf_visual_gates() {
    let root = repository_root();
    let case = root.join("tests/conformance/cases/focused/frame-export");
    let scene = SceneDocument::parse(&fs::read(case.join("scene.excalidraw")).unwrap())
        .expect("approved frame scene is admitted");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(case.join("manifest.json")).unwrap()).unwrap();
    let targets = manifest["targets"]
        .as_array()
        .expect("reference targets exist");
    let case_id = manifest["case_id"].as_str().expect("case id is a string");
    let exceptions = pdf_tolerance_exceptions(&case, case_id);
    let renderer = Renderer::new();
    let scratch = ScratchDir::new("frame-pdf-candidate");
    let mut failures = Vec::new();
    let mut excepted = BTreeSet::new();

    for (index, (target, frame)) in targets.iter().zip(scene.frames()).enumerate() {
        let ordinal = target["frame_ordinal"]
            .as_u64()
            .expect("target frame_ordinal is an integer");
        assert_eq!(ordinal, index as u64 + 1, "targets are in frame order");
        let width = u32::try_from(
            target["outputs"]["svg"]["artifact"]["width"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let height = u32::try_from(
            target["outputs"]["svg"]["artifact"]["height"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let reference = CanonicalRgba8::new(
            width,
            height,
            fs::read(case.join(format!("evidence/frame-{ordinal}/official-svg.rgba"))).unwrap(),
        )
        .expect("official SVG raster is canonical RGBA8");
        let pdf = renderer
            .render_frame(
                frame,
                &FrameRenderRequest::builder(OutputFormat::Pdf)
                    .build()
                    .expect("PDF frame request is valid"),
            )
            .expect("frame renders to PDF");
        let candidate_path = scratch.write(&format!("frame-{ordinal}.pdf"), pdf.artifact().bytes());
        let class = match ordinal {
            4 | 5 => VisualClass::MixedRepresentative,
            7 => VisualClass::Typography,
            _ => VisualClass::GeometryImage,
        };
        for (viewer, script, mode, profile) in [
            (
                "pdfjs",
                "tools/pdf-harness/run-pdfjs.sh",
                ComparisonMode::PdfJs,
                "canonical-pdf.pdfjs.v1",
            ),
            (
                "pdfium",
                "tools/pdf-harness/run-pdfium.sh",
                ComparisonMode::Canonical,
                "canonical-pdf.pdfium.v1",
            ),
        ] {
            let output = scratch.relative(&format!("frame-{ordinal}-{viewer}.rgba"));
            let candidate = CanonicalRgba8::new(
                width,
                height,
                render_viewer(
                    &root,
                    viewer,
                    script,
                    &candidate_path,
                    &output,
                    width,
                    height,
                ),
            )
            .expect("canonical PDF viewer output is valid");
            let canonical = compare(&reference, &candidate, class, &[], mode)
                .expect("frame PDF and official SVG dimensions agree");
            let exception = exceptions
                .iter()
                .find(|record| exception_scope(record) == (ordinal, profile))
                .map(|record| {
                    assert_eq!(
                        visual_class(record["visual_class"].as_str().unwrap()),
                        class,
                        "frame {ordinal} {viewer} exception names the target's visual class"
                    );
                    excepted.insert((ordinal, profile));
                    exception_limits(record)
                });
            if let Some(detail) = divergence(&canonical, exception) {
                failures.push(format!("frame {ordinal} in {viewer}: {detail}"));
            }
        }
    }

    let orphans: Vec<_> = exceptions
        .iter()
        .map(exception_scope)
        .filter(|scope| !excepted.contains(scope))
        .collect();
    assert!(
        orphans.is_empty(),
        "PDF tolerance exceptions name frame targets or viewers the case does not export: \
         {orphans:?}"
    );
    assert!(
        failures.is_empty(),
        "{} frame PDF renders diverge from the official SVG raster:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The approved PDF tolerance exceptions the frame case records, checked
/// against their digest pin and their declared scope: this case, PDF, the
/// whole artifact, and the official-SVG authority.
fn pdf_tolerance_exceptions(case: &Path, case_id: &str) -> Vec<serde_json::Value> {
    let bytes = fs::read(case.join("pdf-tolerance-exceptions.json"))
        .expect("frame PDF tolerance exceptions are present");
    assert_eq!(
        lower_hex(&Sha256::digest(&bytes)),
        PDF_TOLERANCE_EXCEPTIONS_SHA256,
        "frame PDF tolerance exceptions moved without a reviewed re-pin"
    );
    let records: Vec<serde_json::Value> =
        serde_json::from_slice(&bytes).expect("frame PDF tolerance exceptions are a JSON array");
    let mut scopes = BTreeSet::new();
    for record in &records {
        assert_eq!(
            record["schema"],
            "excalidraw-render/pdf-compatibility-exception/v1"
        );
        assert_eq!(record["case_id"], case_id);
        assert_eq!(record["format"], "pdf");
        assert_eq!(record["region"], "whole");
        assert_eq!(
            record["authority"],
            "official SVG rendered by canonical Chromium"
        );
        assert!(
            record["approved_issue"].is_u64(),
            "exception names the issue carrying its approval"
        );
        assert!(
            record["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty()),
            "exception documents its non-defect cause"
        );
        let scope = exception_scope(record);
        assert!(
            scopes.insert(scope),
            "one tolerance exception per frame target and viewer: {scope:?}"
        );
    }
    records
}

/// The frame target and canonical viewer one exception record governs.
fn exception_scope(record: &serde_json::Value) -> (u64, &str) {
    (
        record["frame_ordinal"]
            .as_u64()
            .expect("exception frame_ordinal is an integer"),
        record["viewer_profile"]
            .as_str()
            .expect("exception viewer_profile is a string"),
    )
}
