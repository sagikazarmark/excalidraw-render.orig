//! The PDF backend qualification without its harness: the operation matrices
//! and the isolated fallback render deterministically, vector-first where the
//! qualification says so, with their inputs and outputs pinned by digest. The
//! qpdf structural gates and the fallback's viewer gates over the same renders
//! live in `harness_pdf_viewers`.

use crate::common::pdf_inspect::pdf_text;
use base64::Engine as _;
use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, Renderer, SceneDocument};
use sha2::{Digest, Sha256};

use crate::common::approved_scenes::{
    render_isolated_fallback, render_nested_native_operations, render_project_operations,
    NESTED_NATIVE_OPERATIONS, PROJECT_OPERATIONS,
};

const PROJECT_STRUCTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/project-operations-structure.json"
));
const NESTED_NATIVE_STRUCTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/nested-native-operations-structure.json"
));
const FALLBACK_STRUCTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/conformance/pdf-backend-qualification/isolated-fallback-structure.json"
));

#[test]
fn complete_project_operation_matrix_is_deterministic_and_vector_first() {
    assert_eq!(
        format!("{:x}", Sha256::digest(PROJECT_OPERATIONS)),
        "eaef67702df489f774c8eb862159098b6e220e1dac1e79bc45fada6b6a43afda"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(PROJECT_STRUCTURE)),
        "b6cbc6049556ae87780a83e9c252f64193594bf37c9d6a2f18a80434b31cb7b9"
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
    // Moved with #151 item 1: the XMP manifest carries one attribution notice
    // per embedded font family and the OFL body once; every other stream is
    // unchanged, so `operation_sha256` in the structure expectation holds and
    // only its `inventory_sha256` (which digests the XMP stream) is re-pinned.
    // Moved with #171: the scene's arrow has a `circle` start and a `triangle`
    // end at roughness 1, and the triangle is now drawn at 1, not at the
    // start head's 0.5, so the content stream and `operation_sha256` moved.
    assert_eq!(
        format!("{:x}", Sha256::digest(first.artifact().bytes())),
        "b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43"
    );

    let pdf = pdf_text(first.artifact().bytes());
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
        "207ff3e6b4261d7293e8bb195160c4bf4b59f84814ff35701c83270b6373660f"
    );
    let first = render_isolated_fallback();
    let second = render_isolated_fallback();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert_eq!(
        format!("{:x}", Sha256::digest(first.artifact().bytes())),
        "64bee9cbb44081c7202b47b392fbc8859164ca3a3f383bdc4f54d7cc87b50513"
    );
    let [diagnostic] = first.report().diagnostics() else {
        panic!("one isolated closure produces one diagnostic");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::PDF_RASTER_FALLBACK);

    let pdf = pdf_text(first.artifact().bytes());
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
            pdf_text(first.artifact().bytes()).contains("/Subtype /Image"),
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
        "c767ae971e4d3bd73e7bee8a44cfd91ea1cdd3cbfaf2909db2ba4ade8ccb8f71"
    );
    let outcome = render_nested_native_operations();
    let repeated = render_nested_native_operations();
    let pdf = pdf_text(outcome.artifact().bytes());

    assert_eq!(outcome.artifact().bytes(), repeated.artifact().bytes());
    assert_eq!(outcome.report(), repeated.report());
    assert_eq!(
        format!("{:x}", Sha256::digest(outcome.artifact().bytes())),
        "8dea3f9c57db8bd680076875802bc7b39e144b27e6638e1cf79b4d88dd979ddc"
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

fn nested_svg_scene(nested: &[u8]) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(nested);
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}}],"appState":{{}},"files":{{"asset":{{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}}}}}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("fallback-family scene is admitted")
}
