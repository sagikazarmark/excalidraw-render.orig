# Bounded WebP samples, alpha and first-frame composition (#82 / #85 / #191)

This tranche starts at `2fa700f` without production, dependency or existing
approval changes. It adds **48 cells**: 36 sample cells / 72 browser vectors and
12 uncollected container controls.

## Independent encoded inputs and reference capture

Original RGBA patterns are independently encoded using C libwebp **1.3.2**,
preinstalled in the pinned Playwright OCI image, through Python ctypes. No Rust
production codec, Pillow, browser encoder, download or runtime network is used.
Encoder calls, quality and library/interpreter/OCI hashes are recorded. The 28
standalone frames are frozen and reproduced byte-for-byte during verification.

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
bash tools/reference-oracle/run-webp-sample.sh verify
cargo test --locked -p excalidraw-render --test fast webp_sample_contract::
node tools/reference-oracle/scripts/webp-sample-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 with incremental/debug profiles disabled. Ordinary Rust tests use
frozen data without external processes or networking. Both input preparation and
reference generation are create-only; verification re-encodes/recollects read-only.

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
