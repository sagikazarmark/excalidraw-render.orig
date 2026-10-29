# Resource recovery: native and browser-WASM execution

## Producer and execution

Executed the clean committed renderer at
`aafed1ddd7fd3d08f2d980e597674e248fa0e6df`
(`complete derived coordinate admission and local recovery`) with the
repository's Dagger runner, Rust 1.97.1, pinned Chromium, packaged crate
archives, and unchanged inventory/tool/lock bindings.

```sh
dagger --progress=report api call full-results --output target/resource-recovery-results
target/debug/xtask conformance verify-joined-results target/resource-recovery-results
target/debug/xtask conformance replay-native-snapshots target/resource-recovery-results/native-snapshots
dagger --progress=report check excalidraw-render:packaging:public-facade excalidraw-render:wasm:public-facade
```

Collection completed in 28m36s; both packaged-facade checks passed (native
4m32s, browser WASM 6m38s). Every one of the 168 inventory keys executed in
forward and reverse processes/browser invocations, twice per fresh-session
sample, for both public snapshots and test-only traces. Each collection covers
1,344 renders. The exported logs, package provenance, artifacts and complete
comparison are under `target/resource-recovery-results/`.

## Results

- Native packaged replay matches **168/168** approved artifact/report pairs.
- Public versus traced-build outcome/report comparisons have **zero
  disagreements**.
- Native/browser-WASM artifact/report equality holds for **164/168** keys.
- Trace equality holds for **165/168** keys.
- The combined equality verifier **fails**, as required by its contract.

The four artifact differences are PNG for:

1. `focused.freedraw-loop-fills.v1`
2. `focused.legacy-stroke-sharpness.v1`
3. `focused.rotated-shape-bounds.v1`
4. `focused.rough-fillers-dashed-zigzag-line.v1`

The three trace differences are the first, second and fourth entries: their
different PNG byte lengths also change artifact-retention accounting. The
comparison contains 16 artifact/report difference records and 12 trace
difference records because each key repeats across sessions and orders.

This is the same affected key set documented under upstream-blocked **#178** in
[trace-collection.md](../trace-collection.md#release-order-diagnosis-and-fix).
No new differing key was observed. Successful collection and packaged-facade
tests do not turn that equality failure into a passing release gate. No baseline,
comparison tolerance, or exception pin was changed.

The diagnostic ownership additions following this run are consuming tests and
ledger/audit updates. This execution identifies the committed renderer above;
it does not claim execution of later uncommitted source changes.
