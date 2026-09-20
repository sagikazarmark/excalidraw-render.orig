use excalidraw_render_compare::{compare, compare_with_limits, CanonicalRgba8, ComparisonLimits, ComparisonMode, SsimOutcome, VisualClass};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::Path};

fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args[1] == "canonical-sha256" {
        let value: Value = serde_json::from_slice(&fs::read(&args[2]).unwrap()).unwrap();
        println!("sha256:{}", sha(&serde_json::to_vec(&value).unwrap()));
        return;
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    let cases: Value = serde_json::from_slice(&fs::read(out.join("cases.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    for row in cases.as_array().unwrap() {
        let case = row["case"].as_str().unwrap().strip_prefix("focused.").unwrap().strip_suffix(".v1").unwrap();
        let dir = root.join(row["path"].as_str().unwrap());
        let manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        let raw = &manifest["variants"][0]["formats"]["png"]["raw_raster"];
        let (width, height) = (raw["width"].as_u64().unwrap() as u32, raw["height"].as_u64().unwrap() as u32);
        let reference_bytes = fs::read(dir.join(raw["path"].as_str().unwrap())).unwrap();
        assert_eq!(sha(&reference_bytes), raw["sha256"].as_str().unwrap());
        let reference = CanonicalRgba8::new(width, height, reference_bytes).unwrap();
        let png = fs::read(out.join(format!("native/{case}/native.png"))).unwrap();
        let mut decoder = png::Decoder::new(Cursor::new(png));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).unwrap();
        pixels.truncate(info.buffer_size());
        assert_eq!((info.width, info.height), (width, height));
        let rgba = match info.color_type {
            png::ColorType::Rgba => pixels,
            png::ColorType::Rgb => pixels.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
            _ => panic!("unexpected native PNG format"),
        };
        assert_eq!(rgba, fs::read(out.join(format!("diagnosis/{case}-current.rgba"))).unwrap());
        for mode in ["current", "legacy"] {
            let pixels = if mode == "current" { rgba.clone() } else { fs::read(out.join(format!("diagnosis/{case}-legacy.rgba"))).unwrap() };
            let candidate = CanonicalRgba8::new(width, height, pixels).unwrap();
            let report = compare(&reference, &candidate, VisualClass::GeometryImage, &[], ComparisonMode::Canonical).unwrap();
            let SsimOutcome::Score(ssim) = report.whole.ssim else { panic!("missing SSIM") };
            assert_eq!(report.whole.changed_pixels, row[mode]["changed"].as_u64().unwrap());
            assert_eq!(format!("{:016x}", ssim.to_bits()), row[mode]["ssim_bits"].as_str().unwrap());
            assert_eq!(report.passed(), row[mode]["canonical"].as_bool().unwrap());
            let effective_pass = if mode == "current" && !row["proposed_exceptions"].as_array().unwrap().is_empty() {
                let scope = &row["proposed_exceptions"][0];
                let limits = ComparisonLimits::tolerance_exception(scope["max_changed_pixels"].as_u64().unwrap(), f64::from_bits(u64::from_str_radix(scope["minimum_ssim_bits"].as_str().unwrap(), 16).unwrap())).unwrap();
                compare_with_limits(&reference, &candidate, limits, &[], ComparisonMode::Canonical).unwrap().passed()
            } else { report.passed() };
            if mode == "current" { assert!(effective_pass); }
            rows.push(json!({"case":row["case"],"mode":mode,"width":width,"height":height,"changed_pixels":report.whole.changed_pixels,"ssim_bits":format!("{:016x}",ssim.to_bits()),"canonical_pass":report.passed(),"effective_pass":effective_pass,"rgba_sha256":sha(candidate.rgba())}));
        }
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
