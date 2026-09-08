//! The PDF backend qualification without its harness: the operation matrices
//! and the isolated fallback render deterministically, vector-first where the
//! qualification says so, with their inputs and outputs pinned by digest. The
//! qpdf structural gates and the fallback's viewer gates over the same renders
//! live in `harness_pdf_viewers`.

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
        "2e59dad185c4aabcb0c29a11a6354806dd31d42d6bc3a70cb685e157a2bf7076"
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
        "8e2eb6f5795e144a322649e347df0a05515655c96f874823b9a22e8a4c7da82c"
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
        "20c6ab1a6d4026cf57668323cdc3a21e7a187238275cb69be6c5c5dba036da10"
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
        "5c2041e7ed4506d931187a453626c91470815549bfa8d3301bb28b213b19df4c"
    );
    let outcome = render_nested_native_operations();
    let repeated = render_nested_native_operations();
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

fn nested_svg_scene(nested: &[u8]) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(nested);
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"image","type":"image","x":0,"y":0,"width":20,"height":10,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}}],"appState":{{}},"files":{{"asset":{{"id":"asset","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{encoded}","created":1}}}}}}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("fallback-family scene is admitted")
}
