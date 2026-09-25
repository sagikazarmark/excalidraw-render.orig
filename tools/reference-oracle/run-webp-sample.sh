#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
# webp-helper.Dockerfile pins both stages to the canonical Playwright digest.
HELPER_IMAGE="excalidraw-reference-webp-helper:local"
case "${1:-}" in
  build-helper)
    docker build --platform linux/amd64 --file "${SCRIPT_DIR}/webp-helper.Dockerfile" --tag "${HELPER_IMAGE}" "${SCRIPT_DIR}"
    exit ;;
  prepare|generate|verify|test-helper) ;;
  *) echo "usage: $0 <build-helper|test-helper|prepare|generate|verify>" >&2; exit 2 ;;
esac
docker image inspect "${HELPER_IMAGE}" >/dev/null || { echo "run $0 build-helper first (compiler acquisition may use network)" >&2; exit 1; }
HELPER_IMAGE="$(docker image inspect --format '{{.Id}}' "${HELPER_IMAGE}")"
OUTPUT_MOUNT=()
if [[ "$1" == prepare || "$1" == generate ]]; then
  OUTPUT=reference.json
  [[ "$1" != prepare ]] || OUTPUT=inputs.json
  [[ ! -e "${REPO_ROOT}/tests/oracle/webp-sample/${OUTPUT}" ]] || { echo "immutable ${OUTPUT} exists; use verify" >&2; exit 1; }
  OUTPUT_MOUNT=(--mount "type=bind,src=${REPO_ROOT}/tests/oracle/webp-sample,dst=/workspace/tests/oracle/webp-sample")
fi
docker run --rm --init --ipc=host --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 --env REFERENCE_ORACLE_CANONICAL=1 \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace,readonly" "${OUTPUT_MOUNT[@]}" \
  --tmpfs /workspace/tools/reference-oracle/dist:rw,mode=1777 \
  --tmpfs /workspace/tools/reference-oracle/node_modules/.vite-temp:rw,mode=1777 \
  --workdir /workspace/tools/reference-oracle \
  "${HELPER_IMAGE}" bash -eu -c 'if [[ "$1" == test-helper ]]; then exec node --test scripts/webp-encode-inputs.test.mjs; fi; if [[ "$1" != prepare ]]; then npm run build -- --logLevel error && npm run profile:verify; fi; node scripts/webp-sample-evidence.mjs "$@"' webp-sample "$@"
