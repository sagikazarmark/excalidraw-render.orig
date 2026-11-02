# Features and WebAssembly

## Cargo features

`excalidraw-render` has three Cargo features, all on by default:

| Feature | Adds | Without it |
| --- | --- | --- |
| `png` | PNG output, with the `resvg` rasterizer and `tiny-skia` | Building a PNG request fails with `request.format-unavailable` |
| `pdf` | PDF output, with the Krilla PDF writer and the rasterizer for isolated raster fallbacks | Building a PDF request fails with `request.format-unavailable` |
| `cjk` | Xiaolai, Excalifont's Chinese, Japanese and Korean fallback font (about 12 MiB) | A render whose text needs Xiaolai fails with `render.font-unavailable` |

SVG output is always available, and every other font, including Noto Emoji, is
always bundled.

Three promises make the features safe to combine:

- **The API is the same in every combination.** `OutputFormat::Png` and the
  PDF options exist in every build, so Cargo's feature unification can never
  make another crate's code stop compiling; it can only turn an error into a
  render.
- **A missing feature is an error, never different output.** A build without
  `cjk` does not substitute another font for Chinese text; it fails with a
  typed error that names the feature.
- **Turning a feature off never changes the bytes of the formats that remain.**
  An SVG from an SVG-only build is byte-identical to the SVG of a full build.

An SVG-only build with CJK text support:

```toml
[dependencies]
excalidraw-render = { version = "0.1", default-features = false, features = ["cjk"] }
```

Features are additive across your dependency graph: if another crate enables
`png`, your build has it too. Disabling a feature removes its crates from the
build, not necessarily from your `Cargo.lock` or `cargo fetch`.

The CLI always has every feature.

## WebAssembly

The renderer is pure Rust and compiles for `wasm32-unknown-unknown` without
changes: it makes no browser calls, needs no JavaScript glue, and bundles its
fonts in the module. The project tests every format in headless Firefox, so
all three work in the browser, but browser WebAssembly is **experimental** in
0.1: it is not release-qualified, and four PNG test cases differ from native
output by one channel value in one or two pixels
([#178](https://github.com/sagikazarmark/excalidraw-render/issues/178)). SVG
and PDF output, and every report, match native output exactly.

No JavaScript package is published; wrap the API with
[`wasm-bindgen`](https://docs.rs/wasm-bindgen) yourself. A
minimal SVG-only module:

```toml
[package]
name = "excalidraw-svg"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
excalidraw-render = { version = "0.1", default-features = false }
wasm-bindgen = "0.2"
```

```rust,ignore
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use wasm_bindgen::prelude::*;

/// Renders the bytes of an `.excalidraw` file to an SVG string.
#[wasm_bindgen]
pub fn render_svg(scene: &[u8]) -> Result<String, JsError> {
    let scene = SceneDocument::parse(scene)?;
    let request = RenderRequest::builder(OutputFormat::Svg).build()?;
    let outcome = Renderer::new().render(&scene, &request)?;
    Ok(String::from_utf8(outcome.into_parts().0.into_bytes())?)
}
```

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg \
  target/wasm32-unknown-unknown/release/excalidraw_svg.wasm
```

(or `wasm-pack build --target web`). From JavaScript, pass the file's bytes as
a `Uint8Array`, and show the SVG as an image rather than inserting it into the
page, so its links and metadata stay inert:

```js
import init, { render_svg } from "./pkg/excalidraw_svg.js";

await init();
const bytes = new Uint8Array(await file.arrayBuffer());
const svg = new Blob([render_svg(bytes)], { type: "image/svg+xml" });
document.querySelector("img#preview").src = URL.createObjectURL(svg);
```

Keep in mind:

- **Size.** Most of the module is fonts. Leaving out `cjk` saves about 12 MiB;
  leaving out `png` and `pdf` removes the rasterizer and the PDF writer. Run
  `wasm-opt -Oz` on the result.
- **Threads.** Rendering is synchronous; run it in a Web Worker so a large
  drawing does not block the page.
- **Untrusted drawings.** The advice in
  [Serving untrusted input](untrusted-input.md) applies in the browser too,
  particularly bounding PNG sizes.
