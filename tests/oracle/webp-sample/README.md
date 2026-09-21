# Bounded WebP sample and first-frame evidence

This isolated collection records independently authored pixels and WebP RIFF
structure, external libwebp encoding, and official Excalidraw v0.18.1 / Chromium
149 observations. Authored hypotheses and observed differences remain separate.

```sh
bash tools/reference-oracle/run-webp-sample.sh prepare  # create-only inputs.json
bash tools/reference-oracle/run-webp-sample.sh generate # create-only reference.json
bash tools/reference-oracle/run-webp-sample.sh verify   # read-only re-encode/recollect
```

All stages use the existing digest-pinned linux/amd64 Playwright container with
network disabled. Preparation uses its preinstalled Ubuntu libwebp 1.3.2 through
Python ctypes; no apt, download, compiler, Pillow, browser encoder or production
Rust codec is involved. The input manifest pins encoder ABI calls, quality,
library/interpreter hashes, distribution version, OCI identity, and author source
closure. Its native libwebp decode is diagnostic, not an independent encoder
validation. Original RGBA and compressed frame bytes are frozen independently of
browser collection. Lossless invisible RGB may be discarded by the simple API;
authored inputs retain that RGB and canvas expectations canonicalize alpha-zero
pixels to zero RGBA.

## Finite matrix

**36 cells / 72 valid vectors**: 2 codecs (VP8L, VP8 with optional ALPH) × 3 alpha
modes (opaque, partial, binary) × 6 configurations × 2 asymmetric patterns.
The six configurations are static full, animated full blend/d0/transparent,
animated full replace/d1/opaque, animated subrect blend/d1/opaque, animated subrect
replace/d0/transparent, and animated subrect replace/d1/opaque. This is a bounded
configuration matrix, not a full factorial over all flags. Static WebP has no
subrectangle placement. Canvas is 9×7; animated subrect is 5×3 at even offset
(2,2). Patterns include ramps, hard edges and nonzero invisible RGB. ANIM BGRA
bytes encode transparent black or opaque RGBA (41,83,137,255).

Each animation has a visibly distinct full opaque later frame. First duration
is **0xffffff ms**, with browser and host capture clocks bounded to **30 seconds**
starting before the first image load. Disposal 0/1 is first-frame metadata only;
later-frame disposal/composition and arbitrary capture times are outside scope.

**12 malformed controls remain uncollected**: RIFF size short/long, chunk length
overflow, VP8X reserved flag/byte, ANMF reserved flag, frame outside canvas,
intrinsic frame-size mismatch, missing ANIM, malformed first/later payload, and
truncated later chunk. Intent is separate from browser admission or parent policy.

## Exact evidence and consumption

The author emits container bytes, offsets, nested chunk lengths, raw encoded
frame payloads, frame identities, original RGBA and independent transparent-canvas
and ANIM-background first-frame hypotheses. This supports a Rust checker that
reconstructs RIFF/ANMF from metadata without trusting production decoding.
Lossless visible straight RGB and all alpha bytes are checked exactly by the
external encoder bridge. Browser canvas unpremultiply can change partial-alpha
RGB: straight source equality and canvas equality are distinct recorded checks.
Lossy source RGB is never an exact decoded-pixel expectation. Exact pinned browser
RGBA is canonical; there is no ±RGB tolerance or expectation refresh.

Each vector runs in a fresh context in each of two fresh browser processes,
forward and reverse. Complete official PNG, SVG, SVG-raster PNG, intrinsic and
rendered RGBA observations must repeat exactly. Each raster also has black and
white opaque canvas composites, avoiding guesses about hidden premultiplied
precision from straight readback. Authored RGBA canvas controls isolate canvas
quantization; standalone first/later encoded-frame observations isolate container
composition and frame selection. Every discrepancy is retained.

Collection builds in tmpfs and verifies the existing reference profile. Generation
refuses replacement; verification re-encodes and recollects with the repository
read-only. Bound author/collector/runner/helper files must remain frozen after
generation. Production behavior, tests, approval and ledger policy belong to the
parent task.

## Collected result

Generation and a separate read-only verification both passed, including exact
re-encoding of all **28 frozen standalone frames** and complete forward/reverse
observation equality. Both runs verified the existing profile digest
`1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71`.
The reference binds **20 source/profile/input files**.

- **72/72 admitted**, including 12 static and 60 animated vectors.
- **216/216** intrinsic/official-PNG/SVG-raster surfaces match the standalone
  first-frame composition on a transparent canvas. All preserve exact authored
  alpha, canonical zero invisible RGB, and 9×7 dimensions.
- All official surfaces match intrinsic RGBA and its black/white composites.
  **0/180 animated surfaces** match the distinct standalone later frame.
- **56 vectors retain hypothesis differences** (20 lossless, 36 lossy). This is
  a union of failed hypotheses, not 56 decoder failures. Every lossy vector
  differs from original source RGB. All 24 lossless opaque/binary vectors match
  authored straight RGBA; all 12 lossless partial-alpha vectors differ in RGB
  readback while preserving alpha. For both codecs, standalone partial-alpha
  WebP canvas readback also differs from putting external native straight RGBA
  into ImageData and sampling it. Both observations remain recorded; no rounding
  tolerance or inferred conversion formula is substituted.
- For **24 opaque-ANIM-background subrectangle vectors** (72 surfaces), the
  background-composition hypothesis disagrees. All instead match transparent
  initial-canvas composition. This holds for both blend and replace configurations.
- All **12 malformed controls** remain explicitly uncollected.

Generation collected 144 captures in **19,384 ms**, maximum capture **48.5 ms**;
verification collected 144 in **20,400 ms**, maximum **47.2 ms**. Timings are run
diagnostics, excluded from byte-equality evidence; each observation stores the
checked under-30-second fact. Create-only refusal was also checked for both files.

SHA-256:

| File | Digest |
| --- | --- |
| `inputs.json` | `21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4` |
| `reference.json` | `b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6` |

Sources were frozen before preparation and remain unchanged after collection.
The initial worktree HEAD was `2fa700fbc93e6e1472d89de8b83b404dbd22bdea`.
