# Canonical PDF harness

This development-only harness qualifies PDFium and PDF.js as independent 96 DPI
PDF viewers and qpdf as the independent structural parser. None of these tools
is a production renderer dependency.

Install exact-pinned tools and package locks from the network:

```sh
bash tools/pdf-harness/install.sh
```

The installer verifies the standalone `bblanchon/pdfium-binaries` build 7947
archive and relocatable qpdf archive by SHA-256, extracts them with `tar` and
`unzip`, checks the extracted PDFium library hash, builds the isolated Rust
PDFium worker, and installs the exact PDF.js npm lock in the canonical
Linux/amd64 Node container. The PDFium archive contains byte-identical
`libpdfium.so` to the previously used wheel; it installs native libraries,
headers, build metadata and licenses without Python packages. Installed bytes
live under ignored `target/` or `node_modules/` paths.
Viewer and parser execution is offline after installation.

For just PDFium and its Rust worker (Linux x86_64, no Docker required):

```sh
bash tools/pdf-harness/install.sh --pdfium-only
# Or from devenv, using the same pinned installer:
devenv shell pdfium-install
```

`pdfium-render` supplies Rust bindings, not a bundled native engine. The devenv
command deliberately uses the same upstream archive as CI rather than a Nixpkgs
PDFium build. A different engine build would need rendering qualification; merely
sharing the version number does not establish identical output. Re-running the
installer replaces the old wheel extraction with `pdfium/lib/libpdfium.so` and
the native distribution's licenses. An old wheel in `target/pdf-harness/downloads/`
is an unused download cache and can be removed.

The narrow process interfaces are:

```sh
bash tools/pdf-harness/run-pdfium.sh render input.pdf output.rgba width height
bash tools/pdf-harness/run-pdfjs.sh render input.pdf output.rgba width height
bash tools/pdf-harness/run-structure.sh inspect input.pdf expectation.json pass output.json
bash tools/pdf-harness/run-xtask.sh generate-fixtures target/pdf-fixtures
```

Structural inspection and fixture generation run the Rust `xtask pdf-harness`
commands. The wrappers accept `XTASK_PDF_HARNESS` as a prebuilt xtask executable
(prefer an absolute path):

```sh
cargo build --locked -p xtask
export XTASK_PDF_HARNESS="$PWD/target/debug/xtask"
```

Without that override, the launcher uses `cargo run --locked` with the absolute
xtask manifest and a dedicated `target/pdf-harness/xtask` target directory. This
avoids the parent target lock when renderer tests invoke the structural wrapper
from `cargo test`. Cargo checks freshness on each invocation and reuses compiled
artifacts; there is no unchecked stale-binary shortcut. Reserve that directory
for the launcher, or supply the prebuilt override when testing in it. Inspection
keeps the repository path checks and pinned qpdf selection; it needs no Python.

PDFium emits packed top-left straight RGBA after explicit byte-order selection
and zeroes invisible RGB. Independent vector-opacity and image-soft-mask controls
in `pdfium-rust/tests/` verify its straight-alpha bitmap representation; no
second unpremultiplication is applied. PDF.js accepts only in-memory bytes, disables
network, streaming, range fetching, system fonts, external font data, forms,
annotations, XFA, WASM, and hardware acceleration, and emits canvas
`getImageData` bytes. Both adapters reject nonconforming page shape before
writing a canonical buffer.

Both viewers render the page at exactly `96 / 72` scale with the page origin at
the canvas origin onto a canvas of exactly the supplied `width` and `height`.
A page is admitted when each of its 96 DPI dimensions is within one pixel of
the supplied size, so an integral page must match exactly; a rejected request
names the page's 96 DPI size and the supplied size. The page is never scaled,
stretched, or offset to fit: device pixels beyond the canvas are discarded and
canvas pixels the page does not cover stay transparent. PDFium renders through
`FPDF_RenderPageBitmapWithMatrix` using fixed-size safe `pdfium-render`
configuration; bitmap dimensions and matrix scale are independent.
The `#[ignore]`d `harness_pdf_viewers::adapters` tests in
`crates/excalidraw-render` pin this contract for both adapters.

The PDFium library is initialized with the checked `empty-fonts/` directory as
its complete user font search path. Structural admission separately requires
every used text font to be embedded, so host font directories cannot affect a
candidate render.

The structural adapter first runs `qpdf --check`, retains normalized warning
text, and treats every warning as failure. It then consumes qpdf JSON v2 and
uses an owned content tokenizer to report operations and resource inventories.
Page inheritance, Form XObjects, and soft-mask forms are traversed recursively.
The expectation file supplies exact operation-sequence and structural-inventory
digests in addition to readable field checks; the report always contains the
observed data even when that gate fails.

The existing qualification manifest and profiles retain their approved
Python-era producer identities. Verification treats the explicitly enumerated
retired or migrated producer sources as historical provenance, including the
Python inspector, fixture generator and PDFium adapter/alpha tests, migrated
wrappers, installer and `xtask/src/pdf_viewers.rs`. Their descriptor shape and
required membership are checked; approval metadata is historical (ADR-0019).
Parser/viewer locks, schemas, comparator, fixtures, expectations, reports and
other live bindings are still verified against current bytes. The PDFium
profile still names the retired pypdfium2 wheel lock; that record is historical
and checked for shape only. The Rust worker's native library identity is
recorded in `pdfium-native.lock.json`.

The Rust inspector also validates the expectation object before invoking qpdf;
missing or malformed fields are configuration errors, even when the requested
PDF outcome is `fail`. Rust tests compare generated fixtures and all 14 retained
calibration/backend structural reports against their checked-in bytes, without
executing Python. Independent protocol and expectation tests cover malformed
and boundary inputs. Nonempty malformed qpdf JSON uses
Rust parser diagnostics rather than promising byte-identical Python exception
wording; the malformed PDF still fails structural qualification.

### Historical tooling and retained checks

The Python fixture generator, structural inspector, PDFium adapter and alpha
tests, and optional Python parity driver have been deleted. The completed PDF
backend requalification production commands are retired.
`xtask/src/conformance_pdf_requalification.rs` retains test-only scoped approval
and exact pin-transition checks. Fixture reproduction, structural report replay,
wrapper checks and independent Rust PDFium alpha controls remain active.

Deleted diagnostic and proposal drivers described in archived research are
historical reproduction recipes, with source available in Git history.
`node tools/pdf-harness/diagnostics/webp191-verify-boundary.mjs` is the offline
WebP evidence audit. It checks retained artifact hashes, all 144 paired viewer
observations, and 672 negative buffer/semantic mutations. It also runs as
`cargo test --locked -p xtask --test webp_boundary`; no viewers are launched.

After installing the pinned harness tools, run the current Rust checks:

```sh
cargo test --locked -p xtask \
  --test pdf_fixtures --test pdf_inspection --test pdf_inspection_expectations \
  --test pdf_inspection_parity --test pdf_harness_wrappers
```

### Rust PDFium runtime

The old `candidate-pdf-requalify`, `pdf-requalification-subject`, and
`refresh-pdf-viewer-dependencies` commands are retired. They now return explicit
guidance instead of reading deleted producers or refreshing historical hashes.
Use current harness reproduction and `conformance verify-pdf-viewers` for
verification; meaningful historical pin-transition assertions remain Rust tests.

`pdfium-rust/` is an isolated unpublished crate with a separate lockfile and
`unsafe_code = "forbid"`. The launcher uses its release binary via locked Cargo,
with an isolated target directory so it can be called from workspace tests.
It verifies the exact library SHA-256, selects AGG/FreeType explicitly, and uses
the controlled empty font directory. `pdfium-native.lock.json` records native
archive/library identity.
`pdfium-rust/sources.json` is the shared producer-file inventory consumed by the
Rust and Node collection paths.

Fresh receipts contain `adapter: pdfium-render/0.9.4` and `library_sha256`, not
`pypdfium2`. Representative evidence accepts either a complete historical Python
receipt/source set or the current Rust set; mixed identities are rejected.
The approved profile's original Python runtime record is preserved as provenance,
and all retained raster bytes remain unchanged. The installer/launcher descriptors
are historical; the library is checked by the worker on every render.

`pdfium-runtime.json` records the current Rust producer and six fresh-process
receipts (two renders of each retained calibration PDF), verified byte-for-byte
against the retained rasters. Refresh it to record a new calibration run:

```sh
cargo run --locked -p xtask -- conformance refresh-pdfium-runtime
cargo run --locked -p xtask -- conformance verify-pdf-viewers
```

Refresh executes the real viewer and rejects changed pixels or receipts before
writing the record. Verification checks native-lock identity and receipt bindings
offline; source hashes are historical and do not require refresh after edits.
New collections no longer include the wheel lock;
historical Python and early Rust inventories remain readable.

```sh
cargo test --locked --release --manifest-path tools/pdf-harness/pdfium-rust/Cargo.toml \
  --target-dir target/pdf-harness/pdfium-rust
```

The prototype was retired after safe-API parity: 92 distinct PDFs from 502 tracked
paths, four independent alpha/fractional controls, and 11 rejection comparisons,
each run twice per adapter. The maintained worker keeps the same configuration.

### Reproduction

Reproduce every fixture, fresh-process render, comparison, and structural report:

```sh
bash tools/pdf-viewer-qualification.sh reproduce
```

The valid PDF is explicitly a vector-only calibration control rather than a
renderer snapshot or reference PDF. Official SVG rendered by canonical Chromium
remains the visual authority. That authority has one recorded blind spot: the
official exporter writes frame and rounded-image clips as a namespaced
`clip-path` attribute that Chromium does not apply, so for a scene with frames
the authority raster is unclipped while a correct renderer PDF, which follows
the official canvas, clips (spec 8.4). The frame-export gate escapes it because
each exported clip coincides with its page; a scene-export PDF gate would not
(see the Canonical PDF harnesses section of `tests/conformance/README.md`).
