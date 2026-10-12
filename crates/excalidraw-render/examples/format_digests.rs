//! Prints a digest of every artifact and report this build renders for the
//! given scenes, one tab-separated line per render:
//!
//! ```text
//! <scene> <target> <format> <color mode> <artifact sha256> <report sha256>
//! ```
//!
//! `target` is `scene` or `frame:<ordinal>`. A failed render prints
//! `error:<code>` in place of both digests. A format this build leaves out
//! prints no line, so the output of a reduced build is the default build's
//! output filtered to the formats it has. `tools/ci/check-format-features.sh`
//! compares the two, which is how ADR-0023's promise that a feature never
//! changes the bytes of another format is tested.
//!
//! ```sh
//! cargo run --example format_digests -- tests/public-scenes/*/scene.excalidraw
//! ```

use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use excalidraw_render::{
    ColorMode, ErrorCode, FrameRenderRequest, OutputFormat, RenderError, RenderOutcome,
    RenderRequest, Renderer, SceneDocument, Sizing,
};
use sha2::{Digest, Sha256};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const MODES: [ColorMode; 2] = [ColorMode::Light, ColorMode::Dark];
/// Keeps every raster within the default limits and the debug build quick.
const MAX_DIMENSION: f64 = 1024.0;

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: format_digests <scene.excalidraw>...");
        return ExitCode::from(2);
    }
    let mut out = String::new();
    for path in &paths {
        if let Err(error) = digest_scene(Path::new(path), &mut out) {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    }
    print!("{out}");
    ExitCode::SUCCESS
}

fn digest_scene(path: &Path, out: &mut String) -> Result<(), Box<dyn Error>> {
    let scene = SceneDocument::parse(&std::fs::read(path)?)?;
    let renderer = Renderer::new();
    let name = path.display();
    for format in FORMATS {
        for mode in MODES {
            let request = RenderRequest::builder(format)
                .sizing(Sizing::MaxDimension(MAX_DIMENSION))
                .color_mode(mode);
            let request = match request.build() {
                Err(error) if error.code() == ErrorCode::REQUEST_FORMAT_UNAVAILABLE => continue,
                built => built?,
            };
            let result = renderer.render(&scene, &request);
            line(out, &name, "scene", format, mode, &result)?;

            let request = FrameRenderRequest::builder(format)
                .sizing(Sizing::MaxDimension(MAX_DIMENSION))
                .color_mode(mode)
                .build()?;
            for frame in scene.frames() {
                let result = renderer.render_frame(frame, &request);
                let target = format!("frame:{}", frame.ordinal());
                line(out, &name, &target, format, mode, &result)?;
            }
        }
    }
    Ok(())
}

fn line(
    out: &mut String,
    scene: &dyn std::fmt::Display,
    target: &str,
    format: OutputFormat,
    mode: ColorMode,
    result: &Result<RenderOutcome, RenderError>,
) -> Result<(), Box<dyn Error>> {
    let mode = match mode {
        ColorMode::Dark => "dark",
        _ => "light",
    };
    let digests = match result {
        Ok(outcome) => format!(
            "{}\t{}",
            sha256(outcome.artifact().bytes()),
            sha256(&serde_json::to_vec(outcome.report())?)
        ),
        Err(error) => format!("error:{}", error.code()),
    };
    writeln!(out, "{scene}\t{target}\t{format}\t{mode}\t{digests}")?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}
