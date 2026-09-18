# Generic cubic bounds: retain the current helper

**Issue [#184](https://github.com/sagikazarmark/excalidraw-render.orig/issues/184),
2026-09-18.** Production source at
`6a42d61a8a3c0e97c749b99187f0923db18d8695`, candidate **kurbo 0.11.3**.

**Recommendation: retain the current generic bounds implementation.** The narrow
adapter saves about 89 lines but is not numerically equivalent: it changes
17 of 36 measured PDFs and changes public output-limit outcomes in both
directions. Better-conditioned roots do not authorize a compatibility change.
This is a completed no-go evaluation, not a production migration.

## Reproducible evidence

- [Probe and commands](../../tools/kurbo-evaluation/README.md).
- [Generated matrix](../../tools/kurbo-evaluation/matrix.md).
- Four `tools/kurbo-evaluation/expected-*.json` observations, with exact binary64
  witnesses, stream hashes, complete scene elements, artifact/report records,
  decoded PNG digests, PDF matrices, and resource-limit boundary outcomes.

The probe compiles the actual private generic implementation extracted at build
time. Renderer A/B runs replace only that helper region in a temporary source
copy; the candidate used for rendering is the same adapter used for numeric
comparison. All resolved registry versions/checksums match the production lock.
This is a bounded differential experiment, not a full-corpus release qualification
or a comparison with higher-precision mathematical ground truth.

### Eligible seam and callers

All paths below are under `crates/excalidraw-render/src/render/`.

The eligible seam is **only** `geometry.rs::include_cubic_bounds` and its private
`cubic_extrema` / `retain_cubic_extremum` machinery. `Path::cubic_bounds` stays as
the path traversal. The adapter constructs a `kurbo::CubicBez`, calls
`ParamCurveExtrema::bounding_box`, and unions its four coordinates into the
existing optional bounds.

| Caller | Coordinate space and consumer | Policy to preserve |
| --- | --- | --- |
| `geometry.rs::local_outline_bounds`, via `GeneratedGeometry::canvas_bounds` | Drawn linear outline's local coordinates; PNG/PDF canvas rotation pivot | Use only the outline, not decorations or fill. Fall back to the authored linear points' box when no cubic contributes. Nonlinear kinds retain their existing boxes. This is not the reference scene-extent calculation. |
| `png/coverage.rs::frame_clip_coverage_bounds` | Clip-local cubic box; map its four corners to device space, floor/ceil, clamp and intersect | No stroke inflation. Empty/missing cubic bounds clip everything away. Do not replace this with extrema of transformed control points. |
| `png/coverage.rs::conservative_freedraw_coverage_area` | Local cubic bounds of optional background fill, unioned with projected freedraw bounds; transform the resulting box corners | Keep the existing sketch-fill outset `source_stroke_width * scale / 4`; solid fill adds none. Keep floor/ceil, surface clamp and area charging. The freedraw stroke's own quadratic bounds are outside the seam. |
| `png/coverage.rs::conservative_text_coverage_area` | Local frame-clip bounds mapped to device space; intersect the text control-point box | Keep zero coverage on absent cubic clips and the existing floor/ceil/clamp policy; text geometry is not delegated. |

`resolve_tests.rs` also calls the helper to check the warm-outline pivot. There
are no other direct production callers. Ordinary Rough drawing coverage uses
control-point bounds and stroke outset, not this helper.

**Traversal and edge behavior:** `Move` and `Line` set the current point but
never contribute bounds themselves. A following cubic starts at that point.
Cubics update the current point and contribute endpoint/extrema bounds; separate
subpaths are unioned. Empty, move-only and line-only paths return `None`. A cubic
without any preceding current point immediately returns `None`, even if a later
operation could supply one. A finite point cubic returns `Some` with zero width
and height. No stroke, padding, transform, rounding or coverage policy belongs
inside the adapter. A generic whole-`BezPath` bounding box violates this contract.

### Explicit exclusions

The temporary replacement ends before `GeneratedGeometry`; it cannot reach the
reference-specific `bezier_value_for_t`, `solve_quadratic`,
`cubic_bezier_curve_bound`, or `reference_curve_bounds`. Their coefficient
spelling, exact-zero tests, once-rounded cubes, arithmetic ordering and traversal
remain intact. In particular, the reference traversal ignores `Line` even as a
current-point update, unlike the generic traversal.

`bound_text.rs::cubic_point` and the rounded-arrow label's fixed 21-sample length
approximation are untouched. No kurbo arc-length or inverse-length API is called.
The historical descriptions in #172/#173 explain the sensitivity, but the code
at the recorded commit is the evaluated implementation (including its later
reference-exact analytic scene bounds), not the older issue descriptions.

## Numeric result

**32,768 samples**, eight families of 4,096: point degeneracy, nearly straight
cubics, nearly linear *derivatives* (near-quadratic cubic coefficients), repeated
controls, portable-libm rotations, large offsets, rounding-boundary translations,
and general curves. The fixed seed is 184; generated coordinates are bounded by
`1e12`, the hard admitted coordinate ceiling. Large offsets alternate just inside
the default `1e9` and configurable hard `1e12` limits. These are admitted-range
geometry probes, not an assertion that every abstract cubic is emitted by a scene
or that its full extent fits a default raster/PDF surface.

**17,147 bounds differ bitwise; 2,000 unclamped floor/ceil boxes differ.** All four
native/browser and current/candidate runs agree on every numeric result and the
SHA-256 of all input/output bit streams. The two straight/point families agree
exactly. See the matrix for counts and maxima by family.

There are several concrete reasons for inequivalence:

1. The current helper expands coefficients from the four coordinates and uses
   `abs(a) <= EPSILON` for its linear branch. Kurbo forms control differences
   first and solves a scaled quadratic with a cancellation-resistant root formula
   and reciprocal-based recovery. Near a zero leading coefficient, this is an
   algorithm change, not merely a different evaluator spelling.
2. Kurbo's nested Bernstein evaluation has a different multiplication/addition
   order from the current four-term expression. It evaluates both axes at the
   combined x/y extrema; the current helper evaluates only the relevant axis.
3. The current helper retains roots in `[0,1]`; kurbo retains only `(0,1)` and
   separately includes endpoints. Mathematical equivalence does not imply equal
   floating-point observations at these boundaries.

The nearly linear derivative family has a **368.52778150733491** maximum absolute
difference: the current formula can lose an interior extremum through root
cancellation. The large-coordinate family differs by up to **0.0006103515625**
(five ULPs), with one integer-grid crossing. The boundary-targeted family has
150 crossings despite a maximum absolute difference of about **1.71e-13**.
Very large ordered-ULP distances around zero are not a relative-error estimate;
absolute deltas and the exact signed bit patterns are recorded separately.

These observations reveal a potential current-helper numerical defect deserving
its own reachable-scene diagnosis. They do not justify silently replacing it:
reachability, reference expectations, and its pivot/coverage impact need a separate
decision. No claim of kurbo accuracy against a high-precision oracle is made here.

## Artifact and resource consequences

The public-renderer corpus has 32 rotated rounded lines/arrows (seeds 1–32,
roughness 0/1/2), plus framed text, solid and sketch freedraw fills, and a rounded
arrow with reciprocal bound text. Each renders to all three formats with empty
compatibility reports at default limits: **108 artifacts per run**.

- **SVG: 0/36 changed. PNG: 0/36 changed. PDF: 17/36 changed**, on both targets.
- For `rotated-linear-1`, the PDF's element matrix final translation changes from
  `10.326535562363802` to `10.326535562363816`; both files are 1,996 bytes, but their
  SHA-256 values differ. The records contain both complete matrix operand lists.
  This demonstrates propagation through the canvas pivot, with reference scene
  bounds and SVG placement unaffected in these samples.
- Nine of those PDFs also change encoded length. At the same
  `max_output_bytes = 1997`, `rotated-linear-5` changes from **rejected** (1,998
  bytes) to **accepted** (1,997 bytes). At `1999`, `rotated-linear-17` changes from
  **accepted** (1,999 bytes) to **rejected** (2,001 bytes). Both public API outcomes
  are measured directly, including exact failure details, on both targets.
- The four clean PNG element-work boundaries remain **9,096**, **191,456**,
  **193,248**, and **282,096** units respectively. Each binary search checks exact
  equality and the recovery/error one unit below it. These sampled equalities do
  not prove all coverage ledgers equal; numeric floor/ceil crossings demonstrate
  why that would require broader qualification.

The integer-grid statistic is an unclamped projection of abstract curves. It is
not presented as 2,000 reachable resource-limit regressions. The two PDF caps,
in contrast, are actual admitted-scene resource-outcome differences.

## Version, features and portable math

The candidate edge is:

```toml
kurbo = { version = "=0.11.3", default-features = false, features = ["libm"] }
```

The resolved features on **both targets** are **default, std, libm**. Existing
`svgtypes 0.15.3` and `usvg 0.45.1` default edges enable `std`; requesting `libm`
does not subtract it. Kurbo explicitly selects std when both are enabled
(`lib.rs`; the `FloatFuncs` imports in `common.rs` and `cubicbez.rs` are gated on
`not(feature = "std")`). The production graph already contains kurbo 0.11.3;
the separate 0.13.1 instance in the font stack is not the evaluated candidate.

For this *specific* bounding-box call chain, arithmetic is basic binary64
operations, comparisons, min/max, reciprocal, `sqrt` and `copysign`, not adaptive
arc length or transcendental trigonometry. That is compatible with ADR-0007's
IEEE-operation policy, and the full numeric replay agrees across targets. It is
not evidence that kurbo as a whole has forced portable math, or that the renderer's
dependency graph is portable. General forced-libm adoption would need upstream
feature policy or a separately qualified adapter; it is unnecessary to establish
this narrow no-go result.

| Target | Environment | Observed result |
| --- | --- | --- |
| Native Linux x86-64 | Rust 1.97.1, opt-level 2 | Numeric, artifacts, and resource observations recorded and replayed |
| Browser WASM | Rust 1.97.1, wasm-bindgen-test-runner 0.2.126, Chrome/ChromeDriver 149.0.7827.55 | Same numeric and resource observations; same A/B artifact differences |

One PNG (`rotated-linear-12`) differs **between native and WASM with either
implementation**, including its decoded-pixel digest. Within each target it is
unchanged by the adapter. This is pre-existing cross-target behavior in the area
tracked by #178; this evaluation does not isolate its specific dependency call or
claim to resolve #178. Separate experimental target observations keep that
difference visible without changing the approved snapshot inventory.

## Net benefit and qualification decision

The three current generic helpers occupy **103 lines** including their spacing;
the adapter is **14 lines**, a gross saving of **89**, plus a direct manifest edge.
`Path::cubic_bounds`, all callers, reference-specific geometry, and bound-text
sampling still have to stay. The candidate adds no new resolved package, but the
portable-math feature audit and cross-target byte/resource evidence remain costs.

Preserving old answers with a fallback that detects every difference would need
the old solver/evaluator too, eliminating the substantive code reduction. The
measured output and admission changes are too much compatibility work to justify
this small cleanup. Retain the helper; keep this tool as reproducible evidence.

If adoption or a numerical fix is reconsidered, it needs:

1. A deliberate decision about current root-cancellation behavior and reachable
   scenes, rather than accepting kurbo's mathematical robustness by default.
2. Full native/browser artifact, resource-transcript, limit-boundary and viewer
   qualification, especially rotated linear pivots, frame/text clips and
   freedraw fill coverage, alongside the independent #178 work.
3. Explicit review of every changed PDF/PNG digest and resource outcome, with
   any affected evidence refresh authorized under the compatibility revision's
   rules. A retained revision cannot silently acquire these behavior changes.
4. An updated dependency/feature record if the selected kurbo version or math
   policy changes. There is no demonstrated kurbo upstream defect to file from
   this bounded comparison.

No production dependency, geometry result, approved snapshot or reference profile
is repinned by this evaluation.

## Verification and review

- Both current/candidate native exact-observation tests and both browser WASM
  replays pass without recording or changing expectations.
- The matrix/evidence consistency check, production-lock package/checksum audit,
  all-target probe Clippy (`-D warnings`), and Rust formatting pass.
- The complete default workspace nextest suite passes: **1,117 passed**, eight
  skipped tests and two excluded harness binaries under the existing default
  profile. `cargo test --workspace --doc --locked --offline` also passes.
  This is the normal workspace suite, not the external-viewer/release-qualification
  gates required for an actual migration.
- **Standards review:** zero hard violations; one nonblocking fixture-modeling
  suggestion. Invalid traversal tags now fail explicitly rather than becoming
  cubics. **Spec review:** zero findings against #184.

## Primary sources

- [kurbo 0.11.3 `ParamCurveExtrema`](https://docs.rs/kurbo/0.11.3/kurbo/trait.ParamCurveExtrema.html)
- [Cubic implementation](https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/src/cubicbez.rs)
- [Quadratic solver](https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/src/common.rs)
- [Feature selection](https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/src/lib.rs)
- [ADR-0007](../adr/0007-route-transcendental-math-through-libm.md),
  [#172](https://github.com/sagikazarmark/excalidraw-render.orig/issues/172),
  [#173](https://github.com/sagikazarmark/excalidraw-render.orig/issues/173),
  [#178](https://github.com/sagikazarmark/excalidraw-render.orig/issues/178).
