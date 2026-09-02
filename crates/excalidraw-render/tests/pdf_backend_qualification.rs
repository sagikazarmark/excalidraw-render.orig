use std::fs;
use std::path::{Path, PathBuf};

mod common;

use common::pdf_harness::{assert_structure, render_viewer};

use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest, Renderer,
    SceneDocument,
};
use excalidraw_render_compare::{
    compare_with_limits, CanonicalRgba8, ComparisonLimits, ComparisonMode,
};
use sha2::{Digest, Sha256};

const PROJECT_OPERATIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/project-operations.excalidraw"
));
const PROJECT_STRUCTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/project-operations-structure.json"
));
const NESTED_NATIVE_OPERATIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/nested-native-operations.excalidraw"
));
const NESTED_NATIVE_STRUCTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/nested-native-operations-structure.json"
));
const FALLBACK_STRUCTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/isolated-fallback-structure.json"
));
const ISOLATED_FALLBACK: &[u8] = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PGRlZnM+PGZpbHRlciBpZD0iYmx1ciIgeD0iMCIgeT0iMCIgd2lkdGg9IjEwMCUiIGhlaWdodD0iMTAwJSI+PGZlR2F1c3NpYW5CbHVyIHN0ZERldmlhdGlvbj0iMSIvPjwvZmlsdGVyPjwvZGVmcz48cmVjdCB3aWR0aD0iMjAiIGhlaWdodD0iMTAiIGZpbGw9IiMwMGZmMDAiIGZpbHRlcj0idXJsKCNibHVyKSIvPjwvc3ZnPg==","created":1}}}"##;

#[test]
fn complete_project_operation_matrix_is_deterministic_and_vector_first() {
    assert_eq!(
        format!("{:x}", Sha256::digest(PROJECT_OPERATIONS)),
        "eaef67702df489f774c8eb862159098b6e220e1dac1e79bc45fada6b6a43afda"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(PROJECT_STRUCTURE)),
        "ba1c6902feb31e5bc420364bcab36c92a01b1996499ea3fa9a9f0b40c56c5771"
    );
    let first = render_project_operations();
    let second = render_project_operations();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert!(!first
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
    assert_eq!(
        format!("{:x}", Sha256::digest(first.artifact().bytes())),
        "a19ac4b3ee3b066910dcfe76baa2715f94fbb5a676aa261b66a7efd7cbde4a06"
    );

    let pdf = String::from_utf8_lossy(first.artifact().bytes());
    for structure in [
        "/Subtype /Type0",
        "/ToUnicode ",
        "/ActualText <FEFF",
        "/Subtype /Link",
        "/URI (https://example.com/shape)",
        "/Subtype /Image",
        "/ColorSpace /DeviceGray",
        "/SMask",
        "/PatternType 1",
        "/ShadingType 2",
        "/ShadingType 3",
        "/BM /Multiply",
        "/Type /OutputIntent",
        "/Type /Metadata",
    ] {
        assert!(pdf.contains(structure), "missing PDF structure {structure}");
    }
    assert!(pdf.matches("/Subtype /Link").count() >= 2);
    assert!(pdf.matches("W\nn\n").count() >= 1);
}

#[test]
fn isolated_fallback_is_deterministic_and_confined_to_the_nested_svg() {
    assert_eq!(
        format!("{:x}", Sha256::digest(FALLBACK_STRUCTURE)),
        "eea223f7b1bf8f859cbbe138f2185c8c4d4e0681cad93ae68d878cbc5e24508c"
    );
    let first = render_isolated_fallback();
    let second = render_isolated_fallback();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert_eq!(
        format!("{:x}", Sha256::digest(first.artifact().bytes())),
        "fca49caf756b24b7191d5773ba083af96979650eb830cc1d48c96d32b2f93d28"
    );
    let [diagnostic] = first.report().diagnostics() else {
        panic!("one isolated closure produces one diagnostic");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::PDF_RASTER_FALLBACK);

    let pdf = String::from_utf8_lossy(first.artifact().bytes());
    assert_eq!(pdf.matches("/Subtype /Image").count(), 2);
    assert!(pdf.contains("/Subtype /Form"));
    assert!(pdf.contains("/Width 20"));
    assert!(pdf.contains("/Height 10"));
}

#[test]
fn every_pixel_domain_fallback_family_is_isolated_and_deterministic() {
    let cases = [
        ("blur", r#"<feGaussianBlur stdDeviation="1"/>"#),
        (
            "convolution",
            r#"<feConvolveMatrix order="3" kernelMatrix="0 -1 0 -1 5 -1 0 -1 0"/>"#,
        ),
        (
            "morphology",
            r#"<feMorphology operator="dilate" radius="1"/>"#,
        ),
        (
            "displacement",
            r#"<feTurbulence baseFrequency="0.1" numOctaves="1" result="noise"/><feDisplacementMap in="SourceGraphic" in2="noise" scale="2"/>"#,
        ),
        (
            "turbulence",
            r#"<feTurbulence baseFrequency="0.1" numOctaves="1"/>"#,
        ),
        (
            "lighting",
            r#"<feDiffuseLighting surfaceScale="2"><feDistantLight azimuth="45" elevation="60"/></feDiffuseLighting>"#,
        ),
        (
            "post-composite-color",
            r#"<feColorMatrix type="saturate" values="0.5"/>"#,
        ),
        (
            "filter-blend",
            r#"<feBlend in="SourceGraphic" in2="SourceGraphic" mode="overlay"/>"#,
        ),
    ];

    for (name, primitives) in cases {
        let nested = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect" x="0" y="0" width="100%" height="100%">{primitives}</filter></defs><rect width="20" height="10" fill="#00ff00" filter="url(#effect)"/></svg>"##
        );
        let scene = nested_svg_scene(nested.as_bytes());
        let request = RenderRequest::builder(OutputFormat::Pdf)
            .build()
            .expect("fallback-family request is valid");
        let first = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("{name} fallback renders: {error}"));
        let second = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("{name} fallback renders repeatedly: {error}"));

        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{name}"
        );
        assert_eq!(first.report(), second.report(), "{name}");
        assert!(
            first
                .report()
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK }),
            "{name}"
        );
        assert!(
            String::from_utf8_lossy(first.artifact().bytes()).contains("/Subtype /Image"),
            "{name}"
        );
    }
}

#[test]
fn complete_nested_svg_native_matrix_never_uses_raster_fallback() {
    assert_eq!(
        format!("{:x}", Sha256::digest(NESTED_NATIVE_OPERATIONS)),
        "6aebb7ca70ae3742d23597ca3ff109341219269d70d134ace5157d3112bfa257"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(NESTED_NATIVE_STRUCTURE)),
        "3a6c6f0ef646fc5be6d00f2b5379a2b5d4332a003be59d447fb8e802894bef4d"
    );
    let scene = SceneDocument::parse(NESTED_NATIVE_OPERATIONS)
        .expect("nested native qualification scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("nested native qualification request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("every nested native family renders without fallback");
    let repeated = Renderer::new()
        .render(&scene, &request)
        .expect("every nested native family renders repeatedly");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert_eq!(outcome.artifact().bytes(), repeated.artifact().bytes());
    assert_eq!(outcome.report(), repeated.report());
    assert_eq!(
        format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
        "ddc239fdeea14a6cfc72d7c8eadbdfd2130647a4cec5f5afd9fdbeb3e61af4b5"
    );
    assert!(outcome.report().diagnostics().is_empty());
    for structure in [
        "/PatternType 1",
        "/ShadingType 2",
        "/ShadingType 3",
        "/SMask",
        "/BM /Multiply",
        "/BM /Screen",
        "/BM /Overlay",
        "/BM /Darken",
        "/BM /Lighten",
        "/BM /ColorDodge",
        "/BM /ColorBurn",
        "/BM /HardLight",
        "/BM /SoftLight",
        "/BM /Difference",
        "/BM /Exclusion",
        "/BM /Hue",
        "/BM /Saturation",
        "/BM /Color",
        "/BM /Luminosity",
    ] {
        assert!(pdf.contains(structure), "missing PDF structure {structure}");
    }
    assert!(
        pdf.contains("/Subtype /Image"),
        "nested source image remains an image"
    );
    assert!(!pdf.contains("/Type /Font"), "nested text is outlined");
    assert!(pdf.contains(" c\n"), "nested text emits vector curves");
}

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
    let scene = SceneDocument::parse(NESTED_NATIVE_OPERATIONS)
        .expect("nested native qualification scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("nested native qualification request is valid");
    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("every nested native family renders without fallback");
    assert_structural_gate(
        "pdf-nested-native-qualification",
        outcome,
        "tests/conformance/pdf-backend-qualification/nested-native-operations-structure.json",
    );
}

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harnesses"]
fn isolated_fallback_passes_both_canonical_pdf_visual_gates() {
    let root = repository_root();
    let evidence = root.join("tests/conformance/cases/focused/pdf-isolated-fallback");
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(fs::read(evidence.join("scene.excalidraw")).unwrap())
        ),
        "fda8b589d6010f75bc73b5694c8832d9400e401d61c634442f0faea4f4c3eaf0"
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(fs::read(evidence.join("evidence/default/official.svg")).unwrap())
        ),
        "8efab916e92d110a76ac8ecb692027895a6b021d3bf594ca747f742ac600c363"
    );
    let reference_bytes = fs::read(evidence.join("evidence/default/official-svg.rgba"))
        .expect("approved official-SVG fallback evidence is present");
    assert_eq!(
        format!("{:x}", Sha256::digest(&reference_bytes)),
        "e08641597e5c3a4c2e222153a196f591639d97355552a6e25e3b007dae3821d6"
    );
    let reference = CanonicalRgba8::new(40, 30, reference_bytes)
        .expect("official-SVG fallback evidence is canonical RGBA8");
    let exception_bytes = fs::read(evidence.join("pdf-visual-exception.json"))
        .expect("approved fallback visual exception is present");
    assert_eq!(
        format!("{:x}", Sha256::digest(&exception_bytes)),
        "83d5bdfbedd9844f9a4214bc4e1b01c5e1a6c9d10943675b8f5a11a777c98079"
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(
                fs::read(root.join("tools/reference-oracle/scripts/pdf-fallback-evidence.mjs"))
                    .expect("fallback evidence generator is present")
            )
        ),
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
    let stem = format!("pdf-fallback-viewers-{}", std::process::id());
    let candidate = format!("target/{stem}.pdf");
    fs::write(
        root.join(&candidate),
        render_isolated_fallback().artifact().bytes(),
    )
    .expect("write renderer-owned fallback PDF under target");

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

fn assert_structural_gate(
    stem_prefix: &str,
    outcome: excalidraw_render::RenderOutcome,
    expectation: &str,
) {
    let root = repository_root();
    let stem = format!("{stem_prefix}-{}", std::process::id());
    let candidate = format!("target/{stem}.pdf");
    let output = format!("target/{stem}-structure.json");
    fs::write(root.join(&candidate), outcome.artifact().bytes())
        .expect("write renderer-owned qualification PDF under target");

    assert_structure(&root, &candidate, expectation, &output);
}

fn render_project_operations() -> excalidraw_render::RenderOutcome {
    let scene = SceneDocument::parse(PROJECT_OPERATIONS).expect("qualification scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("qualification request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("the complete project operation matrix renders")
}

fn render_isolated_fallback() -> excalidraw_render::RenderOutcome {
    let scene = SceneDocument::parse(ISOLATED_FALLBACK).expect("fallback scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("fallback request is valid");
    Renderer::new()
        .render(&scene, &request)
        .expect("the isolated fallback renders")
}

fn nested_svg_scene(nested: &[u8]) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(nested);
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}}],"appState":{{}},"files":{{"asset":{{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}}}}}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("fallback-family scene is admitted")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_path_buf()
}
