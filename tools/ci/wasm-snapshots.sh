#!/usr/bin/env bash
set -euo pipefail

mkdir -p /wasm-snapshots
rustc --version > /wasm-snapshots/rustc.txt
git rev-parse HEAD > /wasm-snapshots/commit.txt
git status --porcelain --untracked-files=all > /wasm-snapshots/source-status.txt
sha256sum ../../target/package/*.crate > /wasm-snapshots/packages.sha256
# Each cargo test invocation launches a separate runner/browser/WASM instance.
for order in forward reverse; do
  cargo test --locked --release --target wasm32-unknown-unknown \
    --test generated_snapshots "snapshot_${order}" -- --nocapture \
    2>&1 | tee "/wasm-snapshots/${order}.log"
done
