# excali-local-slides #84 evidence proposal, v2

Unapproved compact collection for `representative.excali-local-slides.v1`,
re-collected after PDF began lowering dark mode into nested-SVG content
(`4713c2e`, ADR-0013, #154). The reference collection is unchanged from v1.

**8 of 10 comparisons pass** (v1: 7). All light comparisons and the dark SVG
and PNG comparisons are identical to v1.

| Dark PDF | v1 `slide1-avatar` | v2 `slide1-avatar` | v2 result |
| --- | --- | --- | --- |
| PDFium | 11948 changed px, SSIM 0.765717 | 0 px, 0.998655 | fail (avatar crop only) |
| PDF.js | 12037 px, 0.766863 | 0 px, 0.999152 | **pass** |

The only remaining failure is the PDFium avatar crop, in both modes (light
0.998471, dark 0.998655) with 0 changed pixels against the geometry limit
0.999. PDF.js passes the same crop. See `../slides84-evidence-v1/DECISION.md`.
