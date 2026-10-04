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
# Font package versions carry the Excalidraw release as build metadata.
for stem in \
  excalidraw-render-cli-0.1.0 \
  excalidraw-render-0.1.0 \
  excalidraw-render-fonts-0.1.0+excalidraw.0.18.1 \
  excalidraw-render-fonts-xiaolai-0.1.0+excalidraw.0.18.1 \
  excalidraw-render-fonts-xiaolai-extended-0.1.0+excalidraw.0.18.1
do
  package="${stem%-0.1.0*}"
  archive="target/package/${stem}.crate"
  if [[ ! -f "${archive}" ]]; then
    echo "missing packaged crate archive: ${archive}" >&2
    exit 1
  fi
  # Common Xiaolai deliberately matches the published 165-chunk group. Only
  # that package exceeds the usual 9.5 MB archive budget; it must remain below
  # the stricter decimal 10 MB ceiling (also below the registry's 10 MiB).
  ceiling=9500000
  if [[ "${package}" == excalidraw-render-fonts-xiaolai ]]; then
    ceiling=10000000
  fi
  if (( $(wc -c < "${archive}") >= ceiling )); then
    echo "package archive exceeds exclusive ${ceiling}-byte ceiling: ${archive}" >&2
    exit 1
  fi
  tar -xzf "${archive}" -C target/package
done

cargo check \
  --target "${TARGET}" \
  --manifest-path target/package/excalidraw-render-cli-0.1.0/Cargo.toml \
  --config 'patch.crates-io.excalidraw-render.path="target/package/excalidraw-render-0.1.0"' \
  --config 'patch.crates-io.excalidraw-render-fonts.path="target/package/excalidraw-render-fonts-0.1.0+excalidraw.0.18.1"' \
  --config 'patch.crates-io.excalidraw-render-fonts-xiaolai.path="target/package/excalidraw-render-fonts-xiaolai-0.1.0+excalidraw.0.18.1"' \
  --config 'patch.crates-io.excalidraw-render-fonts-xiaolai-extended.path="target/package/excalidraw-render-fonts-xiaolai-extended-0.1.0+excalidraw.0.18.1"'
