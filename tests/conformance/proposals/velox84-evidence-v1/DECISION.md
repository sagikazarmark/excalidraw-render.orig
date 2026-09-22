# Velox representative: pending evidence decision

## Result

**Do not promote yet.** This collection supplies two reference renders per
variant, two native renders per format/variant, and two fresh executions of each
viewer. All repeat bytes agree. Four of ten complete visual comparisons pass:
both SVG viewers in both modes. The six PNG/PDF comparisons have at least one
failed whole/crop gate. No tolerance exception or approval was created.

The source is the immutable `tests/conformance/intake/velox-design-v1` board:
53 live elements, 20 bound texts, 11 bound arrows, Comic Shanns family 8. Native
reports are empty in all six executions. Manually recorded build provenance:
the native artifact capture used the
Rust 1.97.1 `--release --locked` CLI built from `d3b5da4`; `native/capture.json`
binds its executable digest. This is an observation, not a packaged-source or
cross-target qualification claim.

## Proposed request/region scope

- Light and dark whole-scene exports, background included, padding 10, scale 1,
  all optional metadata on; no frames or images exist in this upstream board.
- Official SVG dimensions: `1955.710387025309 × 515.8543746214824`.
- Official canvas/native PNG pixel grid: **1955 × 515**.
- Whole visual class: mixed representative.
- `client-typography`: `(1310,35,174,34)`, typography class.
- `client-binding`: `(944,30,665,161)`, geometry/image class; includes the named
  client rectangle and bound arrow, and also contains typography. That class
  assignment requires review, not automatic widening after a failed result.
- Crops derive from the immutable source anchors plus four logical pixels of
  context. They do not fit native discrepancies. The committed original
  `regions.json` remains the measurement authority for this proposal.

## Results under canonical budgets

Full measurements, exact SSIM bits, changed-pixel counts and input digests are
in `viewers/{light,dark}/*-comparison.json` and `viewers/viewers.json`.

| Comparison | Light | Dark | Main observation |
| --- | --- | --- | --- |
| SVG / Chromium | pass | pass | zero changed pixels whole/crops |
| SVG / pure Rust | pass | pass | zero changed pixels whole/crops |
| PNG / official canvas | fail | fail | whole passes; binding crop SSIM fails; light typography crop changed-pixel gate fails |
| PDF / PDFium | fail | fail | whole SSIM and focused gates fail |
| PDF / PDF.js | fail | fail | whole/crop SSIM fail |

PDF whole changed pixels: light PDFium **4235**, PDF.js **1655**; dark PDFium
**3682**, PDF.js **1471**. Whole mixed-class changed-pixel ceilings pass, but SSIM
does not. This is not evidence of a diagnosed non-defect tolerance cause.

## Diagnostic evidence and open causes

The light PNG typography crop reports **174** changed pixels. Raw-channel
localization places 174 substantial differences on its first row (`y=35`),
where it cuts a rectangle edge. A diagnostic-only crop expanded upward by three
rows (preserving every original pixel) reports **2** changed pixels and passes;
see `crop-context-probe/png-context.json`. Pixelmatch's anti-alias classification
is region-context dependent, so crop changed-pixel counts need not be subsets
of whole-image counts. The original failed crop remains retained.

That probe localizes a crop-boundary contribution; it neither excuses all PNG
differences nor changes the canonical budget. Binding-crop SSIM and PDF text
differences remain unresolved. Both SVG passes narrow the likely causes, but
do not distinguish PDF font positioning from viewer/font rasterization or PNG
coverage. No renderer defect is asserted without that diagnosis.

## Viewer adapter changes requiring review

The old Chromium viewer rejects all fractional dimensions and numeric XML
entities; Velox legitimately has fractional dimensions and source-line `&#10;`
attributes. The proposed adapter preserves geometry/viewBox, truncates screenshot
extent at the origin, and permits only LF numeric entities while retaining
resource-policy checks. It also uses the existing text-audit corrections.
Pure-Rust SVG output rounds dimensions, so its extra right/bottom pixels are
discarded at origin, never scaled. PDF viewers already use their exact 96-DPI
origin-anchored clip rule.

These choices are explicit proposal scope, **not an approved successor profile**.
The committed command-boundary browser controls cover fractional viewport,
unscaled paint, LF, forbidden numeric entities and external resources. Wider
qualification and the complete adapter identity must be reviewed before promotion.

## Promotion work still required

1. Decide the source sanitization/no-change record, light/dark applicability and
   region classes/context. Add a genuinely non-text focused crop as required by
   spec §14.3: the current client-binding crop contains text and does not fulfill
   that requirement. If crops change, regenerate a successor proposal;
   do not overwrite this measured failure to make it disappear.
2. Diagnose remaining PNG/PDF differences with isolated geometry/text controls.
   Fix confirmed renderer defects, or propose exact scoped tolerances only for
   established non-defect causes.
3. Qualify/approve the fractional-viewport and LF viewer adapter changes.
4. Add independently authored semantics, notice/metadata, reports and meaningful
   resource outcomes. Repeated native output is not that independent oracle.
5. Add the approved representative to the extensible registry and shared target
   inventory, execute native/WASM/process-order equality, and approve the exact
   complete subject. Existing Stage-4 approval authorizes none of these changes.

`reference/reference.json`, `native/capture.json` and `viewers/viewers.json` are
source/input-bound collections. `evidence.py --verify` checks retained bytes and
recomputes canonical comparisons; it does not rerun viewers, validate external
approval, or declare the proposal eligible for conformance.
