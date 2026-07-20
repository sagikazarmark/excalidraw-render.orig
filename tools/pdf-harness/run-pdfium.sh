#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if [[ "${1:-}" != "render" || $# -ne 5 ]]; then
  echo "usage: $0 render <input.pdf> <output.rgba> <width> <height>" >&2
  exit 2
fi
if [[ "$(python3 --version)" != "Python 3.12.3" ]]; then
  echo "PDFium harness requires Python 3.12.3" >&2
  exit 1
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

PYTHONPATH="${REPO_ROOT}/target/pdf-harness/pdfium" \
  python3 "${SCRIPT_DIR}/pdfium/render.py" "${INPUT}" "${OUTPUT}" "${4}" "${5}"
