# PNG edge correction attempts and upstream reuse

Investigation under #84; concluded by ADR-0014 (judge representative regions
against identical-input controls).

## Outcome

**No qualified production correction was found. All experimental production
changes were removed.** The user selected investigation of upstream reuse
instead of committing a larger local analytic-raster port.

The public-facade red regression reproduced the original row-33 value
`[171,171,171,255]` against Chromium `[198,198,198,255]`. Its template is retained
at `tools/representative-intake/png_edge_regression.rs`; copy it into the renderer
integration-test directory for the next red/green implementation. It is not
registered as a passing test or a new tolerance.

## Rejected implementation trials

1. **Flatten stroked device curves (at most 64 segments each), then quarter-Y
   admission.** Corrected the three independently measured straight-edge samples.
   Applying it to all Rough outlines caused ten registered PNG variants to exceed
   existing approved ceilings; it is not a qualified general adapter.
2. **Narrow admission** to smooth, axis-aligned, solid-stroked rectangles above
   the hairline width, excluding patterned fills. Existing registered PNG tests
   passed, but the full Velox binding crop still failed SSIM. Restricting scope
   did not establish the missing curve/coverage behavior.
3. **Asymmetric sampling layer**, four horizontal samples and one device row
   (tiny-skia internally supersamples that row). Also fixed the straight-edge
   samples but failed the binding crop. Increasing horizontal sampling further
   did not resolve it. The additional surface was experimental, not qualified
   semantic-resource accounting.

The narrow flattened candidate's 12-case independent browser matrix passed
only scale 1.5 and the unchanged rotated-path control; ten small-rectangle
cases failed geometry/image SSIM despite meeting changed-pixel ceilings.
The deterministic collection recipe was widths 1.5/2/3/4 at phases 0.125 and
0.632993081447, a negative phase, scale 1.5, 50% opacity and rotation 0.25
(the `qualify_edges.py` collector is retired and lives in Git history).
Outputs from rejected
implementations are not promoted to expected renderer results. This narrative
is manually recorded experimental provenance, not an implementation
qualification.

## Upstream reuse result

Primary-source assessment: `png-analytic-raster-upstream-reuse.md`.
tiny-skia 0.12 still uses supersampling; the analytic-port issue remains open.
Zeno and Vello have analytic area accumulation but neither exposes the pinned
Skia edge policy unchanged. No production dependency was added.

The standalone Zeno 0.3.3 experiment retains the existing stroke expansion and
compares alpha masks against 27 captured browser controls. **17/27 pass within
one alpha unit.** The ten failures are horizontal line/rounded-path phases;
at phase 0.632993081447 Zeno returns `[0,93,255,161,0]`, versus the rounded
reference `[0,64,255,192,0]`. The known mismatch survives reuse unchanged.
`tools/representative-intake/raster-reuse/result.json` records every sample.
The exact lockfile and source make this small rejection screen reproducible.
It does not justify adopting Zeno, relaxing visual budgets, or claiming WASM
or resource qualification.

## Next implementation decision

Prefer an upstream-supported analytic edge seam or a controlled port of the
pinned raster behavior. Qualification must include Skia's branch-specific
X/fixed-point/alpha handling, curves and clipping—not only `SnapY`. The retained
Velox facade reproducer is the starting red test. General coverage correctness,
resource limits and cross-target bytes are prerequisites to shipping; a
straight-sample improvement is insufficient.
