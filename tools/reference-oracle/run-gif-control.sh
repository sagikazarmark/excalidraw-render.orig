#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
case "${1:-}" in generate|verify) ;; *) echo "usage: $0 <generate|verify>" >&2; exit 2 ;; esac
OUTPUT_MOUNT=()
if [[ "$1" == generate ]]; then
  [[ ! -e "${REPO_ROOT}/tests/oracle/gif-control/reference.json" ]] || { echo "immutable reference exists; use verify" >&2; exit 1; }
  OUTPUT_MOUNT=(--mount "type=bind,src=${REPO_ROOT}/tests/oracle/gif-control,dst=/workspace/tests/oracle/gif-control")
fi
docker run --rm --init --ipc=host --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp \
  --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 --env REFERENCE_ORACLE_CANONICAL=1 \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace,readonly" "${OUTPUT_MOUNT[@]}" \
  --tmpfs /workspace/tools/reference-oracle/dist:rw,mode=1777 \
  --tmpfs /workspace/tools/reference-oracle/node_modules/.vite-temp:rw,mode=1777 \
  --workdir /workspace/tools/reference-oracle \
  "${IMAGE}" bash -eu -c 'npm run build -- --logLevel error && npm run profile:verify && node scripts/gif-control-evidence.mjs "$@"' gif-control "$@"
