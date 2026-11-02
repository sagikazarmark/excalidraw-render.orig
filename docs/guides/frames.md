# Exporting frames

Excalidraw frames group part of a drawing under a name, and are often used as
slides or as separate figures on one board. Exporting a frame renders just that
frame: its contents, clipped to its bounds, with no padding and without the
frame's name and outline, the way Excalidraw's own frame export does. Both
ordinary frames and AI-generated (magic) frames can be exported.

## From the command line

### See what a scene contains

```sh
excalidraw-render frames slides.excalidraw --list
```

```text
1	frame	Slide 1	Slide-1
2	frame	Slide 2	Slide-2
3	frame	Appendix	Appendix
```

Each line has four tab-separated columns: the frame's number in the scene
(counting from 1 in source order), its kind (`frame` or `magicframe`), its name
(empty when it has none) and the file name an export would write. The file name
gets an extension once you pass `--format`. Nothing is rendered or written.

### Export every frame

```sh
excalidraw-render frames slides.excalidraw -o slides --format png
```

This writes `slides/Slide-1.png`, `slides/Slide-2.png` and
`slides/Appendix.png`, creating the directory when it is missing. Since the
output is a directory, there is no extension to infer the format from, so
`--format` is required.

File names come from frame names, made safe for every file system: each run of
characters that are not portable becomes one `-`, and a name used twice gets a
numeric suffix. An unnamed frame is `frame-N`, after its number:

```text
1	frame	Slide/1: intro	Slide-1-intro.svg
2	frame	Slide/1: intro	Slide-1-intro-2.svg
3	magicframe		frame-3.svg
```

### Select frames

`--name-regex` keeps only the frames whose name matches a
[Rust regular expression](https://docs.rs/regex/latest/regex/#syntax):

```sh
excalidraw-render frames slides.excalidraw -o slides --format pdf --name-regex '^Slide'
```

Frame numbers in `--list` and in file names are positions in the whole scene,
whatever the selection, so the same frame always gets the same number.
`--max-frames` (default 256) fails a selection that is larger, which guards
against accidentally exporting thousands of frames from an unexpected file.

### Size, appearance and metadata

The rendering options of a whole-scene export apply to each frame:
`--scale`, `--max-dimension`, `--dark-mode`, `--no-background`,
`--embed-scene`, `--asset` and the rest; see the
[command-line guide](command-line.md). `--padding` and the frame drawing
switches do not apply, since a frame export is the frame itself.

### Failures and existing files

By default the export is all or nothing: every frame is rendered first, and
nothing is written unless all of them succeed. `--allow-partial` writes the
frames that succeeded and still exits 1 when any failed.

Existing files are kept: if any target file exists, the command lists them and
writes nothing. Pass `--force` to replace them.

### Make a slide deck

The PDF of a frame is a single page. To get one multi-page PDF, export the
frames to PDF and join them with a PDF tool, in the order `--list` prints:

```sh
excalidraw-render frames talk.excalidraw -o pages --format pdf --name-regex '^Slide'
qpdf --empty --pages pages/Slide-*.pdf -- talk.pdf
```

(Shell globbing sorts `Slide-10` before `Slide-2`; use the order of `--list`
when you have more than nine slides.)

## From Rust

`SceneDocument::frames` lists the frames of a parsed scene in source order.
Each `FrameRef` has its `ordinal()` (the number `--list` prints), `kind()`,
`id()` and `name()`. A `FrameRenderRequest` describes the output of one frame;
it has the same options as a `RenderRequest` except padding and frame drawing.

Render one frame:

```rust,no_run
use excalidraw_render::{FrameRenderRequest, OutputFormat, Renderer, SceneDocument};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scene = SceneDocument::parse(&std::fs::read("slides.excalidraw")?)?;
    let request = FrameRenderRequest::builder(OutputFormat::Svg).build()?;
    let renderer = Renderer::new();

    if let Some(frame) = scene.frames().find(|frame| frame.name() == Some("Slide 1")) {
        let outcome = renderer.render_frame(frame, &request)?;
        std::fs::write("slide-1.svg", outcome.artifact().bytes())?;
    }
    Ok(())
}
```

For many frames, `Renderer::render_frames` is faster: it prepares the scene
once for the whole batch, then renders the selected frames lazily, in source
order. Each entry carries its own result, so one failing frame does not stop
the others:

```rust,no_run
use excalidraw_render::{FrameRenderRequest, OutputFormat, Renderer, SceneDocument, Sizing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scene = SceneDocument::parse(&std::fs::read("slides.excalidraw")?)?;
    let request = FrameRenderRequest::builder(OutputFormat::Png)
        .sizing(Sizing::MaxDimension(1920.0))
        .build()?;
    let slides = scene
        .frames()
        .filter(|frame| frame.name().is_some_and(|name| name.starts_with("Slide")));

    for entry in Renderer::new().render_frames(&scene, slides, &request)? {
        let ordinal = entry.frame().ordinal();
        match entry.into_result() {
            Ok(outcome) => std::fs::write(format!("slide-{ordinal}.png"), outcome.artifact().bytes())?,
            Err(error) => eprintln!("frame {ordinal} failed: {error} [{}]", error.code()),
        }
    }
    Ok(())
}
```

`render_frames` itself fails only when the batch cannot start: a frame from
another scene, the same frame twice, or a failure in the shared preparation.
Resource limits apply to each frame separately, so with untrusted scenes bound
the number of frames you render.

The [`frames` example](../../crates/excalidraw-render/examples/frames.rs) is
a complete program built on this.
