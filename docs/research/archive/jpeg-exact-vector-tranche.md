# JPEG exact-vector tranche (#82 / #85)

This tranche adds five public-facade tests in
`crates/excalidraw-render/tests/fast/jpeg_contract.rs`. The baseline is Stage-4
commit `c9fc43d`. It is bounded exact-vector evidence, not full JPEG profile
qualification or a new reference-renderer collection.

## Authored expectations

The independent test encoder is pinned `jpeg-encoder 0.6.1`; production decodes
with `zune-jpeg`. An asymmetric 24×16 grayscale image contains six constant 8×8
blocks with levels 32, 64, 96, 160, 192 and 224. Constant blocks have zero AC
coefficients; their DC coefficient is `8 * (gray - 128)`. Quality 100 uses unit
quantization, so these authored integer values reconstruct exactly. No decoder
or renderer output supplies the expected samples, and no channel tolerance is
used. Uniform 17×9 images exercise partial edge blocks, including black, middle
gray and white.

The eight TIFF orientation permutations are written out as six block indices.
Both byte orders cross baseline and progressive JPEG. The expected dimensions
and every pixel are checked; square or constant images cannot accidentally
hide an orientation error. Invalid TIFF magic, offsets, field types/counts,
truncation and orientation values exercise metadata fallback independently of
JPEG marker validation.

## Scope

| Test family | Inputs / configurations | Checks |
| --- | ---: | --- |
| Exact gray samples | 48 | Baseline/progressive × restart off/on × JFIF/raw, asymmetric blocks and five odd-size solid levels |
| Valid orientation | 32 | Eight orientations × TIFF byte order × baseline/progressive |
| Missing/invalid orientation | 42 | Unchanged source layout, silent metadata fallback |
| Malformed marker/ICC framing | 13 | Missing EOI, illegal segment lengths, absent frame/scan, concatenation/trailing garbage; missing/zero/duplicate/inconsistent ICC sequence fields |
| Dimension/pixel limits | 12 format/configuration pairs | Equality and first crossing; direct-SVG exemption |

The first four families run in SVG, PNG and PDF: **417 total format/configuration
checks**, including the 12 boundary configurations. Every configuration renders
twice with fresh `Renderer` instances and compares complete reports and artifact
bytes. This establishes within-target replay, not native/WASM equality.

Accepted vectors require an exactly empty structured report. SVG parses the
actual image node and compares its complete canonical data URL to the original
compressed octets. PNG checks dimensions, every RGBA sample and the absence of
color/profile metadata. PDF uses the existing binding-aware image oracle: page
`/Im0`, actual ICC profile/OutputIntent and attached grayscale soft mask. It
checks every decoded sample, opaque alpha and lossless rather than DCT embedding.

Malformed vectors require exact ordered machine report fields, including the
validated media type, JSON pointer, severity/effects and target-specific recovery
recipe. Only human-readable message text is excluded, after checking it is
nonempty. Recovery artifacts equal independently authored missing-asset controls.
Boundary failures likewise check the complete machine report and recovery artifact.

## Coverage disposition

All eight coarse JPEG obligations remain explicit gaps. In particular:

- Gray DC vectors do not qualify RGB/YCbCr/CMYK/YCCK, sampling/color conversions,
  ICC matrix/LUT families or metadata precedence.
- Basic EXIF vectors do not establish duplicate APP1/tag or late-marker
  precedence against the canonical browser.
- The previous synthetic gain-map unit fixture has `MPF\0fixture-index`, not a
  valid MP index. It is not evidence of real Ultra HDR container qualification.
- Thirteen rejection examples do not establish exhaustive malformed handling,
  decompression bounds or physical allocation safety.
- Local dimension/pixel boundaries do not close byte/work/aggregate limits.

Accordingly the ledger remains **1,874 obligations, 1,803 assigned/excluded,
71 gaps**. No criteria, source bindings, approvals, corpus bytes, tolerances or
production code change. The existing V8 representative owner still verifies.
The new tests and their fast-tier registration do not require requalifying an
approved producer or assigning an umbrella obligation prematurely.

## Next qualification slice

Collect independent canonical-browser evidence for JPEG color models and
sampling, plus duplicate/late EXIF and real MPF/gain-map containers. Expand those
axes into append-only, source-bound inventory cells under ADR-0011 before
recording narrow owners. Preserve the coarse facets until every accepted path is
characterized. #178 and native-baseline reapproval are deferred by user direction.

## Verification

Focused command (Rust 1.97.1; incremental/debug profiles disabled):

```sh
cargo test --locked -p excalidraw-render --test fast jpeg_contract::
```

All five tests pass. Workspace formatting and Clippy with warnings denied pass;
inventory, ownership and approved V8 representative-owner verification pass.

The completed `cargo test --locked --workspace --no-fail-fast` run passed
**1,212 tests with zero failures and 19 existing ignored tests**, including all
577 fast tests and both registered-case parity targets. Ignored external harness
and explicit collection tests were not executed by this command. Browser-WASM
execution was not part of this tranche.

Independent Standards review found zero documented violations or actionable
heuristic findings. Independent Spec review found zero scoped blockers, confirmed
the eight permutations and 417 configurations, and confirmed that the broader
JPEG qualification requirements remain explicitly open.
