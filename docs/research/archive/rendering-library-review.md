# Rendering library review

**Code-specific review and primary-source research — 2026-09-16.** Scope:
the rendering pipeline, geometry, text/fonts, raster assets, nested SVG/CSS,
SVG/PNG/PDF output, and the conformance machinery relevant to substitutions.
This is a source/API review, not an experimental qualification of replacements.
No renderer implementation or dependency was changed, and no replacement
benchmarks or native/WASM comparison runs were performed.

## Executive recommendation

**Use more library-owned mechanisms inside the existing pipeline. Keep the
Excalidraw compatibility decisions at the boundaries.** Most of the difficult
rendering algorithms are already delegated. The best immediate opportunities
are smaller than the largest source files suggest.

| Order | Concrete opportunity | Recommendation | Scope / qualification cost |
| --- | --- | --- | --- |
| 1 | PNG's handwritten CRC → existing `crc32fast` | Adopt ([#179](https://github.com/sagikazarmark/excalidraw-render.orig/issues/179)) | Tiny change; require identical complete PNG bytes. |
| 2 | ECMAScript number formatting → `ryu-js` | Strong candidate ([#180](https://github.com/sagikazarmark/excalidraw-render.orig/issues/180)) | Small adapter; check exact JS spelling and Rough trimming. |
| 3 | Hand-split CSS values/styles → existing `cssparser` | Prefer for syntax handling ([#181](https://github.com/sagikazarmark/excalidraw-render.orig/issues/181)) | Medium refactor; preserve the admitted subset, source attribution, and numeric precision. |
| 4 | Manual OpenType name-table handling → existing Fontations readers/writers | Adopt incrementally ([#182](https://github.com/sagikazarmark/excalidraw-render.orig/issues/182)) | Narrow font-table refactor; raw legal records and derived bytes need qualification. |
| 5 | Custom BMP pixel decoding → `image::BmpDecoder` | Prototype before adopting ([#183](https://github.com/sagikazarmark/excalidraw-render.orig/issues/183)) | Meaningful potential reduction, but several confirmed semantic mismatches. |
| 6 | Generic curve extrema → `kurbo` | Narrow, conditional candidate ([#184](https://github.com/sagikazarmark/excalidraw-render.orig/issues/184)) | Small reduction; explicitly exclude reference-exact bounds and sampled label placement. |
| 7 | Handwritten PDF object serialization → more `pdf-writer` | Strategic investigation ([#186](https://github.com/sagikazarmark/excalidraw-render.orig/issues/186)) | Larger potential; fallible buffers, precision, file ID, and chunk adaptation block a simple swap. |
| 8 | CSS color conversion math → `palette` or Linebender `color` | Lower-priority experiment ([#185](https://github.com/sagikazarmark/excalidraw-render.orig/issues/185)) | Significant numerical policy remains; no drop-in equivalence established. |

### What the renderer already gets right

The implementation is already a composition of substantial libraries:

| Layer | Existing delegation | Code evidence |
| --- | --- | --- |
| PNG path rasterization, dashes, stroke expansion, masks | tiny-skia | `render/png/paint.rs:258–352`, `PaintMasks` |
| Nested SVG parsing/rendering | roxmltree, svgtypes, usvg, resvg | `render/asset/svg.rs`, `svg_coordinates.rs`, `render/png.rs:1184` |
| Text shaping and bidi | rustybuzz, unicode-bidi | `render/text.rs:818`, `shape` at `:1748` |
| Font subsetting and containers | skera, read-fonts/write-fonts, wuff, ttf2woff2 | `render/font_subset.rs:174–270` |
| PNG/JPEG/GIF/WebP input | png, zune-jpeg, gif, image-webp | `render/asset/{png,jpeg,gif,webp}.rs` |
| Raster color management | moxcms | `render/asset/color.rs:43–73`, `:186–292` |
| Nested SVG → PDF | svg2pdf and pdf-writer chunks | `render/pdf/nested_svg.rs:569–673` |
| Deflate and portable transcendental math | miniz_oxide, libm | `render/png/encoder.rs`, `render/pdf/streams.rs`, `portable_math.rs` |

Paths in this review beginning with `render/` are under
`crates/excalidraw-render/src/`. Large orchestration/admission files should not
be counted as custom implementations of the algorithms they call.

## Code-specific findings

### A. Replace the PNG CRC implementation with `crc32fast`

**Confidence: high. Benefit: small maintenance reduction and removal of a
bit-at-a-time checksum loop. Effort: very small.**

[`render/png/encoder.rs:407–427`](../../crates/excalidraw-render/src/render/png/encoder.rs#L407)
assembles PNG chunks and implements IEEE CRC-32 with eight bit steps per byte.
The crate already directly depends on `crc32fast`, and
[`render/asset/png.rs:148`](../../../crates/excalidraw-render/src/render/asset/png.rs#L148)
uses its `Hasher` for input chunks. Use `crc32fast::hash` for the encoder's
same type-and-payload byte slice. [A1]

The existing library's accelerated and baseline implementations compute the
same integer checksum; this is a different question from target-dependent
floating-point rendering. No PNG filter, chunk order, compression, metadata,
or resource policy needs to change. Require complete artifact byte equality;
there is no reason to accept new output pins for this substitution. No measured
end-to-end speedup is claimed.

### B. Delegate ECMAScript number spelling to `ryu-js`

**Confidence: high that this is the right boundary; parity is unqualified.
Benefit: specialized correctness. Effort: small.**

[`render/number_format.rs:99–119`](../../../crates/excalidraw-render/src/render/number_format.rs#L99)
implements JavaScript number spelling using Rust formatting plus exponent
threshold/sign adjustments. The same module implements `toFixed(2)` rounding
by decomposing IEEE bits into integer arithmetic (`:25–93`). These are language
runtime mechanisms, not Excalidraw-specific rendering algorithms.

`ryu-js 1.0.3` explicitly implements ECMAScript number-to-string and `toFixed`,
with safe `Buffer::format_finite` and `Buffer::format_to_fixed` entry points.
It is maintained in the Boa JavaScript-engine organization and derives from
Ryū. This is a better specialization match than generic `ryu` or ordinary Rust
`Display`; the Ryū correctness paper does not by itself qualify this adapter. [A2]

Recommended sequence:

1. Replace `ecmascript_number`, used by `freedraw.rs:531` and
   `svg/write.rs:1292`, with a pinned `ryu-js` call.
2. Evaluate replacing the bit-level fixed-decimal formatter with
   `format_to_fixed(value, 2)`, retaining rejection of non-finite/out-of-range
   values, trailing-zero removal, and suppression of rounded negative zero.
3. Keep SVG/PDF's separate number policies explicit: PDF cannot contain
   exponent notation, so `ryu-js` is not a blanket replacement for
   `format_pdf_number`.

Compare exact tokens with the pinned JavaScript oracle across adjacent floats
at `1e-6`, `1e21`, half-cent ties, subnormals, negative zero, and the admitted
coordinate range. Existing formatting tests cover only a few such cases. This
review establishes a maintainability opportunity, not a reproduced formatting
bug in the current implementation.

### C. Use `cssparser` for the remaining handwritten CSS syntax

**Confidence: high for tokenization; medium for net code reduction.
Benefit: fewer custom grammar edge cases. Effort: medium.**

There are concrete sites, beyond the already-delegated ordinary color parser:

- [`color.rs:372–523`](../../../crates/excalidraw-render/src/color.rs#L372):
  `color-mix()` handling manually finds parentheses, splits commas by a depth
  counter, and finds a trailing percentage through string scanning.
- [`render/asset/svg.rs:353–395`](../../../crates/excalidraw-render/src/render/asset/svg.rs#L353):
  stylesheet admission splits on `{`, `}`, `;`, and `:`.
- [`render/asset/svg/css.rs:52`](../../../crates/excalidraw-render/src/render/asset/svg/css.rs#L52):
  PDF stylesheet lowering splits blocks on `}` before handing them to simplecss.

The directly pinned `cssparser` has `parse_nested_block`, delimiter-aware
`parse_until_before`, `expect_function_matching`, source positions, and slices.
These are appropriate mechanisms for recognizing nested functions, quoted
strings, comments, and delimiters. Use bounded iteration rather than replacing
a fixed-size three-part parse with an unbounded accumulating helper. [A3]

Two important adapter details:

- **Preserve the supported grammar.** The current stylesheet gate deliberately
  rejects at-rules, escapes, comments, and other constructs. A better tokenizer
  must not silently admit new syntax, remove a diagnostic, or disagree with
  usvg's simplecss-based interpretation. Supporting more CSS is a separate
  compatibility decision. Keep simplecss matching for now; a Servo `selectors`
  migration would need coordinated downstream support.
- **Preserve precision.** `cssparser::expect_number` and `expect_percentage`
  return `f32`; the current color-mix weight parse uses `f64`. Use token source
  slices where necessary instead of accidentally introducing a narrowing step.

This is a stronger actual opportunity than suggesting more `svgtypes`: the
SVG coordinate-admission path already uses its pull parsers and intentionally
checks source doubles before the viewer can narrow or discard them.

### D. Use Fontations for the remaining name-table mechanics

**Confidence: medium-high. Benefit: remove manual binary offset handling.
Effort: small-to-medium plus font qualification.**

[`render/font_subset.rs:367–490`](../../../crates/excalidraw-render/src/render/font_subset.rs#L367)
builds name-table records, lengths, offsets, and UTF-16 payloads by hand;
`english_name` manually parses the same binary format. Most surrounding font
work already uses Fontations and skera.

Prefer typed readers first. Then evaluate `write_fonts::tables::name::{Name,
NameRecord}` with the existing `FontBuilder` for serialization. Keep the
project-owned list of name IDs, language selection, deterministic aliases,
and preserved attribution/license records. The typed writer takes strings,
whereas this code preserves selected source records as raw bytes: malformed
UTF-16 or an encoding mismatch must not be silently normalized. String pooling
and offsets may also produce different sfnt/WOFF2 bytes. [A4], [F1], [F2]

The upstream writer still has length/encoding assumptions, so the source-profile
bounds remain important. Keep the independent layout/checksum/metric validation;
using the same writer is not evidence that output satisfies the project profile.
Do not remove the older traversal-capable `read-fonts` dependency just to unify
versions; its reason is documented explicitly in `Cargo.toml`.

### E. Prototype `image` for BMP decoding, retaining format policy

**Confidence: medium that this can pay off; low that it is a drop-in swap.
Benefit: potentially the largest self-contained codec reduction. Effort: medium.**

[`render/asset/bmp.rs`](../../../crates/excalidraw-render/src/render/asset/bmp.rs)
is a complete custom 487-line validator/decoder: packed palettes, row layout,
bitfields, alpha, and RLE4/RLE8. The 439-line
[`ico.rs`](../../../crates/excalidraw-render/src/render/asset/ico.rs)
contains related DIB pixel logic. Those file sizes describe the opportunity
surface, not promised deletions.

`image 0.25.10` is already transitive via svg2pdf. A direct, narrowly featured
dependency can expose `BmpDecoder`, using a caller-admitted buffer through
`ImageDecoder::read_image`. This is worth a bounded prototype before maintaining
more codec machinery. [I1], [I3]

The implementation audit makes the caveats specific:

- Current BMP RLE initializes skipped pixels with **palette entry zero**
  (`bmp.rs:48–52`); image's zero-filled skipped regions can differ.
- Current `scale_mask` rounds a channel over its full integer range
  (`bmp.rs:427–434`); image truncates channels wider than eight bits.
- Current admission accepts compression code 6; image's inspected decoder
  does not. Current mask validation rejects overlapping masks, unlike the
  inspected upstream per-mask check.
- Current ICO choice is **area, then depth, earliest tie** (`ico.rs:41–56`);
  image's facade chooses depth before area and has different tie behavior.

Start by comparing a supported BMP subset and malformed/recovery cases. Adopt
only if the residual preflight/adapter is substantially simpler than the
current decoder. Retaining the full custom RLE validator plus a second decoder
may not earn its complexity. If mismatches dominate, an upstream improvement
is preferable to a large private fork. Do not switch all existing codecs behind
`image::load_from_memory`: current adapters expose format-specific admission,
diagnostics, color management, and compressed-source preservation.

A smaller companion opportunity is
[`asset/color.rs:94–137`](../../../crates/excalidraw-render/src/render/asset/color.rs#L94):
`image::metadata::Orientation::from_exif_chunk` already exposes orientation
extraction from an independently obtained EXIF block. Compare malformed tags,
endianness, duplicates, and first-match behavior before replacing the manual
reader. A full EXIF package is unnecessary for that one field. [A5]

### F. Limit `kurbo` adoption to genuinely generic geometry

**Confidence: medium. Benefit: modest. Effort: small code change, potentially
substantial numeric qualification.**

[`render/geometry.rs:40–164`](../../../crates/excalidraw-render/src/render/geometry.rs#L40)
implements generic cubic extrema/evaluation. `kurbo::CubicBez` and
`ParamCurveExtrema::bounding_box` are plausible replacements. The existing
`Path::cubic_bounds` treats only cubic segments as bounds contributors; passing
the entire path to a generic path bounding box would change behavior. [K1]

**Do not merge all the seemingly duplicate curve math.** The same file's
`solve_quadratic` and `cubic_bezier_curve_bound` (`:478–539`) explicitly preserve
the reference's coefficient spelling, exact-zero test, evaluation order, and
once-rounded cubes, with issue #173 explaining the one-ulp sensitivity.
`reference_curve_bounds` also intentionally ignores line operations.

Similarly, [`bound_text.rs:123–188`](../../../crates/excalidraw-render/src/render/bound_text.rs#L123)
places rounded-arrow labels using a fixed 21-sample length approximation.
Replacing it with kurbo's more accurate adaptive arc length changes the placement
algorithm. Keep that reference path. The generic bounds candidate is much
narrower than a whole-geometry migration, and `std` feature unification means
adding kurbo's `libm` feature alone does not settle portable math.

### G. PDF is the largest architectural opportunity, but needs API work

**Confidence: high that there is duplicated PDF machinery; medium-low that
current library APIs reduce it cleanly. Benefit: potentially high. Effort: high.**

[`render/pdf.rs:520–785`](../../../crates/excalidraw-render/src/render/pdf.rs#L520)
shows manual object layout, buffer sizing, dictionaries, references, and offsets;
the rest of `encode` writes fonts/images, xref, and the trailer. `pdf-writer`
is currently used for nested SVG chunks, so its presence in the manifest does
not mean the scene PDF serializer is already library-owned.

Prefer evaluating **more use of the existing `pdf-writer`**, especially object
dictionaries, strings, streams, and object graph assembly, before adding a
second PDF stack. Its `Primitive` trait is public and implementable by a local
wrapper, so although built-in real numbers and typed geometry are `f32`,
preformatted validated `f64` tokens can conceptually be retained via lower-level
dictionary/array APIs and the existing content streams. [A6]

This is not a ready-made migration:

- `Pdf`/`Chunk` own infallibly growing `Vec`s; capacity hints do not replace the
  current fallible-buffer error contract.
- `Pdf::finish` writes xref and trailer together, but ADR-0008 derives the file
  ID from all bytes **through xref and before the trailer**. `set_file_id` alone
  is not the necessary hook.
- `pdf-writer` does not allocate or validate reference IDs for the caller, and
  `finish` panics on duplicate IDs. Semantic object planning remains necessary.
- [`pdf/nested_svg.rs:690–898`](../../../crates/excalidraw-render/src/render/pdf/nested_svg.rs#L690)
  rewrites serialized chunks, dictionaries, and image wrappers. A higher-level
  writer alone does not remove that precision/compression adaptation.

**Best strategic direction:** investigate upstream fallible-output/finalization
hooks and structured chunk/stream access, plus the usvg serializer limitations
identified in ADR-0009. These could remove more long-term fragile code than
swapping rasterizers. Availability of such new APIs is unverified; this review
does not claim an existing hook solves those gaps.

Krilla is a credible higher-level alternative with substantial upstream viewer
testing, but its inspected `f32` geometry and automatic font subsetting conflict
with the current full-precision/full-reviewed-program PDF contract. A wholesale
switch requires an explicit spec/ADR-0008 decision or upstream extension.
Whole-scene SVG → svg2pdf also conflicts with spec §8.6. Keep svg2pdf's
already-useful nested-SVG role.

### H. Color math is a worthwhile later candidate, not a blind parser swap

**Confidence: medium. Benefit: medium potential. Effort: medium-to-high.**

[`color.rs`](../../../crates/excalidraw-render/src/color.rs) combines three existing
CSS-related dependencies with hand-coded RGB/XYZ/Lab conversions, adaptation
matrices, transfer functions, hue interpolation, and gamut handling. This is
generic color science that could benefit from maintained library implementations.

`palette 0.7.6` supplies typed RGB/XYZ/Lab/Oklab spaces, white-point adaptation,
conversion traits, and `f64` components. Linebender `color 0.3.3` more directly
targets CSS Color 4 conversions/interpolation and dynamic color spaces, but
always uses `f32` components. Neither should be assumed to reproduce this
renderer's complete `color-mix`, clipping, missing-component, or quantization
policy. [A7], [A8]

Prototype only the conversion kernel first. Keep `CanonicalColor`, CSS admission,
system-color policy, and reference theme filters around it. In particular,
`hue-rotate(180deg)` here is the CSS filter matrix, not a perceptual hue rotation
in Lab, and conversion to linear light is not appropriate for every reference
filter operation. Validate gamut edges and half-byte rounding, as well as
native/WASM equality. Raster ICC handling already correctly delegates to
`moxcms`; no replacement is proposed there.

## Retain these major components

- **tiny-skia raster backend:** `paint.rs:316–333` already delegates dashing and
  stroking explicitly and maps failures. The remaining supersampling, path
  lowering, theme application, and mask reuse have concrete project purposes.
  `png/coverage.rs` is a semantic resource estimator, not an unimplemented
  rasterizer that a coverage-mask library can replace.
- **Rough.js and Perfect Freehand compatibility ports:** generic similarity
  does not preserve PRNG consumption, point vectors, or serialized paths.
  roughr has a confirmed different randomizer; the inspected freehand port
  lacks exact version/portable-math qualification.
- **rustybuzz + unicode-bidi text composition:** shaping/bidi are already
  outsourced. Remaining chunk/CJK/emoji fallback, family metrics, source-text
  carriage, and probe accounting are project semantics. cosmic-text would
  replace many of those policies rather than simply remove boilerplate.
- **SVG sanitization/resource admission:** usvg's supported static rendering
  is not a replacement for source-attributed removals and recursive payload
  accounting. Preserve the explicit deny-all resource resolver.
- **Owned PNG assembly around miniz_oxide:** ADR-0004 records a measured reason
  for the custom row-filter heuristic and reusable IDAT stream. A general PNG
  encoder would need to preserve those useful controls to earn a migration.
- **The conformance comparator:** the pinned Pixelmatch and SSIM arithmetic are
  the measurement protocol. An arbitrary `imageproc`/SSIM implementation is not
  interchangeable; see the existing
  [comparator research](../conformance-comparator-primary-sources.md) and ADR-0005.

## Recommended execution order

1. **Small exact substitutions:** CRC, then the ECMAScript formatting adapter.
   Their limited scope gives clear evidence of benefit without a pipeline rewrite.
2. **Existing-dependency consolidation:** token-aware CSS parsing and typed
   font-name reading/writing, each as an independent change.
3. **Bounded codec experiment:** compare BMP output and failure behavior on the
   actual codec obligations; measure how much owned decoding remains afterward.
4. **Selective numerical reuse:** generic kurbo bounds, then color conversion
   only if their adapters produce a real reduction.
5. **Strategic PDF work:** pursue the concrete upstream interface gaps before
   committing to a new backend.

For adoption, use the existing fast/render tests, registered format-specific
parity corpus, package-consumer native/browser-WASM artifact checks, relevant
font/viewer qualification, and resource trace/batch tests. Compare diagnostics
and limit outcomes as well as pixels. Byte changes affect exact output-size
charges even if the semantic work algorithm is unchanged; classify and qualify
such changes under the existing revision/evidence rules. The acceptance measure
is **less custom mechanism and fewer dependency-specific assumptions**, not just
a larger dependency list or shorter call sites.

## 1. Ecosystem evidence conventions

The sections below contain the primary-source investigation supporting the
recommendations and exclusions above. API possibilities described there are
not additional blanket migration recommendations: the implementation audit
above narrows them, especially for tiny-skia, svgtypes, and kurbo.

**Labels:** **Confirmed** means directly supported by the cited upstream API,
source, manifest, or repository contract. **Assessment** is a recommendation
derived from that evidence. **Unverified** means additional code audit or
qualification is needed. Upstream test counts and portability statements are
attributed claims, not independently reproduced results. “Pure Rust” does not
mean “no unsafe in the entire dependency graph,” “no allocations,” or
“cross-target byte-identical.”

Versioned sources are used where possible. For current development sources,
the inspected snapshots are: krilla `1141e3145919da434389ebab431a14b783fd1731`,
Vello `0268b37f5c54fc3970707b1c525d555c568d67dd`, cosmic-text
`daae9c75d52322f8fb3af6168d76561540914e1f`, rough-rs
`b17a417d13a6ce339896639a6a931440d2a23017`, and perfect_freehand
`e76196d9e8fddd5b31c1d9da7ba37a9041af7fdc`. A development manifest version is
not evidence that all inspected behavior is available in a published release.

## 2. Project constraints that decide suitability

Read for this inventory: [README](../../../README.md), [workspace manifest](../../../Cargo.toml),
[renderer manifest](../../../crates/excalidraw-render/Cargo.toml),
[CONTEXT](../../../CONTEXT.md), [domain guidance](../../agents/domain.md),
ADRs 0001–0004 and 0006–0009, and pertinent sections of the
[renderer specification](../../renderer-spec-v1.md).

| Constraint | Consequence for recommendations |
| --- | --- |
| Spec §§3, 10: Excalidraw `v0.18.1`, Rough.js `4.6.4`, Perfect Freehand `1.2.0` | A crate implementing the same general algorithm is not automatically the pinned reference algorithm. Spec §10 explicitly excludes `roughr` as compatibility authority. |
| Spec §§8.2, 10: pure Rust, offline, no production JavaScript/OS services/host fonts; deny-all external resource access | In-memory APIs and explicitly controlled feature/resolver configurations are necessary. A library's filesystem convenience examples are not suitable production configuration. |
| [ADR-0006](../../adr/0006-narrow-supported-targets-to-linux-and-browser-wasm.md), [ADR-0007](../../adr/0007-route-transcendental-math-through-libm.md) | Linux x86-64 and browser-hosted `wasm32-unknown-unknown` must produce identical artifact bytes. `std` math, SIMD, feature unification, numeric reduction, and encoding all need scrutiny. WASM compilation alone proves little. |
| CONTEXT: perceptual parity versus artifact byte stability | Reference visual parity is tolerance-based; native-versus-WASM artifact equality is exact. These are separate qualification questions. |
| CONTEXT and spec §13: semantic resource accounting; [ADR-0002](../../adr/0002-scope-the-determinism-promise-to-artifact-bytes.md) | Preserve canonical events, charge order, first-crossing outcome, per-target ledgers, and cache/batch replay. Library allocations or operation counts cannot replace them. Physical boundedness/panic freedom is a separate obligation. |
| Spec §8.2: direct SVG preserves validated compressed raster octets | A decoder replacement must not turn direct SVG into a decode/re-encode path. Validation and pixel derivation are distinct responsibilities. |
| Spec §8.6: direct semantic vector-first PDF, full `f64` except named reductions, selectable text, complete reviewed font programs | Reject whole-scene serialized-SVG pivots and backends that necessarily narrow geometry or subset those programs. |
| [ADR-0009](../../adr/0009-share-pdf-raster-closure-planning-and-encoding.md) | One occurrence-specific PDF raster closure plan owns input/output grids, admission, diagnostics, rasterization, and embedding. A converter's private raster scale is not an alternative authority. |
| [ADR-0004](../../adr/0004-compress-png-idat-with-miniz-oxide.md), [ADR-0008](../../adr/0008-deflate-pdf-streams-tag-chunk-programs-and-derive-the-file-identifier.md) | PNG filtering/assembly and PDF streams, identifiers, and font naming have explicit byte policies. A generic encoder is not automatically a simplification. |
| [ADR-0001](../../adr/0001-treat-font-subsets-as-modified-versions.md), [ADR-0003](../../adr/0003-retire-the-brotli-fork-for-upstream-portable-float.md) | SVG subset names, legal records, and deterministic WOFF2 encoding remain profile-controlled. ADR-0003's final amendment records the coordinated refresh completed on 2026-09-11; earlier pending language is historical. |

**Confirmed dependency context.** The manifests already directly select
`svgtypes 0.15.3`, `usvg/resvg 0.45.1`, `tiny-skia 0.11.4`, `simplecss 0.2.2`,
`cssparser 0.37.0`, `svg2pdf 0.13.0`, `pdf-writer 0.12.0`, `rustybuzz 0.20.1`,
`unicode-bidi 0.3.18`, `read-fonts 0.42.1`, `write-fonts 0.53.0`, and `skera 0.7.0`.
The [lockfile](../../../Cargo.lock) also contains `image 0.25.10` and kurbo
`0.11.3` / `0.13.1`. Read-only dependency queries confirmed that the renderer
already reaches `image 0.25.10` through svg2pdf, with GIF/JPEG/PNG features, and
kurbo `0.11.3` through svgtypes/usvg with `std` enabled:

```text
cargo tree --locked --offline -p excalidraw-render -i image -e features
cargo tree --locked --offline -p excalidraw-render -i kurbo@0.11.3 -e features
```

Thus promoting a dependency may add only an edge, but activating BMP/ICO still
adds decoder code, and a feature request does not undo features enabled elsewhere.
This inventory reviews kurbo `0.11.3` APIs, not the newer `0.13.1` algorithms.

## 3. Image decoding: strongest candidate is BMP, not the ICO facade

### Confirmed APIs and format coverage

`image 0.25.10` exposes `BmpDecoder::new(reader)` and
`BmpDecoder::new_without_file_header(reader)` for `BufRead + Seek` inputs.
The latter accepts a DIB without a BMP file header. `ImageDecoder` provides
`dimensions()`, `color_type()`, `total_bytes()`, `set_limits(...)`, and
`read_image(self, &mut [u8])`; the output buffer must have exactly the reported
length. These allow an in-memory cursor and caller-owned, admitted pixel buffer.
[I1], [I3]

The BMP source implements CORE/INFO/V2/V3/V4/V5 headers; indexed 1/2/4/8-bit
INFO-family pixels; 16/24/32-bit uncompressed pixels; RLE4 and RLE8; and 16/32-bit
`BI_BITFIELDS`. CORE-header support is narrower: 1/4/8/24-bit. RLE supports
encoded runs, absolute runs, end-of-line/end-of-bitmap, delta moves, and padding.
This is a materially useful codec implementation, not just BMP identification.
[I1]

**Important confirmed semantic details:**

- Bitfields must be contiguous and fit the source word. RGB masks must be
  nonzero. Channels wider than eight bits are truncated to their high eight bits;
  smaller widths use integer expansion tables/operations. The inspected
  `Bitfields::from_mask` validates each mask but does not reject overlap between
  different channels. Do not describe it as a complete bitfield validator. [I1]
- V3/V4/V5 alpha masks are read for bitfield images. Ordinary 32-bit `BI_RGB` BMP
  uses RGB output, while the internal ICO mode treats the fourth byte as alpha.
  These are distinct decoding policies. [I1]
- RLE8 encoded runs extending past a row are deliberately clamped; RLE4 and
  absolute-run overruns can fail. Delta/EOL/EOF skipped areas are zero-filled.
  A successful decode is not proof of strict conformance or browser-equivalent
  recovery. [I1]
- JPEG-in-BMP, PNG-in-BMP, and CMYK compression variants are unsupported.
  Compression code 6 (`BI_ALPHABITFIELDS`) falls through to the unknown-type
  branch; V3+ alpha masks under code 3 are a different case. [I1]
- The BMP implementation does not override the default `icc_profile()` result
  and skips unhandled extended-header content. Supporting V5 header structure
  therefore does **not** establish V5 embedded-profile/color interpretation. [I1], [I3]

### Limits: useful interface, incomplete enforcement boundary

`Limits` documents strict width/height limits and a **non-strict**, potentially
ignored `max_alloc` (default 512 MiB). Both BMP and ICO use the default
`ImageDecoder::set_limits`, which checks dimensions but does not meter decoder
allocations or forward an ICO limit into its inner decoder. Their constructors
already parse metadata; ICO constructs its PNG/BMP decoder before the caller
can set these limits. [I1], [I2], [I3], [I4]

BMP has a nominal `MAX_WIDTH_HEIGHT = 65535` check, but in the inspected INFO
header code it checks signed height **before** converting a negative top-down
height to positive. This constant is not a sufficient absolute-height guard.
There are checked buffer-size calculations and bounded palette storage, but also
ordinary allocating paths. The public read API asserts the exact buffer length.
Retain project pre-admission, checked arithmetic, format validation, and typed
failure mapping; do not substitute `Limits::default()` for those obligations.
[I1], [I3]

### ICO: concrete selection mismatch and restricted access

`IcoDecoder::new` selects one entry internally. `best_entry` compares
`(bits_per_pixel, width * height)`, **depth first**. Spec §8.2 requires
**largest area, then depth**. The selected entry and selection routine are
private; there is no public “select this directory entry” parameter in this
version. This is a confirmed reason not to replace the ICO policy with the
facade. Tie behavior also deserves explicit characterization: selection starts
from the last entry and only replaces it on a strictly larger score. [I2]

ICO supports embedded PNG or BMP/DIB; the PNG result must be `Rgba8`. The BMP
path adjusts doubled height, applies the bottom-up AND mask when present, and
accepts an absent mask when entry data ends exactly at the pixel data. Its
dimension comparison clamps actual dimensions to 256 when comparing directory
fields, so directory bytes alone do not establish decoded-pixel limits. [I2]

**Assessment:** retain ICO directory selection/validation and investigate
delegating only the selected payload. For PNG payloads, the already-pinned PNG
adapter is a natural boundary. DIB delegation needs care: public
`new_without_file_header` is not equivalent to the private ICO constructor,
particularly for doubled height and alpha. Do not rely on the doc-hidden
`read_metadata_in_ico_format` as a qualified stable integration seam without
further investigation. [I1], [I2]

### Recommendation and maturity evidence

Start with admitted BMP pixel derivation. Compare 555/565/alpha masks, >8-bit
channel truncation, top-down and truncated files, RLE deltas/overruns, palette
holes, and profile-bearing V4/V5 samples against the codec profile obligations.
Measure constructor and decode boundedness independently of semantic charges.
**Unverified:** exact supported/rejected set and canonical pixels versus the
pinned browser, and whether an adapter still eliminates enough custom work.

Use a narrow feature selection (`default-features = false`, `bmp`, optionally
`ico`) if adopted. The manifest makes `ico` imply `bmp` and `png`; defaults enable
Rayon and many formats, while `avif-native` separately enables a native dav1d
dependency. BMP/ICO need none of that native codec path. The current closure
already contains image's `png 0.18` and `moxcms 0.8` lines alongside the project's
other versions; do not claim zero dependency cost for every version choice. [I5]

The upstream changelog records a long release history and concrete BMP/ICO
overflow/crash fixes; decoder sources contain targeted regression tests. That is
maintenance evidence, not a panic-free guarantee. Its changelog also retains a
broad known-issues warning about malicious-input panics. No format-specific
fuzz-coverage percentage or browser-parity claim was established here. [I1], [I2], [I6]

## 4. Kurbo: analytic bounds first, arc-length replacement second

**Confirmed.** `CubicBez`, `QuadBez`, and `PathSeg` implement
`ParamCurveExtrema::extrema()` and `bounding_box()`. Extrema returns a fixed-capacity
`ArrayVec` with at most four interior parameters, sufficient for cubic Béziers;
the bounding box includes curve endpoints and extrema. Cubic extrema solve the
quadratic derivative separately in x and y. This is the right kind of primitive
for eliminating generic hand-written Bézier-extrema code. [K1], [K3]

`ParamCurveArclen::arclen(accuracy)` and `inv_arclen(length, accuracy)` explicitly
expose tolerance. The inverse uses ITP root finding. Cubic length uses adaptive
subdivision and Legendre–Gauss quadrature, with a depth-20 termination in the
inspected implementation. The documentation's accuracy statement is subject to
roundoff, and compute time depends on accuracy. [K2], [K3]

**Assessment:** bounds are a stronger first candidate than length. Preserve
which rough outline supplies the extent, rotation center, stroke expansion,
degenerate behavior, and local/device coordinate choice. A mathematical curve
bound is not automatically the Excalidraw element bound. Similarly, a more
accurate curve length can change dash placement, bound text, or a reference
algorithm based on a particular sampled polyline. The project owns that choice.

**Portable-math caveat, confirmed.** Kurbo supports `default-features = false`
with `libm`, but `std` takes precedence when both are enabled. The renderer's
existing `0.11.3` dependency has `std` enabled transitively, so a direct `libm`
feature request would **not** force portable math. Cubic length calls `hypot`
and `powi`; quadratic length also calls `powf`. The feature issue is real even
though the crate is pure Rust. [K3], [K4], [K5], [K6] ADR-0007 already treats dependency
math as an empirical two-target qualification surface; it does not prove every
new kurbo call safe.

**Unverified:** cross-target byte equality of newly used operations, exact
boundary behavior for degenerate/large-coordinate curves, and equivalence of
length algorithms. Bound work using admitted inputs and fixed tolerances;
adaptive subdivisions are physical operations, not new semantic ledger events.

**Maturity evidence:** a released Linebender geometry crate with documented
numerical APIs and source-level numerical tests, already used in this renderer's
SVG dependency closure. This supports reuse consideration; it is not evidence
that all numerical methods are equally robust. For example, kurbo's own
`solve_cubic` documentation says it is not yet fully robust; the bounds method
above uses derivative **quadratics**, a useful distinction. [K3], [K5]

## 5. SVG stack: delegate syntax and normalized rendering, retain sanitization

**Confirmed.** `svgtypes 0.15.3` parses SVG values including path data,
transforms, lengths, points, paint, and IRIs. Its pull parsers include
`PathParser`, `TransformListParser`, `NumberListParser`, and `PointsParser`;
path parsing expands implicit commands. It expects XML-normalized values and
does not resolve XML entities itself. Keyword case rules and unsupported color
forms are documented limitations. [S1], [S2]

**Assessment:** prefer these typed parsers to bespoke token splitting for
supported attributes. Do not confuse them with XML admission, CSS cascade, URL
authorization, or a resource graph. For geometry accounting, determine whether
the contract counts source commands or normalized segments before replacing a
token/segment census.

`usvg::Tree::from_data/from_str` resolves supported CSS, inheritance, references,
`use`, markers, basic shapes, and path normalization into a typed tree.
Unsupported features are ignored and malformed elements can be removed.
`resvg` rasterizes that normalized tree using tiny-skia. These are mechanisms
the project already owns as dependencies, not new proposals. [S3], [S4], [S5]

**Confirmed security/diagnostic boundary:** usvg's default
`ImageHrefResolver::resolve_string` reads filesystem paths.
`resources_dir: None` does not disable this. The upstream resolver docs explicitly
say no network requests are made, but permit custom URL resolution. Set explicit
deny-all/custom admitted-resource resolvers, fixed font data, and fixed options.
Further, `resolve_data` receives **already decoded** data-URL bytes; it is too
late to use that callback as the sole pre-allocation admission check. [S4], [S6]

Ignoring scripts or unsupported content during rendering is not equivalent to
the project's sanitization contract: the project needs source-attributed
diagnostics, admitted data occurrences, deterministic rejection/recovery,
namespace policy, and bounded recursive processing. Preserve that boundary
ahead of usvg, as spec §§8.2/10 require. Usvg's default nested-SVG image loader
also installs resolvers that discard further image nodes; blindly delegating
recursive image semantics could therefore lose content. [S3], [S6]

**Maturity evidence:** resvg's tagged README reports approximately 1,600
SVG-to-PNG regression tests, a public feature/support suite, pure-Rust
portability including WASM, and cross-platform pixel reproducibility. These
are useful first-party claims. They do not establish Excalidraw/browser text
layout equivalence or this release's full artifact bytes. CONTEXT already
records usvg bidi/paragraph limitations in the reference harness. [S5]

**Recommendation:** consolidate syntax/normalization mechanisms where possible;
do not recommend deleting a sanitizer because resvg renders only static SVG.
ADR-0009 also records why replacing the normalized tree through serialized SVG
has precision and anonymous-`feImage` constraints. Its serializer adapter is a
known qualification boundary, not presumptively unnecessary glue.

## 6. Tiny-skia: strong reuse candidate for stroke and raster mechanisms

**Confirmed APIs in 0.11.4:** `Pixmap::stroke_path`, `Paint::anti_alias`,
`Stroke { width, miter_limit, line_cap, line_join, dash }`,
`StrokeDash::new(Vec<f32>, offset)`, `Path::dash`, `Path::stroke`,
`PathStroker`, and `Mask::fill_path` / `Mask::intersect_path` / `Mask::data`.
The stroke example demonstrates round caps and dashes. The stroker can retain
temporary allocations across calls. [T1], [T2], [T3], [T4]

These are plausible replacements for generic dash splitting, stroke expansion,
or mask painting, **if** the implementation audit finds equivalents:

- `StrokeDash::new` requires an even-length list with at least two entries,
  nonnegative finite values, positive finite sum, and finite offset. An SVG
  odd-length dash list must first receive the appropriate SVG normalization;
  raw lists cannot simply be passed through. [T2]
- `Path::dash` can return `None` for excessive dash production (documented
  threshold 1,000,000) or invalid output bounds. The source omits some Skia
  specialized degenerate-path branches. [T2]
- `PixmapMut::stroke_path` returns `()`. A dash failure logs and returns without
  painting. That is incompatible with silently trusting the call where the
  project requires typed diagnostic/failure behavior. Explicit preparation or
  proven admission bounds are needed. [T5]
- The painter has distinct wide-stroke outline/fill and thin/hairline coverage
  paths. The inspected code deliberately preserves an older coverage adjustment
  in one branch to avoid changing results. A manual expanded fill is not
  necessarily interchangeable with native hairline stroking. [T5]
- Geometry is `f32` and raster storage is RGBA8888; text layout, ICC profiles,
  PDF generation, and advanced path operations are outside the tagged README's
  scope. Keep full-precision semantic geometry until the raster boundary. [T1], [T3], [T6]

**Coverage distinction:** masks offer access to rasterized coverage bytes, but
those bytes are not the project's semantic pixel-coverage definition. Spec §8.5
even distinguishes PNG's snapped text baseline from the exact outlines used
for the semantic coverage charge. Do not let antialiasing thresholds, opacity,
supersampling, or a backend change alter the ledger. Whether a custom coverage
routine can delegate its geometric core is **unverified**.

**Maturity evidence:** a released Skia-algorithm Rust port used by resvg, with
dedicated stroke/dash/hairline regression tests. Its README calls it a research
project and explicitly says it is not a complete Skia replacement. Its stated
Skia-result goal is not proof of matching the pinned Chromium revision. [T6], [T7]
The direct dependency disables default features, but that does not establish
the resolved feature set. Existing [#178](https://github.com/sagikazarmark/excalidraw-render.orig/issues/178),
read while filing the follow-up issues, records transitive SIMD unification and
four reproducible native/WASM PNG differences caused by host-dependent cubic-root
math in tiny-skia-path. Its controlled probe identifies the math calls, not SIMD,
as the cause. Audit the complete feature graph and distinguish those existing
failures from a proposed replacement's effects; ADR-0007's historical disabled-SIMD
assumption is not current qualification evidence. [T8]

## 7. Other pure-Rust rasterizers: plausible, lower priority

### Vello CPU

**Confirmed at the recorded development snapshot:** `RenderContext` exposes
paint selection, `fill_path`, `stroke_path`, and `glyph_run`, followed by
`render` into a `Pixmap` with `Resources`. The README describes a Rust CPU
renderer, SIMD support including WASM, and `u8_pipeline` / `f32_pipeline`
quality-speed choices. `std` overrides `libm`; `png` and `multithreading` imply
`std`. The manifest currently declares `0.2.0`. [V1], [V2]

The same README states that complex filter graphs are unsupported and panic,
simple filters are unsupported in multithreaded mode, and some APIs such as
glyph caching remain experimental. It links benchmarks and an implementation
thesis. This is more substantive evidence than a minimal prototype, but not a
cross-target byte-equality guarantee. [V1]

**Assessment:** consider only if a concrete tiny-skia coverage/performance defect
cannot be resolved locally. Restrict an experiment to already-resolved scene
paths with filters handled elsewhere. Different coverage, SIMD dispatch,
compositing precision, math feature unification, and resource handling could
increase the compatibility surface substantially. The documented speed/quality
mode names are not determinism modes. **Unverified:** a published-version
configuration that meets this project's exact equality and admitted-surface
panic-freedom requirements.

### Raqote

Its official README documents `DrawTarget::fill/stroke`, `StrokeStyle` dashes,
gradients, clips, blending, and layers, and lists Servo canvas among its users.
Treat that as an upstream usage claim, not independently verified current
deployment. The `0.8.5` development manifest defaults to `text` and `png`;
`text` pulls `font-kit`. A pure-Rust-only evaluation should disable the host-font
path and supply resolved outlines. [V3], [V4]

**Assessment:** an established alternative worth knowing, but no inspected
evidence shows a correctness or determinism advantage over the already-pinned
tiny-skia for this renderer. No recommendation to migrate. Tessellation alone
would likewise not replace antialiasing, compositing, and clipping policy.

## 8. PDF: reuse the existing converter; krilla has concrete contract conflicts

### svg2pdf 0.13.0

**Confirmed.** `to_pdf(&Tree, ConversionOptions, PageOptions)` creates a PDF;
`to_chunk(&Tree, ConversionOptions) -> Result<(pdf_writer::Chunk, Ref)>` creates
an embeddable Form XObject with dependent objects. `ConversionOptions` exposes
`compress`, `raster_scale`, `embed_text`, and `pdfa`. Filtered groups rasterize
while surrounding content remains vector. Documented gaps include gradient
`spreadMethod` and raster color management (DeviceRGB). `pdfa` does not turn a
standalone output into a valid PDF/A document. [P1]

**Assessment:** this already supplies the high-value generic nested-SVG-to-PDF
mechanism. Spec §8.6 expressly forbids a whole-scene serialized-SVG pivot, and
requires more than the converter's documented native feature subset. Keep
project lowering and ADR-0009's explicit closure plan. `raster_scale` is not a
replacement for the admitted final-device grids. Whole-scene delegation would
also reintroduce SVG text/layout decisions and different serialization policy.

### Krilla

**Confirmed at snapshot `1141e31…`:** krilla builds on pdf-writer and offers paths,
fills/strokes, gradients, masks, isolation, positioned-glyph input, images,
metadata, links, and tagged-PDF facilities. Concrete entry points include
`Document::new`, `start_page_with(PageSettings)`, `page.surface()`,
`Surface::draw_path`, and `Document::finish`. Its optional `simple-text` feature
uses HarfRust; pre-positioned glyphs are the appropriate conceptual input for a
renderer that already shapes text. [P2], [P3]

Two blockers are **confirmed**, not merely absent documentation:

1. `geom::Point`, `Rect`, `Transform`, and `PathBuilder` are `f32`-based. Passing
   ordinary project geometry through them conflicts with spec §8.6's explicit
   full-`f64`, no-second-quantization requirement. [P4]
2. The inspected CID-font serialization calls `subset_font`, which invokes
   `subsetter::subset_with_variations`, embeds the resulting program, and
   remaps glyphs. The inspected `SerializeSettings` has no disable-subsetting
   field. This path cannot preserve the complete reviewed font programs required
   by spec §8.6 and ADR-0008. Supplying already-shaped glyphs does not bypass
   this subsetting. [P5], [P6]

**Maturity evidence:** upstream documents snapshot checks against Arlington and
veraPDF, visual tests against six viewers (five in CI), and more than 1,500
SVG integration tests. These are unusually relevant validation practices, but
the counts are upstream reports, not results reproduced here. The manifest
declares `0.8.2`; the README gives MSRV 1.92. [P2], [P3]

**Recommendation:** do not propose krilla as an unchanged revision-1 PDF backend.
Its higher-level abstractions remain useful design references, and it could be
reconsidered after upstream extension or a deliberate contract decision.
Unverified remaining issues include exact stream filters, byte-derived file ID,
custom manifest/attachment carriage, `ActualText` segmentation, allocation
failure handling, and native/WASM byte equality. A switch that accepts narrower
geometry or derived PDF subsets would explicitly conflict with the current spec
and ADR-0008, rather than being an ordinary internal refactor.

## 9. Fontations: reuse table machinery without erasing the subset profile

**Confirmed API boundaries:**

- `read-fonts 0.42.1`: `FontRef::new(&bytes)` / `FontRef::from_index`,
  `TableProvider` accessors such as `head()` and `maxp()`, typed cmap mapping,
  and name decoding. Its stated design is memory-safe zero-allocation parsing
  and raw OpenType access; higher-level metadata/outlines are delegated to
  `skrifa`. [F1]
- `write-fonts 0.53.0`: `FontBuilder::add_table`, `add_raw`,
  `copy_missing_tables`, and `build`; `dump_table` serializes typed tables.
  The builder handles table ordering, directory construction, and checksum
  adjustment. `copy_missing_tables` can log and skip malformed source data,
  so it is not a preservation validator. [F2]
- `skera 0.7.0`: `Plan`, `SubsetFlags`, and
  `subset_font(&FontRef, &Plan) -> Result<Vec<u8>, SubsetError>` provide the
  subsetter boundary. None of these interfaces promises the project's canonical
  ledger or profile-specific name/license policy. [F3], [F6]

**Assessment:** good candidates for replacing any remaining generic sfnt binary
table packing/parsing discovered by the audit. Keep approved table treatment,
glyph/layout closure validation, metric checks, source-byte identity, legal
records, deterministic aliases, and the resource transcript above those tools.
`FontRef::new` succeeding is not full-font validation; lazy table access and
the profile's bounded traversal answer different questions.

**Maturity evidence:** Fontations documents generated OpenType readers/writers
and an OSS-Fuzz integration. This is ecosystem maintenance/testing evidence;
skrifa's use in Google applications cannot be generalized into a claim that
every skera feature is deployed there. [F4] Upstream PR #1996 fixes an
out-of-range name-record-count panic, merged 2026-08-05. ADR-0003 records the
repository's adoption of skera 0.7.0 and retirement of its safety adapter. [F5]

**Important existing constraint:** the manifest deliberately keeps
`read-fonts 0.42.1` with `experimental_traverse` for the bounded layout validator;
skera/write-fonts use a newer parser whose traversal API was removed. Replacing
the old parser solely to unify versions would lose an explicitly documented
capability. Cross the boundary through sfnt bytes as the current manifest/ADR
describe. **Unverified:** whether current typed table APIs can replace that
traversal with equivalent bounds and failure behavior; no such migration is
recommended from ecosystem evidence alone.

## 10. Rough and freehand ports: compatibility matters more than resemblance

### roughr

`roughr 0.13.0` documents a Rough.js Rust port with drawing primitives and
hachure/zigzag/cross-hatch/dots/dashed/zigzag-line fillers. It produces geometry
for other renderers rather than rendering pixels itself. [R1]

**Confirmed mismatch:** inspected `Options` uses many `f32` option values and
an `Option<StdRng>` randomizer; `Options::random()` seeds `rand::rngs::StdRng`
from a u64, or from randomness when no seed is supplied. Rough.js's inspected
upstream `Random::next` uses the 48271 `Math.imul` recurrence with a 31-bit mask.
These are different random streams even when both are seeded. [R2], [R3]

Spec §10 is decisive: directly port the used Rough.js 4.6.4 paths/fillers,
preserve signed-32-bit PRNG and consumption order, and do not use `rand`.
**Recommendation:** exclude roughr as a revision-1 replacement. The port's
published release and broad filler coverage are evidence of available work, not
pinned Excalidraw parity. Making it conform would be a substantive controlled
port/fork, not simple library adoption.

### perfect_freehand

The original Perfect Freehand README links a community Rust port. The published
`perfect_freehand 0.1.1` API exports `get_stroke`, `get_stroke_points`,
`get_stroke_outline_points`, and a stroke-to-SVG helper. The inspected port uses
`f64` points and exposes pressure simulation, easing, streamline/smoothing,
start/end caps/tapers, and completion options. [R4], [R5], [R6]

**Qualification gap:** the port's inspected vector helpers call `sin`, `cos`,
and `powi` directly, rather than this project's portable-math boundary. Its
README describes improvements over earlier Rust ports; it does not claim exact
Perfect Freehand 1.2.0 vectors. A community link from upstream is not proof of
version-specific equivalence or broad production use. [R7], [R8]

**Assessment:** a conditional candidate under the spec's explicit allowance,
not a strong recommendation to replace an already-qualified port. Require exact
1.2.0 point/outline vectors for single points, short/repeated segments, pressure,
taper/cap edges, and completed strokes on both targets. Retain the reference's
two-decimal stroke-to-path wrapper; the existence of a crate helper does not
establish identical token trimming. No unchanged-use qualification was found.

## 11. CSS selectors: prefer coherence with the existing SVG interpreter

**Confirmed.** `simplecss 0.2.2` provides `StyleSheet::parse`, `Selector::parse`,
`Selector::matches`, and an `Element` trait. It sorts rules by specificity and
recognizes `!important`. It explicitly skips at-rules, leaves property values
unparsed, requires lowercase keywords, and does not support Unicode escapes.
Its own README points users needing browser-grade handling to cssparser plus
selectors. [C1], [C2]

`selectors 0.40.0` supplies selector parsing/serialization and matching over a
generic element tree. Integration types are `Parser`, `SelectorImpl`,
`SelectorList`, `Element`, and `MatchingContext`;
`matching::matches_selector_list` performs matching. The crate depends on
`cssparser ^0.37`, compatible in range with this workspace's 0.37.0 pin, and
is MPL-2.0 licensed. The official Stylo repository owns it and documents frequent
breaking API changes. [C3], [C4], [C5]

**Assessment:** use selectors when there is a measured supported-syntax gap
that cannot reasonably stay within simplecss. It is a selector engine, not a
complete cascade/value/URL sanitizer. XML namespace behavior, supported
pseudo-classes, matching work limits, and error attribution still need policy.
The larger adapter and dependency surface must earn their cost.

ADR-0009 deliberately uses the same simplecss version as usvg when resolving
stylesheet filters before inserting private effect IDs. A broader parser only
in the sanitizer/preflight can disagree with downstream rendering about which
rules apply. **Unverified:** a coherent wider-selector integration across both
stages and its canonical work accounting. No blanket parser migration follows
from the existence of selectors.

## 12. Text layout: cosmic-text is capable, but changes the policy surface

**Confirmed at snapshot `daae9c7…`:** cosmic-text offers `FontSystem`, `Buffer`,
`Metrics`, `Attrs`, `Shaping`, `set_text`, `layout_runs`, and optional
`SwashCache` rasterization. It owns wrapping/layout, fallback, editing, and bidi
integration. The inspected `0.19.0` development manifest uses **HarfRust 0.5**,
not rustybuzz, plus `unicode-bidi 0.3.18`. Defaults include `std`, `swash`, and
`fontconfig`; `std` activates fontdb memory mapping and system locale support.
These features do not make fixed in-memory operation impossible, but default
font discovery and locale-dependent fallback are inappropriate here. [X1], [X2]

By contrast, pinned `rustybuzz 0.20.1` explicitly targets HarfBuzz 10.0.1-era
shaping and reports 2,221/2,252 HarfBuzz shaping tests passing. It shapes in
font units and does not supply subsetting or system-font integration. This is
a focused mechanism the project can compose with its pinned unicode-bidi and
font policy. [X3]

**Assessment:** retain that composition for ordinary scene text unless the audit
finds a concrete generic layout burden that cosmic-text can remove without
changing behavior. The project fixes element-wide bidi base detection, source
line/paragraph handling, declared family metrics, chunk/CJK/emoji fallback,
default-ignorable handling, logical text carriage, and 32-unit per-occurrence
fallback-probe charges. A higher-level engine's cached fallback and shaping
choices cannot become the new definition of those concepts. Adding cosmic-text
now could also introduce a second shaper/version rather than merely package
the current one.

**Maturity evidence:** the official README documents multilingual examples and
a UDHR editing test over almost 500 languages, preserving 106,746 lines. That
demonstrates meaningful text-handling work, not Excalidraw layout or raster
parity. The README also notes platform-dependent fallback support. [X4]
**Unverified:** exact pinned-font fallback/run control, semantic probe replay,
source terminator preservation, and native/WASM artifact equality under a
restricted configuration. Do not enable swash rasterization merely to get
layout APIs; changing glyph rasterization is a separate compatibility decision.

## 13. Questions for replacement qualification

For each proposed replacement, attach the actual call sites and answer:

1. **What mechanism becomes library-owned?** Identify the generic decoder,
   numerical routine, parser, stroker, or table writer, and the policy that still
   surrounds it. Avoid counting a contract adapter as removable duplication.
2. **Where is the first semantic difference?** Compare admission/recovery,
   numeric precision, ordering, text mapping, and format-specific behavior,
   rather than only successful sample output.
3. **Can the controlled surface be bounded before calling it?** Distinguish
   constructor allocation, lazy parsing, internal expansion, fallible output
   allocation, and log-and-skip APIs from semantic resource charges.
4. **Does it preserve both equality obligations?** Qualify visual behavior
   against pinned reference evidence and exact artifact/report behavior across
   Linux/browser WASM; preserve cold/warm/batch ledger equality independently.
5. **Is the adapter smaller and clearer than the mechanism replaced?** ICO
   selection and PDF font/precision conflicts are examples where “use a library”
   can merely relocate or enlarge the custom implementation.

The source review establishes concrete reuse boundaries and several exclusions.
It does not establish replacement whole-corpus parity, performance gains,
WASM footprint changes, exhaustive panic freedom, or exact code reduction.

## Primary-source references

Image claims above use 0.25.10 source, inspected from the local crates.io package
as well as upstream release documentation. Initial 0.25.8 discovery was
rechecked against the locked 0.25.10 decoder/limits source before writing the
recommendations.

[A1]: https://docs.rs/crc32fast/1.4.2/crc32fast/
[A2]: https://docs.rs/ryu-js/1.0.3/ryu_js/struct.Buffer.html
[A3]: https://docs.rs/cssparser/0.37.0/cssparser/struct.Parser.html
[A4]: https://docs.rs/write-fonts/0.53.0/src/write_fonts/tables/name.rs.html
[A5]: https://docs.rs/image/0.25.10/image/metadata/enum.Orientation.html
[A6]: https://docs.rs/pdf-writer/0.12.0/pdf_writer/
[A7]: https://docs.rs/palette/0.7.6/palette/
[A8]: https://docs.rs/color/0.3.3/color/
[I1]: https://github.com/image-rs/image/blob/v0.25.10/src/codecs/bmp/decoder.rs
[I2]: https://github.com/image-rs/image/blob/v0.25.10/src/codecs/ico/decoder.rs
[I3]: https://github.com/image-rs/image/blob/v0.25.10/src/io/decoder.rs
[I4]: https://github.com/image-rs/image/blob/v0.25.10/src/io/limits.rs
[I5]: https://github.com/image-rs/image/blob/v0.25.10/Cargo.toml
[I6]: https://github.com/image-rs/image/blob/v0.25.10/CHANGES.md
[K1]: https://docs.rs/kurbo/0.11.3/kurbo/trait.ParamCurveExtrema.html
[K2]: https://docs.rs/kurbo/0.11.3/kurbo/trait.ParamCurveArclen.html
[K3]: https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/src/cubicbez.rs
[K4]: https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/Cargo.toml
[K5]: https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/src/common.rs
[K6]: https://github.com/linebender/kurbo/blob/v0.11.3/kurbo/src/quadbez.rs
[S1]: https://github.com/linebender/svgtypes/blob/v0.15.3/README.md
[S2]: https://docs.rs/svgtypes/0.15.3/svgtypes/
[S3]: https://github.com/linebender/resvg/blob/v0.45.1/crates/usvg/src/lib.rs
[S4]: https://github.com/linebender/resvg/blob/v0.45.1/crates/usvg/src/parser/options.rs
[S5]: https://github.com/linebender/resvg/blob/v0.45.1/README.md
[S6]: https://github.com/linebender/resvg/blob/v0.45.1/crates/usvg/src/parser/image.rs
[T1]: https://github.com/linebender/tiny-skia/blob/v0.11.4/examples/stroke.rs
[T2]: https://github.com/linebender/tiny-skia/blob/v0.11.4/path/src/dash.rs
[T3]: https://github.com/linebender/tiny-skia/blob/v0.11.4/path/src/stroker.rs
[T4]: https://github.com/linebender/tiny-skia/blob/v0.11.4/src/mask.rs
[T5]: https://github.com/linebender/tiny-skia/blob/v0.11.4/src/painter.rs
[T6]: https://github.com/linebender/tiny-skia/blob/v0.11.4/README.md
[T7]: https://github.com/linebender/tiny-skia/tree/v0.11.4/tests/integration
[T8]: https://github.com/linebender/tiny-skia/blob/v0.11.4/Cargo.toml
[V1]: https://github.com/linebender/vello/blob/0268b37f5c54fc3970707b1c525d555c568d67dd/vello_cpu/README.md
[V2]: https://github.com/linebender/vello/blob/0268b37f5c54fc3970707b1c525d555c568d67dd/vello_cpu/Cargo.toml
[V3]: https://github.com/jrmuizel/raqote/blob/master/README.md
[V4]: https://github.com/jrmuizel/raqote/blob/master/Cargo.toml
[P1]: https://github.com/typst/svg2pdf/blob/v0.13.0/src/lib.rs
[P2]: https://github.com/LaurenzV/krilla/blob/1141e3145919da434389ebab431a14b783fd1731/README.md
[P3]: https://github.com/LaurenzV/krilla/blob/1141e3145919da434389ebab431a14b783fd1731/crates/krilla/Cargo.toml
[P4]: https://github.com/LaurenzV/krilla/blob/1141e3145919da434389ebab431a14b783fd1731/crates/krilla/src/geom.rs
[P5]: https://github.com/LaurenzV/krilla/blob/1141e3145919da434389ebab431a14b783fd1731/crates/krilla/src/text/cid.rs
[P6]: https://github.com/LaurenzV/krilla/blob/1141e3145919da434389ebab431a14b783fd1731/crates/krilla/src/serialize.rs
[F1]: https://docs.rs/read-fonts/0.42.1/read_fonts/
[F2]: https://docs.rs/write-fonts/0.53.0/src/write_fonts/font_builder.rs.html
[F3]: https://docs.rs/skera/0.7.0/skera/fn.subset_font.html
[F4]: https://github.com/googlefonts/fontations/blob/main/README.md
[F5]: https://github.com/googlefonts/fontations/pull/1996
[F6]: https://docs.rs/skera/0.7.0/skera/
[R1]: https://docs.rs/roughr/0.13.0/roughr/
[R2]: https://github.com/orhanbalci/rough-rs/blob/b17a417d13a6ce339896639a6a931440d2a23017/roughr/src/core.rs
[R3]: https://github.com/rough-stuff/rough/blob/56a2762171b1294d643501e8d14f120db6b27bd7/src/math.ts
[R4]: https://github.com/steveruizok/perfect-freehand/blob/main/README.md
[R5]: https://docs.rs/perfect_freehand/0.1.1/perfect_freehand/
[R6]: https://github.com/sibaiper/perfect_freehand/blob/e76196d9e8fddd5b31c1d9da7ba37a9041af7fdc/src/types.rs
[R7]: https://github.com/sibaiper/perfect_freehand/blob/e76196d9e8fddd5b31c1d9da7ba37a9041af7fdc/src/vec.rs
[R8]: https://github.com/sibaiper/perfect_freehand/blob/e76196d9e8fddd5b31c1d9da7ba37a9041af7fdc/README.md
[C1]: https://github.com/linebender/simplecss/blob/v0.2.2/README.md
[C2]: https://docs.rs/simplecss/0.2.2/simplecss/
[C3]: https://docs.rs/selectors/0.40.0/selectors/
[C4]: https://docs.rs/selectors/0.40.0/selectors/matching/fn.matches_selector_list.html
[C5]: https://github.com/servo/stylo/blob/main/selectors/README.md
[X1]: https://github.com/pop-os/cosmic-text/blob/daae9c75d52322f8fb3af6168d76561540914e1f/src/lib.rs
[X2]: https://github.com/pop-os/cosmic-text/blob/daae9c75d52322f8fb3af6168d76561540914e1f/Cargo.toml
[X3]: https://github.com/harfbuzz/rustybuzz/blob/v0.20.1/README.md
[X4]: https://github.com/pop-os/cosmic-text/blob/daae9c75d52322f8fb3af6168d76561540914e1f/README.md
