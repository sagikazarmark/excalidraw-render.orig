---
status: accepted
---

# Retire the brotli fork in favor of upstream `portable-float`

`crates/brotli-deterministic` exists because released rust-brotli 8.0.4 lets the
platform choose floating-point evaluation order, which breaks byte-identical
WOFF2 output across the five supported targets. Upstream PR #259 adds a
`portable-float` feature that fixes this in tree, so the fork stops being
necessary once a release carries it.

Retirement runs in three stages.

**Stage A (branch only).** Point `ttf2woff2-deterministic` at
`brotli = { git, rev = "fb3104d", default-features = false, features = ["std", "portable-float"] }`
with the profile identity string unchanged, and prove byte-identity: `catalog
verify` plus `cargo test --lib font_subset` on native and wasm, and a green
five-target aggregate at unchanged pins. Because `cargo package --locked
--workspace` runs in CI on every push and a git dependency cannot be packaged,
the stage-A branch comments out the packaging steps so the target jobs reach the
aggregate. The branch never merges; it only banks the proof, together with a
scripted mechanical diff of the fork against upstream 8.0.4.

**Stage B.** Request an upstream rust-brotli release containing PR #259.

**Stage C (on release).** Pin `=8.0.x`, delete `crates/brotli-deterministic`,
and perform exactly one identity-string plus full evidence refresh — the
identity string feeds every subset font's name table, so it regenerates all
font-subset evidence and must never be refreshed twice. Spec and profile edits
land in the same change-set, all other evidence-touching work freezes during it,
and stage C is serialized behind the ADR-0002 work. At stage C, re-run the
mechanical diff against the released tag and compare decompressed sfnt tables
(excluding the rewritten name table) before and after the refresh, so the
identity-string delta is isolated from any encoder delta.

If no upstream release lands by 2026-12-01, the fallback is a zero-patch
re-vendor of upstream at `fb3104d`. It is not free: the identity token becomes
`excalidraw-render-brotli-0.2.0(upstream-fb3104d)` — the current token claims
`upstream-8.0.4`, which an `fb3104d` snapshot is not — and it consumes the same
single evidence refresh.

Interim stripping of the fork's unused surface (ffi, concat, bin, threading) is
deliberately not done: it is wasted effort if the upstream release lands, and is
resurrected only as part of the fallback.

First crates.io publication is blocked until stage C or the fallback completes,
because git dependencies cannot be published and `portable-float` does not exist
in released 8.0.4. That block is a consequence of this decision, not a technical
limit — a vendored crate is publishable, and CI packages the fork today.
