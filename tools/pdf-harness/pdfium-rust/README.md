# Isolated PDFium worker

Maintained native PDF harness adapter using the safe `pdfium-render 0.9.4`
API (`pdfium_7881`, default features disabled). This unpublished crate has its
own workspace and pinned dependency lock; owned Rust code forbids `unsafe`.

From the repository root, with Rust **1.97.1** and the pinned native library
already installed:

```sh
rustup run 1.97.1 cargo build --locked --manifest-path tools/pdf-harness/pdfium-rust/Cargo.toml --target-dir target/pdf-harness/pdfium-rust
target/pdf-harness/pdfium-rust/debug/excalidraw-render-pdfium input.pdf output.rgba 112 84
rustup run 1.97.1 cargo test --locked --manifest-path tools/pdf-harness/pdfium-rust/Cargo.toml --target-dir target/pdf-harness/pdfium-rust
```

## Contract

- Linux x86_64 only, one render per fresh worker process. PDFium has global
  initialization state; integration tests invoke `CARGO_BIN_EXE_excalidraw-render-pdfium`
  rather than initializing PDFium in the test process.
- Load only `target/pdf-harness/pdfium/pypdfium2_raw/libpdfium.so` relative to the
  build-time crate's repository root (three parents). Its SHA-256 must be
  `61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33`,
  identifying PDFium `152.0.7947.0`. No Python runtime is used.
- AGG rendering, FreeType, and the existing verified
  `tools/pdf-harness/pdfium/empty-fonts/.keep` directory. Constants are compiled
  into the worker; the installer owns native artifact acquisition.
- Exactly one unrotated PDF 1.7 page. Positive signed-32-bit bitmap dimensions
  (with representable packed stride), each strictly less than one pixel from
  the page's 96-DPI size. Render at exactly `96/72` scale at the origin, clipping
  or leaving transparent padding to the requested dimensions; never fit/stretch.
- Transparent BGRA bitmap with reverse byte order, annotations/forms disabled.
  Output is packed top-left row-major **straight-alpha sRGB RGBA8**. Only RGB at
  alpha zero is cleared; nonzero-alpha samples are never unpremultiplied again.
- Success writes raw bytes then one JSON receipt line to stdout. Receipt fields:
  `adapter` (`pdfium-render/0.9.4`), `pdfium`, `library_sha256`, `width`, `height`,
  `byte_length`, `layout` (`top-left-row-major`), `pixel_format`
  (`straight-alpha-srgb-rgba8`), and `sha256` of the output bytes.
- Rejection exits nonzero with a stderr diagnostic and no receipt; validation
  completes before opening output. Existing output remains untouched on rejected
  input. Output symlinks, including dangling symlinks, are rejected. The launcher
  owns repository containment checks for input/output paths. Normal filesystem
  write failures are not transactional, and paths must not be concurrently changed.

Tests hand-author PDF objects and xrefs independently of the production PDF
writer. They pin exact vector-opacity and image-soft-mask samples, all 256 alpha
values, fractional-page scale/origin, fresh-process byte/receipt repeatability,
profile rejection, invalid dimensions, and untouched outputs/symlink targets.
