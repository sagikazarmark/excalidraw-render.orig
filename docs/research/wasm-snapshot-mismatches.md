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
policy scopes these as a tracked experimental-WASM limitation: the golden
test in browser WASM (`just golden-wasm`) accepts exactly the differences
recorded in `tests/conformance/golden/experimental-wasm.json` and fails on any
other, on a change in these, or on a recorded difference that no longer
appears.

## Reproduce

```sh
devenv --profile browser shell -- just golden-wasm
```

The test renders every golden target forward and reverse in a dedicated
worker and compares each with `tests/conformance/golden/native.json`; a
failure prints the keys that differ with the recorded and observed bytes.
(The original replay ran the packaged facade through a staged harness and a
Python gate; that machinery retired with the 2026-11-01 testing review.)
