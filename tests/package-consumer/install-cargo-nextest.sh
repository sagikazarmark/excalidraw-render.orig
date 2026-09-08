#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/tool-manifest.json"
DESTINATION="${1:?usage: install-cargo-nextest.sh <download-directory> [<target>]}"
TARGET="${2:-x86_64-unknown-linux-gnu}"
VERSION="$(jq -er '.cargo_nextest.version' "${MANIFEST}")"
URL="$(jq -er --arg target "${TARGET}" '.cargo_nextest.targets[$target].url' "${MANIFEST}")"
SHA256="$(jq -er --arg target "${TARGET}" '.cargo_nextest.targets[$target].sha256' "${MANIFEST}")"

mkdir -p "${DESTINATION}"
ARCHIVE="${DESTINATION}/cargo-nextest-${VERSION}-${TARGET}.tar.gz"
curl --fail --silent --show-error --location "${URL}" --output "${ARCHIVE}"
printf '%s  %s\n' "${SHA256}" "${ARCHIVE}" | sha256sum --check -

# Cargo looks for subcommands in its home's bin directory whether or not that
# directory is on PATH.
BIN="${CARGO_HOME:-${HOME}/.cargo}/bin"
mkdir -p "${BIN}"
tar -xzf "${ARCHIVE}" -C "${BIN}" cargo-nextest
cargo nextest --version | grep -F "cargo-nextest ${VERSION}"
