# Codec and nested-SVG fuzzing (#82)

The public `SceneDocument::parse` → validated `RenderRequest` → `Renderer::render`
facade is the seam confirmed by #30. `tests/asset_fuzz` is test-only. Every input
renders twice in fresh sessions in SVG, PNG and PDF. Success compares complete
artifact bytes and ordered reports; failure compares complete descriptors and
reports against an explicit permitted-code set. Internal failures are failures
of the harness. Spec §12's mandatory-notice encoding failure is permitted under
resource pressure. SVG output is recursively inspected for active content and
external references; PNG is decoded, and PDF must have its complete carrier.

## Coverage and execution

- PR: intact valid seeds for PNG/APNG, baseline/progressive/CMYK JPEG, GIF,
  lossless/lossy WebP, indexed/RGB/alpha/bitfield/RLE4/RLE8 BMP and
  PNG/indexed/RGB/bitfield ICO render directly and as
  repeated nested data images. Valid controls must reach projection without
  recovery. Sixty-four deterministic mutations rotate through all seven asset
  families. Browser WASM has the same smoke entry point.
- Nightly: every seed runs intact directly and nested, followed by 4,096
  mutations (configurable 1–16,384). The population includes the original scene
  payloads from the PNG SDR/HDR/ICC/gain-map, JPEG sampling/ICC/EXIF/gain-map,
  GIF first-frame/control/LZW and WebP sample evidence. Declared accepted
  evidence must reach projection. Byte-authored PNG vectors add channel/depth,
  palette/transparency, scanline/interlace and APNG controls. AVIF has an explicit
  rejection test because revision 1 does not admit it.
- SVG grammar seeds cover malformed XML, entities, namespaces, active content,
  CSS URLs and escapes, local and cyclic references, invalid data images,
  recursive sanitization, filters, masks, patterns, deep nodes and numeric excess.
  A focused assertion proves PDF fallback is reached and forbidden policy fails.
- Mutations truncate, flip bits, overwrite spans, insert grammar/container
  tokens, and reseal PNG chunk CRCs to reach interpretation beyond validation.
  Chains vary depth and duplicate siblings/aliased file IDs. Fifteen limit
  profiles cover ordinary admission, bytes, occurrences, nesting, nodes, pixels,
  intrinsic/aggregate work, surfaces, output, diagnostics and logical memory.
  Separate compressed-stream and dimension probes exercise expansion refusal.

This is reproducible mutation fuzzing, not coverage-guided fuzzing or a claim of
exhaustive byte-space/visual qualification. The existing profile-specific
appearance and exact resource-boundary gates remain the independent oracles.

```sh
cargo test --locked -p excalidraw-render --test asset_fuzz
just nightly-asset-fuzz        # 4,096 cases; pass a count to change it
```

GitHub's nightly scheduler runs the same recipe. Its 90-minute
job timeout is a backstop for hangs; timeout, abort and OOM are failed runs, never
typed successes. Inputs are bounded independently: payloads ≤256 KiB, ≤6 linear
wrappers or ≤2 repeated wrappers, scenes ≤8 MiB, ≤8 byte mutations. Semantic
ceilings bound decoded pixels (65,536/asset), surfaces (65,536), work
(4,194,304/asset; 16,777,216 total), output (2 MiB) and logical memory (32 MiB).
These are semantic limits, not claims about allocator RSS. No fuzz dependency
or production feature is added.

## Reproduce, minimize, retain

An assertion prints the seed, index, source ID, depth, repetition, limit mode,
fallback policy and **complete scene JSON**. Save that JSON and replay without
the generator:

```sh
EXCALIDRAW_ASSET_FUZZ_REPLAY=/tmp/failure.excalidraw \
EXCALIDRAW_ASSET_FUZZ_MODE=0 EXCALIDRAW_ASSET_FUZZ_FORBID=false \
EXCALIDRAW_ASSET_FUZZ_VALID=false \
cargo test --locked -p excalidraw-render --test asset_fuzz \
  asset_fuzz::replay_captured_asset_fuzz_scene -- --ignored --exact
```

Set `VALID` to the captured value: accepted controls also assert no local loss.
For a native trap/hang, rerun with `EXCALIDRAW_ASSET_FUZZ_TRACE=1` and
`--nocapture`: it flushes each complete case before execution, so the last case
identifies the failing input even without unwinding. Use process
isolation/timeouts for minimization. Reduce scene
siblings and wrappers first, then XML nodes/attributes or codec chunks/segments,
then payload bytes, retaining the same failure predicate and request. Repair
checksums where needed to keep the failing decoder path reachable. Do not
classify a different rejection as the original failure.

Every confirmed renderer failure must land as a codec- or sanitizer-specific
public-facade regression with the minimized payload, discovery seed/index,
failure description, and exact expected diagnostic/resource/artifact assertion.
Run it red before fixing production code and keep it in the ordinary PR suite.
The initial development runs found harness expectation errors (mandatory-notice
failure policy and the extra generated SVG placeholder depth), not confirmed
renderer defects; no invented failure corpus is recorded.
