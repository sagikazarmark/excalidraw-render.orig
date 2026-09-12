#!/usr/bin/env bash
set -euo pipefail
target="${1:?Rust target required}"
mkdir -p /traces
for order in forward reverse; do
  cargo test --locked --release -p excalidraw-render --lib --target "$target" \
    "trace_snapshot_${order}" -- --nocapture 2>&1 | tee "/traces/${order}.log"
done
