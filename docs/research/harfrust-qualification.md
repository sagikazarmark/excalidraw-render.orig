# HarfRust 0.13.3 for scene text and font-subset validation: adopt

**Issue [#202](https://github.com/sagikazarmark/excalidraw-render.orig/issues/202),
2026-09-29.** Production source at `fbc7d35` (main after #224). The candidate
is **HarfRust 0.13.3**, default features (`std`), against **rustybuzz 0.20.1**.

**Recommendation: adopt with specified changes.** Over every bundled face,
HarfRust shapes identically to rustybuzz in the renderer's configurations,
except for three right-to-left mirroring cases, where rustybuzz is wrong and
HarfRust matches Unicode and HarfBuzz. A renderer prototype that shapes
through HarfRust passes the whole test suite, the viewer harness and every
conformance check without moving any pinned artifact. HarfRust is also about
2.8× faster. The specified changes are the parser split, the scope of the
move, and the text that names the shaper (see
[Specified changes](#specified-changes)).

The maintainer decisions this qualification works under are recorded on #202:
latest HarfRust (0.13.3), ttf-parser kept directly for parsing, `icu` off,
and the per-user split below.

## Reproducible evidence

- **Probe:** [`tools/harfrust-evaluation`](../../tools/harfrust-evaluation).
  `cargo run --release` prints the summary; `--record <path>` writes the full
  report.
- **Recorded report:**
  [`expected-native.json`](../../tools/harfrust-evaluation/expected-native.json).
  The native test (`tests/native.rs`) requires it exactly, with and without
  HarfRust's `libm` feature. The browser test (`tests/wasm.rs`, run with the
  manifest-pinned wasm-bindgen-test-runner 0.2.129 and Chrome 149.0.7827.55)
  requires the same bytes.
- **Supporting examples:** `examples/mirror.rs` prints the mirroring table
  entries behind every difference, and `examples/bench.rs` measures throughput.
- **Renderer prototype:** branch `prototype/202-harfrust-renderer`, described
  in [Renderer prototype](#renderer-prototype).

Both shapers see the exact SFNT bytes the renderer loads. Each catalog WOFF2 is
decoded with wuff 0.2.8 and checked against the catalog's reviewed decode
digest, and Noto Emoji comes from `load_noto_emoji_sfnt`. Positions are
compared in font units, with no scale, as the renderer shapes.

### Configurations

The renderer shapes in two feature configurations, each with a guessed or an
explicit direction:

| Configuration | Caller | Features |
| --- | --- | --- |
| `render/*` | `render/text.rs::shape_text` (scene text, labels, PNG/SVG/PDF) | `liga=0`, `clig=0` |
| `none/*` | `render/font_subset.rs` source-vs-subset validation, and the test-only qualification corpus | none |

Each configuration runs with the direction guessed by
`guess_segment_properties`, forced left-to-right, and forced right-to-left:
six in all.

### Corpus

- **Faces:** all 231 bundled faces: every Xiaolai, Excalifont, Comic Shanns,
  Nunito and Lilita chunk, plus Virgil, Cascadia, Liberation Sans and Noto
  Emoji.
- **Fixed texts (29 per face):**
  - Latin, precomposed and decomposed combining marks, Vietnamese, Greek,
    Cyrillic, Arabic, Hebrew and CJK.
  - Mixed bidi and paired brackets.
  - Ligature candidates (`ffi`, `fl`, `Th`, `st`).
  - Default ignorables: ZWJ, ZWNJ, soft hyphen, word joiner, variation
    selectors, bidi marks, BOM, CGJ, the Mongolian vowel separator, tags.
  - Emoji ZWJ, skin-tone, flag and keycap sequences.
  - Every line terminator (CR LF, LF, CR, LS, PS, NEL, VT, FF) and tab.
  - C0/C1 controls, wide and narrow spaces, the `?` replacement text, digits,
    symbols, private use and supplementary planes.
  - Two long inputs: 10,000 Latin characters, and 200 repetitions of mixed
    bidi/CJK/emoji text.
- **Every mapped scalar of every face** (47,654), shaped alone and in
  consecutive 24-scalar windows. The windows exercise kerning, contextual
  lookups and mark attachment between neighbours.
- **Every Unicode scalar** (1,112,064), shaped as `a<c>b` through Liberation
  Sans and an Excalifont chunk, in `render/guess` and `render/rtl`. This covers
  the default-ignorable class the spec pins to the shaper, bidi mirroring, and
  `.notdef` handling beyond what any face maps.

## Results

### Glyphs, clusters, advances, offsets

| Set | Cases | Identical | Differing |
| --- | ---: | ---: | ---: |
| Fixed texts, mapped scalars, windows (231 faces × 6 configurations) | 338,640 | 338,622 | 18 |
| Full-scalar sweep (2 faces × 2 configurations) | 4,448,256 | 4,448,253 | 3 |

Every difference is a bidi mirroring glyph in a right-to-left run:

| Scalar | Mirror (UAX #9 L4) | Faces affected | rustybuzz 0.20.1 | HarfRust 0.13.3 |
| --- | --- | --- | --- | --- |
| U+2265 `≥` | U+2264 `≤` | Cascadia, Comic Shanns, Liberation Sans | `≥` unmirrored | `≤` |
| U+29F5 `⧵` | U+2215 `∕` | Liberation Sans | `.notdef` (the face lacks U+29F5) | `∕` |
| U+2BFE `⯾` | U+221F `∟` | Liberation Sans | `.notdef` (the face lacks U+2BFE) | `∟` |

**Cause:** rustybuzz takes mirroring from `unicode-bidi-mirroring` 0.4.0.
That table has `≤`→`≥`, `∕`→`⧵` and `∟`→`⯾`, but not the reverse of any of
the three. Unicode 17's `BidiMirroring.txt` lists all six directions.

**Effect:** HarfRust is correct. It carries HarfBuzz's Unicode data, and the
reference (Chromium) shapes with HarfBuzz. Spec 8.3 requires a right-to-left run to show a
character's mirror glyph when the face has one (UAX #9 L4). Under rustybuzz,
the renderer does that for `≤` but not for `≥`.

**Scope:** these are upstream correctness improvements, not regressions. No
case in the renderer's corpus contains one of these scalars in a right-to-left
run, so no pinned artifact moves.

### Default ignorables

The spec pins the default-ignorable class to the shaper (spec 8.3, #167), and
`render/text.rs::is_default_ignorable` keeps its own copy.
- **Hidden scalars:** in the full-scalar sweep, both shapers hide exactly the
  same scalars as one zero-advance glyph: 7,612 cases per face.
- **Glyphs:** the sweep shows no glyph, cluster, advance or offset difference
  outside the three mirroring pairs.

So the class the renderer copies is HarfRust's class as well. Only the text
naming the shaper needs to change.

### Native, browser WASM and `libm`

- **Browser WASM:** reproduces the native report exactly, both shapers and all
  4.8 million cases, in about 36 seconds.
- **`libm`:** HarfRust's float use is limited to `floor`, `ceil` and `round`
  (scale conversion and the AAT `trak` table), which are exact IEEE operations
  on every target. The `libm` build's report is identical to the default
  build's.
- **Recommendation:** `std` only. `libm` adds nothing for ADR-0007's
  determinism rules.

### Throughput

`examples/bench.rs` shapes the fixed corpus 30 times through Excalifont,
Liberation Sans and a Xiaolai chunk, in the renderer's configuration: 10.2
million glyphs, identical counts from both shapers, release build.

| | rustybuzz 0.20.1 | HarfRust 0.13.3 |
| --- | ---: | ---: |
| New face per call (as `render/text.rs` does) | ~910 ms | ~320 ms |
| Reused face | ~905 ms | ~320 ms |

HarfRust is about 2.8× faster. Building `ShaperData` per call costs nothing
measurable, so the adapter needs no cache.

### Allocation

This is unchanged. Neither shaper allocates fallibly inside `shape`. The
renderer bounds shaping input the same way under both: text is charged per
normalized scalar before shaping. The swap neither adds nor removes an
infallible allocation site, so it doesn't change §13.3 compliance.

## Renderer prototype

Branch `prototype/202-harfrust-renderer` puts the renderer behind a small
adapter, `crates/excalidraw-render/src/shaping.rs`:

- **`Face`** holds a `ttf_parser::Face`, a `harfrust::FontRef` and its
  `ShaperData`. `from_slice` needs both parsers to accept the face, and `Face`
  dereferences to the parsed face, so metric, outline and table code reads as
  before.
- **`shape(face, features, buffer)`** shapes in font units.

The call sites change only in paths: `rustybuzz::` becomes `crate::shaping::`,
and `rustybuzz::ttf_parser` becomes `ttf_parser`. One type changes:
rustybuzz's `units_per_em()` returned `i32` and ttf-parser's returns `u16`,
which removes two fallible conversions.

Results with that change:

| Check | Result |
| --- | --- |
| `cargo nextest run --workspace` | 1,579 passed |
| Harness profile (`--run-ignored only`): both PDF viewers, both SVG viewers | 10 passed |
| Doc tests, `clippy -D warnings`, `fmt` | pass |
| wasm32 library and test build (release) | clean |
| `conformance verify-all`, `verify-inventory`, `verify-ownership`, `verify-svg-font-subsets`, `verify-svg-font-subset-release`, `verify-svg-viewers`, `verify-pdf-viewers`, `catalog verify` | pass |

No snapshot, oracle reference, baseline or evidence digest moves. The only
rebinding is test bookkeeping: `tests/common/pdf_inspect.rs` is pinned by
SHA-256 in eight contract test files, and its helper's parameter type changed.

### Dependency footprint

- **Renderer closure** (`cargo tree -p excalidraw-render -e normal`): gains
  `harfrust` 0.13.3 and `read-fonts` 0.43.3, and loses nothing. rustybuzz stays
  in the closure through usvg/resvg 0.47 until #209, and ttf-parser 0.25.1 was
  already there.
- **Lock:** the same two packages, with no other version moves.
- **Release CLI binary:** +2.7%; see [Binary size](#binary-size).
- **Fontations copies:** `read-fonts` 0.43 joins 0.39.2 (Krilla via skrifa
  and subsetter) and 0.44.0 (write-fonts/skera). After #209, usvg 0.48.1 adds
  HarfRust 0.12 and `read-fonts` 0.41 until usvg accepts HarfRust 0.13. That
  was accepted on #202.

### Binary size

`cargo build --release -p excalidraw-render-cli`, x86_64 Linux, Rust 1.97.1:

| | Bytes |
| --- | ---: |
| main (`fbc7d35`) | 27,569,400 |
| prototype | 28,303,992 |
| Difference | +734,592 (+2.7%) |

The growth is HarfRust and read-fonts 0.43 compiled in beside rustybuzz, which
usvg 0.47 still links. After #209 moves usvg off rustybuzz, rustybuzz leaves
the binary, which offsets part of it.

## Specified changes

1. **Parser split.**
   - The renderer and its tests parse and measure with `ttf-parser =0.25.1`
     directly, with the features rustybuzz enabled minus its layout tables:
     `std`, `variable-fonts`, `glyph-names`, `no-std-float`.
   - Only shaping goes through HarfRust.
   - A move to read-fonts/skrifa waits until HarfRust and write-fonts share a
     read-fonts line (today 0.43 vs 0.44).
2. **Scope of the move.**
   - **HarfRust:** the renderer's shaping (`render/text.rs`,
     `render/font_subset.rs`, and the test-only
     `render/font_subset_qualification.rs`).
   - **Direct ttf-parser:** every parse-only user.
     - The `excalidraw-fonts` and renderer integration tests.
     - xtask's `catalog.rs`, `conformance_candidate.rs` and
       `representative/viewer_assets.rs`, plus its test.
     - #202 listed only some of these.
   - **Stays on rustybuzz:** `xtask/src/svg_font_subset_runner.rs`.
     - This corrects decision 4 on #202. The runner is the controlled pure-Rust
       SVG viewer (`canonical-svg.pure-rust.v1`), and its report and
       `svg_viewers.rs` pin that viewer's runtime as resvg/usvg 0.47 with
       rustybuzz 0.20.1, the shaper usvg 0.47 itself uses.
     - It moves with usvg in #209, not with the renderer.
   - **Stay on rustybuzz:** `tools/representative-intake/viewer-fonts` (its
     source and lock are bound representative evidence) and
     `tools/kurbo-evaluation` (a finished evaluation).
3. **Text that names the shaper.**
   - Spec 8.3's default-ignorable paragraph and spec 8.6's "PDF text uses
     deterministic `rustybuzz` shaping" name rustybuzz 0.20.1, as does the doc
     comment on `render/text.rs::is_default_ignorable`. All of them become
     HarfRust 0.13.3.
   - The class itself is unchanged (see [Default ignorables](#default-ignorables)).
   - ADR-0007's list of crates that compute internally gains HarfRust.
4. **Output review.** The three mirroring pairs change right-to-left output
   where a face maps the mirror. The migration PR states this as a reviewed
   improvement under ADR-0016. The renderer's corpus has no such case today.

## Acceptance-criteria disposition (#202)

| Criterion | Disposition |
| --- | --- |
| Compare glyph IDs, clusters, advances, offsets and source/subset shaping over the bundled fonts and scripts | Done: 231 faces, both feature configurations, three directions; see [Results](#results). |
| Bidi/mirroring, combining marks, ligatures, default ignorables, fallback, line terminators, generated labels, long input | Done: fixed corpus, full-scalar sweep and the renderer prototype. The only differences are three mirroring pairs. |
| Visual, diagnostic, semantic-resource, allocation and native/browser-WASM differences | No diagnostic or resource-trace change in the prototype, and the viewer harness passes. Allocation is unchanged. WASM equals native. |
| Improvements vs regressions against the reference | All three differences are rustybuzz table gaps. HarfRust matches Unicode and HarfBuzz. |
| Map every production edge retaining rustybuzz | After the move: only usvg/resvg 0.47 (until #209). Tooling: the pure-Rust viewer runner, viewer-fonts, kurbo-evaluation. |
| Pinned version and features, `icu`, the Rust 1.85 minimum, footprint | HarfRust `=0.13.3`, `default-features = false, features = ["std"]`, no `icu` and no `libm`. The workspace is on Rust 1.97.1, above HarfRust's 1.85 minimum. For footprint, see [Dependency footprint](#dependency-footprint). |
| Non-production direct users | See [Specified changes](#specified-changes), item 2. |
| Adopt / adopt-with-changes / defer | **Adopt with the specified changes.** |
