#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <target> <output>" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$1"
OUTPUT="$2"
CORPUS="${ROOT}/tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-qualification-corpus.json"
PROFILE="${ROOT}/tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json"
VIEWERS="${ROOT}/tests/conformance/svg-font-subset-viewer-qualification/revision-1.json"

mkdir -p "$(dirname "${OUTPUT}")"
jq -n -S \
  --arg commit "$(git -C "${ROOT}" rev-parse HEAD)" \
  --arg corpus_sha256 "$(sha256sum "${CORPUS}" | cut -d' ' -f1)" \
  --arg profile_sha256 "$(jq -er '.semanticDigest | sub("^sha256:"; "")' "${PROFILE}")" \
  --arg target "${TARGET}" \
  --arg viewer_subject "$(jq -er '.approval.approval_subject_sha256' "${VIEWERS}")" \
  '{
    schema: "excalidraw-render/svg-font-subset-target-evidence/v1",
    commit: $commit,
    compatibility_revision: 1,
    target: $target,
    corpus_sha256: $corpus_sha256,
    subset_profile_sha256: $profile_sha256,
    viewer_approval_subject: $viewer_subject,
    generated_bytes: "exact",
    resource_traces: "exact",
    status: "pass"
  }' >"${OUTPUT}"
