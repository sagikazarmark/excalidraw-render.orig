#!/usr/bin/env bash
# Reproduce or generate every registered case in one container session.
#
# The per-case runners (run-conformance.sh and its typography and frame
# siblings) each build the bundle and verify the profile before one case. This
# runner does that once, for both bundles and both profiles, and then runs each
# case's generator in turn, so a reproduce of the whole registry costs one
# build rather than thirty-seven. Reproduction writes under target/; generation
# writes to the checkout, and replaces a manifested case only with --supersede
# (which every registered case is, so regenerating the registry needs it).
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
    echo "usage: $0 <generate|reproduce> [--supersede]" >&2
    exit 2
    ;;
esac
SUPERSEDE="${2:-}"
if [[ -n "${SUPERSEDE}" && ( "${SUPERSEDE}" != "--supersede" || "${MODE}" != "generate" || -n "${3:-}" ) ]]; then
  echo "usage: $0 <generate|reproduce> [--supersede]" >&2
  exit 2
fi

# Everything after the mode runs inside the container, where node is the
# pinned version. The registry names each case's generator, so the loop needs
# no table of its own.
read -r -d '' SCRIPT <<'EOS' || true
set -euo pipefail
npm run build
npm exec vite -- build --config vite.frame.config.mjs
node scripts/profile.mjs verify
node scripts/frame-profile.mjs verify
failed=0
while IFS=$'\t' read -r id generator; do
  echo "==> ${MODE} ${id}"
  if ! node "/workspace/${generator}" "${MODE}" "${id}" ${SUPERSEDE}; then
    echo "!!! ${id} failed" >&2
    failed=$((failed + 1))
  fi
done < <(node -e '
  const registry = JSON.parse(require("fs").readFileSync("/workspace/tests/conformance/cases/registry.json", "utf8"));
  for (const entry of registry.cases) process.stdout.write(`${entry.id}\t${entry.generator}\n`);
')
if (( failed > 0 )); then
  echo "${failed} case(s) failed to ${MODE}" >&2
  exit 1
fi
EOS

docker run --rm --init --ipc=host \
  --network none \
  --platform linux/amd64 \
  --user "${USER_ID}:${GROUP_ID}" \
  --env HOME=/tmp \
  --env PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
  --env REFERENCE_ORACLE_CANONICAL=1 \
  --env MODE="${MODE}" \
  --env SUPERSEDE="${SUPERSEDE}" \
  --mount "type=bind,src=${REPO_ROOT},dst=/workspace" \
  --workdir /workspace/tools/reference-oracle \
  "${IMAGE}" \
  bash -lc "${SCRIPT}"
