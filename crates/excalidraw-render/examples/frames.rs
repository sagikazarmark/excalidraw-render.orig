//! Exports every frame of an Excalidraw scene whose name starts with a prefix
//! to its own file, the way you would turn a board of frames into slides.
//!
//! ```sh
//! cargo run --release --example frames -- slides.excalidraw out/ png Slide
//! cargo run --release --example frames -- slides.excalidraw out/ pdf
//! ```
//!
//! `Renderer::render_frames` prepares the scene once and renders the selected
//! frames lazily, in source order; each frame succeeds or fails on its own.
//! See `docs/guides/frames.md`.

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use excalidraw_render::{
    DiagnosticSeverity, FrameRenderRequest, OutputFormat, Renderer, SceneDocument, Sizing,
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, directory, format, rest @ ..] = args.as_slice() else {
        return Err(
            "usage: frames <scene.excalidraw> <directory> <svg|png|pdf> [name-prefix]".into(),
        );
    };
    let prefix = rest.first().map_or("", String::as_str);
    let format = match format.as_str() {
        "svg" => OutputFormat::Svg,
        "png" => OutputFormat::Png,
        "pdf" => OutputFormat::Pdf,
        other => return Err(format!("unknown format {other:?}").into()),
    };

    let scene = SceneDocument::parse(&std::fs::read(input)?)?;

    // Frame exports have no padding: the frame's own bounds are the page.
    // Bound the larger side so a huge frame cannot ask for a huge PNG.
    let request = FrameRenderRequest::builder(format)
        .sizing(Sizing::MaxDimension(1920.0))
        .build()?;

    // `frames()` lists ordinary and magic frames in source order. Select by
    // name here; a frame without a name never matches a non-empty prefix.
    let selected = scene
        .frames()
        .filter(|frame| frame.name().unwrap_or("").starts_with(prefix));

    std::fs::create_dir_all(directory)?;
    let mut failed = 0;
    for entry in Renderer::new().render_frames(&scene, selected, &request)? {
        let frame = entry.frame();
        // A file name from the frame's name, or its position when unnamed.
        // Real code should sanitize it; the CLI's `frames` command does.
        let stem = frame
            .name()
            .map_or_else(|| format!("frame-{}", frame.ordinal()), str::to_owned);
        match entry.into_result() {
            Ok(outcome) => {
                let path = PathBuf::from(directory).join(format!("{stem}.{format}"));
                std::fs::write(&path, outcome.artifact().bytes())?;
                println!("{}", path.display());
                for diagnostic in outcome.report().diagnostics() {
                    if diagnostic.severity() == DiagnosticSeverity::Warning {
                        eprintln!("  {diagnostic}");
                    }
                }
            }
            Err(error) => {
                failed += 1;
                eprintln!(
                    "frame {} ({stem}) failed: {error} [{}]",
                    frame.ordinal(),
                    error.code()
                );
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} frame(s) failed").into());
    }
    Ok(())
}
