use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use excalidraw_render::__mvp::{self, OutputFormat, SceneDocument};

#[derive(Parser)]
#[command(
    name = "excalidraw-render",
    version,
    about = "Render Excalidraw scene documents to SVG, PNG, or PDF"
)]
struct Cli {
    /// Input Excalidraw scene document.
    input: PathBuf,

    /// Output path; format is inferred from .svg, .png, or .pdf.
    #[arg(short, long)]
    output: PathBuf,

    /// PNG raster scale.
    #[arg(long, default_value_t = 1.0)]
    scale: f32,

    /// Canvas padding on each side.
    #[arg(long, default_value_t = 20.0)]
    padding: f32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let format = output_format(&cli.output)?;
    let input = fs::read_to_string(&cli.input)
        .with_context(|| format!("failed to read input file '{}'", cli.input.display()))?;
    let document: SceneDocument = serde_json::from_str(&input).with_context(|| {
        format!(
            "failed to parse Excalidraw JSON from '{}'",
            cli.input.display()
        )
    })?;
    let artifact = __mvp::render(&document, format, cli.scale, f64::from(cli.padding))?;
    fs::write(&cli.output, artifact).with_context(|| {
        format!(
            "failed to write {} to '{}'",
            format.name(),
            cli.output.display()
        )
    })
}

fn output_format(path: &Path) -> Result<OutputFormat> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("svg") => Ok(OutputFormat::Svg),
        Some("png") => Ok(OutputFormat::Png),
        Some("pdf") => Ok(OutputFormat::Pdf),
        Some(extension) => {
            bail!("unsupported output extension '.{extension}'; expected .svg, .png, or .pdf")
        }
        None => bail!("output path must have a .svg, .png, or .pdf extension"),
    }
}
