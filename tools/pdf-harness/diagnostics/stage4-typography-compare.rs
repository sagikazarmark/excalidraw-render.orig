//! Diagnostic driver of the unmodified canonical comparator; exit 1 means red.
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "reference.rgba actual.rgba pdfium|pdfjs");
    let reference = std::fs::read(&args[1]).unwrap();
    let actual = std::fs::read(&args[2]).unwrap();
    let mode = match args[3].as_str() {
        "pdfium" => ComparisonMode::Canonical,
        "pdfjs" => ComparisonMode::PdfJs,
        _ => panic!("unknown viewer"),
    };
    let mut rows = Vec::new();
    let mut failed = false;
    for crop in [false, true] {
        let raster = |bytes: &[u8]| {
            assert_eq!(bytes.len(), 200 * 120 * 4);
            if crop {
                let data = (12..53)
                    .flat_map(|y| bytes[(y * 200 + 10) * 4..(y * 200 + 134) * 4].iter().copied())
                    .collect();
                CanonicalRgba8::new(124, 41, data).unwrap()
            } else {
                CanonicalRgba8::new(200, 120, bytes.to_vec()).unwrap()
            }
        };
        let class = if crop { VisualClass::Typography } else { VisualClass::GeometryImage };
        let report = compare(&raster(&reference), &raster(&actual), class, &[], mode).unwrap();
        let SsimOutcome::Score(ssim) = report.whole.ssim else { panic!("non-finite SSIM") };
        rows.push(format!(
            "{{\"region\":\"{}\",\"changed_pixels\":{},\"ssim\":{},\"ssim_bits\":\"{:016x}\",\"passed\":{}}}",
            if crop { "typography" } else { "whole" },
            report.whole.changed_pixels, ssim, ssim.to_bits(), report.passed()
        ));
        failed |= !report.passed();
    }
    println!("[{}]", rows.join(","));
    if failed { std::process::exit(1); }
}
