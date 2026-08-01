# Deterministic Brotli fork

This directory is a controlled fork of `brotli 8.0.4` for SVG font-subset WOFF2 encoding.

The upstream quality-11 encoder calls target-provided `log2` and `powf` operations. Qualification issue #98 demonstrated different native and `wasm32-unknown-unknown` Brotli streams for byte-identical sfnt input. This fork uses the upstream crate's existing table/integer `FastLog2u64` and repeated-squaring `FastPow2` implementations on every target. No other behavior is changed.
