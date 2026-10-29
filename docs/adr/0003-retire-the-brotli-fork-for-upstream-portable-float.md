---
status: implemented 2026-09-11; the brotli, ttf2woff2 and skera forks are gone
---

# Retire the brotli fork in favor of upstream `portable-float`

`crates/brotli-deterministic` existed because released rust-brotli 8.0.4 lets
the platform choose floating-point evaluation order, which breaks byte-identical
WOFF2 output across the supported targets. Upstream PR #259 adds a
`portable-float` feature that fixes this in tree, so the fork stops being
necessary once a release carries it. Two more controlled forks existed only to
reach the deterministic brotli or to guard a since-fixed upstream panic:
`crates/ttf2woff2-deterministic` (a `src/` identical to released ttf2woff2
0.13.2) and the skera safety adapter.

## Decision

Retire the forks in three stages, and refresh the subset profile's identity
tokens exactly once.

- **Stage A (branch only).** Point `ttf2woff2-deterministic` at
  `brotli = { git, rev = "fb3104d", features = ["std", "portable-float"] }`
  with the profile identity string unchanged, and prove byte identity:
  `catalog verify` plus the font-subset qualification on native and WASM, and
  a scripted mechanical diff of the fork against upstream. The branch never
  merges; it only banks the proof.
- **Stage B.** Request an upstream rust-brotli release containing PR #259.
- **Stage C (on release).** Pin the released version, delete the fork, and
  perform exactly one identity-string plus full evidence refresh. The identity
  string feeds every subset font's name table, so it regenerates all
  font-subset evidence and must never be refreshed twice; spec and profile
  edits land in the same change-set, and all other evidence-touching work
  freezes during it. Re-run the mechanical diff against the released tag and
  compare decompressed sfnt tables (excluding the rewritten name table) before
  and after, so the identity-string delta is isolated from any encoder delta.

A swap whose bytes are proven identical merges ahead of the refresh: no
evidence moves and `catalog verify` holds, at the cost that the name tables
keep claiming the fork's token until the refresh. Depending on a fork by git
in the meantime is rejected: `cargo package` normalizes a git dependency to
its registry version, where the feature does not exist, so packaging fails on
every push.

## What the evidence showed

The mechanical diff against released 8.0.4, and later against the 9.0.0 tag
(`a51b65e`), reports one substantive divergence on the encoder path:
`src/enc/util.rs`, where `log2` and `powf` are replaced by software
implementations so evaluation order cannot vary by platform (`FastLog2` →
`FastLog2u64`, `FastPow2` → repeated squaring). That is exactly the code
upstream selects under `portable-float`; the rest of `src/enc/` differs by a
`to_int` → `to_simd` rename, import and `cfg` reshuffles, and added test
modules, and `bin/`, `concat/`, `ffi/` are not on the renderer's path.

Each swap reproduced the fork's output byte for byte before it merged: on
`x86_64-unknown-linux-gnu`, `catalog verify` passed and every per-cell
`generatedWoff2Sha256` pin and both cross-target measurement digests were
unchanged; on `wasm32-unknown-unknown`, the complete corpus qualification
emitted the same digests in a browser. Running the WASM leg with CI's exact
tooling surfaced one defect in the gate itself: the measurement line was
emitted with `println!`, which `wasm32-unknown-unknown` discards, so the
target aggregate could not assemble from a WASM report. A `console_log!`
route fixed it as part of the refresh.

## Outcome

- Stage B closed on 2026-09-02: upstream released PR #259 as **rust-brotli
  9.0.0**. The major bump is carried by the dependency closure
  (`brotli-decompressor ~6.0`, `alloc-no-stdlib 3`, `alloc-stdlib 0.3`), not
  the encoder; the pin is `default-features = false, features = ["std",
  "portable-float", "disable-timer"]`.
- The brotli fork and `tools/brotli-fork-diff.sh` were deleted in the same
  commit, identity string unchanged; both diffs are reproducible from the
  parent commit.
- Upstream released **ttf2woff2 0.13.3** on 2026-09-10 with
  [0x6b/ttf2woff2#28](https://github.com/0x6b/ttf2woff2/pull/28) (`brotli =
  "9"` plus a `portable-float` passthrough), byte-identical in `src/` to the
  fork less a CLI entry point the fork had already removed (#134). The
  ttf2woff2 fork retired the same day, the same way, with
  `default-features = false` keeping upstream's `cli` feature and `clap` out.
- Upstream **skera 0.7.0** carried
  [googlefonts/fontations#1996](https://github.com/googlefonts/fontations/pull/1996),
  the malformed-name-record panic fix, so the skera adapter and its packaging
  patches retired on 2026-09-10 as well. Upstream can omit a malformed name
  table instead of failing immediately, but the renderer's name rewrite
  requires that table and preserves the typed `Sfnt` failure; the existing
  malformed-name regression passes on both targets. The skera release removed
  `traversal`, which the bounded layout validator had used; ADR-0020 replaced
  that validator with a typed walk (#210), so the workspace has no direct
  read-fonts dependency.
  [`skera-upstream-name-record-fix.md`](../research/archive/skera-upstream-name-record-fix.md)
  records that verification.
- The single refresh completed on 2026-09-11 (`e6f1344`, `3f06c91`): all three
  identity tokens name their upstream implementations, spec §8.3.3 names the
  same versions, the corpus, catalog, cross-target measurement and both viewer
  qualifications were regenerated once, and the target aggregate was bound
  into release evidence. The publication block and the evidence-change freeze
  this decision imposed are lifted.

Later dependency moves (ttf2woff2 0.14.1, skera 0.8.0, wuff 0.2.9 on
2026-10-27) are ordinary profile-token changes, not fork retirements; spec
§8.3.3 and the subset profile name the running versions.
