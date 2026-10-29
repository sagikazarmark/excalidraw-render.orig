# Points, freehand and arrows ownership (#85)

Base: `3fee7ebf6b12382241683735ba1495f0f5dcffe4`.

Four public-facade selectors in `fast/point_stroke_contract.rs` own 30 obligations:
linear points/legacy draw migration (6), nested point components (6), freedraw
points/pressures/simulatePressure (9), and arrowheads/elbowed (9), all SVG/PNG/PDF.
Exact selectors, source hashes and rationales are recorded in the ledger.

The matrices exercise missing/default and malformed values, synthesized short
linear geometry, first-point origin rebasing and extent correction, legacy draw
equivalence to line, all head kinds at both ends with both elbow states, narrow
point-component placeholder recovery and diagnosed ignored extra components.
Pressure checks cover source-count matching, range boundaries, simulation,
malformed points with otherwise valid pressure samples and mismatched lengths.

Every contract checks authored reports, preserved original bytes, fresh-call
equality, direct/lazy selected frames, nonselected silence and deleted silence.
Independent geometry oracles include the existing arrow vector/raster/PDF pins,
perfect-freehand simulated-pressure vectors and a public explicit-pressure path
hash from the qualified corpus. The malformed-point placeholder is independently
checked against the specified SVG fill/cross/dash/label, PNG dimensions/pixels,
and PDF vector operators before comparing other malformed cases to it.

Two-axis review found no hard Standards violations and one nonblocking shared
test-helper duplication suggestion. Spec findings prompted explicit-pressure
and placeholder oracles plus malformed-point/pressure interaction checks. The
final Spec review supports all 30 assignments with no remaining essential gaps
in scope. No production behavior change was needed.

The maintainer's continuing “approve all” / “do it” authorization covers these
assignments. Totals: **769 assigned / 159 gaps** (8 surface, 151 other). Remaining
surface fields are element type, links and nested SVG resources; global #85
closure is still incomplete.

Targeted tests, workspace/all-target typechecking and all ten canonical Dagger
conformance/lint checks pass. The final full workspace suite
(`dagger check excalidraw-render:test`) passed fast, render/parity and doctest
tiers in 18m34s.
