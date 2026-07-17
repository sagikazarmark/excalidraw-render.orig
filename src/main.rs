mod backends;
mod model;
mod render_svg;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use backends::OutputFormat;
use clap::Parser;
use model::ExcalidrawFile;

#[derive(Parser)]
#[command(
    name = "excalidraw-render",
    version,
    about = "Render Excalidraw files to SVG, PNG, or PDF"
)]
struct Cli {
    /// Input .excalidraw JSON file.
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
    let format = OutputFormat::from_path(&cli.output)?;
    let input = fs::read_to_string(&cli.input)
        .with_context(|| format!("failed to read input file '{}'", cli.input.display()))?;
    let document: ExcalidrawFile = serde_json::from_str(&input).with_context(|| {
        format!(
            "failed to parse Excalidraw JSON from '{}'",
            cli.input.display()
        )
    })?;
    let svg = render_svg::render(&document, f64::from(cli.padding))?;
    backends::write(&cli.output, format, &svg, cli.scale)
}
