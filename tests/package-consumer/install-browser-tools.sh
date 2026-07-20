#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/tool-manifest.json"
DESTINATION="${1:?usage: install-browser-tools.sh <destination>}"

mkdir -p "${DESTINATION}"

install_archive() {
  local tool="$1"
  local archive="${DESTINATION}/${tool}.zip"
  local url
  local sha256
  url="$(jq -er ".browser.tools.${tool}.url" "${MANIFEST}")"
  sha256="$(jq -er ".browser.tools.${tool}.sha256" "${MANIFEST}")"

  curl --fail --silent --show-error --location "${url}" --output "${archive}"
  printf '%s  %s\n' "${sha256}" "${archive}" | sha256sum --check -
  unzip -q -o "${archive}" -d "${DESTINATION}"
}

while IFS= read -r tool; do
  install_archive "${tool}"
done < <(jq -er '.browser.tools | keys[]' "${MANIFEST}")
