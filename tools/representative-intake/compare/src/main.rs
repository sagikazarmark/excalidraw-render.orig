use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, Crop, RegionReport, SsimOutcome, VisualClass,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path, process::ExitCode};

fn class(name: &str) -> Result<VisualClass, String> {
    match name {
        "typography" => Ok(VisualClass::Typography),
        "geometry/image" => Ok(VisualClass::GeometryImage),
        "mixed-representative" => Ok(VisualClass::MixedRepresentative),
        _ => Err("unknown visual class".into()),
    }
}

fn region(report: &RegionReport) -> Value {
    json!({"width":report.width,"height":report.height,"area":report.area,
        "changed_pixels":report.changed_pixels,"changed_pixels_pass":report.changed_pixels_pass,
        "ssim_bits":match report.ssim { SsimOutcome::Score(v) => Some(format!("{:016x}",v.to_bits())), _ => None },
        "ssim_pass":report.ssim_pass})
}

fn raster(path: &Path, width: u32, height: u32) -> Result<CanonicalRgba8, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let bytes = if path.extension().is_some_and(|e| e == "zlib") {
        let limit = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or("raster size overflow")?;
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bytes, limit)
            .map_err(|e| format!("raster inflate: {e:?}"))?
    } else {
        bytes
    };
    let bytes = if path.extension().is_some_and(|e| e == "png") {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .map_err(|e| e.to_string())?;
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
        if (info.width, info.height, info.color_type, info.bit_depth)
            != (width, height, png::ColorType::Rgba, png::BitDepth::Eight)
        {
            return Err("PNG dimensions or RGBA8 layout differ".into());
        }
        pixels.truncate(info.buffer_size());
        pixels
    } else {
        bytes
    };
    CanonicalRgba8::new(width, height, bytes).map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: compare <reference> <candidate> <canonical|rust|pdfjs> <regions.json> <report.json>".into());
    }
    let mode = match args[2].as_str() {
        "canonical" => ComparisonMode::Canonical,
        "rust" => ComparisonMode::PureRustSvg,
        "pdfjs" => ComparisonMode::PdfJs,
        _ => return Err("invalid mode".into()),
    };
    let plan: Value = serde_json::from_slice(&fs::read(&args[3]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let n = |value: &Value| {
        value
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| "invalid coordinate".to_owned())
    };
    let (width, height) = (n(&plan["width"])?, n(&plan["height"])?);
    let crops = plan["crops"]
        .as_array()
        .ok_or("missing crops")?
        .iter()
        .map(|c| {
            Crop::new(
                c["id"].as_str().ok_or("crop id")?,
                n(&c["x"])?,
                n(&c["y"])?,
                n(&c["width"])?,
                n(&c["height"])?,
                class(c["class"].as_str().ok_or("class")?)?,
            )
            .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let report = compare(
        &raster(Path::new(&args[0]), width, height)?,
        &raster(Path::new(&args[1]), width, height)?,
        class(plan["whole_class"].as_str().ok_or("whole class")?)?,
        &crops,
        mode,
    )
    .map_err(|e| e.to_string())?;
    let bind = |path: &str| -> Result<Value, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        Ok(json!({"sha256":format!("{:x}",Sha256::digest(&bytes)),"bytes":bytes.len()}))
    };
    let value = json!({"schema":"excalidraw-render/representative-intake-comparison/v1", "passed": report.passed(),
        "mode":args[2],"reference":bind(&args[0])?,"candidate":bind(&args[1])?,"plan":bind(&args[3])?,
        "whole":region(&report.whole),"crops":report.crops.iter().map(|c|json!({"id":c.id,"region":region(&c.region)})).collect::<Vec<_>>()});
    // A valid out-of-budget observation is a report, not a tool failure. No
    // tolerance overrides or approval mutation are available in this command.
    fs::write(
        &args[4],
        serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}", value);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
