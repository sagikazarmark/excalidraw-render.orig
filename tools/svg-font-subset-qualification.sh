#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
INPUTS="target/svg-font-subset-viewer-qualification/inputs.json"
CHROMIUM_REPORT="target/svg-font-subset-viewer-qualification/chromium-report.json"
RUST_REPORT="target/svg-font-subset-viewer-qualification/pure-rust-report.json"
RUST_IMAGE="rust@sha256:1bcff4befb740599103a2c7cb51058e14479b2e35e3a34a3f0dc4ede09927488"
PLAYWRIGHT_IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
EVIDENCE_ROOT="${REPO_ROOT}/tests/conformance/svg-font-subset-viewer-qualification"

case "${1:-}" in
  generate)
    if [[ -e "${EVIDENCE_ROOT}/revision-1.json" ]]; then
      echo "generation refuses to replace manifested SVG font subset qualification evidence" >&2
      exit 1
    fi
    ;;
  reproduce) ;;
  *)
    echo "usage: $0 <generate|reproduce>" >&2
    exit 2
    ;;
esac

cd "${REPO_ROOT}"
cargo test --locked --release -p excalidraw-render --lib \
  'render::font_subset_qualification::materialize_complete_font_subset_viewer_inputs' \
  -- --ignored --exact

docker run --rm --init --ipc=host --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  --env HOME=/tmp \
  --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
  --env REFERENCE_ORACLE_CANONICAL=1 \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/reference-oracle \
  "${PLAYWRIGHT_IMAGE}" \
  node scripts/svg-font-subset-viewer.mjs "${INPUTS}" "${CHROMIUM_REPORT}"

docker run --rm --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  --env HOME=/tmp \
  --env CARGO_HOME=/usr/local/cargo \
  --env CARGO_TARGET_DIR=/workspace/target/svg-font-subset-rust-1.97.1 \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --mount "type=bind,src=${HOME}/.cargo/registry,dst=/usr/local/cargo/registry,readonly" \
  --workdir /workspace \
  "${RUST_IMAGE}" bash -c \
  'cargo build --release --locked -p xtask && cargo build --release --locked --manifest-path tools/svg-viewer-rust/Cargo.toml && target/svg-font-subset-rust-1.97.1/release/xtask conformance generate-svg-font-subset-rust-report target/svg-font-subset-viewer-qualification/inputs.json target/svg-font-subset-rust-1.97.1/release/excalidraw-render-svg-viewer target/svg-font-subset-viewer-qualification/pure-rust-report.json'

if [[ "$1" == "generate" ]]; then
  cargo run --quiet --locked -p xtask -- conformance materialize-svg-font-subset-evidence \
    "${INPUTS}" "${CHROMIUM_REPORT}" "${RUST_REPORT}"
else
  cmp "${REPO_ROOT}/${CHROMIUM_REPORT}" "${EVIDENCE_ROOT}/chromium-report.json"
  cmp "${REPO_ROOT}/${RUST_REPORT}" "${EVIDENCE_ROOT}/pure-rust-report.json"
  cargo run --quiet --locked -p xtask -- conformance verify-svg-font-subsets
fi

echo "$1 completed dual-viewer SVG font subset qualification"
