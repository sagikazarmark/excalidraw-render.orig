# Rust PDFium crate primary-source findings

Research for replacing Python `pypdfium2` in the PDF test harness.
Sources and the current working-tree harness checked on **2026-09-25**. This note
combines external crate source inspection with repository-specific fit analysis.
No implementation, compilation, or raster comparison was performed.

## Recommendation

1. **First investigate `pdfium-render =0.9.4`**, loading
   an independently checksum-pinned native PDFium library by explicit path. It
   has the broadest established release history in this comparison, exposes the
   required raw methods, and does not force a native download. First evaluate its
   explicitly configured safe fixed-size/matrix rendering against the existing
   controls: the workspace forbids owned unsafe code. If direct FFI is needed,
   an isolated tooling boundary with a deliberate lint exception is required.
   Keep the returned
   `Box<dyn PdfiumLibraryBindings>` and own the raw lifecycle if using this route.
   There are significant published-source caveats: the high-level bindings
   accessor is private despite README claims, and the matrix-render method in
   the `thread_safe` adapter lacks the lock used by adjacent methods. A
   single-threaded worker process avoids relying on that incomplete locking.
2. **`pdfium-bind-sys =0.1.0` is the clearest conventional sys alternative** when
   build-time bindgen/libclang and link-time dynamic-library packaging are
   acceptable. Supply `PDFIUM_DIR` with the exact library and matching headers;
   own initialization, synchronization, handles, and pixel normalization. Its
   short maintenance history and platform-specific build-script assumptions
   make it a higher-ownership choice.
3. **`firecrawl-pdfium =0.1.0` merits a small raw-layer evaluation**: its public
   `sys::Bindings` has the needed methods and a minimal dependency surface.
   However, the normal safe loader fixes default font paths, and its high-level
   renderer fits the page to rounded integer dimensions. A raw-only lifecycle
   using `Bindings::load_from_library()` is needed for unrestricted initial
   configuration; this bypasses the safe layer's process-global guard.

`pdfium-auto` and `pdfium-bundled` are acquisition helpers around `pdfium-render`,
not independent renderers. They add little for an already version-owned harness.
`pdfium-bind` has useful embedding/checksum machinery but its published public
API cannot express the requested raster contract. The other discovered wrappers,
forks, and sys crates are assessed below. These rankings are judgments based on
the cited source inspection, not proof of byte-equivalent output.

**All shortlisted crates wrap the native C/C++ PDFium engine.** Replacing Python
with Rust does not remove PDFium, its build flags, native dependencies, or its
platform-specific raster behavior.

## Fit to this repository

### What Python currently does

The dependency is **pypdfium2 5.12.1**, supplying native **PDFium
152.0.7947.0 / build 7947**, on pinned Linux x86_64 / Python 3.12.3. The lock hashes
the complete wheel. Installation extracts it and rendering imports its Python
helpers and raw bindings. The Python adapter is only 157 lines and already does
the important rendering through the native C API; Python is not essential to
that operation. Sources: [lock](../../../tools/pdf-harness/pdfium.lock.json),
[installer](../../../tools/pdf-harness/install.sh),
[launcher](../../../tools/pdf-harness/run-pdfium.sh),
[adapter](../../../tools/pdf-harness/pdfium/render.py).

The replacement needs to preserve:

- A single unrotated PDF 1.7 page, caller-supplied positive bitmap dimensions,
  and rejection when either scaled page dimension differs by at least one pixel.
- Exactly `96 / 72` scale, computed in binary64 for admission and passed as
  binary32 matrix fields for rendering. No rounded-page fitting.
- `FPDF_RenderPageBitmapWithMatrix`, transparent bitmap initialization,
  `FPDFBitmap_BGRA`, and only `FPDF_REVERSE_BYTE_ORDER` in the render flags.
- Checked packed top-left RGBA layout, **straight alpha**, and zeroing RGB only
  where alpha is zero. Forms and annotations are not drawn.
- The checked empty-font directory as the complete Linux user-font path list.
- Fresh-process execution, output bytes, failure behavior, and JSON receipts
  containing dimensions, digest, layout, and implementation/native identities.

These requirements come from the live adapter and
[harness README](../../../tools/pdf-harness/README.md), rather than assuming a
generic PDF-to-image interface is sufficient.

**The alpha question is resolved by current local code.** `canonicalize()` at
lines 141–153 explicitly avoids unpremultiplication. The independent
[alpha controls](../../../tools/pdf-harness/pdfium/test_alpha.py) cover vector
opacity and image soft masks with alpha 0, 1, 128, and 255, plus every alpha value
in the adapter-only check. The older
[`pdf-harness-primary-sources.md`](../pdf-harness-primary-sources.md) describes the
previous unpremultiplication behavior and must not be used as the current porting
contract. This review inspected the tests but did not execute them.

### Practical migration shape

1. **Preserve the exact native library initially.** Load the existing
   `target/pdf-harness/pdfium/pypdfium2_raw/libpdfium.so` by explicit path. The
   verified wheel can be unpacked as a ZIP without a Python runtime; alternatively
   pin a standalone native archive after comparing its actual library bytes.
   A matching build number alone does not prove binary equivalence. Record both
   the archive and extracted library hashes for the new acquisition path.
2. **Use a small development-only Rust worker with the existing process
   interface.** Start with `pdfium-render`'s safe API, custom font configuration,
   fixed bitmap dimensions, explicit scale/matrix, transparent clear, and forms/
   annotations disabled. Avoid default/system-fallback loading. Disable optional
   image conversion dependencies when using raw RGBA output. If exact behavior
   requires raw FFI, use an isolated non-published adapter with explicit unsafe
   ownership rather than inserting raw calls into `xtask`.
3. **Prove byte equivalence.** Compare Python and Rust on the existing fractional
   sizing/admission tests, both alpha controls, typography, images, and repeated
   fresh-process runs. Exercise invalid inputs as well as matching RGBA hashes.
   A successful native-library load is also necessary: the crate's latest API
   feature is 7881, while the harness native engine is 7947.
4. **Update implementation provenance.** Current validators explicitly require
   `pypdfium2`, Python, and the Python adapter's source hashes. A Rust worker must
   emit its real identity and refresh the corresponding profile/receipt
   validation and evidence, even if every pixel is identical. Sources:
   [`pdf_viewers.rs:303–348`](../../../xtask/src/pdf_viewers.rs),
   [`receipts.rs:143–158`](../../../xtask/src/representative/evidence/receipts.rs),
   [`conformance_pdf_requalification.rs`](../../../xtask/src/conformance_pdf_requalification.rs).

The raw approach is a concrete policy decision here: the workspace sets
`unsafe_code = "forbid"`, and `xtask` inherits it. Safe use of a dependency that
internally uses FFI is compatible with that lint; owned unsafe calls are not.
Sources: [workspace manifest](../../../Cargo.toml), [xtask manifest](../../../xtask/Cargo.toml).

### Utilities beyond the core adapter

The two raw-pixel audit utilities mostly reuse the adapter, but the
[typography diagnostic](../../../tools/pdf-harness/diagnostics/stage4-typography.py)
also enumerates page objects, inspects text-object matrices and font embedding,
and extracts glyph outlines using `FPDFFont_GetGlyphPath` and path-segment APIs.
That makes a broad binding such as `pdfium-render` more useful than a restricted
render-only wrapper for the wider Python-removal goal. Those utility APIs should
be included in the eventual compatibility spike; this review has not compiled
ports of them. Other Python utilities also perform work unrelated to PDFium, so
replacing this binding is one component of the broader migration.

**Decision:** PDFium does not require keeping Python. `pdfium-render` is the best
first candidate, with `pdfium-bind-sys` as a lower-level fallback if safe rendering
cannot reproduce the contract and a dedicated FFI boundary is acceptable. The
main work is preserving the existing controlled rendering and evidence contract,
not finding a Rust equivalent for Python syntax.

## Registry snapshot and maintenance evidence

Discovery used the [crates.io search API][registry-search], then each crate's
version records. Dates below are UTC `versions[].created_at` dates, not repository
push times. Every listed selected version was non-yanked in the response. The
registry returned releases through September 2026; no older-than-context registry
snapshot was observed. “Current” means the latest stable returned by that source
on this research date, not a forecast or a claim that all upstream development
is published.

| Crate / owner repository | Verified version | Published UTC | License metadata | Declared Rust version | Maintenance evidence |
| --- | --- | --- | --- | --- | --- |
| [`pdfium-render`][reg-render], `ajrcarey/pdfium-render` | `0.9.4` | 2026-09-06 | MIT OR Apache-2.0 | 1.61, dependency caveat below | 89 registry versions; `0.9.2` June 13, `0.9.3` July 14, `0.9.4` September 6; release notes describe font-path, double-free, and concurrency fixes |
| [`pdfium-auto`][reg-auto], `raphaelmansuy/edgequake-pdf2md` | `0.3.1` | 2026-07-31 | MIT OR Apache-2.0 | 1.91 | Four versions; `0.3.0` February 19; July release contains cache-poison/concurrent-extraction work |
| [`pdfium-bundled`][reg-bundled], `ceejbot/pdfium-bundled` | `0.1.1+pdfium-7881` | 2026-07-16 | MIT OR Apache-2.0 | 1.88 | Two releases, both July 16; small acquisition-helper history |
| [`pdfium-bind`][reg-bind], `houqp/kiorg` | `0.1.0` | 2025-12-26 | MIT | 1.85 | One registry release; docs.rs reports this version failed to build |
| [`pdfium-bind-sys`][reg-bind-sys], `hhubb22/pdfium-bind` | `0.1.0` | 2026-03-05 | MIT | Not declared; edition 2024 | One registry release |
| [`pdfium-rs`][reg-rs], `hhubb22/pdfium-bind` | `0.1.0` | 2026-03-05 | MIT | Not declared; edition 2024 | One registry release; safe companion to `pdfium-bind-sys` |
| [`pdfium-sys`][reg-sys], `aar0ndev/pdfium-sys` | `0.1.1` | 2021-03-31 | MIT OR Apache-2.0 | Not declared; edition 2018 | Two releases in March 2021; no newer registry release observed |
| [`firecrawl-pdfium`][reg-firecrawl], `firecrawl/pdfium-rs` | `0.1.0` | 2026-08-11 | MIT OR Apache-2.0 | 1.77 | `0.0.0` and `0.1.0` published the same day; young public release history |
| [`pdfium`][reg-pdfium], `newinnovations/pdfium-rs` | `0.10.4` | 2026-05-17 | GPL-3.0 | 1.87 | 51 registry versions; `0.10.2` January 11, `0.10.3` February 28, `0.10.4` May 17 |
| [`liteparse-pdfium-sys`][reg-lite-sys] / [`liteparse-pdfium`][reg-lite], `run-llama/liteparse` | both `1.11.0` | 2026-09-15 | Apache-2.0 | Not declared in registry | 14 / 13 registry versions; `1.9.0` August 27, `1.10.0` September 14, `1.11.0` September 15 |
| [`kreuzberg-pdfium-render`][reg-kreuzberg], `kreuzberg-dev/kreuzberg-lts` | `4.10.4` | 2026-09-21 | MIT | 1.91 | Recent ecosystem-specific fork release |
| [`xberg-pdfium-render`][reg-xberg], `xberg-io/xberg` | `1.2.9` | 2026-09-24 | MIT | 1.92 | Recent ecosystem-specific fork release |

Publication cadence is evidence of released maintenance, not a security or
support guarantee. The similarly named `pdfium-bind` and `pdfium-bind-sys` are
**different projects**, not a wrapper/sys pair. The actual companion of
`pdfium-bind-sys` is `pdfium-rs`.

Source precedence: versioned crate archives and docs.rs source were used for
behavioral claims. Archives were fetched from
`https://static.crates.io/crates/{name}/{name}-{version}.crate` and inspected in
memory. This matters because README prose and repository HEAD can differ from
the release. `pdfium-render 0.9.4` records VCS commit
[`6cee8b9a3951832ac0ff62ce4c32800278001cb8`][render-commit], whose commit date is
2026-08-16, while the registry publication is 2026-09-06; these are distinct facts.

## Native-library supply and exact pinning

| Candidate | Native supply actually implemented | Exact-build control and caveats |
| --- | --- | --- |
| `pdfium-render 0.9.4` | No engine bundled, downloaded, or built. Runtime `libloading` by path/system name; optional `static` API plus build-script dynamic/static link-path support. Browser support binds a separately supplied PDFium WASM module. | Explicit library path or owned static archive can preserve exact engine bytes. `pdfium_7881` etc. select **API declarations**, not the downloaded/loaded engine. `version()` returns the selected API version, not native provenance. Default `pdfium_latest` is `pdfium_7881`; no `pdfium_7947` feature exists in this release. Compatibility with a later exact native build requires verification. [Manifest][render-manifest], [loader][render-loader], [bindings][render-bindings], [build script][render-build]. |
| `pdfium-auto 0.3.1` | Runtime download/cache from bblanchon; optional `bundled` embeds a dynamic library at compile time and extracts it at runtime. Native default is hard-coded **7961** in both build/runtime source. Depends on `pdfium-render ^0.8`, not `0.9`. | `PDFIUM_LIB_PATH` supplies an existing file; `PDFIUM_BUNDLE_LIB` supplies embedded bytes. No arbitrary version parameter. Runtime cache checks minimum size (100,000 bytes), or exact embedded length, with advisory lock/atomic publish; these are **not cryptographic identity checks**. Bundled build downloader uses curl with PowerShell fallback. Invalid runtime override can fall through to download; `PDFIUM_NO_AUTO_DOWNLOAD` blocks download mode. `bind_*` calls `Pdfium::new()` with default configuration; acquisition-only API is needed to arrange custom initialization. [Source][auto-source], [build][auto-build], [manifest][auto-manifest]. |
| `pdfium-bundled 0.1.1+pdfium-7881` | Similar runtime acquisition/optional dynamic-library embedding, native default **7881**, depends on `pdfium-render ^0.9`. | Existing file override and `PDFIUM_BUNDLE_LIB` allow owned supply, but cache is existence-based and no archive digest verification was found in download/extraction paths. `OnceLock` stores a resolved path after extraction; it does not serialize the extraction itself. `bind_*` uses default initialization. Native-version suffix is metadata, not independent verification of loaded bytes. [Source][bundled-source], [build][bundled-build], [manifest][bundled-manifest]. |
| `pdfium-bind 0.1.0` | Default `dynamic`: build downloads headers/library, embeds library with `include_bytes!`, extracts and loads it. `static`: links archive. | Auto dynamic source is bblanchon **7592**, with per-platform SHA-256 checks on download. Auto static source is paulocoutinhox **7442c**, macOS only, also checksummed. Override `PDFIUM_DYNAMIC_LIB_PATH` or `PDFIUM_STATIC_LIB_PATH` (file paths) **and** `PDFIUM_INCLUDE_PATH`; own override identity externally. Existing extraction directories skip verification. Windows uses a fixed temp DLL name and, if overwrite fails, attempts to load the existing file: embedding alone does not guarantee exact loaded bytes there. [Build][bind-build], [dynamic loader][bind-dynamic]. |
| `pdfium-bind-sys 0.1.0` | Build-time download of bblanchon **7713**, generated FFI, link-time `dylib=pdfium`; no static mode. | `PDFIUM_DIR` overrides download and supplies `include/` and `lib/`. No download checksum was found. Build emits an absolute rpath and may rewrite the macOS dylib install name; redistribution/relocatability and resulting byte identity need explicit ownership. [Build][bind-sys-build], [FFI][bind-sys-lib]. |
| `pdfium-sys 0.1.1` | System dynamic linking; no native acquisition. Bindgen enabled by default, bundled generated Rust bindings when disabled. | Native pinning belongs entirely to the caller/toolchain. Old generated bindings and old build machinery need review against the chosen native headers and target ABI. [Build][sys-build], [bindings][sys-bindings]. |
| `firecrawl-pdfium 0.1.0` | Runtime `libloading`; no crate build-time native download/link step. Repository `xtask fetch-pdfium` is a separate convenience. | Explicit library file loading is available. Safe loader rejects a different path after initialization and retains the engine for process lifetime. Normal safe init has fixed font configuration; raw-table loading is separately public. [Loader][firecrawl-loader], [raw bindings][firecrawl-bindings], [manifest][firecrawl-manifest]. |
| `pdfium 0.10.4` | External runtime dynamic library, selected directory then system fallback. | Direct low-level loader can choose a directory; normal global helper falls back to system loading. No supplied engine identity. Public low-level API permits explicit config; high-level global initializer supplies default font paths. GPL metadata is materially different from the permissive alternatives. [C API][pdfium-c-api], [global guard][pdfium-guard], [manifest][pdfium-manifest]. |
| `liteparse-pdfium-sys 1.11.0` | Build resolves explicit dirs, workspace vendor dirs, then download/cache from **run-llama** PDFium binaries **8058**. Native uses runtime dynamic loading; WASM uses a static archive and WASI C/C++ runtime libraries. | Build overrides are `PDFIUM_LIB_PATH` and `PDFIUM_INCLUDE_PATH` **directories**. No archive checksum was found in download path. Dynamic table is incomplete for this task; see below. [Build][lite-build], [dynamic table][lite-dynamic]. |

Native “bundling” in `pdfium-auto`, `pdfium-bundled`, and default `pdfium-bind`
means **embedding a shared library then extracting/loading it**, not compiling
PDFium into pure Rust or statically linking it.

The binary producer [bblanchon/pdfium-binaries][binary-readme] describes weekly
builds since 2017, platform-specific release archives, and shared libraries only
for its precompiled releases; static builds require its build script/workflow
or another supplier. Its latest release response was
[`chromium/8066`, 2026-09-21 12:48:25 UTC][binary-latest]. The previously documented
harness engine build is available as
[`chromium/7947`, 2026-07-13 07:53:27 UTC][binary-7947]. A crate's default being
newer or older is not a reason to change that native pin during a wrapper-only
replacement. A release tag/build number alone also does not identify compiler,
patches, renderer backend, V8/XFA choices, or exact shared-library bytes.

## `pdfium-render`: strongest general candidate, with release-specific traps

### Raw access and initialization

The public `prelude::PdfiumLibraryBindings` trait includes unsafe
`FPDF_InitLibraryWithConfig`, `FPDF_DestroyLibrary`,
`FPDF_RenderPageBitmapWithMatrix`, bitmap create/fill/get-buffer/get-stride,
document/page lifecycle, and raw integer render flags. `FPDF_LIBRARY_CONFIG`,
`FS_MATRIX`, and `FS_RECTF` are re-exported. `Pdfium::bind_to_library()` returns a
public boxed trait object **without calling InitLibrary**; initialization happens
in `Pdfium::new()` / `new_with_config()`.
[Sources: bindings][render-bindings], [loader][render-loader],
[dynamic binding constructor][render-dynamic].

Important distinction: **the published 0.9.4 source declares
`PdfiumLibraryBindingsAccessor` as `pub(crate)`**, and there is no public inherent
`Pdfium::bindings()` method. The README says 0.9.2 made this trait public and
shows `pdfium.bindings()`, but that prose is not a reliable recipe for 0.9.4.
Using the boxed binding returned before `Pdfium::new()` remains a public raw
route. If it is moved into the high-level global singleton, do not assume it can
subsequently be retrieved through that README accessor. Likewise, the internal
`bindgen` module is private and the curated re-export list does not export all
`FPDF_*` constants; raw flags/format constants need an explicit, reviewed source.
[Sources: published loader lines 66–84, 173–188, 210–250][render-loader],
[crate exports][render-lib], [README][render-readme], [binding re-exports][render-bindings].

The high-level `new_with_config()` does support custom font paths using
`PdfiumLibraryConfig::set_user_font_paths()`. It retains `CString`s during init
and terminates a nonempty pointer list with null. **An empty Rust slice becomes
a null pointer**, meaning native defaults, not “no fonts.” A one-element list
containing a controlled empty directory is different. `BINDINGS` is a global
`OnceCell`: first initialization wins; subsequent high-level construction cannot
replace the configuration. High-level drop does not reset the global binding or
call `FPDF_DestroyLibrary`; it only frees an owned platform font provider.
[Sources: config][render-config], [loader/drop][render-loader].

### Geometry and pixel behavior

It would be incorrect both to assume `render_with_config()` is automatically
equivalent and to claim it can never express precise geometry:

- Defaults clear white, enable annotations and forms, and use auto-sizing.
- Auto-sizing derives scale from target dimensions or supplied factors and
  **rounds** the bitmap dimensions. With forms enabled the page render uses
  `FPDF_RenderPageBitmap`, whose integer size arguments determine page fitting.
- With forms disabled, it calls **`FPDF_RenderPageBitmapWithMatrix`**. Fixed-size
  mode keeps supplied bitmap dimensions independent of scale, and supplied
  `scale_page_by_factor()` values survive fixed-size mode. An explicit matrix,
  clip, format, transparent clear, annotation/form settings, reverse order,
  and smoothing flags can therefore be configured for evaluation.
- This is source evidence of expressive capability, not byte-equivalence.
  Target-size fitting, auto-rounding, extra rotation, matrix composition, and
  form rendering are different paths. The raw entry point exposes the fewest
  hidden policy choices for an existing exact matrix contract.
- `PdfBitmap::as_rgba_bytes()` copies/swaps color channels; it does **not**
  unpremultiply. `as_raw_bytes()` exposes a copy of the underlying bitmap bytes.
  If rendering via raw FFI, do not rely on the high-level object's internal
  reversed-byte-order bookkeeping being updated.

[Sources: render configuration, especially `apply_to_page()`][render-page-config],
[page render branch][render-page], [bitmap conversion][render-bitmap].

### Threading, build requirements, and MSRV

The README claims mutex serialization with the default `thread_safe` feature.
Published `src/bindings/thread_safe.rs` confirms locks on many calls, **but
`FPDF_RenderPageBitmapWithMatrix` at lines 1906–1915 forwards without acquiring
`PdfiumThreadMarshall::lock()`**, unlike `FPDF_RenderPageBitmap` immediately
above it. Treat this as an observed source-level locking gap, not a reproduced
race or an assertion that all operations are unsafe. Independent PDFium users
in the same process would also be outside this crate's mutex. Use one calling
thread/process ownership, or investigate/fix comprehensive synchronization before
depending on concurrent rendering. Custom callbacks re-entering locked bindings
can deadlock, as the loader docs explicitly note.
[Sources: thread-safe adapter][render-thread], [loader callback docs][render-loader].

Default builds use checked-in bindings and do not require libclang. The optional
`bindings` feature uses **bindgen** (despite README text saying cbindgen), which
requires its tooling. Dynamic use supports native library paths; static use can
require C++ runtime and macOS CoreGraphics linkage. WASM needs a separate native
engine compiled to WASM. Manifest `rust-version = 1.61` is not a verified MSRV
for today's resolved dependency graph: default `image_latest` selects `image
0.25`, and multiple other dependency ranges are broad. Disabling image support
reduces dependencies but does not substitute for checking the resolved lockfile.
[Sources: manifest][render-manifest], [build][render-build], [README][render-readme].

## Other substantive candidates

### `pdfium-bind 0.1.0`: public rendering policy is fixed

The root declares **private `mod ffi`**. `PdfDocument::open()` runs
`FPDF_InitLibrary()` through `Once`; it exposes no config/font-path argument.
`render_page(page_number, dpi)` rounds `page_dimension * dpi / 72`, allocates
BGRA, fills **opaque white**, calls ordinary `FPDF_RenderPageBitmap` with
**`FPDF_LCD_TEXT | FPDF_PRINTING | FPDF_ANNOT`**, and swaps B/R afterward.
There is no caller-supplied matrix, clip, bitmap size, flags, or transparent
background in that API. The dynamic FFI forwarding table itself contains only
a subset, excluding the two required config/matrix functions. Merely generating
all headers with bindgen does not make those functions publicly usable.
[Sources: wrapper][bind-lib], [FFI exports][bind-ffi], [dynamic table][bind-dynamic].

`PdfDocument` has `unsafe impl Send`, but `Once` only protects initialization;
no process-wide call mutex appears in this wrapper. Documents independently
opened on different threads can call native PDFium concurrently. No native
shutdown is called by document drop. Build always uses bindgen 0.72/libclang;
auto dynamic targets are Linux/macOS x64/arm64 and Windows x64/arm64/x86.
Static auto-download is macOS-only. Build code mixes target environment values
with host `cfg!` branches, so cross-build support should not be inferred from
the target table alone. [Sources: wrapper][bind-lib], [build][bind-build],
[docs.rs failed build status][bind-docs].

### `pdfium-bind-sys 0.1.0` and `pdfium-rs 0.1.0`

The sys crate publicly includes bindgen output generated from `fpdfview.h` and
other public headers. Its allowlists cover `FPDF.*`, `FS_.*`, and flags, so the
required config, matrix, bitmap, and reverse-order APIs come directly from the
supplied matching headers. It imposes no high-level geometry/alpha policy and
provides no lifecycle/synchronization abstraction. It requires bindgen 0.72 /
libclang at build time. Edition 2024 implies at least a Rust 1.85 language-toolchain
floor, but an exact tested MSRV is not declared.
[Sources: wrapper header][bind-sys-header], [build][bind-sys-build], [lib][bind-sys-lib].

Auto-download maps macOS x64/arm64, Linux glibc/musl x64/arm64, and Windows MSVC
x64/arm64; other targets can provide `PDFIUM_DIR`. The emitted `-Wl,-rpath,...`
is unconditional, and library-layout handling is not a demonstrated Windows
distribution solution. The safe companion `pdfium-rs` initializes via `Once` and
plain `FPDF_InitLibrary`; it does not improve custom-initialization control over
using the sys crate directly. [Sources: sys build][bind-sys-build],
[companion init][rs-library], [companion manifest][rs-manifest].

### `firecrawl-pdfium 0.1.0`: useful public raw table, fixed safe init

The crate has only `libloading 0.8` as a normal dependency and declares Rust
1.77. Its native discovery helpers identify macOS/Linux/Windows x64 and arm64;
there is no static/WASM build feature in the manifest. Its safe layer serializes
FFI through a process-global mutex and provides `ffi_lock()` plus unsafe `raw()`
access. Initialization and loading are separately guarded, and the native
library remains mapped for process lifetime.
[Sources: manifest][firecrawl-manifest], [loader][firecrawl-loader].

The public raw table contains `FPDF_InitLibraryWithConfig`, matrix rendering,
bitmap operations, and raw types/constants, and can be constructed directly
with unsafe `Bindings::load_from_library()`. However, the normal safe loader
immediately initializes version-2 config with **null user-font paths**; no custom
config argument exists there. High-level document/page handles are private.
For custom-font initialization plus raw rendering, use a wholly owned raw
lifecycle or change upstream API, rather than reinitialize behind live safe
objects. [Sources: raw table][firecrawl-bindings], [types][firecrawl-types],
[loader][firecrawl-loader], [page][firecrawl-page].

High-level `RenderConfig` supports transparent backgrounds, RGBA reverse byte
order, annotations/forms, and individual smoothing switches. Nevertheless,
`resolve_dimensions()` rounds sizes and `PdfPage::render()` calls ordinary
`FPDF_RenderPageBitmap` to fit exactly those integer dimensions. Its own comments
describe RGBA/BGRA as straight alpha; there is no unpremultiply step in that
render path. It is not a drop-in high-level exact-matrix renderer.
[Source: rendering implementation][firecrawl-render].

### `pdfium 0.10.4`: established alternative with different license

This is the crate `pdfium`, not `pdfium-rs`. It exposes a substantial public C
API, including config initialization, matrix rendering, flags and bitmap types.
Its global high-level API uses a `OnceLock<ReentrantMutex<...>>`; direct low-level
calls need caller synchronization. Global initialization supplies default fonts,
and location-based high-level loading falls back to a system library. It requires
an external native engine and has mandatory `image 0.25.9`-range dependency.
The registry/manifest say GPL-3.0; source headers additionally say “or any later
version.” Record the license difference rather than assume the permissive terms
of other wrappers. This was not ranked above them because the task is a narrow
controlled raster binding, not adoption of another large high-level API.
[Sources: C API][pdfium-c-api], [FFI methods][pdfium-methods],
[guard][pdfium-guard], [manifest][pdfium-manifest].

### `liteparse-pdfium-sys 1.11.0`: raw declarations are not its dynamic table

The checked-in `bindings.rs` declares `FPDF_InitLibraryWithConfig`,
`FPDF_RenderPageBitmapWithMatrix`, `FPDF_REVERSE_BYTE_ORDER`, BGRA, and BGRA_Premul.
But **`src/dynamic.rs::PdfiumBindings` omits both config-init and matrix-render
functions**; it exposes plain `FPDF_InitLibrary` and the subset used by liteparse.
Native build intentionally emits no `dylib=pdfium` link dependency. Thus the
presence of raw extern declarations does not make them callable through the
provided dynamic table; using those externs requires additional linking, or the
dynamic table must be extended. The sys layer's `OnceLock` caches loading, not
general native-call serialization. [Sources: declarations][lite-bindings],
[dynamic table][lite-dynamic], [build][lite-build].

Default builds copy pregenerated bindings; libclang is needed only with its
optional `bindgen` feature. Download target mapping includes macOS, Linux,
Windows, and WASI/WASM; notably aarch64-musl maps to the same Linux ARM64 asset as
glibc, so portability should be checked rather than inferred. WASM also links
WASI libc/libc++/libc++abi and emulation libraries; this is still native PDFium
compiled to WASM. The companion `liteparse-pdfium` is an active application-
oriented wrapper, but its current sys-loader limitations make it a less direct
choice for this exact raw API subset. [Sources: build][lite-build],
[companion registry][reg-lite].

### `pdfium-sys 0.1.1` and ecosystem forks

The old `pdfium-sys` really exists and its generated bindings include the required
config/matrix APIs and reverse-order constant. It provides no native download or
guard. The default bindgen 0.54 build regenerates **inside `src/bindings.rs`**
rather than `OUT_DIR`; disabling default features uses 2021 generated bindings.
It links the system library (`pdfium.dll` in the Windows branch), with no owned
path/acquisition abstraction. Absence of a newer registry release since 2021,
old tooling, and ABI review burden place it below the current options.
[Sources: build][sys-build], [bindings][sys-bindings], [registry][reg-sys].

The discovered `kreuzberg-pdfium-render 4.10.4` and `xberg-pdfium-render 1.2.9`
are real maintained forks, not invented sys names. Their inspected manifests,
loaders, and build scripts retain explicit dynamic/static supply paths and
custom-font configuration, with ecosystem-specific API/build changes and higher
MSRVs. They are potential follow-ups if a specific upstream limitation is already
fixed there, but this audit did not establish their exact-7947 ABI, raster, and
locking equivalence. Recent release dates alone do not establish that. Sources:
[Kreuzberg source tree][kreuzberg-source], [Xberg source tree][xberg-source].

Search also returned application-specific `ferrugo-pdfium`, `piff-pdfium`, and
`leptos-pdf-pdfium-bundle`; none was established here as a more direct general
native binding candidate. `rpdfium` describes itself in registry metadata as a
Rust port. That claim was not audited, and a port would not preserve the identity
of the selected native PDFium engine. It is outside this wrapper-replacement
comparison. [Source: registry discovery][registry-search].

## Native API facts that constrain every wrapper

These claims were followed to PDFium's **`chromium/7947`** header and implementation,
because native behavior, not wrapper marketing, owns the raster semantics.

### Initialization, font paths, and threads

- The header explicitly says **none of PDFium's APIs are thread-safe**. Calls
  must occur on one thread or be serialized so only one PDFium call runs at a
  time. A crate's `Send` implementation or an initialization `Once` is not a
  replacement for that rule. [Header][native-header].
- `FPDF_LIBRARY_CONFIG::m_pUserFontPaths` is a null-terminated pointer array;
  null means default paths and the field may be ignored depending on platform.
  Linux `ParseFontCfg()` returns false only for a null array; a nonnull array
  uses the supplied paths, bypassing hard-coded default directories. An empty
  directory in that list can therefore isolate Linux font discovery. This is
  not a portable promise of identical Windows/macOS font behavior.
  [Header][native-header], [Linux implementation][native-linux-fonts].
- Initialize before loading documents; close owned objects before destroying
  the library. Keep one owner for process-global initialization and destruction.
  Multiple wrappers in one process need a shared ownership/synchronization
  policy, even if each has its own mutex. [Header][native-header].

### Matrix, dimensions, and flags

`FPDF_RenderPageBitmapWithMatrix` takes `FS_MATRIX` float fields and a clipping
rectangle. In 7947 it starts from `pPage->GetDisplayMatrix()` and composes the
caller matrix on top. `GetDisplayMatrix()` uses the **floating page rectangle**,
not a caller's rounded bitmap rectangle. Ordinary `FPDF_RenderPageBitmap`
instead takes integer destination dimensions. A high-level DPI method that
rounds size and uses the latter entry point can change fractional-page geometry.
The matrix is not simply a replacement PDF bottom-left-to-top-left transform:
PDFium's base display transform, including intrinsic page rotation, already
participates. [Render implementation][native-render], [page matrix][native-page].

`FPDF_REVERSE_BYTE_ORDER` is `0x10`; it affects rendering byte order. Annotation,
LCD, printing, halftone, native text, and per-category no-smoothing flags are
independent choices. Form-field drawing is an additional operation, not merely
another name for annotations. A raw port can retain its exact flags and skip
form drawing rather than inherit a wrapper's display/print defaults.
[Header][native-header], [pdfium-render form branch][render-page].

### Alpha: do not equate RGBA with premultiplied RGBA

**Verified native contract:** the 7947 header defines `FPDFBitmap_BGRA = 4` as
“Pixel components are independent of alpha” (straight), and separately defines
experimental Skia-only `FPDFBitmap_BGRA_Premul = 5` as premultiplied.
`FPDF_RenderPageBitmapWithMatrix` uses a `ScopedPremultiplier` for Skia, whose
destructor unpremultiplies a bitmap that entered as ordinary BGRA. Reverse byte
order swaps channel layout; it does not request premultiplied storage.
[Header][native-header], [render implementation][native-render],
[premultiplier implementation][native-bitmap].

Consequently, neither `as_rgba_bytes()` nor “returns RGBA” proves that an
unpremultiply pass is needed. Applying one to already-straight RGB changes
partially transparent colors. The older local research note describes that
previous behavior, but the **current adapter and alpha controls already agree
with the native straight-alpha contract**, as detailed in the repository-fit
section. Preserve the current behavior when porting. Zero-alpha RGB
canonicalization and row/stride handling are separate concerns.

### Native license and toolchain

Wrapper license metadata does not replace the native engine's license. PDFium
has a BSD-style license with included additional license text and third-party
components. Keep the selected binary distribution's notices with native
provenance. Building the engine itself is a separate C++ build concern; the
Rust wrappers assessed here do not implement PDFium in Rust.
[PDFium license][native-license], [binary producer/build guidance][binary-readme].

## Unresolved questions for fit synthesis / later qualification

1. Does the intended native artifact remain exactly the existing pinned binary,
   or will an alternate producer/static build require new evidence? Compare
   actual bytes and build flags, not just the Chromium build number.
2. Can `pdfium-render 0.9.4`'s `pdfium_7881` declaration/symbol set load that exact
   engine? Dynamic binding eagerly resolves a broad API set; this research did
   not execute a load. A missing symbol can be unrelated to raster functions.
   [Source: dynamic binding constructor][render-dynamic].
3. Is raw-only ownership acceptable, given 0.9.4's private accessor and omitted
   public constants? If mixed high-/low-level use is preferred, verify/fix the
   published accessor behavior rather than copying the README example.
4. Will rendering stay in fresh single-threaded workers? If concurrency is
   required, audit/fix the matrix-lock gap and coordination with every native
   caller; crate `thread_safe` naming alone is insufficient.
5. Do the ported alpha controls still establish the current straight-alpha
   behavior? Current local code resolves the older documentation discrepancy;
   no second unpremultiplication should be introduced.
6. Which configurations reproduce bitmap size, fractional geometry, clipping,
   flags, font isolation, stride, and transparent RGB exactly? Source inspection
   identifies paths to try, but does not certify equivalent raster results.
7. What is the resolved dependency MSRV and deployment target matrix? Declared
   MSRV, edition floor, docs.rs build success, and successful native packaging
   are different pieces of evidence. No builds/tests were requested here.

## Sources

[registry-search]: https://crates.io/api/v1/crates?page=1&per_page=100&q=pdfium
[reg-render]: https://crates.io/api/v1/crates/pdfium-render
[reg-auto]: https://crates.io/api/v1/crates/pdfium-auto
[reg-bundled]: https://crates.io/api/v1/crates/pdfium-bundled
[reg-bind]: https://crates.io/api/v1/crates/pdfium-bind
[reg-bind-sys]: https://crates.io/api/v1/crates/pdfium-bind-sys
[reg-rs]: https://crates.io/api/v1/crates/pdfium-rs
[reg-sys]: https://crates.io/api/v1/crates/pdfium-sys
[reg-firecrawl]: https://crates.io/api/v1/crates/firecrawl-pdfium
[reg-pdfium]: https://crates.io/api/v1/crates/pdfium
[reg-lite-sys]: https://crates.io/api/v1/crates/liteparse-pdfium-sys
[reg-lite]: https://crates.io/api/v1/crates/liteparse-pdfium
[reg-kreuzberg]: https://crates.io/api/v1/crates/kreuzberg-pdfium-render
[reg-xberg]: https://crates.io/api/v1/crates/xberg-pdfium-render
[render-commit]: https://github.com/ajrcarey/pdfium-render/commit/6cee8b9a3951832ac0ff62ce4c32800278001cb8
[render-manifest]: https://docs.rs/crate/pdfium-render/0.9.4/source/Cargo.toml
[render-readme]: https://docs.rs/crate/pdfium-render/0.9.4/source/README.md
[render-build]: https://docs.rs/crate/pdfium-render/0.9.4/source/build.rs
[render-lib]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/lib.rs
[render-loader]: https://docs.rs/pdfium-render/0.9.4/src/pdfium_render/pdfium.rs.html
[render-bindings]: https://docs.rs/pdfium-render/0.9.4/src/pdfium_render/bindings.rs.html
[render-dynamic]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/bindings/dynamic_bindings.rs
[render-config]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/config.rs
[render-page-config]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/pdf/document/page/render_config.rs
[render-page]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/pdf/document/page.rs
[render-bitmap]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/pdf/bitmap.rs
[render-thread]: https://docs.rs/crate/pdfium-render/0.9.4/source/src/bindings/thread_safe.rs
[auto-source]: https://docs.rs/pdfium-auto/0.3.1/src/pdfium_auto/lib.rs.html
[auto-build]: https://docs.rs/crate/pdfium-auto/0.3.1/source/build.rs
[auto-manifest]: https://docs.rs/crate/pdfium-auto/0.3.1/source/Cargo.toml
[bundled-source]: https://docs.rs/crate/pdfium-bundled/0.1.1+pdfium-7881/source/src/lib.rs
[bundled-build]: https://docs.rs/crate/pdfium-bundled/0.1.1+pdfium-7881/source/build.rs
[bundled-manifest]: https://docs.rs/crate/pdfium-bundled/0.1.1+pdfium-7881/source/Cargo.toml
[bind-docs]: https://docs.rs/crate/pdfium-bind/0.1.0
[bind-build]: https://docs.rs/crate/pdfium-bind/0.1.0/source/build.rs
[bind-lib]: https://docs.rs/crate/pdfium-bind/0.1.0/source/src/lib.rs
[bind-ffi]: https://docs.rs/crate/pdfium-bind/0.1.0/source/src/ffi/bindgen.rs
[bind-dynamic]: https://docs.rs/crate/pdfium-bind/0.1.0/source/src/ffi/dynamic_lib.rs
[bind-sys-build]: https://docs.rs/crate/pdfium-bind-sys/0.1.0/source/build.rs
[bind-sys-lib]: https://docs.rs/crate/pdfium-bind-sys/0.1.0/source/src/lib.rs
[bind-sys-header]: https://docs.rs/crate/pdfium-bind-sys/0.1.0/source/wrapper.h
[rs-library]: https://docs.rs/crate/pdfium-rs/0.1.0/source/src/library.rs
[rs-manifest]: https://docs.rs/crate/pdfium-rs/0.1.0/source/Cargo.toml
[sys-build]: https://docs.rs/crate/pdfium-sys/0.1.1/source/build.rs
[sys-bindings]: https://docs.rs/crate/pdfium-sys/0.1.1/source/src/bindings.rs
[firecrawl-manifest]: https://docs.rs/crate/firecrawl-pdfium/0.1.0/source/Cargo.toml
[firecrawl-loader]: https://docs.rs/crate/firecrawl-pdfium/0.1.0/source/src/library.rs
[firecrawl-bindings]: https://docs.rs/crate/firecrawl-pdfium/0.1.0/source/src/sys/bindings.rs
[firecrawl-types]: https://docs.rs/crate/firecrawl-pdfium/0.1.0/source/src/sys/types.rs
[firecrawl-page]: https://docs.rs/crate/firecrawl-pdfium/0.1.0/source/src/page.rs
[firecrawl-render]: https://docs.rs/crate/firecrawl-pdfium/0.1.0/source/src/render.rs
[pdfium-manifest]: https://docs.rs/crate/pdfium/0.10.4/source/Cargo.toml
[pdfium-c-api]: https://docs.rs/crate/pdfium/0.10.4/source/src/c_api/mod.rs
[pdfium-methods]: https://docs.rs/crate/pdfium/0.10.4/source/src/c_api/pdfium.rs
[pdfium-guard]: https://docs.rs/crate/pdfium/0.10.4/source/src/c_api/guard.rs
[lite-build]: https://docs.rs/crate/liteparse-pdfium-sys/1.11.0/source/build.rs
[lite-bindings]: https://docs.rs/crate/liteparse-pdfium-sys/1.11.0/source/bindings.rs
[lite-dynamic]: https://docs.rs/crate/liteparse-pdfium-sys/1.11.0/source/src/dynamic.rs
[kreuzberg-source]: https://docs.rs/crate/kreuzberg-pdfium-render/4.10.4/source/
[xberg-source]: https://docs.rs/crate/xberg-pdfium-render/1.2.9/source/
[binary-readme]: https://github.com/bblanchon/pdfium-binaries/blob/master/README.md
[binary-latest]: https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/8066
[binary-7947]: https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/7947
[native-header]: https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/public/fpdfview.h
[native-render]: https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/fpdfsdk/fpdf_view.cpp
[native-page]: https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/core/fpdfapi/page/cpdf_page.cpp
[native-bitmap]: https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/core/fxge/dib/cfx_dibitmap.cpp
[native-linux-fonts]: https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/core/fxge/linux/fx_linux_impl.cpp
[native-license]: https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7947/LICENSE
