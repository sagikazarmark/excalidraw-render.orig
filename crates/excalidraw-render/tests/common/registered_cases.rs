//! Shared plumbing for the registry-driven parity harnesses: how a registered
//! export variant's options become a render request.

use excalidraw_render::{ColorMode, OutputFormat, RenderRequest, Sizing};
use serde_json::{json, Value};

/// Builds the render request a registered export variant describes.
///
/// Only the options a variant is allowed to vary are honoured: dark mode and the
/// two sizing modes. Any other deviation from the default export panics, so an
/// unrecognised option fails rather than silently rendering the default and
/// comparing it against evidence generated under different settings. `context`
/// names the case and variant in that failure.
pub fn variant_request(format: OutputFormat, options: &Value, context: &str) -> RenderRequest {
    for (key, default) in [
        ("exportBackground", json!(true)),
        ("exportEmbedScene", json!(false)),
        ("exportPadding", json!(10)),
        (
            "frameRendering",
            json!({"clip": true, "enabled": true, "name": true, "outline": true}),
        ),
        ("viewBackgroundColor", json!("#ffffff")),
    ] {
        assert_eq!(
            options[key], default,
            "{context} varies {key}, which the parity harnesses do not build"
        );
    }
    let mut builder = RenderRequest::builder(format);
    match options["exportWithDarkMode"].as_bool() {
        Some(true) => builder = builder.color_mode(ColorMode::Dark),
        Some(false) => {}
        None => panic!("{context} exportWithDarkMode is not a boolean"),
    }
    // The reference wrapper selects exactly one sizing mode; a variant that
    // selects neither or both is malformed rather than a default export.
    builder = match (
        options["exportScale"].as_f64(),
        options["maxWidthOrHeight"].as_f64(),
    ) {
        (Some(scale), None) => builder.sizing(Sizing::Scale(scale)),
        (None, Some(maximum)) => builder.sizing(Sizing::MaxDimension(maximum)),
        (scale, maximum) => panic!(
            "{context} selects exportScale={scale:?} and maxWidthOrHeight={maximum:?}; \
             exactly one sizing mode is expected"
        ),
    };
    builder
        .build()
        .unwrap_or_else(|error| panic!("{context} request is valid: {error:?}"))
}
