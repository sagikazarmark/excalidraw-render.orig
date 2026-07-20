# Canonical PDF harness

This development-only harness qualifies PDFium and PDF.js as independent 96 DPI
PDF viewers and qpdf as the independent structural parser. None of these tools
is a production renderer dependency.

Install exact-pinned tools and package locks from the network:

```sh
bash tools/pdf-harness/install.sh
```

The installer verifies the pypdfium2 wheel and relocatable qpdf archive by
SHA-256 and installs the exact PDF.js npm lock in the canonical Linux/amd64 Node
container. Installed bytes live under ignored `target/` or `node_modules/` paths.
Viewer and parser execution is offline after installation.

The narrow process interfaces are:

```sh
bash tools/pdf-harness/run-pdfium.sh render input.pdf output.rgba width height
bash tools/pdf-harness/run-pdfjs.sh render input.pdf output.rgba width height
bash tools/pdf-harness/run-structure.sh inspect input.pdf expectation.json pass output.json
```

PDFium emits packed top-left RGBA after explicit byte-order selection and
deterministic unpremultiplication. PDF.js accepts only in-memory bytes, disables
network, streaming, range fetching, system fonts, external font data, forms,
annotations, XFA, WASM, and hardware acceleration, and emits canvas
`getImageData` bytes. Both adapters reject nonconforming page shape before
writing a canonical buffer.

The PDFium library is reinitialized with the checked `empty-fonts/` directory as
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

Reproduce every fixture, fresh-process render, comparison, and structural report:

```sh
bash tools/pdf-viewer-qualification.sh reproduce
```

The valid PDF is explicitly a vector-only calibration control rather than a
renderer snapshot or reference PDF. Official SVG rendered by canonical Chromium
remains the visual authority.
