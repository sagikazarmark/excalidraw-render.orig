//! Renders an untrusted scene the way a web service should: a body size check
//! before parsing, lowered resource limits, a bounded raster size, and a
//! machine-readable report or error for the caller.
//!
//! It reads the scene from standard input, writes the PNG to standard output
//! and one JSON document (the report, or the error's descriptor) to standard
//! error:
//!
//! ```sh
//! cargo run --release --example untrusted < drawing.excalidraw > drawing.png
//! ```
//!
//! A real service would call `render_untrusted` from a blocking pool (for
//! example `tokio::task::spawn_blocking`) and, for a hard deadline or memory
//! ceiling, from a separate worker process. See
//! `docs/guides/untrusted-input.md`.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;

use excalidraw_render::{
    ErrorDescriptor, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument, Sizing,
};

/// Bodies larger than this are refused before parsing. `SceneDocument::parse`
/// accepts up to 64 MiB on its own; `max_scene_bytes` is checked only when
/// rendering.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// The output's larger side, in pixels.
const MAX_DIMENSION: f64 = 2048.0;

fn main() -> ExitCode {
    let mut body = Vec::new();
    // Read one byte past the limit so an oversized body is detected without
    // buffering all of it.
    let limit = u64::try_from(MAX_BODY_BYTES).unwrap_or(u64::MAX) + 1;
    if let Err(error) = std::io::stdin().take(limit).read_to_end(&mut body) {
        eprintln!("error: cannot read standard input: {error}");
        return ExitCode::FAILURE;
    }

    let renderer = Renderer::new();
    match render_untrusted(&renderer, &body) {
        Ok((png, report_json)) => {
            eprintln!("{report_json}");
            if std::io::stdout().write_all(&png).is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error_json) => {
            eprintln!("{error_json}");
            ExitCode::FAILURE
        }
    }
}

/// Renders `body` to a bounded PNG. Returns the PNG with the JSON report, or
/// the JSON error to send back to the caller.
fn render_untrusted(renderer: &Renderer, body: &[u8]) -> Result<(Vec<u8>, String), String> {
    if body.len() > MAX_BODY_BYTES {
        return Err(r#"{"code":"service.body-too-large"}"#.to_owned());
    }

    // Every failure type carries a stable code and a serializable descriptor,
    // so the caller gets the same shape whatever went wrong.
    let scene = SceneDocument::parse(body).map_err(|error| to_json(&error.descriptor()))?;

    // Lower the limits that cost memory. The defaults admit large drawings:
    // a PNG holds a 4x supersampled canvas, about 65 bytes per output pixel,
    // so the default surface limit allows renders of over 2 GiB. These keep
    // one render near 300 MB.
    let limits = ResourceLimits::builder()
        .max_scene_bytes(u64::try_from(MAX_BODY_BYTES).unwrap_or(u64::MAX))
        .max_raster_surface_pixels(4_194_304)
        .max_total_rasterized_pixels(8_388_608)
        .build()
        .map_err(|error| to_json(&error.descriptor()))?;

    // `MaxDimension` rather than a caller-chosen scale: a large scene shrinks
    // to fit instead of asking for an enormous image.
    let request = RenderRequest::builder(OutputFormat::Png)
        .sizing(Sizing::MaxDimension(MAX_DIMENSION))
        .resource_limits(limits)
        .build()
        .map_err(|error| to_json(&error.descriptor()))?;

    let outcome = renderer
        .render(&scene, &request)
        .map_err(|error| to_json(&error.descriptor()))?;
    let (artifact, report) = outcome.into_parts();
    let report = serde_json::to_string(&report).map_err(|error| error.to_string())?;
    Ok((artifact.into_bytes(), report))
}

fn to_json(descriptor: &ErrorDescriptor) -> String {
    serde_json::to_string(descriptor).unwrap_or_else(|error| error.to_string())
}
