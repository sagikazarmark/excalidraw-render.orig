# Generic cubic bounds evaluation (#184)

Isolated, non-published comparison of the actual private `Path::cubic_bounds`
implementation with **kurbo 0.11.3**. Read the
[recommendation](../../docs/research/kurbo-cubic-bounds-evaluation.md) and
[numeric/artifact/resource matrix](matrix.md).

## Historical reproduction

This evaluation is complete. Its Python runner and comparator were retired on
2026-09-25. Commands below document the original procedure, not a supported
maintenance workflow. Probe sources, matrices and conclusions remain; recover
the runners from Git history if reopening the investigation.
Use [CONTRIBUTING.md](../../CONTRIBUTING.md) for current renderer checks.

Requirements: Python 3.11+, Rust **1.97.1** with the
`wasm32-unknown-unknown` standard library, wasm-bindgen-test-runner **0.2.126**,
and Chrome for Testing/ChromeDriver **149.0.7827.55**. The runner selects both
`RUSTC` and `RUSTDOC` explicitly, even when the shell overrides rustup's binaries.

From the repository root:

```sh
# Complete isolated native test suite, current and candidate renderer:
python3 tools/kurbo-evaluation/run.py
python3 tools/kurbo-evaluation/run.py --candidate

# Install the repository's checksum-verified browser pair if needed:
bash tests/package-consumer/install-browser-tools.sh /tmp/opencode/kurbo-browser
python3 tools/kurbo-evaluation/run.py \
  --browser /tmp/opencode/kurbo-browser/chrome-linux64/chrome \
  --driver /tmp/opencode/kurbo-browser/chromedriver-linux64/chromedriver
python3 tools/kurbo-evaluation/run.py --candidate \
  --browser /tmp/opencode/kurbo-browser/chrome-linux64/chrome \
  --driver /tmp/opencode/kurbo-browser/chromedriver-linux64/chromedriver

# Check cross-target equality, dependency versions/checksums, and matrix freshness:
python3 tools/kurbo-evaluation/compare.py
python3 tools/kurbo-evaluation/run.py --clippy
rustup run 1.97.1 cargo fmt --manifest-path tools/kurbo-evaluation/Cargo.toml --check
```

Replays use `--locked --offline`. On a fresh machine, add `--fetch` to the
first invocation to populate the locked cache. The browser needs only loopback
WebDriver traffic once tools and dependencies are installed. Test debuginfo is
disabled for WASM to bound wasm-bindgen processing; optimization remains level 2,
with debug assertions and overflow checks enabled.

## What runs

`build.rs` extracts the actual production primitives and path traversal from
`render/geometry.rs` and compiles the complete private `render/geometry/cubic.rs`
module. The numeric candidate uses the same traversal and union policy with
`adapter.rs` in place of that module. Both modules take four points and return
`(min_x, min_y, max_x, max_y)`. The small arithmetic helpers in the probe reproduce
`square(x) = x*x` and `cube(x) = x*x*x`.

`run.py` creates a temporary workspace shell, copies the renderer, and links its
unchanged sibling crates. In candidate mode it replaces only `geometry/cubic.rs`
in this copy with **the same adapter** and adds its dependency edge. It preserves
the original module separately for the numeric comparison. The real
renderer tree and reference geometry are never edited by this experiment.

The lock was seeded from the production lock. All registry package identities
and checksums must still match it (`compare.py` enforces this); the candidate
adds only the already-resolved kurbo dependency to the copied renderer's lock
entry. Both modes enable kurbo's `libm` feature through the probe, while `std`
remains unified on. Feature graphs for the actual native/WASM targets and browser
logs are saved under `target/kurbo-evaluation-*`.

Each run computes:

- **32,768** deterministic cubic comparisons across eight families. SHA-256
  streams cover every input coordinate and every output bound as little-endian
  binary64 bits, in generation order. First, worst-absolute, and grid-crossing
  witnesses retain exact hexadecimal bits as well as readable decimals.
- Seven traversal cases: empty, move-only, line-only, missing initial point,
  ignored remote move/line contributors, point cubic, and multiple subpaths.
- **36 scenes × SVG/PNG/PDF**, through the public renderer. Every record contains
  input elements, artifact length/SHA-256 and compatibility report. PNG records
  include dimensions and decoded-pixel digests; PDF records include inflated
  `cm` matrix operands using the repository's existing PDF inspector.
- Four binary searches for the smallest clean PNG element-work budget, checking
  the exact limit and one unit below; two **fixed** PDF output-byte caps, chosen
  from the observed current/candidate length boundary and identical in both runs.

The four `expected-*.json` files are **experimental observations**, not approved
reference evidence. A pre-existing PNG cross-target difference is visible in
both modes; separate target records do not waive or repair the product promise.
`compare.py` asserts numeric/traversal and resource cross-target agreement and
lists artifact differences. Decimal JSON comparisons normalize both sides through
the same reader; exact floating-point evidence is the hex bits and stream hashes.

To deliberately regenerate measurements, append `--record` to **each of the four
run commands**, then run `python3 tools/kurbo-evaluation/compare.py --record`.
Review the differences before accepting them. Recording is separate from replay;
rerun without `--record` afterward. The maintenance-only `--initialize-lock`
option re-seeds from the production lock and must likewise be reviewed.
