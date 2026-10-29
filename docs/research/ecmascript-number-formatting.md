# ECMAScript number formatting qualification (#180)

## Decision and dependency identity

Use **`ryu-js = "=1.0.3"`, `default-features = false`, no enabled features**
for the renderer's ECMAScript spelling and fixed-two-decimal Rough spelling.
The workspace and packaged-consumer locks add exactly this package and the
renderer dependency edge. Crates.io checksum:
`04d056b875a9d2e6cb9a61d127afee9ac5999b9f87bcb32079d1318e505be714`.
`cargo tree -e features -p ryu-js` confirms the same featureless, dependency-free
closure on native and `wasm32-unknown-unknown`. The `small` and optional
`no-panic` features are disabled. The crate is `no_std`, has no allocator, native
links or build script, and is licensed Apache-2.0 OR BSL-1.0.

- `ecmascript_number` explicitly rejects non-finite inputs before calling the
  safe `Buffer::format_finite(f64)` API. The two freedraw callers retain their
  existing context-specific non-finite errors. Zero canonicalization, shortest
  digits, exponent thresholds and exponent signs belong to the dependency.
- **Adopt the fixed-decimal portion.** `push_rough_number` first rejects
  non-finite inputs and `abs(value) >= 1e21`, then calls
  `Buffer::format_to_fixed(value, 2)`. Its adapter trims fractional trailing
  zeros and the decimal point, then maps `-0` to `0`. It writes nothing on
  input rejection. The owned binary64-to-u128 rounding implementation is removed.
- General SVG `Display` and PDF's plain-decimal/no-exponent policy stay separate.
  Neither is routed through the ECMAScript formatter. Geometry generation,
  freedraw's reference-derived token reduction and semantic accounting are
  unchanged.

Rough.js serializes `Number(value.toFixed(2))`. The renderer's pre-existing
fixed-decimal policy preserves the exact integer digits even above the admitted
coordinate domain, where converting a large fixed integer back through `Number`
can change its last digits. Qualification deliberately preserves that policy
over the serializer's entire accepted range; it does not widen scene admission
or normalize PDF/SVG serialization to JavaScript.

Primary sources: [safe APIs](https://docs.rs/ryu-js/1.0.3/ryu_js/struct.Buffer.html),
[ECMAScript scope and Ryū provenance](https://github.com/boa-dev/ryu-js/blob/v1.0.3/README.md).

## Bounded dependency safety review

Reviewed the downloaded 1.0.3 sources at `src/buffer/mod.rs`, `src/pretty/mod.rs`,
the mantissa/exponent writers, `src/d2s.rs`, `src/d2s_intrinsics.rs`, and
`src/pretty/to_fixed/mod.rs`. First-party code
uses only the safe buffer API and retains `unsafe_code = "forbid"`.

The dependency's safe wrapper owns a 132-byte `MaybeUninit<u8>` buffer, calls
sealed f64 formatting implementations, and constructs a borrowed UTF-8 string
from the written prefix. The shortest formatter bounds output at 25 bytes;
the fixed formatter bounds its full supported 0–100-digit precision within the
132-byte buffer. Our fixed call uses constant precision 2 and finite values
strictly below 1e21. Its decimal output consists only of ASCII digits, sign and
point. The shortest path additionally writes ASCII `e` and exponent sign.
Pointer writes/copies and unchecked lookup tables are internal to these
bounded routines. The fixed implementation checks the carry cursor boundary
before reading its predecessor; integral-table indices are bounded by the
below-1e21 branch, and fractional-table indices by the binary64 exponent and
precision. There are no caller-supplied pointers or externally implementable
numeric traits at this boundary.

This is a targeted renderer-facing dependency review, not a claim that tests
prove upstream memory safety. Generic `ryu` does not qualify for ECMAScript
spelling; retaining the owned Rust formatting adapter would retain the runtime
semantic boundary this issue delegates. No safer equivalent specialized library
was qualified by this change.

## Independent oracle and test seams

The user confirmed the existing internal number-format seam for exact token and
rejection qualification, plus public rendering/package seams for artifact and
resource regressions. The equivalence tests passed on the old adapters before
replacement; this is a reuse refactor, not a manufactured red test for a claimed
formatting defect.

`tools/number-format-vectors/derive.mjs` evaluates `String(value)` and
`value.toFixed(2)` in **Chromium 149.0.7827.55**, under revision 1's pinned
Playwright 1.61.0 / Node 24.16.0 environment. It checks those runtime identities
and stores inputs as binary64 hexadecimal bits so JSON decimal parsing cannot
change them. The fixture contains **595 vectors**:

- Both signs and immediately adjacent floats around 1e-6 and 1e21.
- Exact half-cent ties (0.125, 60.125), inexact decimal ties (1.005, 2.675), and
  the rounded-zero boundary (0.005), each with both adjacent floats and signs.
- Signed zero, minimum subnormal and the normal/subnormal boundary, largest
  finite values, NaN and both infinities.
- Coordinate arithmetic, trailing zeros, the default admitted coordinate ceiling, and
  a large value whose shortest and fixed decimal digits differ.
- 256 deterministic binary64 samples and 256 deterministic coordinates within
  ±1e9, deduplicated with the boundary vectors.

`shortest` and `fixed` are direct JavaScript oracle results. `rough` applies the
existing renderer's trimming/zero policy to the oracle's fixed string; null
marks rejected input. The Rust tests compare exact strings, assert rejection
without sink modification, and run the same corpus in browser WASM through the
existing `wasm_tests` qualification selection.

Reproduce (default verifies; `--write` explicitly regenerates the fixture):

```sh
docker run --rm --init --ipc=host --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --mount "type=bind,src=$PWD,dst=/workspace" --workdir /workspace \
  mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9 \
  node tools/number-format-vectors/derive.mjs
cargo test --locked -p excalidraw-render --lib render::number_format::tests
```

Install the pinned oracle npm closure first if necessary with
`tools/reference-oracle/run-container.sh install`.

## Evidence and revision rules

Revision 1 remains draft. This dependency is outside the comparator's reachable
closure (ADR-0005), and changes neither reference-renderer identity nor the font
subset profile. Oracle vectors here are library qualification data, not a
replacement reference profile or a new maintainer approval. Existing approved
artifact expectations and semantic-resource evidence are checked rather than
repinned. Any output delta must be explained against the reference; an
out-of-scope semantic difference belongs in follow-up work.

The known #178 baseline is four native/browser-WASM PNG artifact differences
and three corresponding output-length resource-trace differences. All four
compatibility reports agree. The equal-length rotated-shape PNG has no trace
difference. Qualification must distinguish those exact keys and measurements
from any delta introduced here.

## Qualification results

The change introduced no artifact, report or trace delta on either target:
the native and browser snapshot replays over all 168 inventory keys were
identical to the runs from the starting commit, and the only cross-target
differences were the exact #178 set. The browser qualification tier executes
the same 595 oracle vectors in release-mode WASM
(`number_formatting_matches_pinned_chromium_vectors`).
