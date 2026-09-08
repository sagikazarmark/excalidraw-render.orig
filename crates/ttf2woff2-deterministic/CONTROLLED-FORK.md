# Deterministic ttf2woff2 adapter

This directory is a controlled fork of `ttf2woff2 0.13.2`. It exists only to reach the deterministic brotli and retires once an upstream release carries the `portable-float` passthrough; see ADR-0003 (`docs/adr/0003-retire-the-brotli-fork-for-upstream-portable-float.md`).

## What differs from upstream

- **Package identity**: name `excalidraw-render-ttf2woff2`, version `0.1.0`, this repository as `repository`, and a description naming the adapter; `homepage` and `documentation` still point at upstream. The library keeps the crate name `ttf2woff2` so callers read `ttf2woff2::encode_with_options`.
- **Dependencies**: upstream `brotli =9.0.0` with `default-features = false` and the `portable-float`, `std` and `disable-timer` features (upstream depends on `brotli ^8` with platform floating-point); `byteorder` and `thiserror` pinned to exact versions.
- **CLI removed**: upstream's `src/main.rs`, its `cli` feature (upstream's default) and the optional `clap` dependency are gone. The vendoring had already dropped upstream's `[[bin]]` stanza, so the file was never compiled here. The workspace still consumes the crate with `default-features = false`: a no-op today, kept because the retirement swap to upstream needs exactly that line (ADR-0003) and because it keeps upstream's `default = ["cli"]` off if the fork is ever re-vendored.
- **Not carried**: upstream's `tests/` (see below), `scripts/generate_golden.py`, and `.github/workflows/` — GitHub Actions only reads the repository root's `.github/`, so the nested `ci.yml` and `release.yml` could never run and only travelled into the packaged crate.

Everything under `src/` other than the removed `main.rs` is byte-identical to the published `ttf2woff2 0.13.2` crate; `rustfmt.toml` is upstream's, and the fork is excluded from the workspace `fmt`/`clippy` gates to keep upstream formatting.

## What is built

Only the library (`src/lib.rs`, exposing `encode`, `encode_with_options`, `encode_no_transform`, `EncodeOptions`, `BrotliQuality`, `Error`). There is no binary target and no feature flag.

## Test coverage

Upstream's integration tests (`tests/encode_test.rs` and the `tests/fonttools_compat/` port of fonttools' `woff2_test.py`, 29 tests) are **not carried**. They were never compiled in this repository, and the fixtures they read — three OFL fonts (WarpnineSans, NotoSansJP-Medium, Recursive VF; ~8 MB) plus fonttools-generated reference WOFF2 files (upstream's `golden/`) — were excluded from the published crate this fork was vendored from and have never existed here. Their strongest assertion (output size within ±5 % of fonttools) is weaker than the byte-level qualification below, so the tree was deleted rather than wired up.

The fork's coverage is:

1. **Eleven in-module unit tests** under `src/woff2/` — `header.rs` (2: header size, big-endian signature), `tag.rs` (2: known-tag index, `is_*` predicates), `varint.rs` (2: Base128 and 255UInt16 encoders), `triplet.rs` (5: glyph-coordinate triplet encoding cases). These run on every push via `cargo nextest run --workspace` and directly via `cargo test -p excalidraw-render-ttf2woff2`. None of them exercises `encode*` end to end.

2. **The SVG font subset qualification corpus** (the renderer's WOFF2 qualification corpus) — `crates/excalidraw-render/src/render/font_subset_qualification.rs`. Every subset the renderer derives goes through `ttf2woff2::encode_with_options` at quality 11, `glyf`/`loca` transform, one thread (`crates/excalidraw-render/src/render/font_subset.rs`, `derive`), is immediately decoded with `wuff`, checksum-verified, and its canonicalized sfnt compared against the pre-encode subset. The corpus test then pins the SHA-256 of every generated WOFF2 against `crates/excalidraw-render/tests/qualification/font-subset-corpus.json`: at revision 1 that is 971 successful cells across all 230 official font chunks. The framed aggregate `generatedBytesSha256` is committed in `tests/conformance/svg-font-subset-cross-target-measurement/revision-1.json` and must agree between `x86_64-unknown-linux-gnu` and `wasm32-unknown-unknown` — this is the check that guards the `portable-float` pin. The corpus test is excluded by name from the push-gate nextest profiles (`.config/nextest.toml`) and runs in release mode on both targets in `.github/workflows/packaged-public-facade.yml`: natively with `--features release-qualification`, and in Chromium through the `wasm_tests` mirror `complete_font_subset_qualification_corpus_in_wasm` (`crates/excalidraw-render/src/lib.rs`).

3. **Fuzz regression corpus** — `crates/excalidraw-render/src/render/font_subset_fuzz.rs` replays `tests/qualification/font-subset-fuzz-regressions.json` and a fixed-seed smoke on every push, and a 4096-case nightly (`.github/workflows/svg-font-subset-fuzz.yml`); mutated sources are re-encoded with `ttf2woff2::encode` (quality 1) to produce WOFF2 inputs, and every case is derived twice with identical outcomes required.

Any change to this crate that alters output bytes will fail (2) before it ships; that, not the upstream compatibility suite, is the contract this fork is held to.
