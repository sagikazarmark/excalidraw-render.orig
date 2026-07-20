#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/tool-manifest.json"
DESTINATION="${1:?usage: install-wasm-test-runner.sh <download-directory>}"
CRATE="$(jq -er '.wasm_test_runner.crate' "${MANIFEST}")"
VERSION="$(jq -er '.wasm_test_runner.version' "${MANIFEST}")"
URL="$(jq -er '.wasm_test_runner.url' "${MANIFEST}")"
SHA256="$(jq -er '.wasm_test_runner.crate_sha256' "${MANIFEST}")"

mkdir -p "${DESTINATION}/source"
ARCHIVE="${DESTINATION}/${CRATE}-${VERSION}.crate"
curl --fail --silent --show-error --location "${URL}" --output "${ARCHIVE}"
printf '%s  %s\n' "${SHA256}" "${ARCHIVE}" | sha256sum --check -
tar -xzf "${ARCHIVE}" -C "${DESTINATION}/source"

cargo install --locked --path "${DESTINATION}/source/${CRATE}-${VERSION}"
