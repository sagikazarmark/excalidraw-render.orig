# excalidraw-render

Pure-Rust rendering of [Excalidraw](https://excalidraw.com) scene documents to
**SVG, PNG, and vector-first PDF**, for a whole scene or frame by frame. Fonts
and other rendering assets are bundled: rendering needs no browser, Node.js,
network access, or installed fonts.

The compatibility target is Excalidraw **0.18.1**. Output is close to, but not
pixel-identical with, the browser exporter; every render returns a
compatibility report describing the substitutions, omissions, and fallbacks it
made. See the [known limitations](../../docs/known-limitations.md).

For a command-line tool, see
[`excalidraw-render-cli`](https://crates.io/crates/excalidraw-render-cli).

## Usage

```toml
[dependencies]
excalidraw-render = "0.1"
```

```rust,no_run
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument, Sizing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scene = SceneDocument::parse(&std::fs::read("drawing.excalidraw")?)?;
    let request = RenderRequest::builder(OutputFormat::Png)
        .sizing(Sizing::Scale(2.0))
        .build()?;

    let outcome = Renderer::new().render(&scene, &request)?;
    for diagnostic in outcome.report().diagnostics() {
        eprintln!("{diagnostic:?}");
    }
    std::fs::write("drawing.png", outcome.artifact().bytes())?;
    Ok(())
}
```

`Renderer::render_frames` exports each frame of a scene separately. The
[API documentation](https://docs.rs/excalidraw-render) covers requests,
resource limits, frame export, and the error and diagnostic types.

## Untrusted input and servers

`Renderer` and `SceneDocument` are `Send + Sync`. Rendering is synchronous and
CPU-bound: call it from `tokio::task::spawn_blocking` or a worker pool in async
code. `ResourceLimits` bounds the work a scene can request, and
`Sizing::MaxDimension` bounds raster size, but there is no deadline or
cancellation: run renders in a separate process when you need a hard time or
memory ceiling.

Unoptimized builds render roughly 50× slower. In development, optimize the
renderer even in debug builds:

```toml
[profile.dev.package."*"]
opt-level = 2
```

## Features

- `cjk` (default): bundles Xiaolai, Excalifont's Chinese/Japanese/Korean
  fallback (about 12 MiB). Without it, a render that needs Xiaolai fails with
  `render.font-unavailable` instead of substituting another font. All other
  fonts, including Noto Emoji, are always bundled.

## Reproducibility

Output is deterministic for a given renderer release, input, options, and
dependency versions. Dependencies that compute output bytes are pinned exactly;
later 0.x releases may intentionally change rendering. When exact output bytes
matter, pin the renderer (`=0.1.0`) and commit your `Cargo.lock`.

Linux x86-64 (glibc) is the release-qualified platform. Other native platforms
build and run but are not release-qualified; browser WASM is experimental.

## License

Code is licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option. Bundled fonts and adapted
third-party code carry their own licenses, collected in
[`LICENSE-BUNDLE.txt`](LICENSE-BUNDLE.txt). Exports carry the attribution
required by the bundled resources they use.
