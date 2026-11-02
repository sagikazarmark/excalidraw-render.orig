# Serving untrusted input

The renderer is built to take scene documents from anyone: every input is
validated, and all work a scene can request is bounded by resource limits, so
every render terminates. A service still has to decide how much CPU time and
memory one request may use. This guide covers the decisions; the
[`untrusted` example](../../crates/excalidraw-render/examples/untrusted.rs)
puts them together.

## Run renders off the async executor

Rendering is synchronous and CPU-bound, from a few milliseconds for a small
diagram to seconds for a large PNG. In an async server, run it on a blocking
pool rather than on the executor's worker threads:

```rust,ignore
let png = tokio::task::spawn_blocking(move || {
    let scene = SceneDocument::parse(&body)?;
    let outcome = renderer.render(&scene, &request)?;
    Ok::<_, Box<dyn std::error::Error + Send + Sync>>(outcome.into_parts().0.into_bytes())
})
.await??;
```

`Renderer`, `SceneDocument` and the requests are `Clone + Send + Sync`: create
the renderer and the request once and share them. Bound concurrency (a
semaphore, or the size of the blocking pool), since every render in flight
holds its own working memory.

## Bound the input before parsing

`SceneDocument::parse` accepts up to 64 MiB of JSON, whatever the request's
`max_scene_bytes` (32 MiB by default) says; that limit is checked when
rendering. Reject larger bodies before you read them all: a size limit on the
request body in your HTTP framework, or a bounded read.

Parsing has its own fixed ceilings on nesting depth, string length and
structure size, so a hostile document fails with a `scene.*` error instead of
exhausting memory.

## Bound the output size

For PNG and PDF, use `Sizing::MaxDimension` rather than a caller-chosen
`Sizing::Scale`: a large scene then shrinks to fit instead of asking for an
enormous image. SVG ignores `MaxDimension`; its logical size is bounded by
`max_svg_dimension`.

## Lower the limits that cost memory

`ResourceLimits` holds deterministic ceilings on element count,
text length, geometry, image bytes and pixels, nesting depth, raster size,
output size, work units and more. Crossing one fails the render with a
`resource.*` error that names the limit and the measured value, identically on
every machine.

The defaults admit large drawings rather than protect a shared server. The
memory that matters is the PNG raster surface: PNG rendering holds a 4×
supersampled canvas, about 65 bytes per output pixel, so the default
`max_raster_surface_pixels` (33,554,432) admits single renders that need over
2 GiB. A ceiling of 4,194,304 pixels (2048 × 2048) keeps one render near
300 MB:

```rust,no_run
use excalidraw_render::{OutputFormat, RenderRequest, ResourceLimits, Sizing};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let limits = ResourceLimits::builder()
    .max_scene_bytes(8 * 1024 * 1024)
    .max_raster_surface_pixels(4_194_304)
    .max_total_rasterized_pixels(8_388_608)
    .build()?;
let request = RenderRequest::builder(OutputFormat::Png)
    .sizing(Sizing::MaxDimension(2048.0))
    .resource_limits(limits)
    .build()?;
# let _ = request;
# Ok(())
# }
```

`max_working_bytes` is not a memory cap: it budgets the renderer's logical
working set so the same input fails the same way everywhere, and does not
measure allocations. Size the worker's memory (or its container's limit) from
your raster ceilings times your concurrency.

No limit can be raised above `ResourceLimits::absolute_maxima()`.

## Frames multiply the work

Limits apply to each render. `Renderer::render_frames` renders every selected
frame within its own limits, so a scene with thousands of small frames is
thousands of renders. Bound the number of frames you accept, or render one
requested frame with `Renderer::render_frame`. The CLI's `frames` command
refuses more than 256 frames unless told otherwise (`--max-frames`).

## Deadlines and hard memory ceilings need a process

There is no deadline or cancellation: a render cannot be interrupted once it
starts. With the default limits, one render of hostile input takes at most a
few seconds of CPU on a current core, but when you need a hard wall-clock
deadline or a guaranteed memory ceiling, render in a separate process you can
kill: a worker pool of `excalidraw-render` CLI processes, or of your own
binary, under a timeout and an OS memory limit (`ulimit -v`, a cgroup or a
container limit). `catch_unwind` is not a substitute, since it does not help
against running out of memory or a build with `panic = "abort"`.

## Return useful errors

Every failure type has a stable code and a serializable `descriptor()`, and
the report of a successful render implements `serde::Serialize`. Return them
as JSON, and let clients match on `code`:

```json
{"family":"scene_parse","category":null,"code":"scene.invalid-envelope","message":"scene document has no \"version\" member; expected envelope version 2","format":null,"element_id":null,"json_pointer":"/version","details":null}
```

## Checklist

- Optimized build (`--release`, or `opt-level = 2` for dependencies in debug).
- Renders on a blocking pool, with bounded concurrency.
- Request bodies bounded before parsing.
- `Sizing::MaxDimension` for PNG and PDF.
- Lowered `max_raster_surface_pixels` and `max_total_rasterized_pixels`.
- Frame batches bounded.
- A separate, killable process when you need a hard deadline or memory ceiling.
