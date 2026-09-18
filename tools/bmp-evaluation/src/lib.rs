//! Bounded differential experiment for issue #183; no production codec switch.
use std::io::Cursor;

use image::{codecs::bmp::BmpDecoder, metadata::Orientation, ColorType, ImageDecoder};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[path = "../../../crates/excalidraw-render/src/render/asset/bmp.rs"]
mod current_bmp;
mod current_exif {
    include!(concat!(env!("OUT_DIR"), "/current_exif.rs"));
}
mod fixtures;
#[allow(dead_code)]
#[path = "../../../crates/excalidraw-render/tests/common/image_scene.rs"]
mod image_scene;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn error_kind(error: image::ImageError) -> &'static str {
    match error {
        image::ImageError::Decoding(_) => "decoding",
        image::ImageError::Unsupported(_) => "unsupported",
        image::ImageError::IoError(_) => "io",
        image::ImageError::Limits(_) => "limits",
        image::ImageError::Parameter(_) => "parameter",
        image::ImageError::Encoding(_) => "encoding",
    }
}

// Independent tiny experimental cap, not the renderer's resource policy.
// Raw candidates bypass current validation to expose acceptance differences.
fn candidate(bytes: &[u8], admitted: Option<(u32, u32)>) -> Result<(u32, u32, Vec<u8>), String> {
    let check = |(width, height): (u32, u32)| {
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 65536 {
            Err("probe:dimension-cap".to_owned())
        } else {
            Ok(())
        }
    };
    if let Some(dimensions) = admitted {
        check(dimensions)?;
    }
    let decoder = BmpDecoder::new(Cursor::new(bytes))
        .map_err(|e| format!("constructor:{}", error_kind(e)))?;
    let (width, height) = decoder.dimensions();
    check((width, height))?;
    if admitted.is_some_and(|dimensions| dimensions != (width, height)) {
        return Err("probe:dimension-mismatch".into());
    }
    let channels = match decoder.color_type() {
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        _ => return Err("probe:unexpected-color-type".into()),
    };
    let size = usize::try_from(decoder.total_bytes()).map_err(|_| "probe:size")?;
    let rgba_size =
        usize::try_from(u64::from(width) * u64::from(height) * 4).map_err(|_| "probe:size")?;
    if size != rgba_size / 4 * channels {
        return Err("probe:output-size-mismatch".into());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(rgba_size)
        .map_err(|_| "probe:allocation")?;
    output.resize(size, 0);
    decoder
        .read_image(&mut output)
        .map_err(|e| format!("decode:{}", error_kind(e)))?;
    // Expand RGB backwards into the same caller-admitted allocation.
    // This probe records raw colors,
    // before finish_rgba's transparent-color zeroing and premultiplication.
    if channels == 3 {
        let pixels = size / 3;
        output.resize(pixels * 4, 0);
        for i in (0..pixels).rev() {
            output.copy_within(i * 3..i * 3 + 3, i * 4);
            output[i * 4 + 3] = 255;
        }
    }
    Ok((width, height, output))
}

fn admitted_candidate(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let info = current_bmp::validate(bytes).map_err(|_| "validation:BmpDecodeFailed")?;
    candidate(bytes, Some((info.width, info.height)))
}

fn pixels(result: Result<(u32, u32, Vec<u8>), String>) -> Value {
    match result {
        Ok((width, height, rgba)) => json!({"width":width,"height":height,"rgba":hex(&rgba)}),
        Err(error) => json!({"error":error}),
    }
}

fn current(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let info = current_bmp::validate(bytes).map_err(|_| "validation:BmpDecodeFailed")?;
    let rgba = current_bmp::decode(bytes, &info).map_err(|_| "decode:BmpDecodeFailed")?;
    Ok((info.width, info.height, rgba))
}

fn render_paths(name: &str, bytes: &[u8]) -> Value {
    use excalidraw_render::{OutputFormat, RenderRequest, Renderer};
    let info = current_bmp::validate(bytes).unwrap();
    let (width, height, rgba) = admitted_candidate(bytes).unwrap();
    assert_eq!((width, height), (info.width, info.height));
    // Test-only bridge to the public renderer. It exercises candidate-derived
    // pixels in PNG/PDF, but does not pretend to qualify a BMP cache adapter.
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
    }
    let original = image_scene::image_scene(bytes, "image/bmp", width * 8, height * 8);
    let derived = image_scene::image_scene(&png, "image/png", width * 8, height * 8);
    let mut artifacts = Vec::new();
    for (path, scene, formats) in [
        (
            "original-bmp",
            &original,
            vec![OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf],
        ),
        (
            "candidate-png-bridge",
            &derived,
            vec![OutputFormat::Png, OutputFormat::Pdf],
        ),
    ] {
        for format in formats {
            let request = RenderRequest::builder(format).build().unwrap();
            let outcome = Renderer::new().render(scene, &request).unwrap();
            assert!(outcome.report().diagnostics().is_empty());
            let artifact = outcome.artifact().bytes();
            if format == OutputFormat::Svg {
                assert!(std::str::from_utf8(artifact)
                    .unwrap()
                    .contains(&image_scene::data_url("image/bmp", bytes)));
            }
            artifacts.push(json!({"path":path,"format":format!("{format:?}"),"length":artifact.len(),"sha256":digest(artifact)}));
        }
    }
    json!({"case":name,"artifacts":artifacts,"svg_original_octets":true})
}

fn resource_probes() -> Value {
    let bytes = fixtures::bmp(40, 2, 1, 8, 0, &[], &[0, 1, 0, 0]);
    let mut decoder = BmpDecoder::new(Cursor::new(&bytes)).unwrap();
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(0);
    let accepted = decoder.set_limits(limits).is_ok();
    let mut output = [0; 6];
    let decoded = decoder.read_image(&mut output).is_ok();
    let mut decoder = BmpDecoder::new(Cursor::new(&bytes)).unwrap();
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1);
    let dimension_error = decoder.set_limits(limits).err().map(error_kind);
    // Constructor-only tests: no output allocation for these dimensions.
    let dimensions: Vec<_> = [(1, 65536), (1, -65536), (65536, 1)]
        .iter()
        .map(|&(width, height)| {
            let bytes = fixtures::bmp(40, width, height, 24, 0, &[], &[]);
            let result = match BmpDecoder::new(Cursor::new(&bytes)) {
                Ok(decoder) => {
                    json!({"dimensions":decoder.dimensions(),"total_bytes":decoder.total_bytes()})
                }
                Err(error) => json!({"error":error_kind(error)}),
            };
            json!({"width":width,"height":height,"constructor":result})
        })
        .collect();
    json!({"max_alloc_zero_accepted":accepted,"max_alloc_zero_decode_succeeded":decoded,
        "strict_width_error":dimension_error,"constructor_only":dimensions})
}

fn mask_sweeps() -> Vec<Value> {
    // Every 16-bit source word, including the ignored high bit for BI_RGB 555.
    // Small per-case rows above carry exact pixels; these exhaustive sweeps
    // report byte digests, difference counts and the first counterexample.
    let source: Vec<u8> = (0..=u16::MAX).flat_map(u16::to_le_bytes).collect();
    [("rgb555", 0, vec![]), ("bitfields565", 3, vec![0xf800, 0x7e0, 0x1f])].iter().map(|(name, compression, masks)| {
        let bytes = fixtures::bmp(40, 256, 256, 16, *compression, masks, &source);
        let (_, _, old) = current(&bytes).unwrap();
        let (_, _, new) = admitted_candidate(&bytes).unwrap();
        let changed: Vec<_> = old.chunks_exact(4).zip(new.chunks_exact(4)).enumerate()
            .filter(|(_, (a, b))| a != b).collect();
        json!({"case":name,"source_words":65536,"current_sha256":digest(&old),"image_sha256":digest(&new),
            "changed_pixels":changed.len(),"first_difference":changed.first().map(|(i,(a,b))| json!({"output_pixel":i,"current":hex(a),"image":hex(b)}))})
    }).collect()
}

pub fn report() -> Value {
    let cases = fixtures::bmps();
    let bmp: Vec<_> = cases.iter().map(|case| {
        let old = pixels(current(&case.bytes));
        let raw = pixels(candidate(&case.bytes, None));
        let admitted = pixels(admitted_candidate(&case.bytes));
        json!({"case":case.name,"input":hex(&case.bytes),"current":old,"image_raw":raw,"image_after_current_validation":admitted,
            "validation_work":current_bmp::validation_work(&case.bytes).ok(),
            "dimensions":current_bmp::dimensions(&case.bytes).ok()})
    }).collect();
    let exif: Vec<_> = fixtures::exifs()
        .iter()
        .map(|case| {
            json!({
                "case":case.name,"input":hex(&case.bytes),
                "current":current_exif::exif_orientation(&case.bytes),
                "image":Orientation::from_exif_chunk(&case.bytes).map(Orientation::to_exif)
            })
        })
        .collect();
    let renders: Vec<_> = [
        "header-40-rgb24",
        "alpha-header-56-compression-3",
        "rle8-complete",
        "rle8-delta-xy",
    ]
    .iter()
    .map(|name| {
        let case = cases.iter().find(|case| case.name == *name).unwrap();
        render_paths(name, &case.bytes)
    })
    .collect();
    json!({"schema":"bmp-evaluation/v1","image_version":"0.25.10","bmp":bmp,"exif":exif,"renders":renders,
        "resource_probes":resource_probes(),"mask_sweeps":mask_sweeps()})
}

pub fn matrix(report: &Value) -> String {
    use std::fmt::Write;
    let mut text = String::from("# BMP / EXIF exact comparison matrix\n\nGenerated by `cargo run --manifest-path tools/bmp-evaluation/Cargo.toml -- --record`.\nSee [the evaluation](../../docs/research/bmp-image-evaluation.md) for scope and interpretation.\n\nRGBA hex is row-major, top-to-bottom, straight alpha, before transparent RGB normalization.\n`image` RGB output is expanded with alpha 255. Inputs and gated results are in [expected.json](expected.json).\nErrors show the stage and stable error category; they are not strings exposed to renderer callers.\n\n## BMP\n\n| Case | Current | image raw | After current validation |\n| --- | --- | --- | --- |\n");
    let cell = |v: &Value| {
        v["rgba"]
            .as_str()
            .or(v["error"].as_str())
            .unwrap()
            .to_owned()
    };
    for row in report["bmp"].as_array().unwrap() {
        writeln!(
            text,
            "| {} | `{}` | `{}` | {} |",
            row["case"].as_str().unwrap(),
            cell(&row["current"]),
            cell(&row["image_raw"]),
            if row["current"] == row["image_after_current_validation"] {
                "same"
            } else {
                "**differs**"
            }
        )
        .unwrap();
    }
    text.push_str("\n## EXIF\n\n`null` means no orientation; it is not orientation 1.\n\n| Case | Current | image |\n| --- | --- | --- |\n");
    for row in report["exif"].as_array().unwrap() {
        writeln!(
            text,
            "| {} | {} | {} |",
            row["case"].as_str().unwrap(),
            row["current"],
            row["image"]
        )
        .unwrap();
    }
    text
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn exact_comparison_and_render_evidence() {
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../expected.json")).unwrap();
        assert_eq!(super::report(), expected);
    }
}
