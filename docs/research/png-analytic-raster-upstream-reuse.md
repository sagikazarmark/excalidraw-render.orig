# PNG analytic rasterization: upstream reuse assessment

Research date: 2026-09-22. Scope: replace or adapt PNG device-space coverage to
match the reference renderer's pinned Skia
`53348aa333da02b77c4b5797e2de722f5abde7d0`, while retaining native/browser-WASM
artifact byte stability and the existing appearance/resource contracts.

## Decision summary

**Execution update:** the source assessment below predates the standalone
experiment completed later in this session. Zeno 0.3.3 passed only **17/27**
sampled phase controls and failed Velox's horizontal phase. See
[the executed attempts/result](png-edge-correction-attempts.md#upstream-reuse-result)
and `tools/representative-intake/raster-reuse/result.json`. The next action is
investigating a supported edge-policy seam/controlled port, not repeating the
unchanged Zeno adoption screen. Statements below about experiments not yet run
describe the research phase only.

**No inspected upstream implementation is an established drop-in match. A
tiny-skia 0.12 upgrade does not supply Skia's analytic path scan converter.**
The release still uses supersampling and contains `TODO: AAA`; the upstream
analytic-port issue remains open. Zeno and Vello CPU provide real analytic
coverage implementations, but with different edge-generation and coverage
arithmetic. Their APIs make bounded reuse experiments possible, not a claim of
compatibility. [T1], [T5], [Z3], [V4], [V5]

Recommended next action: a **standalone mask qualification experiment with
Zeno 0.3.3**, using already-expanded device-space outlines, plus tiny-skia
0.11.4/0.12.0 controls. Stop on the retained curved/rotated/full-Velox failures
rather than extending a straight-edge patch into a local rasterizer. Vello's
published `vello_common 0.2.0` strip generator is a second experiment if a
maintainer-supported coverage seam is acceptable. Ask tiny-skia maintainers
about issue #8 using the reduced reproducer; there is no evidenced delivery date
on which this project can depend. Details and stop conditions are below.

This is source research, not execution evidence: no candidate was compiled or
rendered during this investigation. The ongoing local qualification's failure
after improving straight samples is task context supplied by the requester,
not a new measurement made here. The retained diagnosis is the authoritative
local evidence cited below.

## Existing evidence and acceptance boundary

- [Velox diagnosis][D1]: rectangle strokes alone fail the original binding crop
  `(944,30,665,161)`, with light SSIM approximately `0.9981740109`; the focused
  non-text crop `(1250,26,200,15)` also fails despite zero changed pixels under
  the changed-pixel metric. Text/binding layout is not the cause.
- The browser controls distinguish horizontal rounded-path alpha
  `[0,64,255,192,0]`, horizontal straight-stroke alpha `[0,64,255,191,0]`, and
  vertical alpha `[0,195,255,59,0]` at the documented phases. Matching only the
  first straight horizontal sample is insufficient. Preserve original bounds,
  device phase, crops, comparator and budgets. [D1]
- The workspace directly pins tiny-skia `=0.11.4`; the diagnosed baseline uses
  an outer 4× surface/downsampling arrangement. That outer surface is distinct
  from tiny-skia's own internal 4×4 supersampling scan conversion. [D1], [D2], [T1]
- Spec §10 requires native and `wasm32-unknown-unknown` operation without
  production JS/browser/OS services, host fonts, system packages or network;
  bounded, panic-free renderer-facing behavior; compatible licenses; exact
  dependency identities; audited transitive unsafe; and no production git
  dependencies. Remedies are **adapter → pinned controlled fork → focused
  owned port**. [D3]
- ADR-0006 scopes execution to Linux x86-64 and browser WASM. ADR-0002 preserves
  artifact-byte stability and resource-trace equality. ADR-0007 requires
  portable rendering math, but its historical statement that tiny-skia SIMD is
  disabled is superseded by the recorded feature-unification diagnosis:
  resvg/svg2pdf enable defaults transitively. That diagnosis also isolates four
  existing native/WASM pixel differences to tiny-skia-path's std cubic-root
  math. A direct `default-features = false` edge is not a graph-wide fix. [D4], [D5], [D6], [D7]
- The canonical comparator has its own pinned closure (ADR-0005); changing a
  rasterizer does not authorize changing comparison semantics. Raster coverage
  bytes also must not redefine semantic resource accounting. [D3], [D8]

## What matching the pinned Skia actually entails

`SkAnalyticEdge::kDefaultAccuracy = 2`; `SnapY` adds half a quarter-pixel in
fixed-point representation and clears the lower bits. `setLine` applies it to
Y only. Quadratic/cubic initialization and subsequent updates have their own
snapping and continuity handling. `SkScan_AntiPath.cpp` calls `AAAFillPath`
after admission/clipping. [S1], [S2], [S3]

**“X continuous” needs a precise qualification.** X is not quarter-snapped at
line-edge admission, but it still passes through binary32/FDot6/fixed-point
conversion. The non-`SK_RASTERIZE_EVEN_ROUNDING` branch scales by four before
FDot6 conversion, giving 1/256-pixel input granularity. The analytic convex
walker additionally rounds X to 1/16 pixel in its sloped-edge branch. The
run-based additive blitter snaps alpha below 8 to 0 and above 247 to 255; the
small-mask branch and several trapezoid/partial-alpha paths differ. These are
concrete reasons why ideal area integration plus quarter-rounded path vertices
is not an implementation of the pinned rasterizer. [S2], [S4]

Which branches the complete Velox drawing invokes, and the pinned Chromium
build's rounding define, still need explicit confirmation before claiming an
exact adapter. The direct canvas controls already constrain the observable
behavior. Keep both that browser oracle and source-level branch analysis;
neither “analytic AA” nor common Skia ancestry proves parity. [D1], [S1], [S2], [S3], [S4]

## Candidate findings

### tiny-skia / tiny-skia-path 0.12.0

Inspected release commit `e48b9027762d33731e60e40eb17eaa4f583b0185`; also checked
main at `5d4754777746eef0828be166896eaf482c49f8f2`.

- `src/scan/path_aa.rs` still has `SUPERSAMPLE_SHIFT = 2`, `SuperBlitter`, and
  `TODO: AAA`. The release comparison has **no change to that file or
  `src/scan/path.rs`**. Main retains the same missing analytic implementation.
  [T1], [T2], [T4]
- It would nevertheless be inaccurate to call all raster code unchanged:
  `src/edge.rs::diff_to_shift` changes its rounding term from `1 << 4` to
  `1 << (2 + shift_aa)`; the AA hairline file removes a debug assertion. The
  changelog also records the small-AA-rectangle panic fix, gradients,
  colorspace support and relaxed WASM SIMD. None is an analytic-port entry.
  Path geometry changes in this comparison are spelling corrections, and
  stroker changes are derive/lifetime/comment cleanup. [T2], [T3]
- `tiny-skia-path` supplies `f32` paths, builders, stroking, dashing and geometry,
  **not pixel scan conversion**. It remains useful for retaining existing
  stroke expansion while a different coverage implementation is evaluated.
  Upgrading it alone cannot introduce `SnapY` raster behavior. [T6]
- Both manifests declare BSD-3-Clause. tiny-skia defaults to `std`, `simd`,
  `png-format`; path offers `std` or `no-std-float` with libm. Preserve notices
  and inspect the resolved graph; 0.12's new SIMD support is not a determinism
  mode. The release diff does not repair the documented std cubic-root math
  divergence. [T2], [T7], [T10], [D7]

**Upstream status:** [issue #8][T5], “Port analytical anti-aliased Path filling,”
was opened in 2020 and remains open, unassigned and without a milestone. Its
body names `SkScan_AAAPath.cpp` and questions whether the then-estimated
2000–2500-line port is worthwhile; its only inspected comment is a 2022 thin
polygon example. This is an acknowledged opportunity, **not a committed plan**.
GitHub issue/PR searches for `analytic`, a PR search for `AAA`, and the current
open PR list found no analytic-port PR. That bounded search does not establish
that nobody has private work or an unindexed branch. [T5], [T8]

Issue #179 reports a rounded-rectangle corner artifact on 0.12.0 and remains
open with no comments at inspection. It is relevant regression material, but
its cause has not been established as the Velox defect. [T9]

### Zeno 0.3.3: smallest useful alternative coverage seam

Published package VCS identity and inspected main agree at
`698399439f8035736d01de00cbcdc413b3976a18`. [Z4]

- `Mask::new(PathData).style(Fill::NonZero).size(w,h).render_into(...)` emits an
  8-bit alpha mask into caller storage. `Format::Alpha`, explicit dimensions,
  offset controls and reusable `Scratch` suit an adapter. Supply command
  iterators/commands rather than serialize and reparse SVG. Fill rules,
  quadratic/cubic paths, strokes, caps, joins and dashes exist. [Z1], [Z2]
- The rasterizer is a cell **area/cover accumulator**, not 256 independent
  rendered surfaces. `PIXEL_BITS = 8`; `FixedPoint::from_point` converts both
  axes with `(v * 256.).to_int_unchecked()`. It recursively subdivides curves
  using its own rules. There is no quarter-Y Skia admission policy in this
  implementation. Upstream explicitly says masks are nearly identical to Skia
  **“sans slight AA differences”**—exactly the area at issue here. [Z1], [Z3]
- License: Apache-2.0 OR MIT. `default-features = false, features =
  ["eval", "libm"]` selects evaluation and the no-std math path; `std` otherwise
  takes precedence. The normal manifest's only dependency is optional libm.
  This is a credible pure-Rust/no-host-service candidate, not executed WASM
  qualification. [Z1], [Z5]
- It is **not unsafe-free**: unchecked float-to-int conversion requires finite,
  representable scaled values. Fixed-point differences/products, dimensions,
  pitch and destination length require additional bounds analysis. Caller-owned
  output does not bound internal storage: adaptive cells spill into vectors,
  and scratch reuse amortizes allocation rather than imposing a cap. The
  raster module is private; there is no public post-subdivision edge-policy
  hook. [Z1], [Z2], [Z3]

Assessment: best first low-cost experiment because it can return coverage
without replacing compositing. Preserve the existing stroker and pass its
expanded outlines as fills. An optional **line-only** quarter-Y adapter can
test the admission hypothesis, but curve-control-point rounding cannot be
promoted as a general fix. A general Skia-edge policy would require upstream
API work or a controlled fork; its size and feasibility are unknown.

### Vello CPU / vello_common 0.2.0: credible analytic engine, wider seam

The registry lists published `vello_cpu 0.2.0`; its package VCS identity is
`7073a85d61ee099d1d3595651ddf92302b074348`, under `sparse_strips/`.
Current main was inspected separately at
`3b08372ad1d11a4008d724e9be69470e604aa290`, where the layout has moved.
Do not mistake main's unchanged package version for identical release source.
The findings below use the published source identity. [V1], [V2]

- The full CPU API provides `RenderContext::fill_path/stroke_path`, paint,
  clips and rendering to a `Pixmap`. It needs no GPU. More narrowly,
  `vello_common::strip_generator::StripGenerator::generate_filled_path`
  accepts `kurbo::PathEl` values, fill rule, transform and optional clip;
  `StripStorage` exposes strips and 8-bit alpha data. This permits a coverage
  experiment without adopting its complete compositor. **However, its crate
  documentation explicitly advises against using vello_common on its own.**
  Ask about a supported seam before production adoption. [V3], [V4], [V6]
- The published generator flattens paths, makes analytic-AA tiles, sorts them
  and renders strips. `strip.rs` integrates trapezoid areas in floating point,
  with nonzero coverage mapped via `abs(area) * 255 + 0.5` and a separate
  even-odd calculation. This is a real analytic rasterizer, but not Skia's
  quarter-Y/fixed-point scan converter. The documented/public APIs expose no
  Skia compatibility mode. [V4], [V5]
- Apache-2.0 OR MIT. CPU defaults are `std`, `png`, `text`, `u8_pipeline`;
  `png` and multithreading imply std, and std overrides libm. A narrow probe
  should disable defaults/text/PNG/threading and use libm. Both pipeline choices
  share the upstream coverage generator; selecting `f32_pipeline` is not a
  way to select Skia AA. [V4], [V7], [V10]
- The CPU and common crates forbid unsafe, but they use `fearless_simd` and
  bytemuck, so that is not an unsafe-free transitive graph. Render settings
  default to detected SIMD. `Level::baseline()` means strongest statically
  available SIMD, **not necessarily scalar**. `fearless_simd 0.4.0` offers
  `force_support_fallback` and `Level::fallback()` for a controlled scalar
  probe; even then LLVM may auto-vectorize. Source uses `mul_add` and explicit
  target-sensitive NaN handling, so cross-target bytes require measurement.
  [V3], [V5], [V6], [V7], [V8], [V9], [V10]
- `u16` viewport dimensions, packed alpha indices with assertions and vector
  growth need admission/resource analysis. The full renderer also documents
  unsupported filter cases that panic; those should not enter a mask-only
  evaluation. Reusable strip storage is not a semantic work/memory budget.
  [V3], [V4], [V5], [V6], [V9]

Assessment: second choice for reuse, especially if Zeno's private edge seam
proves too restrictive. There is no evidence it matches the Velox budget, and
replacing the entire renderer would introduce unnecessary stroker/compositor
differences before answering the coverage question.

### Other screened options

| Candidate | Primary-source finding | Disposition |
| --- | --- | --- |
| `ab_glyph_rasterizer` 0.1.10 | Exposes lines/quadratics/cubics and float coverage callbacks; scalar line integration plus curve subdivision, inspired by font-rs/stb_truetype. Apache-2.0; no-std/libm option; unsafe runtime-selected x86 draw functions exist. No matching SnapY rule in the inspected raster code. [A1], [A2], [A3] | Useful small independent area-coverage control, but no public stroke/clip policy comparable to Zeno and no demonstrated advantage for pinned Skia parity. |
| Raqote development manifest 0.8.5, commit `9f1340c8ce3909286601a059a3c2077c3502a059` | Raster source explicitly says 4×4 supersampling; both coordinates enter 30.2 endpoints. Default `text` pulls font-kit; disable defaults for a host-font-free probe. Manifest says BSD-3-Clause; raster header also names MIT portions and Skia BSD notices. Active edges use unsafe pointers. [R1], [R2] | Does not supply the missing analytic converter; not a promising replacement for this defect. Preserve all source notices if ever reused. |
| Skrifa / Fontations | Official README describes OpenType metadata, outlines and hinting, not a general path-to-coverage rasterizer; crate forbids unsafe. [F1] | Wrong layer for rectangle stroke AA. Font outline correctness cannot repair scan conversion. |
| Rust Skia bindings / CanvasKit | The needed reference implementation is C++ Skia; a wrapper does not turn it into a pure-Rust dependency. Production C/C++ build/native-link or JS/browser requirements conflict with this task/spec. [S1], [S2], [S3], [S4], [D3] | Reference/oracle only, not a production reuse recommendation. |

A GitHub code search for `SkAnalyticEdge language:Rust` returned a candidate
`obvirm/skia-rs`. At the result's commit
`84b7f0b617491b73106137ac9511bd4bc2c9e06e`, its `SkAnalyticEdge.rs` and
`SkScan_AAAPath.rs` contain C++ source inside comments, not executable Rust
implementations. This search result is not an available analytic port. The
search is not an exhaustive proof of absence across the ecosystem. [O1], [O2]

## Ranked options

Ranking is by usefulness for this specific mismatch and bounded next-step cost,
not general renderer quality.

1. **Qualify Zeno as a coverage-only adapter.** Smallest public mask API and
   narrow dependency surface; known algorithm mismatch, so a deliberately
   falsifiable experiment rather than a migration commitment.
2. **Qualify Vello's strip coverage with a supported upstream seam.** Existing
   analytic engine with public strip/alpha structures, but more integration,
   SIMD/math and API-support questions.
3. **Pursue tiny-skia upstream #8 / a maintainer-agreed controlled fork.** Closest
   home for actual Skia-algorithm reuse and existing path/compositor continuity,
   but there is no implemented analytic converter to consume today. Establish
   scope and ownership before approving a substantial port.
4. **Use tiny-skia 0.12 as a control/independent maintenance update.** Not a
   source-supported fix for missing analytic AA. Do not spend a full evidence
   refresh expecting an upgrade alone to solve the diagnosis.

The focused owned port remains the spec's last remedy, not the recommendation
from this research. Generic analytic AA may still satisfy perceptual parity,
but passing representative budgets must demonstrate that; a theoretically more
accurate area integral is not a substitute for the reference renderer. [D3]

## Concrete experiment and decision gate

Run outside production integration, with its own exact registry dependencies,
lockfile, source digests and the existing pinned Chromium oracle:

1. **Establish controls.** Replay the retained minimal rectangle and 27 direct
   canvas controls at original device dimensions/phases. Compare current
   tiny-skia 0.11.4 and 0.12.0, each at 1× and the existing outer 4× arrangement.
   Explicitly distinguish the stroking API's narrow/hairline behavior from
   filling an expanded outline. No expectation that 0.12 passes is implied.
2. **Test masks, then scene pixels.** Feed the same already-expanded device-space
   outlines to Zeno `=0.3.3` (`eval,libm`, no defaults), with explicit
   `Format::Alpha`, dimensions and checked caller buffer. Use fill mode, not a
   new stroker. First measure unmodified Zeno. A separate line-only arm may
   apply the pinned binary32/fixed admission and quarter-Y rule to straight
   expanded edges to test whether that is enough for polygons. Label this arm
   incomplete for curves. Compare full alpha rows, then composite each mask
   with the same PNG paint path so mask error and blend error stay identifiable.
3. **Escalate immediately beyond straight samples.** Include the reduced
   rounded rectangle, actual Velox expanded outlines, corners, rotations,
   quadratics/cubics, caps/joins/dashes, widths below/at/above one device pixel,
   positive/negative eighth-pixel ties and adjacent binary32 values, clipping,
   opacity, fills, and the original light/dark Velox binding crop. Include
   small and large bounds to expose Skia mask-versus-run behavior. Retain all
   failures, exact SSIM bits and changed-pixel counts. A good top-edge row
   without passing the original crop is a failed candidate.
4. **Only if justified, add Vello coverage.** Pin the published 0.2.0 graph,
   use `generate_filled_path` and extract strip alpha rather than change
   compositors. Select libm and a forced fallback level, inspect feature
   unification, and compare identical outlines with Zeno/reference. Do not
   introduce a generic flatten-and-snap curve adapter and call it Skia.
5. **Advance only a visually promising bounded adapter.** Execute it on both
   supported targets in repeated fresh processes/orders. Require exact
   native/WASM artifacts and reports, unchanged semantic resource outcomes,
   and complete corpus qualification before production adoption. Distinguish
   the four documented pre-existing math mismatches from new results; they
   remain blockers, not permission for new divergence. [D1], [D3], [D4], [D5], [D6], [D7]

**Stop condition:** if success requires replacing curve decomposition, edge
stepping, coverage accumulation and branch dispatch, the work has become the
large local port the user declined. Report which stage failed and bring that
bounded reproducer to upstream #8 instead of growing the adapter. Do not widen
tolerances or approve a straight-rectangle specialization from this experiment.

### Exact unresolved questions

- Does unmodified Zeno or Vello pass the unchanged full Velox SSIM gate despite
  differing arithmetic? No rendering measurement was made here.
- Which Skia dispatch/convexity/mask branches and rounding define apply to each
  failing draw? Can a supported upstream edge-policy hook reproduce them without
  replacing most of the scan converter?
- Can either adapter cover the renderer-admitted coordinate/complexity range
  with checked arithmetic, bounded scratch growth and no reachable panic/UB?
  Zeno's unchecked conversion and Vello's index/dimension bounds need concrete
  proofs or tighter internal adapters, not silently narrower public admission.
- Can portable-math and scalar feature settings survive the complete packaged
  dependency graph and produce identical bytes on executed Linux/WASM?
- Will tiny-skia maintainers support #8 now, or Zeno/Vello maintainers support a
  pinned-Skia policy seam? No upstream contact or commitment was made here.
- What are the performance, WASM footprint and final transitive license/unsafe
  audit costs of a qualifying candidate? Manifest inspection is not a completed
  dependency qualification.

## Sources

All upstream facts above derive from first-party source, manifests, published
crate metadata/docs or upstream issue records. Issue/search state is a snapshot
at the research date; source identities are fixed where available.

[D1]: ../../tests/conformance/proposals/velox84-png-isolation-v1/DIAGNOSIS.md
[D2]: ../../Cargo.toml
[D3]: ../renderer-spec-v1.md#10-dependencies-assets-and-distribution
[D4]: ../adr/0006-narrow-supported-targets-to-linux-and-browser-wasm.md
[D5]: ../adr/0002-scope-the-determinism-promise-to-artifact-bytes.md
[D6]: ../adr/0007-route-transcendental-math-through-libm.md
[D7]: wasm-snapshot-mismatches.md
[D8]: ../adr/0005-pin-the-comparator-to-its-own-dependency-closure.md
[S1]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkAnalyticEdge.h
[S2]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkAnalyticEdge.cpp#L156-L205
[S3]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkScan_AntiPath.cpp
[S4]: https://github.com/google/skia/blob/53348aa333da02b77c4b5797e2de722f5abde7d0/src/core/SkScan_AAAPath.cpp
[T1]: https://github.com/linebender/tiny-skia/blob/e48b9027762d33731e60e40eb17eaa4f583b0185/src/scan/path_aa.rs
[T2]: https://github.com/linebender/tiny-skia/compare/v0.11.4...v0.12.0
[T3]: https://github.com/linebender/tiny-skia/blob/e48b9027762d33731e60e40eb17eaa4f583b0185/CHANGELOG.md
[T4]: https://github.com/linebender/tiny-skia/blob/5d4754777746eef0828be166896eaf482c49f8f2/src/scan/path_aa.rs
[T5]: https://github.com/linebender/tiny-skia/issues/8
[T6]: https://github.com/linebender/tiny-skia/blob/e48b9027762d33731e60e40eb17eaa4f583b0185/path/README.md
[T7]: https://github.com/linebender/tiny-skia/blob/e48b9027762d33731e60e40eb17eaa4f583b0185/Cargo.toml
[T8]: https://github.com/linebender/tiny-skia/pulls
[T9]: https://github.com/linebender/tiny-skia/issues/179
[T10]: https://github.com/linebender/tiny-skia/blob/e48b9027762d33731e60e40eb17eaa4f583b0185/path/Cargo.toml
[Z1]: https://github.com/dfrg/zeno/blob/698399439f8035736d01de00cbcdc413b3976a18/src/lib.rs
[Z2]: https://github.com/dfrg/zeno/blob/698399439f8035736d01de00cbcdc413b3976a18/src/mask.rs
[Z3]: https://github.com/dfrg/zeno/blob/698399439f8035736d01de00cbcdc413b3976a18/src/raster.rs
[Z4]: https://docs.rs/crate/zeno/0.3.3/source/.cargo_vcs_info.json
[Z5]: https://github.com/dfrg/zeno/blob/698399439f8035736d01de00cbcdc413b3976a18/Cargo.toml
[V1]: https://crates.io/api/v1/crates/vello_cpu/0.2.0
[V2]: https://docs.rs/crate/vello_cpu/0.2.0/source/.cargo_vcs_info.json
[V3]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_cpu/src/render.rs
[V4]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_common/src/strip_generator.rs
[V5]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_common/src/strip.rs
[V6]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_common/src/lib.rs
[V7]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_cpu/Cargo.toml
[V8]: https://docs.rs/fearless_simd/0.4.0/fearless_simd/enum.Level.html
[V9]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_cpu/src/lib.rs
[V10]: https://github.com/linebender/vello/blob/7073a85d61ee099d1d3595651ddf92302b074348/sparse_strips/vello_common/Cargo.toml
[A1]: https://github.com/alexheretic/ab-glyph/blob/3eb21a592819d9ed78e9a13ad7392d46c60ce39e/rasterizer/README.md
[A2]: https://github.com/alexheretic/ab-glyph/blob/3eb21a592819d9ed78e9a13ad7392d46c60ce39e/rasterizer/src/raster.rs
[A3]: https://github.com/alexheretic/ab-glyph/blob/3eb21a592819d9ed78e9a13ad7392d46c60ce39e/rasterizer/Cargo.toml
[R1]: https://github.com/jrmuizel/raqote/blob/9f1340c8ce3909286601a059a3c2077c3502a059/src/rasterizer.rs
[R2]: https://github.com/jrmuizel/raqote/blob/9f1340c8ce3909286601a059a3c2077c3502a059/Cargo.toml
[F1]: https://github.com/googlefonts/fontations/blob/50c9d8f555a0948b6507b80bbdefa60e8ae3dfb0/skrifa/README.md
[O1]: https://github.com/obvirm/skia-rs/blob/84b7f0b617491b73106137ac9511bd4bc2c9e06e/src/core/SkAnalyticEdge.rs
[O2]: https://github.com/obvirm/skia-rs/blob/84b7f0b617491b73106137ac9511bd4bc2c9e06e/src/core/SkScan_AAAPath.rs
