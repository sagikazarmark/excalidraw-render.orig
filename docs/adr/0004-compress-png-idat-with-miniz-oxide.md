---
status: accepted; the profile-binding paragraph is superseded by ADR-0005 and the approval round by ADR-0019
---

# Compress PNG `IDAT` with `miniz_oxide` behind an owned scanline filter

Until #113 the PNG encoder wrote `IDAT` as RFC 1951 *stored* blocks with the
filter type of every scanline fixed at `None`. The artifact bytes were trivially
deterministic and their size was known before the output allocation, but the
result was four bytes per pixel: 5-70 MB for ordinary whiteboards that the
reference renderer's PNG export writes in a few hundred kilobytes. Users compare
against that export, so the size read as a defect regardless of pixel
correctness.

`IDAT` is now one zlib stream produced by `miniz_oxide 0.8.9` at level 6 over
scanlines whose filter type the renderer chooses per row. Every other part of
the encoder is unchanged: one `IDAT`, chunk order `IHDR, IDAT, tEXt?, iTXt?,
IEND`, an 8-bit RGBA `IHDR`, and the spec §12.1 stored-zlib wrapper for the
embedded scene, which is a separate project-owned stream and stays stored.

## Why an adapter over `miniz_oxide`

Spec §10 orders remedies as adapter, pinned controlled fork, focused owned
port. `miniz_oxide` qualifies as the first: it was already locked and compiled
into the renderer through `png`, `flate2`, and `svg2pdf`, is `forbid(unsafe_code)`,
has no build script, and its compressor is integer-only with no host, time, or
random input, so the same bytes come out on every supported target. It already
had a green five-target signal in this repository: the pinned PDF digests carry
`FlateDecode` streams that `svg2pdf` produces with `miniz_oxide` at the same
level. Adding it as a direct dependency changed one edge in each `Cargo.lock`
and brought in no new package. A brotli-style controlled fork was therefore not
warranted, and an owned deflate port would have been the last remedy, not the
first.

The stream is driven through `compress_to_output` so output is appended with
`try_reserve`; allocation failure surfaces as `MemoryUnavailable` like every
other encoder buffer. The "exact size before allocation" property the stored
encoder relied on is kept by producing the `IDAT` stream first and sizing the
final buffer from it.

The stream is a pure function of the raster and independent of the metadata
chunks, so since #117 the plan rasterizes and deflates once into a
`PngIdatStream` and every artifact candidate — the baseline, each optional
metadata component during selection, and the accepted one at emission — is
assembled from that single stream as `IHDR + IDAT + tEXt?/iTXt? + IEND`.
Codec measurement and retained emission remain two semantic passes as spec
§13.2 requires, and the emitted bytes are still compared with the measured
ones; only the physical recomputation between them is gone. Before #117 each
call ran rasterization, alpha canonicalization, filtering, and deflate again,
so a default render paid for them twice and a render with all three optional
components up to five times.

The staged stream is a transient the ledger does not charge separately. Its
lifetime is the one the §13.2 `PNG encoder workspace` row already covers,
"measurement/emission through encoding": it is created after the workspace is
acquired and the `codec_measurement_input` work is charged, outlives metadata
selection, and is dropped as soon as the accepted candidate is emitted,
before the asset records and the workspace are released. On ordinary boards
it is one to two percent of the raster; on an incompressible raster it
approaches the raster size. The raster itself is dropped when compression
ends, so while compressing the physical peak is raster plus stream, and while
assembling it is the stream plus two artifact copies, since metadata
selection keeps the accepted candidate while it assembles the next and
emission keeps the measured bytes while it assembles the emitted ones. The
stored encoder held the raster beside those same two artifact copies, and the
first level-6 encoder, which rasterized and deflated inside every candidate,
held raster, stream, and the two copies. Spec §13.2 prices canonical, not
physical, peaks, and the `2 MiB + 16 * output_width` workspace row still
covers the compressor state (about 320 KiB) plus the two scanline buffers.

Rasterization at four times supersampling dominates a render and now runs
exactly once per PNG, so the level-6 timings below, measured before #117 with
the multiplier, are upper bounds; the same scenes rendered with all three
optional components went from 6.6 s to 1.4 s (`music-server-architecture`) and
10.8 s to 2.2 s (`excali-local-slides`) once the stream was shared, with
identical bytes.

## Why level 6 and a gated adaptive filter

Measured on `tests/public-scenes/` with a release build, level 9 cost two to
three times the deflate time for under two percent smaller output, and the
reference renderer's PNG export runs at level 6. Level 6 is the default the
reference is judged by.

The textbook adaptive filter (minimum sum of absolute residuals over the five
RFC 2083 filters) was also measured and it *loses* on Excalidraw content:
anti-aliased vector rendering is flat runs and repeated rows that deflate
matches directly, and every predictive filter produced output 10-20% larger
than unfiltered scanlines on real boards. On photographic rows the same
heuristic halves the output. The selector therefore keeps a row unfiltered when
at least half of its bytes equal their left or upper neighbour and falls back to
the minimum-residual choice otherwise; ties resolve to the lowest filter type so
the choice is a pure function of the row bytes. The selector is owned code, so
this heuristic can be tuned with new evidence without touching the compressor.

| scene | before | after | render |
| --- | ---: | ---: | ---: |
| `excalirender-sample` (420×370) | 622 KB | 31 KB | 43 ms |
| `agentstack-architecture` (1437×938) | 5.4 MB | 201 KB | 353 ms |
| `k8s-breakglass-diagram` (3795×1667) | 25.3 MB | 278 KB | 1.1 s |
| `excali-local-slides` (2831×3850) | 43.6 MB | 286 KB | 4.2 s |
| `music-server-architecture` (6535×2739) | 71.6 MB | 1.5 MB | 2.7 s |

## What moves and what does not

Registered conformance cases pin the reference renderer's PNG evidence and
compare pixels, so no evidence moved. The PDF backend qualification digests
never touch the PNG encoder and are unchanged. The renderer-owned PNG pins in
`tests/package-consumer` — `expected/rectangle.png` and the fallback-scene
artifact digest — were regenerated once from a release build on
`x86_64-unknown-linux-gnu`, and a third pin was added there for a noisy
embedded raster whose rows take the predictive filters, so both filter paths
and the compressor are pinned on all five targets; in the sense of ADR-0002
they are the cross-target canary for this encoder. The candidate PNG digests
in the arrow, outline-shape, and Excalifont render tests moved with the
encoding while their pixel assertions held. The canonical PNG resource-trace
digest in the renderer's unit tests moved for one reason only: the
`artifact_retention` entry admits the exact output size, and the 20×20
empty-scene PNG went from 1,688 to 91 bytes; every other entry and the header
are byte-identical. The `codec_measurement_input` work charge stays at the
raster size because filtering and deflate remain linear in it.

Adding the direct dependency edge changes the root `Cargo.toml` and
`Cargo.lock`, which the canonical comparator profile recorded at the time
(ADR-0005 has since made the closure a recorded, not bound, fact, and
ADR-0019 retired the approval round that accompanied every dependency
refresh).
