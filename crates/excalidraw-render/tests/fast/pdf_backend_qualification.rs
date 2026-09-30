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
use crate::common::lower_hex;

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
        lower_hex(&Sha256::digest(PROJECT_OPERATIONS)),
        "eaef67702df489f774c8eb862159098b6e220e1dac1e79bc45fada6b6a43afda"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(PROJECT_STRUCTURE)),
        "04febc1ee78a4dd01daceac2d3eeeb0d4910c2da48739bce2135b246da5f8d55"
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
    // Moved with #209 (ADR-0021 slice 1): trees reach svg2pdf through the
    // usvg 0.47 -> 0.45 SVG bridge, and one pattern coordinate rounds
    // differently in the last `f32` digit; the viewer gates hold. Moved again
    // when Krilla became the PDF writer (ADR-0021): every structure is
    // Krilla's, and both viewer gates and the structural gate hold.
    assert_eq!(
        lower_hex(&Sha256::digest(first.artifact().bytes())),
        "7f85f04d3f49585a2b85477c830ee7f69551e7cf6511460cc4e0f5a9cf1e7a31"
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
    assert_eq!(
        lower_hex(&Sha256::digest(FALLBACK_STRUCTURE)),
        "7afc81f4873227495ecca61ffa82b86799cad7a5b29d4f2bae4205d38f665740"
    );
    let first = render_isolated_fallback();
    let second = render_isolated_fallback();

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    // Moved with #209 (ADR-0021 slice 1): the reparsed tree wraps the
    // fallback image in an identity `q 1 0 0 1 0 0 cm Q` group; the image
    // bytes and the viewer gates are unchanged. Moved again when Krilla became
    // the PDF writer (ADR-0021); the fallback's pixels are unchanged.
    assert_eq!(
        lower_hex(&Sha256::digest(first.artifact().bytes())),
        "767770926e5f9f8f0e5eafb9a00e4ae7073a6ca401af589f6d0944a6746e8a23"
    );
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
    assert_eq!(
        lower_hex(&Sha256::digest(NESTED_NATIVE_OPERATIONS)),
        "6aebb7ca70ae3742d23597ca3ff109341219269d70d134ace5157d3112bfa257"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(NESTED_NATIVE_STRUCTURE)),
        "bf55a897acef1a0bd7946c9b88205445292f8d0a419574930d60b330fd80d146"
    );
    let outcome = render_nested_native_operations();
    let repeated = render_nested_native_operations();
    let pdf = pdf_text(outcome.artifact().bytes());

    assert_eq!(outcome.artifact().bytes(), repeated.artifact().bytes());
    assert_eq!(outcome.report(), repeated.report());
    // Moved with #209 (ADR-0021 slice 1): after the usvg 0.47 -> 0.45 SVG
    // bridge, path coordinates differ in the last `f32` digit, `-0` and
    // `4.4e-16` residues print as `0`, and one identity group is added
    // (three operators); the viewer gates hold. Moved again when Krilla became
    // the PDF writer (ADR-0021): the nested tree is drawn in place, and both
    // viewer gates and the structural gate hold. Moved with usvg 0.48 (#209):
    // it outlines the nested text with skrifa instead of ttf-parser, whose
    // pen closed each glyph contour with an explicit line back to its start
    // before `h`; one filled glyph path loses that segment, which closepath
    // draws anyway, so the fill is unchanged.
    assert_eq!(
        lower_hex(&Sha256::digest(outcome.artifact().bytes())),
        "271f105c692f15d8970dd10829b3f6ef2bf9b4755bd6783de3e45a65ddcab802"
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
