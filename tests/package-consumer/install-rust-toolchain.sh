#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/tool-manifest.json"
TARGET="${1:?usage: install-rust-toolchain.sh <target> <download-directory>}"
DESTINATION="${2:?usage: install-rust-toolchain.sh <target> <download-directory>}"
VERSION="$(jq -er '.rust_toolchain.version' "${MANIFEST}")"
URL="$(jq -er '.rust_toolchain.channel_manifest_url' "${MANIFEST}")"
SHA256="$(jq -er '.rust_toolchain.channel_manifest_sha256' "${MANIFEST}")"

mkdir -p "${DESTINATION}"
CHANNEL_MANIFEST="${DESTINATION}/channel-rust-${VERSION}.toml"
curl --fail --silent --show-error --location "${URL}" --output "${CHANNEL_MANIFEST}"
printf '%s  %s\n' "${SHA256}" "${CHANNEL_MANIFEST}" | sha256sum --check -

rustup toolchain install "${VERSION}" --profile minimal --target "${TARGET}" --no-self-update
rustup default "${VERSION}"
