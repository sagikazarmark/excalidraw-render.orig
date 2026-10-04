//! A cluster that shapes to several glyphs gives each glyph the cluster's
//! whole text, which PDF hands Krilla per glyph for `ToUnicode` and
//! `ActualText`. The writer keeps one copy per cluster rather than one per
//! glyph, and the comparisons Krilla still makes per glyph are charged as work
//! before encoding (spec 13.2), so a pathological cluster fails cleanly
//! instead of taking time and memory quadratic in its length.

use std::time::{Duration, Instant};

use excalidraw_render::{ErrorCode, OutputFormat, RenderRequest, Renderer, SceneDocument};
use sha2::{Digest, Sha256};

use crate::common::lower_hex;

fn text_scene(texts: &[(&str, i32)]) -> SceneDocument {
    let elements = texts
        .iter()
        .enumerate()
        .map(|(index, (text, family))| {
            serde_json::json!({
                "id": format!("text-{index}"), "type": "text", "x": 0, "y": index * 40,
                "width": 300, "height": 25, "fontSize": 20, "fontFamily": family,
                "text": text, "updated": 1
            })
        })
        .collect::<Vec<_>>();
    let source = serde_json::json!({
        "type": "excalidraw", "version": 2, "elements": elements,
        "appState": {}, "files": {}
    });
    SceneDocument::parse(source.to_string().as_bytes()).expect("text scene is admitted")
}

fn request(format: OutputFormat) -> RenderRequest {
    RenderRequest::builder(format)
        .build()
        .expect("default request is valid")
}

#[test]
fn multi_glyph_cluster_pdf_bytes_are_unchanged_by_sharing_cluster_text() {
    let joined = format!("a{}b", "\u{200d}".repeat(300));
    let scene = text_scene(&[
        (&joined, 5),
        ("e\u{301}\u{302} o\u{308}", 1),
        ("👨\u{200d}👩\u{200d}👧 family", 5),
        ("مَرْحَبًا", 5),
        ("a\u{fe0f}\u{200d}b", 2),
    ]);

    let outcome = Renderer::new()
        .render(&scene, &request(OutputFormat::Pdf))
        .expect("multi-glyph clusters render");

    // The digest of the bytes the writer produced while it still copied the
    // cluster text once per glyph.
    assert_eq!(
        lower_hex(&Sha256::digest(outcome.artifact().bytes())),
        "3437b262048b475c5f4ab140709b6840fabdf73269a055c66d7bd9f57d82226d"
    );
}

#[test]
fn a_cluster_of_many_glyphs_fails_pdf_cleanly_and_quickly() {
    let joined = format!("a{}", "\u{200d}".repeat(100_000));
    let scene = text_scene(&[(&joined, 5)]);

    let started = Instant::now();
    let error = Renderer::new()
        .render(&scene, &request(OutputFormat::Pdf))
        .expect_err("the repeated cluster text exceeds the total work ceiling");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED);
    assert!(started.elapsed() < Duration::from_secs(10));

    // SVG and PNG repeat nothing per glyph and still draw it.
    for format in [OutputFormat::Svg, OutputFormat::Png] {
        Renderer::new()
            .render(&scene, &request(format))
            .expect("other formats render the cluster");
    }
}

#[test]
fn a_moderate_multi_glyph_cluster_stays_within_the_pdf_budget() {
    let joined = format!("a{}", "\u{200d}".repeat(2_000));
    Renderer::new()
        .render(&text_scene(&[(&joined, 5)]), &request(OutputFormat::Pdf))
        .expect("a moderate cluster renders");
}
