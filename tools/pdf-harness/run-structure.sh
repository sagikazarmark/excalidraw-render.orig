#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if [[ "${1:-}" != "inspect" || $# -ne 5 ]]; then
  echo "usage: $0 inspect <input.pdf> <expectation.json> <pass|fail> <output.json>" >&2
  exit 2
fi
INPUT="$(realpath "${2}")"
EXPECTATION="$(realpath "${3}")"
OUTPUT_PARENT="$(realpath "$(dirname "${5}")")"
OUTPUT="${OUTPUT_PARENT}/$(basename "${5}")"
case "${INPUT}" in "${REPO_ROOT}"/*) ;; *) echo "input must be inside repository" >&2; exit 1;; esac
case "${EXPECTATION}" in "${REPO_ROOT}"/*) ;; *) echo "expectation must be inside repository" >&2; exit 1;; esac
case "${OUTPUT}" in "${REPO_ROOT}"/*) ;; *) echo "output must be inside repository" >&2; exit 1;; esac
if [[ -L "${5}" ]]; then
  echo "output path must not be a symlink" >&2
  exit 1
fi

QPDF="${REPO_ROOT}/target/pdf-harness/qpdf/bin/qpdf" \
  exec bash "${SCRIPT_DIR}/run-xtask.sh" inspect "${INPUT}" "${EXPECTATION}" "${4}" "${OUTPUT}"
