#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_ROOT="${REPO_ROOT}/target/pdf-harness"
DOWNLOAD_ROOT="${TARGET_ROOT}/downloads"
QPDF_ARCHIVE="qpdf-12.3.2-bin-linux-x86_64.zip"
QPDF_SHA256="44f2c53bf784c0143128d80d2b9946e9793962c5bb403b75c0024cb4d8e346b9"
PDFIUM_WHEEL="pypdfium2-5.12.1-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
PDFIUM_SHA256="e10cbf41b21233ec5e20adfc170cf60edd77abead86a97dc708fff55a8a886c7"
IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "PDF harness installation requires Linux x86_64" >&2
  exit 1
fi
if [[ "$(python3 --version)" != "Python 3.12.3" ]]; then
  echo "PDF harness installation requires Python 3.12.3" >&2
  exit 1
fi
if [[ -L "${REPO_ROOT}/target" ]]; then
  echo "repository target directory must not be a symlink" >&2
  exit 1
fi
mkdir -p "${REPO_ROOT}/target"
if [[ "$(realpath "${REPO_ROOT}/target")" != "${REPO_ROOT}/target" ]]; then
  echo "repository target directory resolves outside the repository" >&2
  exit 1
fi
if [[ -L "${TARGET_ROOT}" ]]; then
  echo "PDF harness target directory must not be a symlink" >&2
  exit 1
fi
if [[ -L "${DOWNLOAD_ROOT}" ]]; then
  echo "PDF harness download directory must not be a symlink" >&2
  exit 1
fi
mkdir -p "${DOWNLOAD_ROOT}"
if [[ "$(realpath "${TARGET_ROOT}")" != "${REPO_ROOT}/target/pdf-harness" ]]; then
  echo "PDF harness target directory resolves outside the repository" >&2
  exit 1
fi
if [[ "$(realpath "${DOWNLOAD_ROOT}")" != "${REPO_ROOT}/target/pdf-harness/downloads" ]]; then
  echo "PDF harness download directory resolves outside the repository" >&2
  exit 1
fi

fetch() {
  local url="$1"
  local output="$2"
  local expected="$3"
  if [[ ! -f "${output}" ]]; then
    curl --fail --location --output "${output}" "${url}"
  fi
  printf '%s  %s\n' "${expected}" "${output}" | sha256sum --check --status
}

fetch \
  "https://github.com/qpdf/qpdf/releases/download/v12.3.2/${QPDF_ARCHIVE}" \
  "${DOWNLOAD_ROOT}/${QPDF_ARCHIVE}" "${QPDF_SHA256}"
rm -rf "${TARGET_ROOT}/qpdf"
unzip -q "${DOWNLOAD_ROOT}/${QPDF_ARCHIVE}" -d "${TARGET_ROOT}/qpdf"
chmod 755 "${TARGET_ROOT}/qpdf/bin/qpdf" "${TARGET_ROOT}/qpdf/bin/fix-qdf" \
  "${TARGET_ROOT}/qpdf/bin/zlib-flate"
read -r qpdf_version < <("${TARGET_ROOT}/qpdf/bin/qpdf" --version)
if [[ "${qpdf_version}" != "qpdf version 12.3.2" ]]; then
  echo "installed qpdf version does not match the profile" >&2
  exit 1
fi

fetch \
  "https://files.pythonhosted.org/packages/23/16/21420a6f2bc5f981299c336817dd5d72709dad5fda30ac38cbd5f0f7b372/${PDFIUM_WHEEL}" \
  "${DOWNLOAD_ROOT}/${PDFIUM_WHEEL}" "${PDFIUM_SHA256}"
rm -rf "${TARGET_ROOT}/pdfium"
python3 -m zipfile -e "${DOWNLOAD_ROOT}/${PDFIUM_WHEEL}" "${TARGET_ROOT}/pdfium"
PYTHONPATH="${TARGET_ROOT}/pdfium" python3 -c \
  'from pypdfium2.version import PDFIUM_INFO, PYPDFIUM_INFO; assert str(PYPDFIUM_INFO) == "5.12.1"; assert str(PDFIUM_INFO) == "152.0.7947.0"'

docker run --rm --init --network bridge --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/pdf-harness/pdfjs \
  "${IMAGE}" npm ci --no-audit --no-fund
