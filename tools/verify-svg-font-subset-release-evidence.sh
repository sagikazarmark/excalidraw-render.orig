#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${ROOT}/tests/conformance/svg-font-subset-release-evidence/revision-1.json"

jq -e '
  .id == "svg-font-subsets.release-evidence.revision-1.v1" and
  .compatibilityRevision == 1 and
  .corpus.successfulCells == 762 and
  .licensing.qualifiedSources == 230 and
  .licensing.qualifiedTransformationNotices == 762 and
  .viewerEvidence.executions == 643 and
  .viewerEvidence.status == "pending_maintainer_approval" and
  .targetEvidence.status == "pending_ci" and
  .decision == "pending_approval_and_target_aggregation"
' "${MANIFEST}" >/dev/null

while IFS=$'\t' read -r path expected; do
  actual="$(sha256sum "${ROOT}/${path}" | cut -d' ' -f1)"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "stale SVG font subset release evidence descriptor: ${path}" >&2
    exit 1
  fi
done < <(jq -r '.. | objects | select(has("path") and has("sha256")) | [.path, .sha256] | @tsv' "${MANIFEST}")

echo "verified pending SVG font subset release evidence"
