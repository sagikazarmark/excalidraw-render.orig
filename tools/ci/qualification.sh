#!/usr/bin/env bash
set -euo pipefail

target="${1:?target required}"
log="${2:?log path required}"
args=(cargo test --locked --release -p excalidraw-render --lib)
case "$target" in
  # The native target runs on the host: the bytes do not depend on it
  # (ADR-0024, ADR-0025), and a --target would cross-compile elsewhere. Admits
  # the release-mode qualification set; the tests are ignored otherwise.
  x86_64-unknown-linux-gnu) export EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1 ;;
  wasm32-unknown-unknown) args+=(--target "$target" wasm_tests) ;;
  *) exit 2 ;;
esac
# pipefail is essential: a failed test must never become a passing report.
"${args[@]}" -- --nocapture 2>&1 | tee "$log"
