//! Contract tests for the canonical PDF viewer adapters behind
//! `tools/pdf-harness/run-pdfjs.sh` and `tools/pdf-harness/run-pdfium.sh`.
//!
//! Both adapters render one page at exactly `96 / 72` scale with the page
//! origin at the canvas origin onto a canvas of exactly the caller-supplied
//! size. A page is admitted when each of its 96 DPI dimensions is within one
//! pixel of the supplied size; the page is never scaled, stretched, or offset
//! to fit, device pixels beyond the canvas are discarded, and canvas pixels the
//! page does not cover stay transparent.

use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};

use crate::common::pdf_harness::try_render_viewer;
use crate::common::repository_root;
use crate::common::scratch::ScratchDir;

const VIEWERS: [(&str, &str, ComparisonMode); 2] = [
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
];

/// The fractional page is 84.09 x 62.88 pt, that is 112.12 x 83.84 px at 96
/// DPI, and carries one opaque black square whose device edges land on whole
/// pixels: x from 20 to 60 and y from 20 to 60.
const FRACTIONAL_PAGE_PT: (f64, f64) = (84.09, 62.88);
const PIXELS_PER_POINT: f64 = 96.0 / 72.0;
const SQUARE_DEVICE_RANGE: std::ops::Range<u32> = 20..60;

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harnesses"]
fn adapters_place_a_fractional_page_at_exact_scale_on_the_supplied_canvas() {
    let root = repository_root();
    let scratch = ScratchDir::new("pdf-viewer-adapters-fractional");
    let candidate = scratch.write("fractional.pdf", &fractional_page_pdf());

    for (width, height) in [(112, 84), (113, 83)] {
        let mut rasters = Vec::new();
        for (viewer, script, mode) in VIEWERS {
            let output = scratch.relative(&format!("{width}x{height}-{viewer}.rgba"));
            let render =
                try_render_viewer(&root, viewer, script, &candidate, &output, width, height)
                    .unwrap_or_else(|diagnostics| {
                        panic!(
                            "{viewer} rejected an admitted {width}x{height} canvas:\n{diagnostics}"
                        )
                    });
            assert_eq!(
                render.summary["width"], width,
                "{viewer} summary width on {width}x{height}"
            );
            assert_eq!(
                render.summary["height"], height,
                "{viewer} summary height on {width}x{height}"
            );
            assert_eq!(
                render.summary["byte_length"],
                width * height * 4,
                "{viewer} summary byte length on {width}x{height}"
            );
            assert_square_placement(&render.rgba, width, height, viewer);
            rasters.push((
                viewer,
                mode,
                CanonicalRgba8::new(width, height, render.rgba)
                    .expect("canonical PDF viewer output is valid"),
            ));
        }
        let raster = |name: &str| {
            rasters
                .iter()
                .find(|(viewer, _, _)| *viewer == name)
                .map(|(_, mode, raster)| (*mode, raster))
                .unwrap_or_else(|| panic!("{name} rendered the fractional page"))
        };
        let (pdfjs_mode, pdfjs) = raster("pdfjs");
        let (_, pdfium) = raster("pdfium");
        let report = compare(pdfium, pdfjs, VisualClass::GeometryImage, &[], pdfjs_mode)
            .expect("both viewers rendered the supplied canvas size");
        assert!(
            report.passed(),
            "viewers disagree on the fractional page at {width}x{height}: changed_pixels={}, ssim={:?}",
            report.whole.changed_pixels,
            report.whole.ssim
        );
    }
}

#[test]
#[ignore = "requires the exact-pinned PDFium and PDF.js harnesses"]
fn adapters_reject_a_supplied_size_a_pixel_or_more_off_the_page() {
    let root = repository_root();
    let scratch = ScratchDir::new("pdf-viewer-adapters-rejected");
    let fractional = scratch.write("fractional.pdf", &fractional_page_pdf());
    // The qualification control is an integral 200x120 px page; the fractional
    // page is 112.12x83.84 px, so 111 and 85 miss it by more than a pixel.
    let integral =
        "tests/conformance/pdf-viewer-qualification/rectangle-outline/candidate-control.pdf";
    let fractional_px = (
        FRACTIONAL_PAGE_PT.0 * PIXELS_PER_POINT,
        FRACTIONAL_PAGE_PT.1 * PIXELS_PER_POINT,
    );
    let attempts = [
        (integral, (200.0, 120.0), 199, 120),
        (integral, (200.0, 120.0), 200, 121),
        (fractional.as_str(), fractional_px, 111, 84),
        (fractional.as_str(), fractional_px, 112, 85),
    ];

    for (candidate, page_size, width, height) in attempts {
        for (viewer, script, _) in VIEWERS {
            let output = scratch.relative(&format!("{width}x{height}-{viewer}.rgba"));
            let diagnostics =
                try_render_viewer(&root, viewer, script, candidate, &output, width, height)
                    .err()
                    .unwrap_or_else(|| {
                        panic!("{viewer} admitted a {width}x{height} canvas for {candidate}")
                    });
            let supplied = format!("expected {width}x{height}");
            assert!(
                diagnostics.contains(&supplied),
                "{viewer} rejection names the supplied size:\n{diagnostics}"
            );
            let (named_width, named_height) = named_page_size(&diagnostics).unwrap_or_else(|| {
                panic!("{viewer} rejection names the page's 96 DPI size:\n{diagnostics}")
            });
            assert!(
                (named_width - page_size.0).abs() < 1e-3
                    && (named_height - page_size.1).abs() < 1e-3,
                "{viewer} names the page as {named_width}x{named_height} px, expected {}x{}",
                page_size.0,
                page_size.1
            );
            assert!(
                !root.join(&output).exists(),
                "{viewer} wrote a canvas it rejected: {output}"
            );
        }
    }
}

/// Extracts `<width>x<height>` from an adapter's `page is <width>x<height> px
/// at 96 DPI` rejection.
fn named_page_size(diagnostics: &str) -> Option<(f64, f64)> {
    let (before, _) = diagnostics.split_once(" px at 96 DPI")?;
    let (_, size) = before.rsplit_once(' ')?;
    let (width, height) = size.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

/// Every pixel is either the opaque black square or fully transparent black,
/// which pins exact scale, the origin, and cropping at once: a page stretched
/// to the canvas or offset by even a fraction of a pixel bleeds into the
/// square's neighbouring column or row.
fn assert_square_placement(rgba: &[u8], width: u32, height: u32, viewer: &str) {
    assert_eq!(
        rgba.len(),
        usize::try_from(width * height * 4).unwrap(),
        "{viewer} wrote a {width}x{height} canvas"
    );
    for y in 0..height {
        for x in 0..width {
            let index = usize::try_from((y * width + x) * 4).unwrap();
            let pixel = &rgba[index..index + 4];
            let inside = SQUARE_DEVICE_RANGE.contains(&x) && SQUARE_DEVICE_RANGE.contains(&y);
            let expected: [u8; 4] = if inside { [0, 0, 0, 255] } else { [0, 0, 0, 0] };
            assert_eq!(
                pixel, expected,
                "{viewer} pixel ({x}, {y}) on a {width}x{height} canvas"
            );
        }
    }
}

/// A minimal PDF 1.7 document: one fractional page with one pixel-grid square.
fn fractional_page_pdf() -> Vec<u8> {
    let (page_width, page_height) = FRACTIONAL_PAGE_PT;
    // The square spans 15..45 pt horizontally and, measured from the page top,
    // 15..45 pt vertically; at 96 DPI both become 20..60 device pixels.
    let content = format!("0 0 0 rg 15 {:.2} 30 30 re f", page_height - 45.0);
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width:.2} {page_height:.2}] /Resources << >> /Contents 4 0 R >>"
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}
