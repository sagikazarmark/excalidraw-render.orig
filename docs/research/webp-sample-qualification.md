# Bounded WebP samples, alpha and first-frame composition (#82 / #85 / #191)

This tranche starts at `2fa700f` without production, dependency or existing
approval changes. It adds **48 cells**: 36 sample cells / 72 browser vectors and
12 uncollected container controls.

## Independent encoded inputs and reference capture

Original RGBA patterns are independently encoded using C libwebp **1.3.2**,
preinstalled in the pinned Playwright OCI image. The original collection used
Python ctypes; its interpreter, library, invocation and author-source hashes
remain historical provenance in the unchanged frozen records. Active reproduction
uses a tiny C simple-API helper with a Node JSON/base64/hash wrapper. No Rust
production codec or browser encoder is involved. All 28 standalone frames must
reproduce both encoded bytes and decoded straight RGBA byte-for-byte.

The API/options remain `WebPEncodeLosslessRGBA` with simple-API defaults and
`WebPEncodeRGBA` with `quality_factor=75.0f`, packed RGBA and `stride=width*4`.
`WebPDecodeRGBA` remains a same-library diagnostic, not an independent decoder.
Runtime checks pin both encoder/decoder versions, Ubuntu `libwebp7` package
`1.3.2-0.4build3`, and the SHA-256 of libwebp and libsharpyuv.

The canonical image has neither GCC nor libc/libwebp development headers.
`webp-helper.Dockerfile` acquires GCC/libc headers in a build stage based on that
digest, compiles offline, then copies only the helper and its build records into
the original pinned runtime image. Six public simple-API function declarations
avoid struct ABI bindings and any development-package codec replacement. Build
package versions and compiler identity are recorded, not assumed pinned across
future apt acquisition; helper/source/build-recipe hashes identify each build.
The required reproducibility is exact frame evidence under the pinned runtime
codec libraries. Compiler acquisition may use networking; tests and collection
always run with `--network none` and read-only fixture mounts.

Verification authenticates the original input/reference files by their existing
whole-file SHA-256 values, compares every frame record, reconstructs every case
and control, and compares complete browser observations. Historical producer
hashes are not required to match replacement producers. Current source hashes
are still checked for mutation during collection. Rust skips live-source checks
only for the retired bridge and the migrated WebP caller/runner; all data,
codec expectations, profile bindings and other source checks remain intact.

### Native helper protocol

Each process accepts exactly one binary request followed by EOF. All integers
are unsigned 32-bit little-endian. Input is the four-byte magic `WPI1`, width,
height, codec (`0` lossless / `1` lossy), quality (`0` / `75` respectively), RGBA
byte length, then packed RGBA. Output is `WPO1`, encoder version, decoder version
(both `0x010302`), encoded byte length, decoded byte length, then WebP bytes and
straight RGBA bytes. The fixture protocol bounds dimensions to 1–16383 and RGBA
to 16 MiB. Unsupported options, inconsistent lengths, truncation and trailing
data fail with a nonzero exit and stderr diagnostic; no successful partial frame
is accepted by Node. JSON/base64, fixture metadata and hashes never enter C.

The finite matrix crosses VP8L versus VP8/ALPH, opaque/partial/binary alpha and
six declared static/animation configurations with two asymmetric patterns. It is
not a full factorial of every flag. The canvas is 9×7, with a 5×3 subrectangle at
(2,2) in selected animations. First duration is 0xffffff milliseconds; browser
and host capture clocks must stay below 30 seconds. Later frames discriminate
selection but do not establish later-frame compositing.

Official Excalidraw v0.18.1 / Chromium149 capture uses unchanged profile/OCI,
network disabled, fresh opposite-order browser processes and per-vector contexts.
Observed intrinsic, official PNG and SVG-raster pixels plus actual black/white
composites are retained independently. All 216 observed surfaces match first-frame
composition on a transparent initial canvas, not the opaque ANIM background.
Authored source/lossy/readback/background hypotheses remain separate; 56 differing
source hypotheses are not mislabeled as 56 production defects.

| Record | SHA-256 |
| --- | --- |
| Inputs | `21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4` |
| Reference | `b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6` |
| Native defect measurements | `d31850b8a23ed04039de976437a06b9922bbc963ee83f3b6269833d5cc24ba76` |

## Native gates and findings

Exact selectors check RIFF/VP8X/ANIM/ANMF reconstruction, source/encoder/profile/
helper bindings, independent first/later/background discrimination and negative
gates. SVG preserves the original compressed payload. PNG/PDF check actual page/
image extents, alpha, canonical zero, complete reports and fresh replay. Lossless
straight samples have their own exact invariant; browser partial-alpha readback
is not substituted as straight PDF RGB. Canonical visual gates use separately
observed black/white composites. No ±RGB tolerance is introduced, including for
small artifacts where projected identity is required.

**13 sample cells / 26 vectors pass every gate**. **23 sample cells / 46 vectors
remain gaps under [#191](https://github.com/sagikazarmark/excalidraw-render.orig/issues/191)**.
Across all 72 vectors, SVG passes 72, PNG 26 and PDF 26. Exact reports/replay pass.
Failures include 48/63 alpha mismatches outside opaque-source subrectangles,
lossless blend color differences, and partial-alpha PNG/lossy PDF composite
`SmallArtifactMismatch` despite zero Pixelmatch changed pixels.

Known-defect assertions bind actual/expected pixel hashes, alpha mismatch counts,
straight-sample discrepancies, comparator result/changed count/SSIM bits where
applicable, and normalized diagnostics. Passing companions must still pass.
Pins reject improvement as well as regression so fixes require explicit review;
they are not tolerance exceptions or primary owners.

**Ten strict-recovery controls** check RIFF/chunk/frame faults and exact
`image.decode-failed` recovery. **Two VP8X reserved-field controls** check the
specification-required ignore behavior, with exact baseline output/reports. They
do not qualify the underlying partial-alpha scene's pixels. The frozen author's
initial rejection intent remains recorded; the executable contract distinguishes
it from the normative ignore rule. Reserved ANMF rejection is explicitly a
strict current characterization, not an independently observed browser rejection.

## Append-only ownership

The predecessor catalog authenticates against the `2fa700f` ledger. All 2,370
prior dispositions/criteria, three exclusions, 168 keys and V12 remain exact.

| Measure | Count |
| --- | ---: |
| Obligations | 2,418 |
| Primary owners | 2,307 |
| Approved exclusions | 3 |
| Assigned/excluded | **2,310** |
| Explicit gaps | **108** |
| New owners | 25 |
| New #191 gaps | 23 |

No WebP umbrella facet is closed. Broader profiles, ICC/EXIF, animation, resource
and fuzz qualification remain open, as do the deferred JPEG and #178 work.

## Reproduction

```sh
bash tools/reference-oracle/run-webp-sample.sh build-helper
bash tools/reference-oracle/run-webp-sample.sh test-helper
bash tools/reference-oracle/run-webp-sample.sh verify
cargo test --locked -p excalidraw-render --test fast webp_sample_contract::
node tools/reference-oracle/scripts/webp-sample-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 with incremental/debug profiles disabled. Ordinary Rust tests use
frozen data without external processes or networking. Both input preparation and
reference generation are create-only; verification re-encodes/recollects read-only.
`test-helper` needs no npm packages, browser launch or Python: it exercises native
framing/options/length errors, Node request/response validation, rehashed-byte
drift rejection, all frozen frames in opposite orders, and fixture reconstruction.

## Final verification and review

- Independent preparation and recollection reproduced all 28 encoded frames and
  the complete frozen browser record byte-for-byte.
- Full workspace: **1,261 passed, zero failures, 19 existing ignored tests**,
  including all 625 fast tests, before the final oracle correction below.
- Spec review identified an omitted independent visual gate: lossless PDF used
  exact straight samples instead of additionally checking browser black/white
  composites. Both are now required. Eight already-unqualified partial-alpha
  vectors pass straight samples but fail both composites (10–39 differing pixels,
  maximum RGB delta one, canonical `SmallArtifactMismatch`). All four focused
  tests pass after strengthening; qualified-cell selections are unchanged.
- The uncommitted native measurement record changed from `d8b85832...` to the
  digest above solely to add the omitted gates and update aggregate judgments.
  Removing those additions and restoring earlier flags reproduces the exact old
  hash; no pre-existing pixel measurement or frozen reference changed.
- Formatting and Clippy pass. Standards review found no hard violations; typed
  configuration fields and shared view construction remain advisory suggestions.
  The long workspace suite was not redundantly repeated after the test-only fix.
