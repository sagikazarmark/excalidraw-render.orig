# Full-inventory WASM snapshot replay: initial findings

Remaining PNG defect and dependency decision:
[#178](https://github.com/sagikazarmark/excalidraw-render.orig/issues/178).

## Diagnosis and ICO fix

The ICO failures are fixed in `render/asset/ico.rs`: `validation_work` widened
the directory count to u64, then subtracted its leading-zero count from
`usize::BITS`. On wasm32 that underflows (or panics with overflow checks), charging
approximately 2^32 work units for a one-entry icon. Using `u64::BITS` preserves
native accounting and makes it identical on wasm32. A target-enabled regression
checks directory counts 1, 2, 3, 4 and 255. The complete default-WASM replay after
this fix has 164 matching keys and only the four PNG mismatches below; all three
image-codec formats now render with the approved native bytes and reports.

The PNG mismatches are actual one-channel pixel differences, not compression:
freedraw-loop-fills differs at two pixels, legacy-stroke-sharpness at one,
rotated-shape-bounds at two, and dashed/zigzag-line fillers at two. Every channel
delta is one. Enabling wasm simd128 did not resolve any of these differences and
was removed after the experiment.

Root cause isolated to tiny-skia-path 0.11.4 `path_geometry.rs` cubic polynomial
root solving (`solve_cubic_poly`, `scalar_cube_root`): its std `acos`, `cos` and
`powf` dispatch to host math. In an isolated native probe with Rust 1.97.1 and
the package-consumer lock, replacing only those calls with pinned libm calls
produces **all four exact WASM PNG hashes**. Removing the patch restores **all
four exact approved native hashes**. The probe lives outside the repository;
no dependency fork or pixel baseline changes have been applied to production.

ADR-0007's assumption that tiny-skia SIMD is disabled also does not describe the
resolved graph: resvg/svg2pdf request tiny-skia defaults, which unify SIMD on.
That is a separate audit correction; it was not the cause of these four pixels.

Completing the PNG fix requires a qualified portable-math dependency path
(upstream change or controlled fork) and native snapshot reapproval, because the
portable result changes native pixels. Disabling a dependency's std/default
features at a direct edge cannot undo transitive Cargo feature unification.
The four PNG differences remain release-blocking; no baseline was silently repinned.

The browser collector implemented after `f737ef1` executes all 168 inventory
keys, twice per key, in separate forward/reverse wasm-bindgen-test runner and
Chromium dedicated-worker invocations. The module depends on the packaged public
facade and reuses the approved native collector's request mapping verbatim.
Chromium/ChromeDriver are pinned at 149.0.7827.55, Rust at 1.97.1, runner at
0.2.126. The run is a working-tree measurement, not a clean-commit release proof.

## Result

All 672 WASM render calls terminate; all session repeats and process orders agree.
161 of 168 keys match the approved native artifact/report baseline. Seven keys
disagree in both orders, so `wasm:snapshot-replay` fails as required:

| Case (default variant) | Format | Native | WASM |
| --- | --- | --- | --- |
| focused.freedraw-loop-fills.v1 | PNG | 16,105 bytes, `9d5d54c0…704c` | 16,108 bytes, `69ae2293…f830` |
| focused.legacy-stroke-sharpness.v1 | PNG | 22,117 bytes, `a9d5059c…1085` | 22,116 bytes, `aac82fa6…d6d1` |
| focused.rotated-shape-bounds.v1 | PNG | 7,580 bytes, `51ebdc14…6621` | 7,580 bytes, `145087cf…5a1` |
| focused.rough-fillers-dashed-zigzag-line.v1 | PNG | 118,390 bytes, `4126e39c…3877` | 118,395 bytes, `997bde1e…f61` |
| focused.image-codecs.v1 | SVG | artifact, 11,844 bytes | resource.total-work-exceeded |
| focused.image-codecs.v1 | PNG | artifact, 42,819 bytes | resource.total-work-exceeded |
| focused.image-codecs.v1 | PDF | artifact, 9,352 bytes | resource.total-work-exceeded |

All seven compatibility reports still match the native empty report (37 bytes,
`1f3ef3870c85fb0e7d2b51453022c07aeb8ac52c6d60ce82cf9a6320b538798b`).
Full digest pairs, failure-descriptor digests and both execution orders are in
the generated `replay.json`; successful browser tests alone do not mean native
parity. The collector records outcomes, and the host verifier makes that judgment.

## Reproduce and preserve evidence

```sh
# Automatic gate: rejects any mismatch.
dagger check excalidraw-render:wasm:snapshot-replay
# Export completed execution logs and a report even when parity fails.
dagger api call wasm browser-snapshot-logs --output target/wasm-snapshot-logs
```

The second operation exports `forward.log`, `reverse.log`, `replay.json`, compiler,
Git commit/source-status records, and package archive digests. The report has
`decision: fail` and 14 per-order differences. Its input digest binds the inventory,
scenes/options, native collector, WASM harness, tool manifest, consumer manifest
and lockfile, runner script, and actual tracked/untracked renderer source bytes
plus workspace Cargo files. `verify-wasm-snapshots` fails on that same report;
`report-wasm-snapshots` only materializes diagnostics and is never the CI gate.

These are newly exposed renderer/target discrepancies, not approved baseline
changes. Root causes are not diagnosed by this collection. Next investigation:
capture the differing PNG bytes and failure descriptors, separate pixel from
compression differences, and trace image-codec resource accounting across targets.
Keep the native baseline and limits unchanged while diagnosing. Full trace
collection and global coverage closure remain open under #85/#86.
