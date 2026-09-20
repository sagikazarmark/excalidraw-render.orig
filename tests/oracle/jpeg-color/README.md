# JPEG color/sampling reference matrix

Project-authored, untagged 8-bit DCT JPEG vectors. The independent fixture encoder
is `jpeg-encoder 0.6.1`; no production decoder supplies expected pixels.

The matrix crosses baseline/progressive with gray, raw RGB (Adobe transform 0),
YCbCr (JFIF), CMYK (Adobe transform 0), and YCCK (Adobe transform 2). YCbCr/YCCK
exercise encoder sampling factors 1×1, 2×1, 1×2, 2×2, 4×1, 4×2, 1×4 and 2×4.
Other models use 1×1. Each cell has an odd 33×25 color ramp and a 48×32 asymmetric
edge pattern. The 38 cells / 76 vectors do not claim every JPEG sampling layout,
profile, precision, marker precedence or gain-map combination.

The `jpeg_matrix_inputs` Rust test reconstructs the frozen inputs byte-for-byte. Collection
uses `tools/reference-oracle/run-jpeg-color.sh generate`, twice in fresh pinned
browser processes with reversed case order and fresh contexts. Verification
regenerates in memory and compares byte-for-byte; approved evidence is never
overwritten. Official SVG and direct canvas PNG are independently exported.
