use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass};
use serde_json::json;
use std::{fs, io::Cursor};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args[1] == "decode" {
        let mut decoder = png::Decoder::new(Cursor::new(fs::read(&args[2]).unwrap()));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().unwrap();
        let mut data = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut data).unwrap();
        data.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => data,
            png::ColorType::Rgb => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
            png::ColorType::Grayscale => data.iter().flat_map(|p| [*p, *p, *p, 255]).collect(),
            png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
            _ => panic!("unexpected color type"),
        };
        let image = CanonicalRgba8::new(info.width, info.height, rgba).unwrap();
        fs::write(&args[3], image.rgba()).unwrap();
        println!("{}", json!({"width": info.width, "height": info.height}));
        return;
    }
    let width = args[1].parse().unwrap();
    let height = args[2].parse().unwrap();
    let a = CanonicalRgba8::new(width, height, fs::read(&args[3]).unwrap()).unwrap();
    let b = CanonicalRgba8::new(width, height, fs::read(&args[4]).unwrap()).unwrap();
    let mode = if args.get(5).is_some_and(|v| v == "pdfjs") { ComparisonMode::PdfJs } else { ComparisonMode::Canonical };
    let report = compare(&a, &b, VisualClass::GeometryImage, &[], mode).unwrap();
    let SsimOutcome::Score(score) = report.whole.ssim else { panic!("nonfinite SSIM") };
    println!("{}", json!({"changed_pixels": report.whole.changed_pixels, "ssim": score, "ssim_bits": format!("{:016x}", score.to_bits()), "canonical_pass": report.passed()}));
}
