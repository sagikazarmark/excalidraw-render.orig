#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
docker run --rm --init --ipc=host --network none --platform linux/amd64 \
  --user "$(id -u):$(id -g)" --env HOME=/tmp --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
  --env REFERENCE_ORACLE_CANONICAL=1 --mount "type=bind,src=${ROOT},dst=/workspace" \
  --workdir /workspace/tools/reference-oracle \
  mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9 \
  node ../representative-intake/svg-viewer.mjs "$@"
