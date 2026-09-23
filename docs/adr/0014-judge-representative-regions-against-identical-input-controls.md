---
status: accepted
---

# Judge representative regions against identical-input controls

The canonical class budgets (spec section 14.3) were set before real-world scenes
entered the corpus. On the representative scenes of #84, the references
themselves fail them. Measured with the canonical comparator at the same crops:

| Class | Budget minimum SSIM | Worst reference-vs-reference control | Worst viewer-pair control |
| --- | ---: | ---: | ---: |
| geometry/image | 0.999 | 0.919 (varmq arrow crop: official SVG vs official canvas, and Chromium vs pure Rust) | 0.896 (PDFium vs PDF.js) |
| typography | 0.995 | 0.974 (Velox text) | 0.949 |
| mixed representative | 0.997 | 0.974 (varmq whole page) | 0.980 |

Under the unchanged budgets, Velox PDF needed 16 exact-ceiling exceptions, and
varmq about a dozen, mostly for differences smaller than the references'
own disagreement.

## Options (#195)

1. **Lower the class budgets** to the worst control. The geometry gate would
   drop from 0.999 to about 0.92 for every case, including focused cases that
   pass at 0.9999. Rejected: it stops catching real regressions.
2. **Keep per-case exceptions only.** Rejected: most exceptions would record
   reference disagreement rather than a diagnosed renderer or viewer behavior.
3. **Judge each failing region against its own identical-input controls.**
   Chosen.

## Decision

For representative cases only, a region that fails its class budget passes when
it is no worse, on both changed pixels and SSIM, than the worst control
rendering identical input at that region. The controls are:

- the official SVG in the canonical Chromium viewer vs the pure-Rust viewer;
- the official SVG vs the official canvas;
- for PDF, the candidate PDF in PDFium vs PDF.js.

Controls are computed in the same verification run, through the same comparator
profile and crops, and are never stored as tolerances. The comparator crate,
its profile and every approval bound to them are unchanged. A region beyond
its controls still needs an exact-ceiling exception with a diagnosed cause. An
exception that the budget or the controls already cover is dead and rejected.
Case records bind the rule (`judgment_rule`), so approvals bind it.

## Consequences

- excali-local-slides needs no exceptions; its two PDFium avatar exceptions
  fall within controls.
- Velox registers PDF with 8 PDFium exceptions (double-stroke coverage summing)
  instead of 16.
- varmq registers SVG, PNG and PDF with 8 PDF exceptions for the sub-pixel
  hatch widening. Its PNG, dark SVG Chromium and PDF typography differences are
  within controls.
- The rule tolerates what the canonical viewers and references disagree on at a
  given spot. A renderer defect that every viewer shows the same way still
  fails against the official raster.
