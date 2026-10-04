# Known limitations for 0.1.0

The renderer targets Excalidraw 0.18.1. It is not pixel-identical to the browser
renderer and does not claim complete conformance. Compatibility reports describe
declared repairs, substitutions, omissions, and fallbacks. The differences below
are known defects or unqualified behavior; a report is not guaranteed to diagnose
each one at runtime.

## Platforms and reproducibility

- Linux x86-64 GNU is the release-qualified native target. The prebuilt binary's
  glibc requirement follows the pinned release builder and is recorded with its archive.
- Browser WASM is experimental. Four PNG cases differ from native by one channel
  value in one or two pixels due to `tiny-skia-path` cubic-root math (#178).
  Tests retain exact known output measurements; this is not a general allowance
  for new differences. Output lengths also affect three resource traces.
- Other operating systems and architectures are not release-qualified.
- Later 0.x releases may change rendering. Pin the renderer version, dependencies,
  and build environment when preserving exact output is important.

## Scene input

- A version-2 scene must carry a `files` map, even an empty one. Excalidraw
  treats a missing map as empty; this renderer rejects the scene with
  `scene.invalid-envelope`, and the CLI suggests adding `"files": {}`.
- Only `.excalidraw` JSON is read. PNG and SVG exports with an embedded scene,
  library files, and clipboard data are recognized and rejected.

## Images and rendering

- AVIF is unsupported (#68) and follows unsupported-image recovery.
- Some less common JPEG sampling configurations and CMYK/YCCK conversions differ
  from Chromium (#187). HDR-base JPEG gain-map projection is incomplete (#190).
- The pinned JPEG decoder can accept truncated entropy before a retained scan
  marker even in strict mode (#193). Such malformed JPEGs can produce incorrect
  pixels instead of decode-failure recovery. Marker/framing checks alone do not
  validate all entropy. This is a known correctness limitation, not a claim of
  memory corruption. `jpeg_release_limitation` exercises the public API on a
  bounded 9×9 truncated-scan fixture in all three formats and requires repeatable
  output; it deliberately records the unresolved defect rather than claiming
  malformed data decodes correctly.
- Fractional PNG stroke coverage has remaining browser differences (#194).
- A lossless WebP with bitstream alpha, a clear VP8X alpha flag, and a transforming
  ICC profile has a known color mismatch (#197).
- Emoji render as monochrome Noto Emoji outlines, not the colour emoji a
  browser draws from system fonts (#233 tracks an opt-in colour face).
- Bundled font substitution and glyph fallback differ from ambient browser fonts.
  These are diagnosed. SVG/PDF viewer antialiasing can differ within reviewed
  visual budgets or case-specific exceptions.
- PDF is vector-first, not universally all-vector: unsupported nested-SVG effects
  can use diagnosed raster fallback. Multi-page, tagged, and archival PDF formats
  are outside the initial scope.

## Coverage and issue references

The test suite includes reference comparisons, exact snapshots, resource limits,
fuzz regressions, packaged consumers, and browser execution. Exhaustive corpus
coverage and full cross-target qualification remain open (#85/#86). Passing
existing tests does not mean every valid Excalidraw scene has been qualified.

Issue numbers here refer to the historical development tracker
`sagikazarmark/excalidraw-render.orig`. Public follow-up issues must be created or
mapped when moving to `sagikazarmark/excalidraw`; numbers must not be assumed to
carry over. The descriptions above are self-contained for readers without access
to the old repository.
