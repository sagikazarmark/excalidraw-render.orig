#!/usr/bin/env bash
set -euo pipefail

bash tests/package-consumer/install-rust-toolchain.sh wasm32-unknown-unknown /opt/rust-download
bash tests/package-consumer/install-browser-tools.sh /opt/browser
bash tests/package-consumer/install-wasm-test-runner.sh /opt/wasm-runner
manifest=tests/package-consumer/tool-manifest.json
chrome="$(jq -er '.browser.tools.chromium.binary' "$manifest")"
driver="$(jq -er '.browser.tools.chromedriver.binary' "$manifest")"
version="$(jq -er '.browser.version' "$manifest")"
"/opt/browser/$chrome" --version | grep -F "$version"
"/opt/browser/$driver" --version | grep -F "$version"
ln -s "/opt/browser/$chrome" /usr/local/bin/chromium
ln -s "/opt/browser/$driver" /usr/local/bin/chromedriver
