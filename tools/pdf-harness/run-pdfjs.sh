#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"

if [[ "${1:-}" != "render" || $# -ne 5 ]]; then
  echo "usage: $0 render <input.pdf> <output.rgba> <width> <height>" >&2
  exit 2
fi

INPUT="$(realpath "${2}")"
OUTPUT_PARENT="$(realpath "$(dirname "${3}")")"
OUTPUT="${OUTPUT_PARENT}/$(basename "${3}")"
case "${INPUT}" in "${REPO_ROOT}"/*) ;; *) echo "input must be inside repository" >&2; exit 1;; esac
case "${OUTPUT}" in "${REPO_ROOT}"/*) ;; *) echo "output must be inside repository" >&2; exit 1;; esac
if [[ -L "${3}" ]]; then
  echo "output path must not be a symlink" >&2
  exit 1
fi

docker run --rm --init --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/pdf-harness/pdfjs \
  "${IMAGE}" node render.mjs \
  "/workspace/${INPUT#"${REPO_ROOT}/"}" "/workspace/${OUTPUT#"${REPO_ROOT}/"}" \
  "${4}" "${5}"
