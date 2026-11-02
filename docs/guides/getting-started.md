# Getting started

This tutorial takes you from nothing to a rendered drawing, first with the
command-line tool and then from Rust. It takes about ten minutes, most of it
compiling.

You need Rust 1.92 or newer (`rustup` or your package manager) to build either
the CLI or the library. On Linux x86-64 you can skip the build and download the
prebuilt binary from a GitHub release instead.

## 1. Install the command-line tool

```sh
cargo install --locked excalidraw-render-cli
excalidraw-render --version
```

`--version` prints the renderer version and the Excalidraw release it
reproduces:

```text
excalidraw-render 0.1.0 (Excalidraw 0.18.1)
```

## 2. Get a drawing

Any `.excalidraw` file works: in Excalidraw, **Save to…** in the main menu
saves the drawing to disk as one. For this tutorial, save the following as
`hello.excalidraw`:

```json
{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {
      "id": "box",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 220,
      "height": 100,
      "strokeColor": "#1e1e1e",
      "backgroundColor": "#a5d8ff",
      "fillStyle": "hachure",
      "roughness": 1,
      "seed": 1
    },
    {
      "id": "label",
      "type": "text",
      "x": 30,
      "y": 35,
      "width": 160,
      "height": 25,
      "text": "Hello, Excalidraw!",
      "originalText": "Hello, Excalidraw!",
      "fontSize": 20,
      "fontFamily": 5,
      "seed": 2
    }
  ],
  "appState": { "viewBackgroundColor": "#ffffff" },
  "files": {}
}
```

It is a hand-edited scene, so it leaves out most element properties; the
renderer fills them in with Excalidraw's defaults, as Excalidraw itself does
when it opens such a file. `fontFamily` 5 is Excalifont, the default
hand-drawn font.

## 3. Render it

The output file's extension picks the format:

```sh
excalidraw-render hello.excalidraw -o hello.svg
excalidraw-render hello.excalidraw -o hello.png --scale 2
excalidraw-render hello.excalidraw -o hello.pdf
```

Open the three files. The SVG and PDF are vector documents with the text kept
as text (you can select and search it in the PDF). The PNG is twice the
drawing's size because of `--scale 2`, like Excalidraw's "2x" export. All three
use the fonts bundled in the binary; nothing was downloaded and no browser ran.

Run one of the commands again and it refuses to replace the file:

```text
error: output 'hello.svg' already exists [cli.destination-exists]
hint: use --force to overwrite it
```

Add `--force` to overwrite it.

## 4. Read the report

Every render produces a compatibility report: the list of things the renderer
had to repair, substitute or leave out to produce the output. Warnings are
printed to standard error; informational entries appear with `--verbose`:

```sh
excalidraw-render hello.excalidraw -o hello.svg --force --verbose
```

```text
info: missing required timestamp was repaired to canonical zero: updated [normalization.missing-timestamp] (2 occurrences; elements box, label)
```

That one is harmless: the tutorial scene has no `updated` timestamps. Now
change the label's `text` and `originalText` to `Hello, שלום!` and render
again:

```text
warning: 4 characters have no glyph in the bundled fonts and were drawn as "?" [font.missing-glyph] (element label; characters U+05E9, U+05DC, U+05D5, U+05DD)
```

The bundled fonts have no Hebrew, so those characters are drawn as `?`; a
browser would have used a system font. The render still succeeds (exit status
0), and the warning tells you exactly what differs. The
[known limitations](../reference/known-limitations.md) list what to expect.

For scripts, `--report-format json` prints the same report as one JSON
document.

## 5. Export frames

If your drawing uses frames, each frame can become its own file, for example
one image per slide:

```sh
excalidraw-render frames drawing.excalidraw --list
excalidraw-render frames drawing.excalidraw -o slides --format png
```

[Exporting frames](frames.md) covers selecting frames and naming the files.

## 6. Render from Rust

Create a project and add the library:

```sh
cargo new hello-render
cd hello-render
cargo add excalidraw-render@0.1
```

Copy `hello.excalidraw` into the project directory and replace
`src/main.rs` with:

```rust
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument, Sizing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse once; the scene can then be rendered any number of times.
    let scene = SceneDocument::parse(&std::fs::read("hello.excalidraw")?)?;

    // Describe the output: a PNG at twice the drawing's size.
    let request = RenderRequest::builder(OutputFormat::Png)
        .sizing(Sizing::Scale(2.0))
        .build()?;

    // Render, print the report, and save the artifact.
    let outcome = Renderer::new().render(&scene, &request)?;
    for diagnostic in outcome.report().diagnostics() {
        eprintln!("{diagnostic}");
    }
    std::fs::write("hello.png", outcome.artifact().bytes())?;
    Ok(())
}
```

Build it optimized, since the renderer is many times slower in a debug build:

```sh
cargo run --release
```

That is the whole API in four steps: parse a `SceneDocument`, build a
`RenderRequest`, call `Renderer::render`, and use the artifact and report of
the `RenderOutcome`.

## Next steps

- [Command-line guide](command-line.md): every option of the CLI by task.
- [Rust library guide](library.md): requests, reports, errors and assets.
- [Exporting frames](frames.md): slides and per-frame files.
- [Serving untrusted input](untrusted-input.md): rendering in a web service.
- [The examples](../../crates/excalidraw-render/examples): runnable programs
  for each of these.
