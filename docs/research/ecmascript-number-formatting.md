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

## Packaged artifact/report results

Starting commit: `768fb90d68b02cf501397a6179519aa9fee2a5a9`.
Working-tree qualification uses Rust **1.97.1** and the manifest-pinned browser
through Dagger. A temporary `cargo package --allow-dirty` option permits testing
the edits; it is removed before committing. These runs are working-tree checks,
not clean-commit release attestations.

```sh
dagger api call packaging snapshots --output target/issue-180-results/native-snapshots
dagger api call wasm browser-snapshot-logs --output target/issue-180-results/wasm-snapshots
dagger api call packaging trace-logs --output target/issue-180-results/native
dagger api call wasm trace-logs --output target/issue-180-results/wasm
cargo run --locked -p xtask -- conformance join-trace-results target/issue-180-results
```

Native collection passed all 168 keys, two sessions each in separate forward
and reverse processes (672 renders). Its complete ordered key/sample arrays
are **identical** to the saved `target/issue-179-native-snapshots/snapshots.json`
run from the exact starting commit. Browser collection likewise executed all
168 keys in both orders, and both its full `runs` and `differences` arrays are
**identical** to `target/issue-179-wasm-snapshots/replay.json` from that commit.
No artifact token, artifact byte, or serialized report changed in either target.

Approved-baseline replay still fails. In addition to the four exact #178 WASM
PNG differences, three expectations have pre-existing drift on **both** targets:

| Case / variant / format | Approved SHA-256 prefix / bytes | Current SHA-256 prefix / bytes |
| --- | --- | --- |
| image-codecs / default / PNG | `5590b43c` / 42,819 | `a078a9f6` / 42,939 |
| image-codecs / default / PDF | `d7987458` / 9,352 | `b2f6adbf` / 9,407 |
| transform-opacity-groups / no-background / PNG | `0242002f` / 25,931 | `366b0e8a` / 25,933 |

All reports agree. These three differences are present in the saved clean
starting-commit runs, so they are #85 baseline reconciliation follow-ups, not
number-formatting deltas or additional native/WASM mismatches. Native replay
stops at the first stale key; the browser diagnostic report enumerates all seven
unequal keys against the approved baseline. No expectation was repinned and no
failed equality gate is represented as passing. The
[baseline reconciliation follow-up](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5715325789)
is recorded on #85.

The additional packaged-facade gates each run **9 passing tests and one
pre-existing failure**: `packaged_public_facade_preserves_every_pdf_fallback_family_across_targets`
rejects its turbulence PDF pin (`tests/package-consumer/tests/public_facade.rs:447`).
Expected SHA-256 is `ad1525688146b4016063c2ac96bcc2b066d7b5d0faf8c5148b48b272dda5d528`;
actual is `0048833bf470ed4ebca0acfdae0487390af563509583c771aab394d590cd4c1a`.
A fresh, clean independent clone at starting commit `768fb90` reproduces the
**same failure, exact hash, and nine passes on both targets**, without any #180
changes. Thus this is another stale expectation, not a formatter delta or a
#178 cross-target discrepancy. Its
[separate follow-up](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5717231525)
is also on #85; its cause and any approval-bearing refresh remain follow-up work.

```sh
dagger check excalidraw-render:packaging:public-facade excalidraw-render:wasm:public-facade
```

Current and clean-baseline logs are retained respectively as
`target/issue-180-packaged-facade.log` and `target/issue-180-baseline-facade.log`.

## Semantic resource and browser qualification

The complete native and pinned-browser trace collection executes all 168 keys,
twice per process order, on each target: **1,344 traced renders**. The validated
join to the packaged public results has **zero public/test-build disagreements**.
Exactly **165/168 trace keys agree** across targets. The three unequal keys are
PNG for freedraw-loop-fills, legacy-stroke-sharpness, and
rough-fillers-dashed-zigzag-line: the pre-existing #178 output-length differences.
The cross-target artifact comparison retains exactly #178's four PNG keys;
all reports agree. The three stale approved-baseline expectations above agree
between the current targets and introduce no additional cross-target difference.

`verify-joined-results target/issue-180-results` still rejects the combined
comparison, as required while #178 remains open. No resource charge or
accounting code changed. The existing focused resource-boundary tests pass in
the workspace suite, and the complete trace/public join qualifies both targets'
current semantic outcomes without weakening the equality gate.

Recorded output digests:

- `comparison.json`: `28e91aa26f03ad5ab2b213dc3a65d2d17f66b585bdc818a3aa96a161fa88a977`.
- `results.json`: `f5d16778603fefd666ef70b87d481a9765f054960e78e8ea696068dff93529c2`.
- Number oracle fixture: `1a4c90edecbf5b82a1e95a0a10c5dac75d0aad27910bf1b32651550b07c3df1e`.

`dagger check excalidraw-render:wasm:qualification` passes, including the new
`number_formatting_matches_pinned_chromium_vectors` test and the existing
font/resource qualification. This executes the same 595 oracle vectors in
release-mode browser WASM. Log: `target/issue-180-wasm-qualification-check.log`.

## Workspace checks

- Repeated `cargo check --locked -p excalidraw-render` and final
  `cargo check --locked --workspace --all-targets` pass.
- All six number-format tests pass. Focused rectangle SVG (6), freedraw (9),
  and Rough-selected library tests (18) pass with existing expectations.
- The complete default workspace suite covers **1,108 tests**. Its initial
  `cargo nextest run --locked --workspace --test-threads 2` passed 1,029 before
  the evidence-integrity test correctly rejected the temporary `dagger.dang`
  packaging override. After removing the override, exactly the failed test and
  78 unrun tests passed; the successful tests were not repeated. The resumed
  selection is below. All 1,108 tests are verified, with the profile's 8 ignored
  tests and external viewer harness binaries excluded as usual.
- `cargo test --locked --workspace --doc` and `cargo fmt --all --check` pass.
- All **ten canonical Dagger conformance/lint checks pass**, including pinned
  Rust 1.97.1 Clippy, comparator/viewer evidence, catalog, inventory, ownership,
  registered cases and font-subset release evidence. Command:
  `dagger check excalidraw-render:conformance excalidraw-render:lint`.
  Log: `target/issue-180-conformance.log`.
- The local toolchain is Rust 1.98.1. Its Clippy rejects two pre-existing
  `chunks_exact`/`chunks_exact_mut` calls in `render/png/paint.rs`; the repository's
  canonical lint toolchain is 1.97.1. Local WASM checking also lacks the installed
  target; browser builds execute in Dagger's pinned environment instead.

```sh
cargo nextest run --locked --workspace --test-threads 2 --no-fail-fast \
  -E 'binary_id(=xtask::bin/xtask) | binary_id(=xtask::svg_viewer_verify) | (binary_id(=xtask::svg_font_subset_targets) & test(=verifies_the_committed_svg_font_subset_release_evidence_offline))'
```

## Review

Two independent review axes examined the #180 diff against `768fb90`.
Standards review found no hard violations and one low-priority suggestion to
share the two tests' fixture-loading boilerplate; the assertions remain separate
and no additional abstraction was introduced. Spec review found no production
defect or scope creep; its pending qualification finding is addressed by the
completed browser, trace, packaged-baseline comparison and workspace results
above. No production code changed after these checks.
