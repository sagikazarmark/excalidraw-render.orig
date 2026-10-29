# Full-inventory WASM snapshot replay: findings

Remaining PNG difference and dependency decision:
[#178](https://github.com/sagikazarmark/excalidraw-render.orig/issues/178).

## Diagnosis and ICO fix

The first full replay of the 168 inventory keys in browser WASM disagreed with
the native baseline on seven keys: the three image-codecs formats failed with
`resource.total-work-exceeded`, and four PNGs differed by a few pixels.

The ICO failures were fixed in `render/asset/ico.rs`: `validation_work` widened
the directory count to u64, then subtracted its leading-zero count from
`usize::BITS`. On wasm32 that underflows (or panics with overflow checks), charging
approximately 2^32 work units for a one-entry icon. Using `u64::BITS` preserves
native accounting and makes it identical on wasm32. A target-enabled regression
checks directory counts 1, 2, 3, 4 and 255. After this fix the replay has 164
matching keys and only the four PNG differences below.

The PNG differences are actual one-channel pixel differences, not compression:
freedraw-loop-fills differs at two pixels, legacy-stroke-sharpness at one,
rotated-shape-bounds at two, and dashed/zigzag-line fillers at two. Every channel
delta is one. Enabling wasm simd128 did not resolve any of these differences and
was removed after the experiment.

Root cause isolated to tiny-skia-path's `path_geometry.rs` cubic polynomial
root solving (`solve_cubic_poly`, `scalar_cube_root`): its std `acos`, `cos` and
`powf` dispatch to host math. In an isolated native probe with Rust 1.97.1 and
the package-consumer lock, replacing only those calls with pinned libm calls
produces **all four exact WASM PNG hashes**. Removing the patch restores **all
four exact native hashes**. The probe lives outside the repository; no
dependency fork or pixel baseline changes have been applied to production.

ADR-0007's assumption that tiny-skia SIMD is disabled also does not describe the
resolved graph: resvg requests tiny-skia defaults, which unify SIMD on. That is
a separate audit correction; it was not the cause of these four pixels.

Completing the PNG fix requires a portable-math path inside tiny-skia-path (an
upstream change) and a native baseline restage, because the portable result
changes native pixels. Disabling a dependency's std/default features at a
direct edge cannot undo transitive Cargo feature unification.

## Current state

| Case (default variant) | Format | Native | WASM |
| --- | --- | --- | --- |
| focused.freedraw-loop-fills.v1 | PNG | 16,105 bytes | 16,108 bytes |
| focused.legacy-stroke-sharpness.v1 | PNG | 22,117 bytes | 22,116 bytes |
| focused.rotated-shape-bounds.v1 | PNG | 7,580 bytes | 7,580 bytes (pixels differ) |
| focused.rough-fillers-dashed-zigzag-line.v1 | PNG | 118,390 bytes | 118,395 bytes |

All four compatibility reports match the native report. The 0.1.0 release
policy scopes these as a tracked experimental-WASM limitation: `just
wasm-snapshot-replay` and `just full-equality` accept exactly the differences
recorded in `tests/package-consumer/experimental-wasm.json` and fail on any
other, or on a change in these. Strict equality stays available as a
qualification goal (`verify-joined-results`).

## Reproduce

```sh
just wasm-snapshot-replay   # logs under target/release-evidence/wasm-snapshots
```

The run exports `forward.log`, `reverse.log`, `replay.json`, compiler, browser,
Git commit/source-status records, and package archive digests. Its input digest
binds the inventory, scenes/options, native collector, WASM harness, consumer
manifest and lockfile, runner script, and actual tracked/untracked renderer
source bytes plus workspace Cargo files. `report-wasm-snapshots` materializes
the comparison, and `tools/ci/release-results.py snapshots` is the 0.1.0 gate
over it; `verify-wasm-snapshots` is the strict form that rejects any mismatch.
