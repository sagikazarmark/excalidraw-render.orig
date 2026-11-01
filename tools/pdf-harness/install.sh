#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 0 ]]; then
  echo "usage: $0" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_ROOT="${REPO_ROOT}/target/pdf-harness"
IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"

if [[ -L "${REPO_ROOT}/target" ]]; then
  echo "repository target directory must not be a symlink" >&2
  exit 1
fi
if [[ -L "${TARGET_ROOT}" ]]; then
  echo "PDF harness target directory must not be a symlink" >&2
  exit 1
fi
# The devenv shell links the pinned PDFium and qpdf here (devenv.nix).
if [[ ! -f "${TARGET_ROOT}/pdfium/VERSION" || ! -x "${TARGET_ROOT}/qpdf/bin/qpdf" ]]; then
  echo "PDFium and qpdf come from the devenv shell; enter it first" >&2
  exit 1
fi
if [[ "$(realpath "${TARGET_ROOT}")" != "${REPO_ROOT}/target/pdf-harness" ]]; then
  echo "PDF harness target directory resolves outside the repository" >&2
  exit 1
fi

cargo build --locked --release --manifest-path "${SCRIPT_DIR}/pdfium-rust/Cargo.toml" \
  --target-dir "${TARGET_ROOT}/pdfium-rust"

docker run --rm --init --network bridge --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/pdf-harness/pdfjs \
  "${IMAGE}" npm ci --no-audit --no-fund
