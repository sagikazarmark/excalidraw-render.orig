# CSS conversion-kernel evaluation (#185)

Isolated, non-published experiment against the renderer at
`cbc39ae798dc92e3fb1a599fed5ad926c8d48af9`. See the
[recommendation](../../docs/research/color-conversion-evaluation.md),
[source audit](../../docs/research/color-library-source-audit.md), and
[generated matrix](matrix.md).

## Reproduce

Python 3.11+, Rust **1.97.1** with `wasm32-unknown-unknown`,
wasm-bindgen-test-runner **0.2.126**, Chrome for Testing and ChromeDriver
**149.0.7827.55**. The runner selects RUSTC/RUSTDOC through rustup explicitly.
Native and WASM use opt-level 2, debug assertions, no debuginfo or incremental
cache. From the repository root:

```sh
# Complete isolated native suite, including exact observation replay:
python3 tools/color-evaluation/run.py

# Install the checksum-verified browser pair if necessary:
bash tests/package-consumer/install-browser-tools.sh /tmp/opencode/color-browser
python3 tools/color-evaluation/run.py \
  --browser /tmp/opencode/color-browser/chrome-linux64/chrome \
  --driver /tmp/opencode/color-browser/chromedriver-linux64/chromedriver

# Independent CSS canvas and SVG-image browser observations:
python3 tools/color-evaluation/reference.py \
  --browser /tmp/opencode/color-browser/chrome-linux64/chrome \
  --driver /tmp/opencode/color-browser/chromedriver-linux64/chromedriver

# Cross-target assertions, lock audit, matrix freshness, and static checks:
python3 tools/color-evaluation/compare.py
python3 tools/color-evaluation/run.py --clippy
rustup run 1.97.1 cargo fmt --manifest-path tools/color-evaluation/Cargo.toml --check
```

Replay is `--locked --offline`; use `--fetch` once to populate a fresh cache.
Cargo lock identities/checksums shared with production are checked by `compare.py`.
The only new registry packages are palette 0.7.6, palette_derive 0.7.6,
fast-srgb8 1.0.0, by_address 1.2.1, and color 0.3.3. libm remains 0.2.16.
Both candidates resolve **only libm**, with their std features off, on both targets.
Resolved feature trees and the browser log are saved under `target/color-evaluation`.

## What is compared

`build.rs` compiles the actual non-test `color.rs` and its private
`color/conversion.rs` module three times, appending private measurement hooks.
It inlines the numeric module into each generated copy and replaces eight numeric
functions (including the XYZ→linear sRGB edge used by `color()`) and the rectangular
Oklab mix conversion arms in two copies.
`src/adapter.rs` is the entire candidate conversion adapter. Alpha never enters it.
Raw Lab inverse comparison calls the production numeric function directly, before
the caller's unchanged f32/gamut boundary. Existing `portable_math.rs` and
`css_syntax.rs` are compiled directly; the extracted module uses a local
two-variant ColorMode stand-in.

The module extraction preserves the original evaluation's substitution boundaries
and exact observations. `compare.py` accounts for the current two-file source
layout; the recommendation's original source-line counts describe the evaluated
commit above. Observation JSON files are unchanged by the extraction.

The substitution preserves parser admission and f32 number tokens, system colors,
missing components, nested canonical stop quantization, f64 mix weights and alpha,
all hue methods, gamut mapping, and final RGBA8 rounding. Polar Oklch and direct
CSS Oklab/Oklch parsing still use csscolorparser. Wide-RGB matrices and A98,
ProPhoto and Rec.2020 transfer curves remain in place, with their shared generic
sRGB/XYZ/adaptation edges delegated. This is a deliberately bounded shared-kernel
adapter, not a whole-library CSS/interpolation replacement.

- **36,864 raw numeric samples:** 4,096 each for RGB8→XYZ/Lab/Oklab, sRGB
  decode/encode, both white-point adaptations, and Lab/Oklab→sRGB. RGB8 samples
  include every gray; extended inputs are dyadic fixed-seed values. Transfer
  samples include exact breakpoints and adjacent binary64 values, their negatives,
  signed zero, endpoints and out-of-range values. Lab inverse samples extend L to
  120 and a/b to -100…200; Oklab components cover -1…2. These are conversion-domain
  stress samples, not claims about parser admission for every abstract tuple.
- **133 CSS cases:** pinned unit witnesses, all current mix spaces, wide RGB,
  missing values, rejected syntax, low/zero alpha, and out-of-gamut Lab.
- **1,530 half-byte cases:** adjacent binary64 values around all 255 RGB/alpha
  half-byte boundaries, both direct CSS sRGB and a linear-sRGB black/white mix.
  Direct CSS numbers still pass through the existing parser's f32 boundary;
  percentage weights retain f64. Some strings/parsed inputs coincide intentionally.
- **4,096 admitted Lab strings:** fixed seed, L 0…100, a/b -125…125, alpha 0…1.
- **20 selected swatches × 2 modes × 3 formats × 3 kernels:** exact artifact
  lengths/digests, PNG pixel digests and center RGBA. Candidate canonical results
  are fed to the unchanged renderer as exact `#rrggbbaa`. For every current
  swatch/mode/format, this must equal rendering the original CSS directly.
- **132 unique supplemental browser probes:** the fixed CSS strings plus selected
  swatch witnesses, drawn directly into an sRGB canvas and independently through
  a serialized SVG image. These are separate from Rust/WASM execution.

Numeric SHA-256 streams include every input and output as little-endian binary64
bits. First differences retain readable values, hexadecimal bits and signed
channel deltas. Canonical sweeps hash the length-prefixed CSS and all RGBA8 results.
No tolerance is used. Raw current Oklab streams differ between targets; candidate
streams, canonical observations and swatch artifacts agree exactly. Separate target
files expose the former rather than normalizing it away.

The JSON files are **experimental observations**, not approved reference evidence.
The 17 `pinned_rgb` witnesses originate in the existing pinned-Chromium production
unit test; the supplemental browser probe is not the reference renderer's official
export path. Full-scene perceptual parity and canonical WASM footprint are not
measured here.

To regenerate deliberately, append `--record` to both `run.py` commands and the
`reference.py` command, then run `compare.py --record`. Inspect changes and replay
without `--record`. `--initialize-lock` is maintenance-only: it seeds from production
and resolves the five extra packages. Palette's derive dependency is explicitly
pinned because 0.7.7 generates code that fails to compile with palette 0.7.6.
