use std::fs;
use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use tiny_skia::{Pixmap, Transform};

use crate::render_svg::SvgDocument;

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Svg,
    Png,
    Pdf,
}

impl OutputFormat {
    pub fn from_path(path: &Path) -> Result<Self> {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("svg") => Ok(Self::Svg),
            Some("png") => Ok(Self::Png),
            Some("pdf") => Ok(Self::Pdf),
            Some(extension) => {
                bail!("unsupported output extension '.{extension}'; expected .svg, .png, or .pdf")
            }
            None => bail!("output path must have a .svg, .png, or .pdf extension"),
        }
    }
}

pub fn write(
    output: &Path,
    format: OutputFormat,
    document: &SvgDocument,
    scale: f32,
) -> Result<()> {
    if matches!(format, OutputFormat::Svg) {
        fs::write(output, document.content.as_bytes())
            .with_context(|| format!("failed to write SVG to '{}'", output.display()))?;
        return Ok(());
    }

    // svg2pdf 0.13 uses usvg 0.45, so resvg/usvg are pinned to 0.45.1 for one shared tree.
    let tree = usvg::Tree::from_str(&document.content, &usvg::Options::default())
        .context("generated SVG could not be parsed")?;

    match format {
        OutputFormat::Svg => unreachable!(),
        OutputFormat::Png => write_png(output, &tree, document, scale),
        OutputFormat::Pdf => write_pdf(output, &tree),
    }
}

fn write_png(output: &Path, tree: &usvg::Tree, document: &SvgDocument, scale: f32) -> Result<()> {
    ensure!(
        scale.is_finite() && scale > 0.0,
        "scale must be a finite number greater than zero"
    );
    let width = scaled_dimension(document.width, scale, "width")?;
    let height = scaled_dimension(document.height, scale, "height")?;
    let mut pixmap = Pixmap::new(width, height)
        .with_context(|| format!("could not allocate a {width}x{height} PNG canvas"))?;
    let mut target = pixmap.as_mut();
    resvg::render(tree, Transform::from_scale(scale, scale), &mut target);
    let png = pixmap.encode_png().context("failed to encode PNG")?;
    fs::write(output, png)
        .with_context(|| format!("failed to write PNG to '{}'", output.display()))?;
    Ok(())
}

fn write_pdf(output: &Path, tree: &usvg::Tree) -> Result<()> {
    // svg2pdf's ConversionError implements Display but not std::error::Error.
    let pdf = svg2pdf::to_pdf(
        tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|error| anyhow::anyhow!("failed to convert SVG to PDF: {error}"))?;
    fs::write(output, pdf)
        .with_context(|| format!("failed to write PDF to '{}'", output.display()))?;
    Ok(())
}

fn scaled_dimension(value: f64, scale: f32, name: &str) -> Result<u32> {
    let scaled = (value * f64::from(scale)).ceil();
    ensure!(
        scaled >= 1.0 && scaled <= f64::from(u32::MAX),
        "scaled PNG {name} is outside the supported range"
    );
    Ok(scaled as u32)
}
