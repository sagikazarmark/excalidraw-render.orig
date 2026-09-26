#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 1 || ($# -eq 1 && "$1" != "--pdfium-only") ]]; then
  echo "usage: $0 [--pdfium-only]" >&2
  exit 2
fi
PDFIUM_ONLY="${1:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_ROOT="${REPO_ROOT}/target/pdf-harness"
DOWNLOAD_ROOT="${TARGET_ROOT}/downloads"
QPDF_ARCHIVE="qpdf-12.3.2-bin-linux-x86_64.zip"
QPDF_SHA256="44f2c53bf784c0143128d80d2b9946e9793962c5bb403b75c0024cb4d8e346b9"
PDFIUM_ARCHIVE="pdfium-7947-linux-x64.tgz"
PDFIUM_SHA256="f73d69d309fe1f33cc7269dcc99be31ec44e1cf608e31d7e2fcc6545fc2f9323"
IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "PDF harness installation requires Linux x86_64" >&2
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
  "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7947/pdfium-linux-x64.tgz" \
  "${DOWNLOAD_ROOT}/${PDFIUM_ARCHIVE}" "${PDFIUM_SHA256}"
rm -rf "${TARGET_ROOT}/pdfium"
# This upstream archive contains the same library bytes as the historical wheel.
# Keep its license bundle and build metadata alongside the native library.
mkdir -p "${TARGET_ROOT}/pdfium"
tar -xzf "${DOWNLOAD_ROOT}/${PDFIUM_ARCHIVE}" -C "${TARGET_ROOT}/pdfium"
printf '%s  %s\n' \
  '61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33' \
  "${TARGET_ROOT}/pdfium/lib/libpdfium.so" | sha256sum --check --status
cargo build --locked --release --manifest-path "${SCRIPT_DIR}/pdfium-rust/Cargo.toml" \
  --target-dir "${TARGET_ROOT}/pdfium-rust"

if [[ "${PDFIUM_ONLY}" == "--pdfium-only" ]]; then
  exit 0
fi

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

docker run --rm --init --network bridge --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/pdf-harness/pdfjs \
  "${IMAGE}" npm ci --no-audit --no-fund
