use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let reference_path = PathBuf::from(required_arg(&mut args, "reference RGBA path")?);
    let candidate_path = PathBuf::from(required_arg(&mut args, "candidate RGBA path")?);
    let viewer = required_arg(&mut args, "PDF viewer")?;
    let expected = required_arg(&mut args, "expected outcome")?;
    let output_path = PathBuf::from(required_arg(&mut args, "report path")?);
    if let Some(argument) = args.next() {
        return Err(format!("unexpected argument {argument:?}; {}", usage()));
    }
    let mode = match viewer.as_str() {
        "pdfium" => ComparisonMode::Canonical,
        "pdfjs" => ComparisonMode::PdfJs,
        _ => return Err(format!("unknown PDF viewer {viewer:?}")),
    };
    let expected_pass = match expected.as_str() {
        "pass" => true,
        "fail" => false,
        _ => {
            return Err(format!(
                "expected outcome must be pass or fail; {}",
                usage()
            ))
        }
    };
    let reference = read_raster(&reference_path)?;
    let candidate = read_raster(&candidate_path)?;
    let report = compare(
        &reference,
        &candidate,
        VisualClass::GeometryImage,
        &[],
        mode,
    )
    .map_err(|error| format!("comparison failed: {error}"))?;
    let SsimOutcome::Score(ssim) = report.whole.ssim else {
        return Err("comparison did not produce a finite SSIM score".to_owned());
    };
    let passed = report.passed();
    let bytes = format!(
        "{{\"changed_pixels\":{},\"height\":{},\"mode\":\"{}\",\"passed\":{},\"ssim_bits\":\"{:016x}\",\"width\":{}}}\n",
        report.whole.changed_pixels,
        report.whole.height,
        viewer,
        passed,
        ssim.to_bits(),
        report.whole.width,
    );
    fs::write(&output_path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    if passed != expected_pass {
        return Err(format!(
            "comparison outcome was {passed}, expected {expected_pass}"
        ));
    }
    Ok(())
}

fn read_raster(path: &PathBuf) -> Result<CanonicalRgba8, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    CanonicalRgba8::new(200, 120, bytes)
        .map_err(|error| format!("invalid canonical raster {}: {error}", path.display()))
}

fn required_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing {name}; {}", usage()))
}

fn usage() -> String {
    "usage: compare_pdf_rgba <reference.rgba> <candidate.rgba> <pdfium|pdfjs> <pass|fail> <report.json>"
        .to_owned()
}
