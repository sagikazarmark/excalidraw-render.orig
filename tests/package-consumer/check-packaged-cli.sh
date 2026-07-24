#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET="${1:-}"

if [[ -z "${TARGET}" ]]; then
  echo "usage: $0 <rust-target>" >&2
  exit 2
fi

cd "${REPO_ROOT}"
cargo check \
  --target "${TARGET}" \
  --manifest-path target/package/excalidraw-render-cli-0.1.0/Cargo.toml \
  --config 'patch.crates-io.excalidraw-render.path="target/package/excalidraw-render-0.1.0"' \
  --config 'patch.crates-io.excalidraw-render-assets-base.path="target/package/excalidraw-render-assets-base-0.1.0"' \
  --config 'patch.crates-io.excalidraw-render-assets-data-000.path="target/package/excalidraw-render-assets-data-000-0.1.0"' \
  --config 'patch.crates-io.excalidraw-render-assets-data-001.path="target/package/excalidraw-render-assets-data-001-0.1.0"'
