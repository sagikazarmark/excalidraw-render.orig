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
  value in one or two pixels due to `tiny-skia-path` cubic-root math
  ([#178](https://github.com/sagikazarmark/excalidraw-render/issues/178)).
  Tests retain exact known output measurements; this is not a general allowance
  for new differences. Output lengths also affect three resource traces.
- Other operating systems and architectures are not release-qualified.
- Later 0.x releases may change rendering. Pin the renderer version, dependencies,
  and build environment when preserving exact output is important.

## Scene input

- A scene must be a version-2 document with `type`, `version`, `elements`,
  `appState` and `files` members, even empty ones. Excalidraw's own exports
  always have them, and Excalidraw treats a missing `files` map as empty; this
  renderer rejects a document missing any of them with
  `scene.invalid-envelope`, and the CLI names every missing member in one hint.
- Only `.excalidraw` JSON is read. PNG and SVG exports with an embedded scene,
  library files, and clipboard data are recognized and rejected.

## Images and rendering

- AVIF is unsupported and follows unsupported-image recovery
  ([#68](https://github.com/sagikazarmark/excalidraw-render/issues/68)).
- Some less common JPEG sampling configurations and CMYK/YCCK conversions differ
  from Chromium ([#187](https://github.com/sagikazarmark/excalidraw-render/issues/187)).
  HDR-base JPEG gain-map projection is incomplete
  ([#190](https://github.com/sagikazarmark/excalidraw-render/issues/190)).
- The pinned JPEG decoder can accept truncated entropy before a retained scan
  marker even in strict mode. Such malformed JPEGs can produce incorrect
  pixels instead of decode-failure recovery. Marker/framing checks alone do not
  validate all entropy. This is a known correctness limitation, not a claim of
  memory corruption. `jpeg_release_limitation` exercises the public API on a
  bounded 9×9 truncated-scan fixture in all three formats and requires repeatable
  output; it deliberately records the unresolved defect rather than claiming
  malformed data decodes correctly
  ([#193](https://github.com/sagikazarmark/excalidraw-render/issues/193)).
- Fractional PNG stroke coverage has remaining browser differences
  ([#194](https://github.com/sagikazarmark/excalidraw-render/issues/194)).
- A lossless WebP with bitstream alpha, a clear VP8X alpha flag, and a transforming
  ICC profile has a known color mismatch
  ([#197](https://github.com/sagikazarmark/excalidraw-render/issues/197)).
- Emoji render as monochrome Noto Emoji outlines, not the colour emoji a
  browser draws from system fonts; an opt-in colour face is planned
  ([#233](https://github.com/sagikazarmark/excalidraw-render/issues/233)).
- Text is drawn only with the bundled fonts. A browser falls back to system
  fonts; this renderer draws `?` for a character none of them covers and
  reports `font.missing-glyph`. The bundled fonts cover Latin, Greek and
  Cyrillic (Nunito lacks Greek; Lilita One and Comic Shanns lack both), and
  Chinese, Japanese and Korean only in the Excalifont family (through Xiaolai,
  the default `cjk` feature). Hebrew, Arabic, Armenian, Georgian, Ethiopic,
  Thai, and Indic scripts such as Devanagari, Bengali and Tamil draw as `?`, as
  do Vietnamese stacked diacritics outside Nunito and Cascadia Code.
- Other bundled font substitutions differ from ambient browser fonts.
  These are diagnosed. SVG/PDF viewer antialiasing can differ within reviewed
  visual budgets or case-specific exceptions.
- PDF is vector-first, not universally all-vector: unsupported nested-SVG effects
  can use diagnosed raster fallback. Multi-page, tagged, and archival PDF formats
  are outside the initial scope.

## Resource limits

- PNG work is charged from each element's device-space bounding box, not the
  pixels it paints, so long diagonal arrows and lines cost far more than their
  strokes take to draw. A large diagram of them at `--scale 2` can fail with
  `resource.total-work-exceeded` under the default limits. Library callers can
  raise `max_total_work_units` up to its absolute maximum; the CLI has no limit
  options, so use a lower `--scale` or `--max-dimension`.
- Filter effects in a nested SVG image are charged per primitive at the size
  they are rasterized, so a large or heavily parameterized filter (many
  turbulence octaves, a wide morphology radius, a long primitive chain) can
  fail a PNG or PDF render with `resource.total-work-exceeded` or
  `resource.total-rasterized-pixels-exceeded`; SVG output embeds the image
  and is unaffected.
- The default limits bound one render to a few seconds of CPU on a current
  core for hostile input. Services that render untrusted scenes concurrently
  should still apply their own timeout and concurrency limits.

## Coverage and issue references

The test suite includes reference comparisons, exact snapshots, resource limits,
fuzz regressions, packaged consumers, and browser execution. Exhaustive corpus
coverage and full cross-target qualification remain open. Passing
existing tests does not mean every valid Excalidraw scene has been qualified.

Follow-up work on these limitations is tracked in the repository's issues.
