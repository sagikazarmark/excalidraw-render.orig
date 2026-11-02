# Command-line guide

`excalidraw-render` turns `.excalidraw` files into SVG, PNG or PDF. This guide
goes through it by task; `excalidraw-render --help` and
`excalidraw-render frames --help` list every option.

```text
excalidraw-render [OPTIONS] <INPUT> -o <OUTPUT>
excalidraw-render frames [OPTIONS] <INPUT> -o <DIRECTORY> --format <svg|png|pdf>
excalidraw-render frames [OPTIONS] <INPUT> --list
```

Install it with `cargo install --locked excalidraw-render-cli`, or download a
prebuilt archive or the shell installer from a GitHub release; [Getting started](getting-started.md)
walks through a first render.

## Choose the format

The output file's extension selects the format: `.svg`, `.png` or `.pdf`. For
any other name, or for standard output, pass `--format`:

```sh
excalidraw-render drawing.excalidraw -o drawing.svg
excalidraw-render drawing.excalidraw -o drawing.image --format png
```

- **SVG** is a self-contained vector document. Text stays text, with the
  glyphs it uses embedded as font subsets, so it looks the same without the
  fonts installed. Links are kept.
- **PNG** is a raster image drawn the way Excalidraw's canvas export draws it.
- **PDF** is a single page, vector-first: shapes are vector paths and text is
  selectable and searchable. The few nested-SVG effects that PDF cannot
  express (some filters) become small isolated images; see
  [PDF raster fallback](#pdf-raster-fallback).

An existing output file is kept unless you pass `--force`. Devices and pipes
such as `/dev/null` are written to directly.

## Control the size

```sh
excalidraw-render drawing.excalidraw -o drawing.png --scale 2
excalidraw-render drawing.excalidraw -o drawing.png --max-dimension 1600
excalidraw-render drawing.excalidraw -o drawing.svg --padding 0
```

- `--scale FACTOR` multiplies the output size (default 1, Excalidraw's "1x").
- `--max-dimension PX` shrinks PNG and PDF output so the longer side is at most
  `PX` pixels, and never enlarges it. Use it for thumbnails and for drawings of
  unknown size. It replaces `--scale`; passing both is an error. SVG output
  ignores it, with a warning: resize SVG with `--scale`.
- `--padding PX` is the space around the drawing, in scene pixels (default 10).

Very large drawings can exceed the PNG size limit (33,554,432 pixels, about
5,800 × 5,800) and fail with `resource.raster-surface-exceeded`. Bound them
with `--max-dimension`, or render them to SVG or PDF, which have no such limit.

## Change the appearance

```sh
excalidraw-render drawing.excalidraw -o drawing.png --dark-mode
excalidraw-render drawing.excalidraw -o drawing.png --no-background
```

- `--dark-mode` uses Excalidraw's dark theme colors.
- `--no-background` leaves the background transparent instead of filling it
  with the scene's background color.
- `--no-frame-rendering`, `--no-frame-clipping`, `--no-frame-names` and
  `--no-frame-outlines` turn off frame drawing, or parts of it, when you render
  a whole scene.

## Read from standard input, write to standard output

`-` stands for standard input or standard output. Writing to standard output
needs `--format`:

```sh
git show main:docs/architecture.excalidraw \
  | excalidraw-render - -o - --format svg > architecture.svg
```

Warnings and errors still go to standard error, so the artifact on standard
output stays clean.

## Supply images the scene does not embed

An Excalidraw image element refers to its bytes by a file ID, normally stored
in the scene's `files` map. When a scene comes without them (for example from
a tool that keeps images elsewhere), the renderer draws a placeholder and warns
`image.missing-file`. Supply the bytes with `--asset FILE_ID=PATH`:

```sh
excalidraw-render drawing.excalidraw -o drawing.png \
  --asset logo=assets/logo.png --asset photo=assets/photo.jpg
```

The media type is detected from the bytes; `--asset-mime logo=image/png`
declares it when you know it. PNG, JPEG, GIF, WebP, BMP, ICO and SVG images are
read; AVIF is not.

## Embed metadata

```sh
excalidraw-render drawing.excalidraw -o drawing.png --embed-scene
```

- `--embed-scene` embeds the scene in the output, so Excalidraw can open the
  exported SVG or PNG for editing, as with its own "Embed scene" export
  option. A PDF carries it as the attached file `scene.excalidraw`.
- `--embed-provenance` records the renderer version and the Excalidraw release
  it matches.
- `--embed-diagnostics` records a summary of the rendering warnings.

Outputs always carry the attribution notices of the bundled fonts they use.
`--licenses` prints every bundled license.

## PDF raster fallback

PDF cannot express a few effects of SVG images embedded in a drawing, such as
blur and other pixel filters. By default each such effect is rasterized as an
isolated image, and the report says so with `pdf.raster-fallback`; everything
else stays vector. When a PDF must be all-vector, refuse instead:

```sh
excalidraw-render drawing.excalidraw -o drawing.pdf --pdf-raster-fallback forbid
```

## Reports and exit status

Each render reports what it repaired, substituted or left out. By default the
report is human-readable on standard error, with one line per distinct
warning:

```text
warning: 4 characters have no glyph in the bundled fonts and were drawn as "?" [font.missing-glyph] (element label; characters U+05E9, U+05DC, U+05D5, U+05DD)
```

- `-v, --verbose` also shows informational entries, such as repaired missing
  properties.
- `--report-format json` prints one JSON document instead, with every
  occurrence; `--report-format none` prints nothing on success and one error
  line on failure.
- `--report-output PATH` writes the report to a file instead of standard error.

The exit status is:

| Status | Meaning |
| --- | --- |
| 0 | The artifact was written, possibly with warnings. |
| 1 | Reading, parsing, rendering or writing failed, or the output exists and `--force` was not given. |
| 2 | The command line or a request value is invalid. |

When a reader closes standard output early (`| head`), the command exits 1
without printing an error.

### Scripting with the JSON report

```sh
excalidraw-render drawing.excalidraw -o drawing.svg \
  --report-format json --report-output report.json
jq -r '.diagnostics[] | select(.severity == "warning") | .code' report.json | sort | uniq -c
```

The report has a `command` (`render`, `frames.list` or `frames.export`), a
`status` (`success` or `failure`; a frame batch can also be `partial`) and a
`diagnostics` array whose entries carry `code`, `severity`, `element_id`,
`json_pointer` (the location in the scene) and `message`. A failed render, or
a `frames` command that fails before it has a listing or a batch, has an
`error` object with a stable `code`. The `frames` reports carry a `frames`
array instead of `error` (a listing has no `diagnostics` either), and a frame
batch reports each failure in that frame's `error`: check `status`, then
`.frames[].error`. Match on codes, not messages: codes are stable, messages
may be reworded.

## Export frames

The `frames` command writes each frame of a scene to its own file:

```sh
excalidraw-render frames drawing.excalidraw --list
excalidraw-render frames drawing.excalidraw -o slides --format pdf --name-regex '^Slide'
```

See [Exporting frames](frames.md).

## Report a problem

Check the [known limitations](../reference/known-limitations.md) first. When
filing an issue, include the output of `excalidraw-render --version --verbose`,
which names the exact build, and if you can, the drawing or a reduced copy of
it.
