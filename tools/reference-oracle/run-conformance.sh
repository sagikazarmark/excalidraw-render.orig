#!/usr/bin/env bash
set -euo pipefail

IMAGE="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
USER_ID="$(id -u)"
GROUP_ID="$(id -g)"

case "${1:-}" in
  generate|reproduce)
    MODE="$1"
    ;;
  *)
    echo "usage: $0 <generate|reproduce> <case-id>" >&2
    exit 2
    ;;
esac

if [[ -z "${2:-}" ]]; then
  echo "usage: $0 <generate|reproduce> <case-id>" >&2
  exit 2
fi

docker run --rm --init --ipc=host \
  --network none \
  --platform linux/amd64 \
  --user "${USER_ID}:${GROUP_ID}" \
  --env HOME=/tmp \
  --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
  --env REFERENCE_ORACLE_CANONICAL=1 \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/reference-oracle \
  "${IMAGE}" \
  bash -lc "npm run build && node scripts/profile.mjs verify && node scripts/conformance-case.mjs '${MODE}' '$2'"
