# PNG precision and alpha qualification (#85)

Base: `3f47cc2b880a86bafacb2ebc26ecd819320a46c6`.

## Reference and scope

`tests/oracle/png-precision/inputs.json` authors asymmetric 16-bit samples,
low-byte boundaries, transparency keys and alpha vectors. The collector in
`tools/reference-oracle/scripts/png-precision-evidence.mjs` writes PNG framing,
big-endian samples and chunks directly, then calls the immutable official
Excalidraw v0.18.1 export wrapper. It does not call the Rust renderer to generate
expected pixels.

`bash tools/reference-oracle/run-png-precision.sh generate` rebuilds and verifies
the existing reference profile before collecting **30 cases** in the exact-pinned
linux/amd64 Playwright container, with Chromium **149.0.7827.55** and networking
disabled. Two fresh browser processes execute forward/reverse case orders with
a fresh context per case. Complete PNG bytes and SVG bytes agree between runs.
Generation refuses an existing record; `verify` reproduces and compares it.

`reference.json` binds the authored inputs, collector, runner and reference
profile by SHA-256. The consuming tests additionally pin the complete record's
digest. It contains complete scene/request inputs and official PNG bytes, plus
independently authored straight-color/alpha vectors for PDF. Those PDF vectors
use full-width tRNS comparison, high-byte sample reduction and the specified
linear-to-sRGB transfer before transparent RGB normalization. They are not
copied from premultiplied browser pixels.

### Transparency-key range applicability

The `16bit-transparency-key` criterion also mentions "out-of-range keys".
For a valid 16-bit grayscale/RGB PNG, each tRNS key component occupies exactly
two unsigned big-endian bytes: its domain is **0..65535**, identical to the
sample domain. A numerically out-of-range key therefore cannot be encoded in
this class. The vectors explicitly include both endpoints and neighboring
same-high-byte values. A key above a *lower-bit-depth* sample's maximum belongs
to `codec:png:malformed:palette-trns`; a wrongly sized tRNS payload is malformed
chunk structure under that same obligation, rather than a 17-bit key.
This resolves applicability within the existing criterion; it does not add a
ledger exclusion or waive those malformed-input requirements.

## Recorded owners

The exact selectors in `fast/png_precision_contract.rs` are:

| Obligation | Selector | Evidence |
| --- | --- | --- |
| `codec:png:precision:16bit` | `sixteen_bit_precision_matches_pinned_reference` | Every 16-bit grayscale value; asymmetric RGB and intrinsic-alpha thresholds in all four legal 16-bit channel layouts, untagged/sRGB/linear gAMA cases |
| `codec:png:alpha:16bit-transparency-key` | `sixteen_bit_transparency_keys_compare_before_reduction` | Grayscale/RGB keys at 0, 0x1234, 0xff00 and 65535; exact matches, neighboring low bytes and individual-channel near matches |
| `codec:png:alpha:premultiplication-rounding` | `alpha_rounding_and_compositing_match_pinned_reference` | Every 8-bit channel/alpha pair, channel permutations, gray-alpha and RGBA boundary strips, transparent/white/colored backgrounds |

Every case renders SVG/PNG/PDF through two fresh public-facade sessions. Tests
assert complete empty reports, exact repeated artifact/report bytes, untouched
SVG source payloads, reference PNG pixels/dimensions, and independently authored
PDF RGB/soft-mask planes through actual page/resource/profile/mask bindings.

These precision vectors characterize sample reduction and representative
transfer paths. Full ICC/cICP/HDR profile qualification, APNG controls, masked or
rotated compositing interactions and profile-axis composition retain their
separate inventory obligations.

## Demonstrated corrections

The exhaustive alpha reference exposed two differences hidden by the previous
8-bit-lattice vectors:

1. **Straight-alpha conversion.** The old integer half-up conversion emitted
   `[0,255,213,6]` for source `[3,252,219,6]`; the pinned canvas emits
   `[0,255,212,6]`. Integer ties-to-even alone also failed: premultiplied 3 at
   alpha 10 emits 77, not 76. A numerical probe over every channel/alpha pair
   identified the canvas's float32 normalization, reciprocal-alpha multiply,
   rescaling and ties-to-even store sequence, with zero mismatches. The encoder
   now preserves that operation order; a two-pixel facade regression pins both
   rounding directions. This shared conversion also serves PDF raster closures.
2. **Unmasked image source-over.** At full element opacity, the canvas's byte
   blit uses `source + ((destination * (256 - source_alpha)) >> 8)` on each
   premultiplied channel. The prior tiny-skia blend rounded a `/255` destination
   product. Over `#17395b`, source `[0,255,0,1]` must become `[22,57,90,255]`, not
   `[23,58,91,255]`. The exhaustive colored-background grid confirms the byte
   blit formula. The integral-translation, unit-scale, unmasked, full-opacity
   raster path now composites its device pixels with that arithmetic into the
   admitted supersample blocks, without an additional allocation. Other sampled,
   masked and opacity-modified paths keep their separate qualification scope.

The five precision/alpha facade tests and all four existing PNG/APNG sample tests
pass after these corrections. A consuming PDF effect-closure regression halves
a constant field's alpha to premultiplied `[0,6,5,6]`, then asserts every encoded
RGB sample is `[0,255,212]`, the attached grayscale soft mask is 6, the dimensions
and complete fallback report are exact, and both fresh sessions agree. This
exercises the shared conversion through `PdfRasterAdapter::render_node`, rather
than inferring fallback correctness from ordinary PDF image embedding.

The browser-WASM test also executes every reference scene twice and passes exact
PNG pixel/dimension and complete silent-report comparisons for all 30 cases.
This local worker run used the installed Chromium/ChromeDriver 151.0.7922.173;
the official reference evidence was collected in the pinned Chromium 149
container described above. It verifies the arithmetic on executed WASM, not
the separate packaged cross-target equality/trace gate under #178.

Spec review confirmed all three assignments after the key-range applicability
explanation and PDF effect-closure regression. Standards review found no hard
breaches; its native/WASM test-harness duplication note is nonblocking. The two
harnesses share the same digest-bound reference record, with native-only source
file/SVG/PDF checks and a separate browser-worker PNG consumer.

The three owners are recorded. The precision checkpoint totals were **1,029 assigned/excluded
and 847 gaps** (1,026 proposed owners and three approved exclusions). Existing
PNG/APNG and PDF-fallback selectors retain their assignments with reviewed source
bindings refreshed; the catalog and approved exclusion subjects remain valid.
Subsequent [SDR color qualification](../png-sdr-color-ownership.md) adds 67 owners;
current totals are tracked in the global ownership audit.

### Verification checkpoint

- Five precision/alpha facade tests, four existing sample-matrix tests and all
  17 PDF-fallback contract tests pass.
- Workspace/all-target Clippy with warnings denied passes.
- The targeted browser-WASM test passes all 30 reference scenes as described above.
- Ownership validation passes with the recorded **1,029 / 847** totals.
- Full `cargo test --locked --workspace --no-fail-fast` with Rust 1.97.1:
  **1,097 passed, 18 ignored**. The fast binary passed all 523 tests in 922.57
  seconds; registered SVG/PNG parity and ownership/coverage CLI tests also passed.
  The ignored external-harness/materialization tests are outside this local run.
- `bash tools/reference-oracle/run-png-precision.sh verify` independently
  reproduced all 30 cases and the complete record hash
  `8400494ec5f6e744b00e175db6931aff33c2a659dc57c3b3ab62646da0cf83e5`.
- `conformance verify-all` verified **37** registered cases;
  `verify-inventory` verified **168** existing executions. Formatting and diff
  checks passed, and temporary diagnostic probes were removed.

Existing approved snapshot subjects are not automatically refreshed by these
changes; #178 remains separately tracked.
