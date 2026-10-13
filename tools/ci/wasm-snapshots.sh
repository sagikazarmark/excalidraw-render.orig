#!/usr/bin/env bash
set -euo pipefail

# Run from tests/package-consumer after `xtask conformance stage-wasm-snapshots`.
out="${1:?output directory required}"
mkdir -p "$out"
rustc --version > "$out/rustc.txt"
git rev-parse HEAD > "$out/commit.txt"
git status --porcelain --untracked-files=all > "$out/source-status.txt"
sha256sum ../../target/package/*.crate > "$out/packages.sha256"
# Each cargo test invocation launches a separate runner/browser/WASM instance.
for order in forward reverse; do
  cargo test --locked --release --target wasm32-unknown-unknown \
    --test generated_snapshots "snapshot_${order}" -- --nocapture \
    2>&1 | tee "$out/${order}.log"
done
