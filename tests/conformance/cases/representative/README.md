# Representative cases (#84)

Registered real-world scenes that verify interactions focused cases cannot show
alone. Each case renders an intake bundle (`tests/conformance/intake/`) and
retains the pinned reference collection its comparisons need. Its
`expected-report-<format>.json` files and `case.json` semantic expectations are
authored from the reference and the scene, not copied from native output:

- SVG size is the official SVG's `width`/`height`.
- PNG size is the official canvas grid.
- PDF `MediaBox` is the SVG size × 0.75.
- The embedded scene follows the metadata option.
- Links and text counts come from the scene.

`tools/representative-intake/case.py` is the verifier (see its docstring):

```sh
python3 -B tools/representative-intake/case.py verify-registry --renderer target/release/excalidraw-render            # offline + native
python3 -B tools/representative-intake/case.py verify <case> --renderer target/release/excalidraw-render --viewers    # + pinned viewers
```

A case passes when its native renders repeat in fresh processes, reports,
dimensions and semantics match, and every registered comparison passes its
canonical budget or an approved exact-ceiling exception. Unused exceptions are
rejected. Approval binds the exact `case.json` subject (`case.py subject`) to a
maintainer comment on #85.

| Case | Formats | Exceptions |
| --- | --- | --- |
| `representative.excali-local-slides.v1` | SVG, PNG, PDF (light, dark) | PDFium `slide1-avatar`, light and dark (`../../proposals/slides84-pdfium-avatar-exception-v1`) |

Pending, not registered: varmq (undiagnosed PDF, dark SVG Chromium and PNG
differences; `../../proposals/varmq84-*`) and Velox (undiagnosed PDF; PNG waits
on #194).
