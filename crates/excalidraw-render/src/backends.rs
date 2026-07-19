use anyhow::{ensure, Context, Result};
use tiny_skia::{Pixmap, Transform};

use crate::render_svg::SvgDocument;

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Svg,
    Png,
    Pdf,
}

impl OutputFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::Svg => "SVG",
            Self::Png => "PNG",
            Self::Pdf => "PDF",
        }
    }
}

pub fn encode(format: OutputFormat, document: &SvgDocument, scale: f32) -> Result<Vec<u8>> {
    if matches!(format, OutputFormat::Svg) {
        return Ok(document.content.as_bytes().to_vec());
    }

    // svg2pdf 0.13 uses usvg 0.45, so resvg/usvg are pinned to 0.45.1 for one shared tree.
    let tree = usvg::Tree::from_str(&document.content, &usvg::Options::default())
        .context("generated SVG could not be parsed")?;

    match format {
        OutputFormat::Svg => unreachable!(),
        OutputFormat::Png => encode_png(&tree, document, scale),
        OutputFormat::Pdf => encode_pdf(&tree),
    }
}

fn encode_png(tree: &usvg::Tree, document: &SvgDocument, scale: f32) -> Result<Vec<u8>> {
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
    pixmap.encode_png().context("failed to encode PNG")
}

fn encode_pdf(tree: &usvg::Tree) -> Result<Vec<u8>> {
    // svg2pdf's ConversionError implements Display but not std::error::Error.
    svg2pdf::to_pdf(
        tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|error| anyhow::anyhow!("failed to convert SVG to PDF: {error}"))
}

fn scaled_dimension(value: f64, scale: f32, name: &str) -> Result<u32> {
    let scaled = (value * f64::from(scale)).ceil();
    ensure!(
        scaled >= 1.0 && scaled <= f64::from(u32::MAX),
        "scaled PNG {name} is outside the supported range"
    );
    Ok(scaled as u32)
}
