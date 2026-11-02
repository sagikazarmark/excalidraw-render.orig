# Rust library guide

The `excalidraw-render` crate renders Excalidraw scenes from Rust. This guide
covers the tasks most programs need; the
[API documentation](https://docs.rs/excalidraw-render) describes every type,
and the [examples](../../crates/excalidraw-render/examples) are complete
programs.

## Set up

```toml
[dependencies]
excalidraw-render = "0.1"
```

The crate needs Rust 1.92 or newer. Rendering does a lot of arithmetic, and an
unoptimized build is roughly 50 times slower; optimize dependencies even in
debug builds:

```toml
[profile.dev.package."*"]
opt-level = 2
```

PNG, PDF and the Chinese/Japanese/Korean font are Cargo features, all on by
default; [Features and WebAssembly](features-and-wasm.md) explains when to turn
them off.

## The four steps

Every render goes through the same steps:

```rust,no_run
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse the `.excalidraw` JSON into an admitted scene.
    let scene = SceneDocument::parse(&std::fs::read("drawing.excalidraw")?)?;
    // 2. Describe one output.
    let request = RenderRequest::builder(OutputFormat::Pdf).build()?;
    // 3. Render.
    let outcome = Renderer::new().render(&scene, &request)?;
    // 4. Use the artifact and the report.
    std::fs::write("drawing.pdf", outcome.artifact().bytes())?;
    println!("{} compatibility notes", outcome.report().len());
    Ok(())
}
```

- `SceneDocument` is the parsed, validated scene. Parse once and render it as
  many times as you like, to different formats, from different threads.
- `RenderRequest` is one validated output description. Building it checks every
  option, so an invalid request fails before any rendering.
- `Renderer` holds the compiled-in fonts and Excalidraw profile. It is cheap to
  create, and one renderer can serve every thread.
- `RenderOutcome` holds the complete `Artifact` (the file's bytes and media
  type) and the `CompatibilityReport`.

`SceneDocument`, `Renderer` and the requests are immutable, `Clone`, `Send`
and `Sync`.

## Describe the output

`RenderRequest::builder(format)` starts from Excalidraw's export defaults:
1× scale, 10 px padding, the scene's background, light mode, frames clipped
with their names and outlines, and no embedded metadata. Change only what you
need:

```rust,no_run
use excalidraw_render::{
    ColorMode, FrameOptions, MetadataOptions, OutputFormat, RenderRequest, Sizing,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let request = RenderRequest::builder(OutputFormat::Png)
    .sizing(Sizing::MaxDimension(1600.0)) // or Sizing::Scale(2.0)
    .padding(20.0)
    .color_mode(ColorMode::Dark)
    .include_background(false) // transparent
    .frame_options(FrameOptions::default().with_show_name(false))
    .metadata_options(MetadataOptions::default().with_embedded_scene(true))
    .build()?;
# let _ = request;
# Ok(())
# }
```

- `Sizing::Scale(f)` multiplies the logical size. `Sizing::MaxDimension(px)`
  shrinks PNG and PDF output to fit, never enlarging; SVG ignores it and keeps
  its logical size, with a warning in the report.
- `MetadataOptions` embeds the scene (so Excalidraw can reopen an SVG or PNG for
  editing; a PDF carries it as an attachment), renderer provenance, or a
  diagnostic summary.
- `PdfOptions` sets the PDF raster-fallback policy: by default, the few
  nested-SVG effects PDF cannot express (such as blur) become isolated images;
  `PdfRasterFallback::Forbid` fails the render instead.

`build()` returns a `RequestError` for an invalid value, such as a negative
padding or a format the build leaves out.

## Read the report

A successful render can still differ from what Excalidraw would draw: a
character no bundled font has, an image that could not be decoded, a property
from a newer Excalidraw. The renderer never hides those differences; each is a
`CompatibilityDiagnostic` in the report:

```rust,no_run
use excalidraw_render::{DiagnosticCode, DiagnosticSeverity, RenderOutcome};

fn print_warnings(outcome: &RenderOutcome) {
    for diagnostic in outcome.report().diagnostics() {
        if diagnostic.severity() == DiagnosticSeverity::Warning {
            // `warning: MESSAGE [CODE] (element ID)`
            eprintln!("{diagnostic}");
        }
        if diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH {
            // Some text was drawn as "?": the bundled fonts lack the script.
        }
    }
}
```

Each diagnostic has a stable `code()`, a `severity()` (`Info` for routine
repairs such as a missing timestamp, `Warning` for a visible difference), the
`element_id()` and `json_pointer()` it concerns, a `message()`, its `effects()`
and typed `details()`. Match on codes, never on messages. Reports and
diagnostics implement `serde::Serialize`, so they can be returned as JSON as
they are.

An empty report means the renderer made no repair or substitution.

## Handle errors

Each stage has its own error type, and each carries a stable `ErrorCode` and a
`descriptor()` with a serializable form:

| Stage | Error | Typical causes |
| --- | --- | --- |
| `SceneDocument::parse` | `SceneParseError` | not JSON, not an Excalidraw scene, input too large |
| `RenderRequest::build` | `RequestError` | an invalid option, a format the build leaves out |
| `Renderer::render` | `RenderError` | a resource limit crossed, a required font left out of the build |
| `Renderer::render_frames` | `FrameBatchError` | the batch could not start |

`RenderError::category()` groups render failures: `ResourceLimit` for a crossed
limit (the message names the limit and the measured value), `Capability` for a
feature the build lacks, `Input` for scene content that cannot be rendered, and
so on.

```rust,no_run
use excalidraw_render::{ErrorCode, RenderErrorCategory, RenderRequest, Renderer, SceneDocument};

fn render(scene: &SceneDocument, request: &RenderRequest) -> Option<Vec<u8>> {
    match Renderer::new().render(scene, request) {
        Ok(outcome) => Some(outcome.into_parts().0.into_bytes()),
        Err(error) if error.category() == RenderErrorCategory::ResourceLimit => {
            eprintln!("drawing too large: {error}");
            None
        }
        Err(error) if error.code() == ErrorCode::RENDER_FONT_UNAVAILABLE => {
            eprintln!("this build has no CJK font: {error}");
            None
        }
        Err(error) => {
            eprintln!("render failed: {error} [{}]", error.code());
            None
        }
    }
}
```

## Supply images

Image elements refer to their bytes by file ID, normally through the scene's
`files` map. When you store images elsewhere, pass them with the request; a
missing image otherwise renders as a placeholder with an `image.missing-file`
warning:

```rust,no_run
use excalidraw_render::{OutputFormat, RenderRequest, SupplementalAssets};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let assets = SupplementalAssets::builder()
    .asset("logo", std::fs::read("logo.png")?)
    .asset_with_media_type("photo", std::fs::read("photo.jpg")?, "image/jpeg")
    .build()?;
let request = RenderRequest::builder(OutputFormat::Svg)
    .supplemental_assets(assets)
    .build()?;
# let _ = request;
# Ok(())
# }
```

The renderer reads PNG, JPEG, GIF, WebP, BMP, ICO and SVG images, detecting the
format from the bytes. The scene's own bytes take precedence; a conflicting
supplement is reported.

## Export frames

`scene.frames()` lists the frames, and `Renderer::render_frame` and
`Renderer::render_frames` render them with a `FrameRenderRequest`. See
[Exporting frames](frames.md).

## Next

- [Serving untrusted input](untrusted-input.md): resource limits, blocking
  pools and process isolation for services.
- [Reproducible output](reproducible-output.md): when output bytes can change,
  and how to pin them.
- [Known limitations](../reference/known-limitations.md): what differs from
  Excalidraw.
