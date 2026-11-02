//! Renders one Excalidraw scene to SVG, PNG or PDF, chosen by the output
//! file's extension, and prints the warnings of its compatibility report.
//!
//! ```sh
//! cargo run --release --example render -- \
//!     tests/public-scenes/elasticsearch-spec-overview/scene.excalidraw overview.png
//! cargo run --release --example render -- drawing.excalidraw drawing.pdf --dark
//! ```
//!
//! This is the core of the library in one function: parse the scene once,
//! describe the output with a request, render, and keep both the artifact and
//! the report. See `docs/guides/library.md`.

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use excalidraw_render::{
    ColorMode, DiagnosticSeverity, OutputFormat, RenderRequest, Renderer, SceneDocument, Sizing,
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

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err("usage: render <scene.excalidraw> <output.svg|png|pdf> [--dark]".into());
    };
    let dark = args.next().is_some_and(|flag| flag == "--dark");

    let format = match Path::new(&output).extension().and_then(|e| e.to_str()) {
        Some("svg") => OutputFormat::Svg,
        Some("png") => OutputFormat::Png,
        Some("pdf") => OutputFormat::Pdf,
        _ => return Err("the output file must end in .svg, .png or .pdf".into()),
    };

    // Parsing admits the scene once; the same document can be rendered to any
    // number of requests, from any thread.
    let scene = SceneDocument::parse(&std::fs::read(&input)?)?;

    // A request describes one artifact. Everything not set here keeps
    // Excalidraw's export defaults: 10 px padding, the scene's background,
    // frames clipped and named, no embedded metadata.
    let mut request = RenderRequest::builder(format).color_mode(if dark {
        ColorMode::Dark
    } else {
        ColorMode::Light
    });
    if format == OutputFormat::Png {
        // Twice the logical size, like Excalidraw's "2x" export.
        request = request.sizing(Sizing::Scale(2.0));
    }
    let request = request.build()?;

    let outcome = Renderer::new().render(&scene, &request)?;

    // The report lists every repair, substitution and omission the renderer
    // made. Informational entries (such as repaired timestamps) are routine;
    // warnings are differences from what Excalidraw would draw.
    for diagnostic in outcome.report().diagnostics() {
        if diagnostic.severity() == DiagnosticSeverity::Warning {
            eprintln!("{diagnostic}");
        }
    }

    std::fs::write(&output, outcome.artifact().bytes())?;
    eprintln!(
        "wrote {} ({}, {} bytes)",
        Path::new(&output).display(),
        outcome.artifact().media_type(),
        outcome.artifact().bytes().len()
    );
    Ok(())
}
