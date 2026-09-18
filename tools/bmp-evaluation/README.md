# BMP / EXIF evaluation (#183)

This isolated, non-published workspace compares the actual private BMP decoder
and EXIF reader with `image 0.25.10`. It does not enable BMP in the production
dependency graph. Read the [recommendation and audit](../../docs/research/bmp-image-evaluation.md)
and the [exact matrix](matrix.md).

## Reproduce

From the repository root, with Rust 1.97.1, its `wasm32-unknown-unknown` standard
library, and wasm-bindgen-test-runner 0.2.126 installed:

```sh
export RUSTC="$(rustup which --toolchain 1.97.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.97.1 rustdoc)"
rustup run 1.97.1 cargo test --locked --offline \
  --manifest-path tools/bmp-evaluation/Cargo.toml --target-dir target/bmp-evaluation

# Install checksum-verified browser tools if not already present:
bash tests/package-consumer/install-browser-tools.sh /tmp/opencode/bmp-browser
rustup run 1.97.1 python3 tools/bmp-evaluation/run-browser.py \
  --browser /tmp/opencode/bmp-browser/chrome-linux64/chrome \
  --driver /tmp/opencode/bmp-browser/chromedriver-linux64/chromedriver
```

The runner selects the browser binary explicitly, uses the same committed
`expected.json`, and disables test debuginfo to keep wasm-bindgen processing
bounded. It does not change optimization, debug assertions, or overflow checks.
On a fresh machine, first run `cargo fetch --locked --manifest-path
tools/bmp-evaluation/Cargo.toml` to populate the cache; reproduction itself is
offline apart from loopback WebDriver traffic.

To deliberately regenerate the native observation and human-readable matrix:

```sh
rustup run 1.97.1 cargo run --locked --offline \
  --manifest-path tools/bmp-evaluation/Cargo.toml --target-dir target/bmp-evaluation \
  -- --record
```

Review changed observations before accepting them. These are differential
measurements, not browser-reference expectations. `expected.json` records every
small fixture's input bytes, exact decoded RGBA or failure stage/category, the
current dimension/validation-work readings, two exhaustive mask-sweep digests,
constructor/limits probes, and 20 artifact length/SHA-256 pairs. Both target
tests compare the entire report. With no arguments the executable prints it.

`current_bmp` includes the production file directly. The build script extracts
only the production `exif_orientation` function between explicit function
boundaries, tracking that source as a rebuild input. There is no copied decoder
implementation to maintain. The PNG bridge is only a way to exercise candidate
pixels through the public PNG/PDF renderer; it is not the proposed production
adapter or a semantic-resource-accounting equivalence test.

The fixed tiny-fixture corpus is bounded. This is not an arbitrary-input
decoder/fuzzer: raw candidate calls deliberately bypass renderer admission to
observe acceptance differences. The separate admitted path validates first,
checks a 65,536-pixel experiment cap before construction, verifies decoder
dimensions/type/size, and fallibly reserves RGBA capacity before decoding.
