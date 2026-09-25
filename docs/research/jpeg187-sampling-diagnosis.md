# #187 baseline sampling: stable-decoder diagnosis

Status: archived external experiment, not a production fix or new ownership.
The Python runner was retired on 2026-09-25; reproduction commands below are
historical transcripts, with runner source available in Git history. Patches,
probe sources and results remain. Current regression checks live in
`crates/excalidraw-render/tests/fast/jpeg_color_contract.rs` (sampling) and
`crates/excalidraw-render/tests/jpeg_release_limitation.rs` (truncated entropy).
Base `6eb4b22`; renderer remains on unchanged zune-jpeg 0.5.15 and root/consumer
locks. Six baseline YCbCr/YCCK sampling cells remain gaps.

## Minimal failure and causal experiments

A decoder-only strict, unsafe-disabled decode of a 9×9 constant Y=96/Cb=Cr=128
image leaves nine black pixels in the last row at 4×2, 1×4 and 2×4 sampling.
No AC variation, renderer, CMS or K conversion is needed to reproduce it.
The existing browser-qualified neutral regression corroborates the expectation.

Four changes in an external copy of stable `src/mcu.rs` explain the failures:

1. **Scan height:** single-component baseline scans need
   `ceil(height * component_v / (v_max * 8))` data-unit rows, rather than the
   interleaved image MCU count.
2. **Destination stride:** a buffered component block-row advances by
   `row * component_stride * 8`; multiplying again by component vertical sampling
   inserts holes in the decoded plane.
3. **Entropy lookahead:** the bitreader can discover the next SOS while valid bits
   for remaining rows are still buffered. Parsing that marker after the current
   row discards those bits. Deferring the scan transition until the expected final
   component row fixes the minimal missing-row case.
4. **Scan state:** restart countdown and DC predictors must reset at scan entry.
   Adding restart intervals exposed fifty failures after the first three changes;
   the scan-state reset removed them. The experiment changes both resets together
   and does not independently attribute each one's contribution.

The internal `PROGRESSIVE` const parameter in this function means buffered
baseline decoding; actual progressive JPEG decoding follows `mcu_prog.rs`.

## Reproducible results

`tools/jpeg187-diagnosis/` retains the four-hunk patch, external-only Cargo project,
locked graph, executable probe and raw results. Run:

```sh
python3 tools/jpeg187-diagnosis/run.py /tmp/opencode/jpeg187-new-run
```

The runner copies the cached stable decoder into a fresh temporary directory,
first demonstrates failure, applies the patch there, and executes the corrected
probes. It never patches Cargo's registry cache or the production workspace.

- Rust **1.97.1**, strict mode, unsafe decoder paths disabled.
- Pinned **zune-core 0.5.1**, matching the renderer decoder's dependency. Earlier
  exploratory runs used 0.5.3; final retained reruns use 0.5.1.
- **2,160 neutral cases pass:** three sampling factors × six widths × twelve
  boundary heights × five restart intervals × baseline/progressive modes.
- **76 original color vectors pass both frozen browser authorities**: 152
  canonical geometry/image comparisons, unchanged thresholds. The probe uses
  the existing owned CMYK/YCCK conversion; this conversion is not independent
  expected evidence. Browser PNG and SVG raster observations remain independent.
- Patched mcu.rs SHA-256:
  `07376584a74ae021b615a5ddb2c63fc2adc80408e20cb6b30bd0ffa938c2cd13`.
- Original mcu.rs SHA-256:
  `b2234b35227ee76fc43df76b64f5b68e44a395250d187016765d5922849dcfb3`.

These are dependency-level diagnostics, not renderer/package/native-WASM
qualification. The external lock also contains diagnostic dependencies which
are not the complete production graph. No full workspace run was warranted.

## Adoption blockers found by review

**Malformed entropy scans:** truncating the first scan to 1–5 of its six bytes
is accepted in strict mode by both original and patched decoders, with fabricated
pixels. Removing the final chroma scan's entropy byte can even return the expected
gray image. Across the 24 bounded truncation probes, acceptance/rejection agrees
between original and patched versions. This does not prove no broader regression.
The bitreader supplies zero bits after a prefetched marker; row-count gating alone
does not distinguish legitimate buffered bits from synthetic padding.

**Mixed multi-component baseline scans:** the height/marker changes are restricted
to single-component scans, but the destination offset change applies more broadly.
Legal component subsets, inter-scan DHT/DQT/DRI, missing/misplaced RST markers,
nonuniform blocks across boundaries, and strict/permissive recovery need targeted
controls before adopting the patch. Existing unchecked workspace/allocation and
IDCT write-span assumptions remain outside this four-hunk diagnosis.

Independent Standards and Spec reviews endorse the causal diagnosis but not
production adoption. Debug instrumentation was removed and the temporary failing
repository test deleted; the original exact known-defect tests remain unchanged.

## Recommended next implementation boundary

Continue with a **pinned patch to stable zune-jpeg**, first hardening and testing
the scan/entropy boundary and mixed-scan behavior in this isolated harness. Prefer
an upstreamed release carrying the correction; a vendored stable patch is an
explicit dependency-maintenance choice if that cannot wait. Do not return to the
previous RC merely because it passes the original color matrix: its minimized
missing-row case was already shown to fail.

Before production integration: review package/license provenance and fallible
resource behavior, run real renderer PNG/PDF/SVG contracts and prior JPEG suites,
qualify native/WASM behavior and all affected packaged/representative bindings,
then request exact migration approval. Until that work, all six #187 gaps and
their defect pins remain live; #190 reduced-IDCT strategy is still separate.
