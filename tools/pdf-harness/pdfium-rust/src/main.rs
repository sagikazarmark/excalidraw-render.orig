//! One render per process: PDFium initialization and teardown are process-global.
use excalidraw_render_pdfium::canonicalize_rgba;
use pdfium_render::prelude::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    error::Error,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

const PDFIUM_VERSION: &str = "152.0.7947.0";
const FONT_MARKER_SHA256: &str = "214f294fd6c8df5b4b53d84fd437aee8c30edbee74b929863e40ec2e6b3cf621";
const SCALE: f64 = 96.0 / 72.0;

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The `MAJOR.MINOR.BUILD.PATCH` a pdfium-binaries distribution declares.
fn distribution_version(root: &Path) -> Result<String, Box<dyn Error>> {
    let text = fs::read_to_string(root.join("VERSION"))?;
    let fields = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<BTreeMap<_, _>>();
    let field = |key| fields.get(key).ok_or("PDFium VERSION is incomplete");
    Ok(format!(
        "{}.{}.{}.{}",
        field("MAJOR")?,
        field("MINOR")?,
        field("BUILD")?,
        field("PATCH")?
    ))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(
            "usage: excalidraw-render-pdfium <input.pdf> <output.rgba> <width> <height>".into(),
        );
    }
    let width: i32 = args[2].to_str().ok_or("invalid width")?.parse()?;
    let height: i32 = args[3].to_str().ok_or("invalid height")?.parse()?;
    if width < 1 || height < 1 {
        return Err("expected dimensions must be positive integers".into());
    }
    // PDFium represents its packed row stride as a signed integer.
    let stride = width.checked_mul(4).ok_or("bitmap stride overflow")?;
    let length = (stride as usize)
        .checked_mul(height as usize)
        .ok_or("bitmap length overflow")?;
    let output = PathBuf::from(&args[1]);
    match fs::symlink_metadata(&output) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("output must not be a symlink".into());
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err("output must be a regular file".into());
        }
        Ok(_) => (),
        Err(error) if error.kind() == ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()?;
    // The devenv shell links the pinned distribution here. The receipt records
    // the library that ran; the retained rasters decide whether it qualifies.
    let distribution = root.join("target/pdf-harness/pdfium");
    if distribution_version(&distribution)? != PDFIUM_VERSION {
        return Err(format!("PDFium distribution is not {PDFIUM_VERSION}").into());
    }
    let library = distribution.join(format!("lib/{DLL_PREFIX}pdfium{DLL_SUFFIX}"));
    let library_sha256 = hash(&fs::read(&library)?);
    let fonts = root.join("tools/pdf-harness/pdfium/empty-fonts");
    let entries = fs::read_dir(&fonts)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() != 1
        || entries[0].file_name() != ".keep"
        || hash(&fs::read(fonts.join(".keep"))?) != FONT_MARKER_SHA256
    {
        return Err("empty font directory changed".into());
    }
    let bindings = Pdfium::bind_to_library(&library)?;
    let config = PdfiumLibraryConfig::new()
        .set_user_font_paths(&[fonts.to_str().ok_or("invalid font path")?])?
        .set_renderer_anti_grain_geometry()
        .set_font_backend_freetype();
    let pdfium = Pdfium::new_with_config(bindings, config);
    let document = pdfium.load_pdf_from_file(&PathBuf::from(&args[0]), None)?;
    if document.pages().len() != 1 {
        return Err("PDFium profile accepts exactly one page".into());
    }
    if document.version() != PdfDocumentVersion::Pdf1_7 {
        return Err("PDFium profile accepts PDF 1.7 only".into());
    }
    let page = document.pages().get(0)?;
    if page.rotation()? != PdfPageRenderRotation::None {
        return Err("PDFium profile rejects page rotation".into());
    }
    let device_width = f64::from(page.width().value) * SCALE;
    let device_height = f64::from(page.height().value) * SCALE;
    if !device_width.is_finite()
        || !device_height.is_finite()
        || (device_width - f64::from(width)).abs() >= 1.0
        || (device_height - f64::from(height)).abs() >= 1.0
    {
        return Err(format!(
            "PDFium page is {device_width}x{device_height} px at 96 DPI, expected {width}x{height}"
        )
        .into());
    }
    // Keep the matrix scale independent of the requested bitmap dimensions.
    // Fractional page edges are clipped/padded at the origin, never fitted.
    let config = PdfRenderConfig::new()
        .set_fixed_size(width, height)
        .scale_page_by_factor(SCALE as f32)
        .set_format(PdfBitmapFormat::BGRA)
        .clear_before_rendering(true)
        .set_clear_color(PdfColor::new(0, 0, 0, 0))
        .render_form_data(false)
        .render_annotations(false)
        .set_reverse_byte_order(true)
        .clip(0, 0, width, height);
    let bitmap = page.render_with_config(&config)?;
    if bitmap.width() != width
        || bitmap.height() != height
        || bitmap.format()? != PdfBitmapFormat::BGRA
    {
        return Err("unexpected bitmap layout".into());
    }
    let mut rgba = bitmap.as_raw_bytes();
    if rgba.len() != length {
        return Err("PDFium did not produce packed RGBA".into());
    }
    canonicalize_rgba(rgba.as_chunks_mut::<4>().0);
    let receipt = json!({
        "adapter": "pdfium-render/0.9.4",
        "pdfium": PDFIUM_VERSION,
        "library_sha256": library_sha256,
        "width": width,
        "height": height,
        "byte_length": rgba.len(),
        "layout": "top-left-row-major",
        "pixel_format": "straight-alpha-srgb-rgba8",
        "sha256": hash(&rgba),
    });
    // Rejected inputs never open or truncate the output.
    fs::write(output, &rgba)?;
    println!("{receipt}");
    Ok(())
}
