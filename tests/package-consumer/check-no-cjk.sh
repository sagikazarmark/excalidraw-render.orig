#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${1:-}"
if [[ -z "${TARGET}" ]]; then
  echo "usage: $0 <rust-target>" >&2
  exit 2
fi

# The consumer's own workspace resolves only unpacked publication archives.
# Cargo.lock may retain optional packages; the active dependency graph must not.
cd "${SCRIPT_DIR}"
graph="$(cargo tree --locked --target "${TARGET}" --no-default-features \
  --edges normal,build,dev --prefix none --format '{p}')"
while IFS= read -r package; do
  if [[ "${package}" == excalidraw-fonts-xiaolai\ * || "${package}" == excalidraw-fonts-xiaolai-extended\ * ]]; then
    echo "no-CJK consumer unexpectedly depends on ${package}" >&2
    exit 1
  fi
done <<< "${graph}"
echo "verified no CJK payload crates in the consumer dependency graph"

cargo test --locked --release --target "${TARGET}" \
  --no-default-features --test optional_cjk
