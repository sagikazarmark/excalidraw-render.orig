#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
[[ $# -eq 2 ]] || { printf '%s\n' 'usage: reference.sh <intake> <new-output>' >&2; exit 2; }
python3 -B "${ROOT}/tools/representative-intake/intake.py" verify "$1"
docker run --rm --init --ipc=host --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
  --env REFERENCE_ORACLE_CANONICAL=1 --mount "type=bind,src=${ROOT},dst=/workspace" \
  --workdir /workspace/tools/reference-oracle \
  mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9 \
  bash -eu -c 'npm run build && node scripts/profile.mjs verify && node ../representative-intake/reference.mjs "$1" "$2"' _ "$1" "$2"
