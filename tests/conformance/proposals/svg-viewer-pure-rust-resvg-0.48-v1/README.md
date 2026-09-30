# Pure-Rust SVG viewer on resvg 0.48.1: reviewed in Git

Status: applied and reviewed through normal Git review (ADR-0019). There is no
approval hash. The primary sources and what they establish are in
`docs/research/pure-rust-svg-viewer-primary-sources.md`, section "Requalification
on 0.48.1".

The controlled pure-Rust SVG viewer (`tools/svg-viewer-rust`, profile
`canonical-svg.pure-rust.v1`) moved from `resvg`/`usvg` 0.47.0 (commit
`3a0fdba5`) to 0.48.1 (commit `68b14c4c`). Text now uses skrifa 0.44 and
HarfRust 0.12 instead of ttf-parser and rustybuzz. Every package the viewer
lock shares with the upstream v0.48.1 lockfile carries upstream's exact version
and checksum. The viewer's source (`src/main.rs`) is unchanged, and its CLI
contract tests pass.

## Evidence

- **Rectangle SVG viewer qualification:** both canonical viewers reproduce the
  retained rasters byte for byte (`tools/svg-viewer-qualification.sh
  reproduce`). Only the manifest's profile binding and its sidecar moved.
- **SVG font subset qualification**, regenerated with
  `tools/svg-font-subset-qualification.sh generate`:
  - The Chromium report is byte-identical to the one it replaces, so the viewer
    inputs did not move.
  - All 662 pure-Rust executions pass, with source and subset rasters identical
    (0 changed pixels, SSIM 1) as before.
  - The report runner now reads and shapes each face with the viewer's skrifa
    and HarfRust, so every semantic digest moved. Source and subset digests
    still agree in every execution.
  - Four executions' rasters moved in both source and subset, which is a viewer
    rendering change, not a subset one. They are the `full-intersection` cells
    of two Comic Shanns and two Xiaolai chunks (3, 13, 273 and 904 changed
    pixels against 0.47).
  - Shaping those lines is identical under rustybuzz 0.20.1, HarfRust 0.12.0 and
    HarfRust 0.13.3. The changes come from usvg 0.48's own layout and outline
    code, around glyphs with zero or unusual advance: leading combining marks
    (U+0315, U+3099), spacing diacritics, and the Hangul filler U+1160.

## Frozen candidates

The stage4 representative candidates (v6 to v27) were measured under the 0.47
viewer and bind its profile, `Cargo.toml` and `Cargo.lock`. `before/` preserves
those preimages, and `PURE_RUST_SVG_REQUALIFICATIONS` in
`xtask/src/conformance_candidate_gate.rs` accepts such a binding only when it
names an archived file byte for byte and the live profile is the one this
requalification approved. New candidate evidence measures under the live
viewer.
