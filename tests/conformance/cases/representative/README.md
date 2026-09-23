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
- Mandatory notices, in SVG and PDF, are the font and license identities of
  the scene's font families; Helvetica frame names are shaped with Liberation
  Sans. PNG embeds no font programs.

A case can also declare **resource outcomes**: a variant and format with
request overrides that must fail, with no artifact written and an identical
report in two fresh processes. The report must match independently authored
fields. varmq pins its scale-1 PNG failure,
`resource.raster-surface-exceeded`, at 6524 × 5993 = 39098332 attempted pixels,
the official SVG size at scale 1.

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

Regions that fail their class budget are judged against identical-input
controls before exceptions (ADR-0014).

| Case | Formats | Within controls | Exceptions |
| --- | --- | ---: | --- |
| `representative.excali-local-slides.v1` | SVG, PNG, PDF (light, dark) | 2 | none |
| `representative.velox-design.v2` | SVG, PDF (light, dark) | 8 | 8 PDFium (double-stroke coverage summing); PNG waits on #194 |
| `representative.varmq-queue-diagram.v1` | SVG, PNG, PDF (light, dark) | 9 | 8 PDF `whole`/`worker-arrow` (sub-pixel hatch widening) |
