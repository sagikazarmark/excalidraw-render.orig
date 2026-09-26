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
for package in \
  excalidraw-render-cli \
  excalidraw-render \
  excalidraw-fonts \
  excalidraw-fonts-cjk-000 \
  excalidraw-fonts-cjk-001
do
  archive="target/package/${package}-0.1.0.crate"
  if [[ ! -f "${archive}" ]]; then
    echo "missing packaged crate archive: ${archive}" >&2
    exit 1
  fi
  tar -xzf "${archive}" -C target/package
done

cargo check \
  --target "${TARGET}" \
  --manifest-path target/package/excalidraw-render-cli-0.1.0/Cargo.toml \
  --config 'patch.crates-io.excalidraw-render.path="target/package/excalidraw-render-0.1.0"' \
  --config 'patch.crates-io.excalidraw-fonts.path="target/package/excalidraw-fonts-0.1.0"' \
  --config 'patch.crates-io.excalidraw-fonts-cjk-000.path="target/package/excalidraw-fonts-cjk-000-0.1.0"' \
  --config 'patch.crates-io.excalidraw-fonts-cjk-001.path="target/package/excalidraw-fonts-cjk-001-0.1.0"'
