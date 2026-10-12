---
status: accepted
---

# Make PNG and PDF output optional features

Every build of the renderer carried the PNG rasterizer and the PDF writer.
An application that only needs SVG, a browser WASM build in particular, paid
for both in build time and binary size. Spec section 15 promised that the
library always includes SVG, PNG and PDF.

## Decision

PNG and PDF output are Cargo features of `excalidraw-render`, `png` and
`pdf`, both on by default beside `cjk`. SVG has no feature: every build
renders it. The CLI keeps the default features. Spec section 15 is amended
accordingly.

The public API is the same in every feature combination. `OutputFormat`
keeps its `Png` and `Pdf` variants, and `PdfOptions`, `PdfRasterFallback` and
every request method stay available. Cargo unifies features across a
dependency graph, so a feature that removed items would let one crate's
choice decide whether another crate compiles. With a fixed API, unification
can only turn an error into a render.

A request for a disabled format fails with a typed `RequestError`, code
`request.format-unavailable`, when it is built. Requests bind their format at
construction, so this is the earliest point: nothing is parsed, resolved or
charged for a render that cannot succeed. The check precedes every other
request validation, because no option of such a request can be honored. A
frame request is built through the same builder, so it and a frame batch over
it fail the same way. The message names the format and the feature to enable.
This follows the `cjk` precedent, where a missing Xiaolai payload fails with
`render.font-unavailable` instead of substituting another font: a reduced
build either produces the same bytes as a full one or a typed error, never
different output.

The features gate exactly the dependencies only their formats use:

| Feature | Dependencies |
| --- | --- |
| `png` | `resvg`, `tiny-skia` |
| `pdf` | `krilla`, `resvg`, `tiny-skia` |

PDF rasterizes the isolated closures of nested-SVG effects it cannot draw as
vectors (ADR-0009), so it needs the rasterizer and the owned PNG encoder that
embeds the result. Each feature enables those itself; `pdf` does not turn on
PNG output. The owned encoder (ADR-0004) moved out of the PNG backend so
either feature can compile it.

Everything else stays unconditional, because the SVG path uses it: `usvg`
parses and checks every nested SVG during resolution, which is the same for
all formats, and the image decoders, color management, shaping and font
subsetting serve SVG output too. The tests keep `resvg` and `tiny-skia` as
dev-dependencies to rasterize SVG artifacts, so SVG coverage runs without the
features.

## Consequences

- `cargo tree -p excalidraw-render --no-default-features --edges normal`
  lists `excalidraw-render-fonts` and the SVG path's crates only: no Krilla,
  resvg, tiny-skia or their own dependencies.
- Code only one format reads is compiled with that format. Data the shared
  resolution builds for one backend (decoded image samples, the canvas frame
  stroke, a PDF font's program) stays in every build and allows dead code only
  where its feature is off, so the default build still reports unread fields.
- The test suite targets the default features. In a reduced build every
  test that asks for a disabled format fails with
  `request.format-unavailable` and nothing else, and the SVG tests pass.
  Coverage owners must stay unconditional (ADR-0018), so tests are not
  feature-gated wholesale; only code that cannot compile without a format
  is, and the one owner among it asserts the refusal instead.
- The `lint:features` check runs clippy on the renderer without default
  features and with each one alone, so a reduced build cannot rot unnoticed.
