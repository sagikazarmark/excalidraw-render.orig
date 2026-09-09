use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{
    CompatibilityRevision, OutputFormat, RenderOutcome, RenderRequest, Renderer, SceneDocument,
};

const EMPTY_SCENE: &[u8] =
    br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;
const PROFILE_DIGEST: &str =
    "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";

#[test]
fn valid_empty_scene_renders_a_complete_svg_artifact() {
    let outcome = render_empty_scene(OutputFormat::Svg);

    assert_outcome_contract(&outcome, OutputFormat::Svg);
    let artifact = outcome.artifact();
    let svg = std::str::from_utf8(artifact.bytes()).expect("SVG is UTF-8");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .expect("SVG is well-formed and renderable");

    assert_eq!(tree.size().width(), 20.0);
    assert_eq!(tree.size().height(), 20.0);
    assert!(svg.starts_with(concat!(
        "<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" ",
        "viewBox=\"0 0 20 20\" width=\"20\" height=\"20\">"
    )));
    assert!(svg.contains("<rect x=\"0\" y=\"0\" width=\"20\" height=\"20\" fill=\"#ffffff\"/>"));
    assert_eq!(svg.matches("<rect ").count(), 1);
}

#[test]
fn valid_empty_scene_renders_a_complete_png_artifact() {
    let outcome = render_empty_scene(OutputFormat::Png);

    assert_outcome_contract(&outcome, OutputFormat::Png);
    let artifact = outcome.artifact();
    let decoded = tiny_skia::Pixmap::decode_png(artifact.bytes()).expect("PNG pixels decode");

    assert_eq!(decoded.width(), 20);
    assert_eq!(decoded.height(), 20);
    assert_eq!(artifact.bytes()[24], 8, "PNG uses eight-bit samples");
    assert_eq!(artifact.bytes()[25], 6, "PNG uses RGBA color");
    assert_eq!(decoded.data().len(), 20 * 20 * 4);
    assert!(decoded
        .data()
        .chunks_exact(4)
        .all(|pixel| pixel == [255, 255, 255, 255]));

    let chunks = png_chunk_types(artifact.bytes());
    assert_eq!(chunks.first(), Some(b"IHDR"));
    assert_eq!(chunks.last(), Some(b"IEND"));
    assert!(chunks[1..chunks.len() - 1]
        .iter()
        .all(|chunk| chunk == b"IDAT"));
    assert!(!artifact.bytes().windows(4).any(|bytes| bytes == b"<svg"));
}

#[test]
fn valid_empty_scene_renders_a_complete_one_page_pdf_artifact() {
    let outcome = render_empty_scene(OutputFormat::Pdf);

    assert_outcome_contract(&outcome, OutputFormat::Pdf);
    let artifact = outcome.artifact();
    let bytes = artifact.bytes();
    assert!(bytes.starts_with(b"%PDF-1.7\n"));
    assert!(bytes.ends_with(b"%%EOF"));

    let pdf = pdf_text(bytes);
    assert_eq!(pdf.matches("/Type /Page\n").count(), 1);
    assert!(pdf.contains("/Count 1"));
    assert!(pdf.contains("/MediaBox [0 0 15 15]"));
    assert!(pdf.contains("/CropBox [0 0 15 15]"));
    assert!(!pdf.contains("/BleedBox"));
    assert!(!pdf.contains("/TrimBox"));
    assert!(!pdf.contains("/ArtBox"));
    assert!(!pdf.contains("/Rotate"));
    assert!(!pdf.contains("/UserUnit"));
    assert!(pdf.contains(concat!(
        "q\n0.75 0 0 -0.75 0 15 cm\n",
        "/CS1 cs\n1 1 1 scn\n0 0 20 20 re\nf\nQ"
    )));
    assert!(pdf.contains("/ColorSpace << /CS1 [/ICCBased 5 0 R] >>"));
    assert!(pdf.contains("/OutputIntents [6 0 R]"));
    assert!(!bytes.windows(4).any(|window| window == b"<svg"));
}

fn render_empty_scene(format: OutputFormat) -> RenderOutcome {
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("empty scene is valid");
    let request = RenderRequest::builder(format)
        .build()
        .expect("default request is valid");

    Renderer::new()
        .render(&scene, &request)
        .expect("empty scene produces a meaningful artifact")
}

fn assert_outcome_contract(outcome: &RenderOutcome, format: OutputFormat) {
    assert_eq!(outcome.artifact().format(), format);
    assert_eq!(outcome.artifact().media_type(), format.media_type());
    assert_eq!(
        outcome.compatibility_revision(),
        CompatibilityRevision::REVISION_1
    );
    assert_eq!(outcome.excalidraw_release(), "v0.18.1");
    assert_eq!(outcome.profile_digest(), PROFILE_DIGEST);
    assert!(outcome.report().is_empty());
}

fn png_chunk_types(bytes: &[u8]) -> Vec<[u8; 4]> {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let mut chunks = Vec::new();
    let mut offset = 8;
    while offset < bytes.len() {
        let length = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("PNG chunk has a length"),
        ) as usize;
        let chunk_type = bytes[offset + 4..offset + 8]
            .try_into()
            .expect("PNG chunk has a type");
        chunks.push(chunk_type);
        offset += 12 + length;
    }
    assert_eq!(offset, bytes.len());
    chunks
}
