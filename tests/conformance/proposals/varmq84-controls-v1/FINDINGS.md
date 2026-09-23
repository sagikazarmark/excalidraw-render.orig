# varmq: remaining differences against identical-input controls (#84, #195)

Status: diagnosis only; no approval or exception is proposed here.

`tools/representative-intake/controls.py` measures every failing region of the
v3 collection (`../varmq84-evidence-v3`, renderer `a9d9e5e` onward) next to
control pairs that render **identical input**: the official SVG in Chromium vs
the pure-Rust viewer, the official SVG vs the official canvas, and (for PDF)
the same native PDF in PDFium vs PDF.js. A region is `within` when it is no
larger than the worst control on both metrics.

## Results (`regions-{light,dark}.json`)

| Failing region | Light | Dark | Explanation |
| --- | --- | --- | --- |
| PNG whole, `worker-arrow` | within | within | reference disagreement → #195 |
| dark SVG / Chromium `worker-arrow` | — | within (0 px, 0.997467 vs controls 0.927653) | frame-clip divergence under the root filter (ADR-0012) |
| PDF `queue-manager-typography`, both viewers | within | within | glyph rasterization (`../varmq84-pdf-typography-exception-v1`) |
| PDF whole, `worker-arrow`, both viewers | beyond | beyond | sub-pixel hatch widening (`../varmq84-pdf-subpixel-exception-v1`) |

The references disagree heavily on this scene: the official SVG vs its own
canvas differs by 54466 changed pixels / SSIM 0.973827 on the light whole page.

## PDF outside the cross-hatch rectangle (`pdf-tiles-{light,dark}.json`, 200 px tiles)

| | failing tiles outside the hatch | beyond controls | total excess over controls | max SSIM shortfall |
| --- | ---: | ---: | ---: | ---: |
| light PDF.js | 107 | 16 | 17 px | 0.0020 |
| dark PDF.js | 107 | 19 | 37 px | 0.0015 |
| light PDFium | 109 | 61 | 2896 px | 0.0035 |
| dark PDFium | 109 | 49 | 2773 px | 0.0035 |

PDF.js outside the hatch is reference-level variance. PDFium's excess sits on
stroke-dense tiles (about 1.5–3× the controls' changed pixels). That matches
its summing of overlapping Rough.js double-stroke coverage, measured directly
on Velox (`../velox84-pdf-exception-v1`) and in the `band-twice` control. The
earlier estimate of about 73000 differing pixels outside the hatch (any channel
over 32) was dominated by differences the references show against each other.

## Conclusion

No varmq difference needs a renderer change. What remains is:

- reference-disagreement budget calibration (#195);
- the known viewer behaviors already documented as exceptions: sub-pixel stroke
  widening, PDFium overlap summing and glyph rasterization;
- the recorded frame-clip divergence.

varmq can register once #195 settles the budgets, with exceptions only for what
the recalibrated budgets do not cover.

```sh
python3 -B tools/representative-intake/controls.py target/varmq-viewers-v3/light --output regions-light.json
python3 -B tools/representative-intake/controls.py target/varmq-viewers-v3/light --comparison pdf-pdfium --comparison pdf-pdfjs --tiles 200 --output pdf-tiles-light.json
```
