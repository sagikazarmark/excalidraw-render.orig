//! The PDF backend qualification without its harness: the operation matrices
//! and the isolated fallback render deterministically, vector-first where the
//! qualification says so; the golden record holds their bytes. The
//! qpdf structural gates and the fallback's viewer gates over the same renders
//! live in `harness_pdf_viewers`.

use crate::common::pdf_inspect::pdf_text;
use base64::Engine as _;
use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, Renderer, SceneDocument};

use crate::common::approved_scenes::{
    render_isolated_fallback, render_nested_native_operations, render_project_operations,
};

#[test]
fn complete_project_operation_matrix_is_deterministic_and_vector_first() {
    let first = render_project_operations();
    let second = render_project_operations();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert!(!first
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
    // The golden record holds the bytes (`facade.pdf-project-operations.v1`),
    // and `harness_pdf_viewers` holds its operations and viewer rasters.

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
        "/F (artifact-manifest.json)",
    ] {
        assert!(pdf.contains(structure), "missing PDF structure {structure}");
    }
    // Krilla claims no output intent and writes no XMP packet (ADR-0021).
    assert!(!pdf.contains("/OutputIntents"));
    assert!(!pdf.contains("/Type /Metadata"));
    assert!(pdf.matches("/Subtype /Link").count() >= 2);
    assert!(pdf.matches("W\nn\n").count() >= 1);
}

#[test]
fn isolated_fallback_is_deterministic_and_confined_to_the_nested_svg() {
    let first = render_isolated_fallback();
    let second = render_isolated_fallback();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    // The golden record holds the bytes (`facade.isolated-fallback.v1`).
    let [diagnostic] = first.report().diagnostics() else {
        panic!("one isolated closure produces one diagnostic");
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::PDF_RASTER_FALLBACK);

    let pdf = pdf_text(first.artifact().bytes());
    assert_eq!(pdf.matches("/Subtype /Image").count(), 2);
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
    let outcome = render_nested_native_operations();
    let repeated = render_nested_native_operations();
    let pdf = pdf_text(outcome.artifact().bytes());

    assert_eq!(outcome.artifact().bytes(), repeated.artifact().bytes());
    assert_eq!(outcome.report(), repeated.report());
    // The golden record holds the bytes
    // (`facade.pdf-nested-native-operations.v1`), and `harness_pdf_viewers`
    // holds its operations and viewer rasters.
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
