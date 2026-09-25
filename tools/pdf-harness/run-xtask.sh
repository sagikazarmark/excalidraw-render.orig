#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if [[ -n "${XTASK_PDF_HARNESS:-}" ]]; then
  exec "${XTASK_PDF_HARNESS}" pdf-harness "$@"
fi

# Structural callers also run inside cargo test, which holds its target lock.
# Keep fallback builds isolated and let Cargo's fingerprints check freshness;
# merely finding an existing binary is not sufficient after source changes.
exec cargo run --quiet --locked --manifest-path "${REPO_ROOT}/xtask/Cargo.toml" \
  --target-dir "${REPO_ROOT}/target/pdf-harness/xtask" --bin xtask -- pdf-harness "$@"
