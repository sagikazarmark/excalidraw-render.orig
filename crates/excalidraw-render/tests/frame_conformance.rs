use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
mod common;

use common::pdf_harness::render_viewer;
use excalidraw_render::{FrameRenderRequest, OutputFormat, Renderer, SceneDocument};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use png::{BitDepth, ColorType, Decoder, Transformations};
use tiny_skia::{Pixmap, Transform};

#[test]
fn frame_svg_and_png_match_the_pinned_official_evidence() {
    let case = repository_root().join("tests/conformance/cases/focused/frame-export");
    let scene = SceneDocument::parse(&fs::read(case.join("scene.excalidraw")).unwrap())
        .expect("approved frame scene is admitted");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(case.join("manifest.json")).unwrap()).unwrap();
    let targets = manifest["targets"]
        .as_array()
        .expect("reference targets exist");
    let frames = scene.frames().collect::<Vec<_>>();
    assert_eq!(targets.len(), frames.len());
    let renderer = Renderer::new();

    for (index, (target, frame)) in targets.iter().zip(frames).enumerate() {
        let ordinal = u32::try_from(index + 1).unwrap();
        assert_eq!(target["frame_ordinal"], ordinal);
        assert_eq!(target["frame_kind"], frame.kind().as_str());
        assert_eq!(target["frame_name"].as_str(), frame.name());
        let png_width = u32::try_from(
            target["outputs"]["png"]["artifact"]["width"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let png_height = u32::try_from(
            target["outputs"]["png"]["artifact"]["height"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let svg_width = u32::try_from(
            target["outputs"]["svg"]["artifact"]["width"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let svg_height = u32::try_from(
            target["outputs"]["svg"]["artifact"]["height"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let evidence = case.join(format!("evidence/frame-{ordinal}"));
        let visual_class = match ordinal {
            4 | 5 => VisualClass::MixedRepresentative,
            7 => VisualClass::Typography,
            _ => VisualClass::GeometryImage,
        };

        let png = renderer
            .render_frame(
                frame,
                &FrameRenderRequest::builder(OutputFormat::Png)
                    .build()
                    .expect("PNG frame request is valid"),
            )
            .unwrap_or_else(|error| panic!("frame {ordinal} renders to PNG: {error}"));
        assert_eq!(
            serde_json::to_vec(png.report()).expect("PNG report serializes"),
            fs::read(case.join("expected-report.json")).unwrap(),
            "frame {ordinal} PNG report differs"
        );
        let candidate_png = decode_png(png.artifact().bytes());
        let reference_png = CanonicalRgba8::new(
            png_width,
            png_height,
            fs::read(evidence.join("official-png.rgba")).unwrap(),
        )
        .expect("official PNG evidence is canonical RGBA8");
        let png_comparison = compare(
            &reference_png,
            &candidate_png,
            visual_class,
            &[],
            ComparisonMode::Canonical,
        )
        .expect("PNG dimensions match");
        assert!(
            png_comparison.passed(),
            "frame {ordinal} PNG differs: changed_pixels={}, ssim={:?}",
            png_comparison.whole.changed_pixels,
            png_comparison.whole.ssim
        );

        let svg = renderer
            .render_frame(
                frame,
                &FrameRenderRequest::builder(OutputFormat::Svg)
                    .build()
                    .expect("SVG frame request is valid"),
            )
            .unwrap_or_else(|error| panic!("frame {ordinal} renders to SVG: {error}"));
        assert_eq!(svg.report(), png.report());
        let candidate_svg = std::str::from_utf8(svg.artifact().bytes()).expect("SVG is UTF-8");
        assert!(
            candidate_svg.contains("viewBox=\"0 0 "),
            "frame {ordinal} SVG has target-local bounds"
        );
        assert!(!candidate_svg.contains("data-frame-name"));
        assert!(!candidate_svg.contains("data-frame-outline"));

        let candidate_svg = CanonicalRgba8::new(
            svg_width,
            svg_height,
            rasterize_svg(svg.artifact().bytes(), svg_width, svg_height),
        )
        .expect("candidate SVG raster is canonical RGBA8");
        let official_svg = fs::read(evidence.join("official.svg")).unwrap();
        let reference_svg = CanonicalRgba8::new(
            svg_width,
            svg_height,
            rasterize_svg(&official_svg, svg_width, svg_height),
        )
        .expect("official SVG raster is canonical RGBA8");
        let svg_comparison = compare(
            &reference_svg,
            &candidate_svg,
            visual_class,
            &[],
            ComparisonMode::PureRustSvg,
        )
        .expect("SVG dimensions match");
        assert!(
            svg_comparison.passed(),
            "frame {ordinal} SVG differs: changed_pixels={}, ssim={:?}",
            svg_comparison.whole.changed_pixels,
            svg_comparison.whole.ssim
        );

        let pdf = renderer
            .render_frame(
                frame,
                &FrameRenderRequest::builder(OutputFormat::Pdf)
                    .build()
                    .expect("PDF frame request is valid"),
            )
            .unwrap_or_else(|error| panic!("frame {ordinal} renders to PDF: {error}"));
        assert_eq!(pdf.report(), png.report());
        let repeated_pdf = renderer
            .render_frame(
                frame,
                &FrameRenderRequest::builder(OutputFormat::Pdf)
                    .build()
                    .expect("repeated PDF frame request is valid"),
            )
            .unwrap_or_else(|error| panic!("frame {ordinal} repeats as PDF: {error}"));
        assert_eq!(pdf.artifact().bytes(), repeated_pdf.artifact().bytes());
        let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
        assert!(
            pdf.contains("/MediaBox [0 0 "),
            "frame {ordinal} has one PDF page"
        );
        assert!(
            pdf.contains("/CropBox [0 0 "),
            "frame {ordinal} clips to its target page"
        );
        assert!(
            pdf.contains("/Count 1"),
            "frame {ordinal} PDF has exactly one page"
        );
        assert!(
            !pdf.contains("/Subtype /Image"),
            "frame {ordinal} remains vector-first"
        );
        if ordinal == 1 {
            assert!(pdf.contains("0.8784313725490196 0.19215686274509805 0.19215686274509805"));
            assert!(pdf.contains("0.09803921568627451 0.44313725490196076 0.7607843137254902"));
            assert!(pdf.contains("0.611764705882353 0.21176470588235294 0.7098039215686275"));
            assert!(!pdf.contains("0.1843137254901961 0.6196078431372549 0.26666666666666666"));
            assert!(!pdf.contains("0.9411764705882353 0.5490196078431373 0"));
        }
        if ordinal == 3 {
            assert!(!pdf.contains("0.047058823529411764 0.6509803921568628 0.47058823529411764"));
        }
        if ordinal == 4 {
            assert!(pdf.contains("0.047058823529411764 0.6509803921568628 0.47058823529411764"));
        }
        if ordinal == 7 {
            assert!(
                pdf.contains("/ToUnicode"),
                "bound frame text remains selectable"
            );
            assert!(pdf.contains("/Subtype /Link"));
            assert!(pdf.contains("/URI (https://example.com/frame)"));
        }
    }
}

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
    let renderer = Renderer::new();
    let stem = format!("frame-pdf-candidate-{}", std::process::id());

    for (index, (target, frame)) in targets.iter().zip(scene.frames()).enumerate() {
        let ordinal = index + 1;
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
        let candidate_path = format!("target/{stem}-{ordinal}.pdf");
        let pdf = renderer
            .render_frame(
                frame,
                &FrameRenderRequest::builder(OutputFormat::Pdf)
                    .build()
                    .expect("PDF frame request is valid"),
            )
            .expect("frame renders to PDF");
        fs::write(root.join(&candidate_path), pdf.artifact().bytes())
            .expect("write renderer-owned frame PDF under target");
        let visual_class = match ordinal {
            4 | 5 => VisualClass::MixedRepresentative,
            7 => VisualClass::Typography,
            _ => VisualClass::GeometryImage,
        };
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
            let output = format!("target/{stem}-{ordinal}-{viewer}.rgba");
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
            let report = compare(&reference, &candidate, visual_class, &[], mode)
                .expect("frame PDF and official SVG dimensions agree");
            assert!(
                report.passed(),
                "frame {ordinal} differs in {viewer}: changed_pixels={}, ssim={:?}",
                report.whole.changed_pixels,
                report.whole.ssim
            );
        }
    }
}

fn decode_png(bytes: &[u8]) -> CanonicalRgba8 {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    assert_eq!(reader.info().color_type, ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, BitDepth::Eight);
    let mut rgba = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
    rgba.truncate(frame.buffer_size());
    CanonicalRgba8::new(frame.width, frame.height, rgba).expect("PNG is canonical RGBA8")
}

fn rasterize_svg(bytes: &[u8], width: u32, height: u32) -> Vec<u8> {
    let svg = std::str::from_utf8(bytes).expect("SVG is UTF-8");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .expect("SVG is accepted by the pure-Rust viewer");
    let mut pixmap = Pixmap::new(width, height).expect("frame raster dimensions are valid");
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    pixmap.data().to_vec()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}
