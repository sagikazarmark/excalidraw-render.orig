# PNG coverage reuse experiment

Standalone, non-production Zeno 0.3.3 mask probe. The existing tiny-skia-path
0.11.4 stroker expands bounded 100×100 paths; Zeno rasterizes those outlines as
nonzero fills. Output samples are compared with the independently captured
pinned Chromium canvas controls, with at most one alpha-unit difference.
This is an early rejection screen, not visual, resource or WASM qualification.

```sh
cargo run --locked --manifest-path tools/representative-intake/raster-reuse/Cargo.toml -- \
  tests/conformance/proposals/velox84-png-isolation-v1/canvas-stroke.json \
  /tmp/zeno-stroke-reuse.json
```

The command refuses to overwrite its report. The recorded native run passes
17/27 sampled controls. Ten horizontal-edge phase controls fail, including
Velox's phase. No production dependency or renderer behavior changed.

See `docs/research/png-analytic-raster-upstream-reuse.md` for source research and
`docs/research/png-edge-correction-attempts.md` for the failed adapter trials.
