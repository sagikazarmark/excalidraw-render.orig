#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <input-directory> <output>" >&2
  exit 2
fi

INPUT="$1"
OUTPUT="$2"
mapfile -t REPORTS < <(find "${INPUT}" -type f -name '*.json' -print | sort)
if [[ ${#REPORTS[@]} -ne 5 ]]; then
  echo "expected five SVG font subset target reports, got ${#REPORTS[@]}" >&2
  exit 1
fi

jq -e -s '
  map(.target) | sort == [
    "aarch64-apple-darwin",
    "wasm32-unknown-unknown",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu"
  ]
' "${REPORTS[@]}" >/dev/null
jq -e -s '
  all(.;
    .schema == "excalidraw-render/svg-font-subset-target-evidence/v1" and
    .compatibility_revision == 1 and
    .generated_bytes == "exact" and
    .resource_traces == "exact" and
    .status == "pass"
  ) and
  (map(.commit) | unique | length) == 1 and
  (map(.corpus_sha256) | unique | length) == 1 and
  (map(.subset_profile_sha256) | unique | length) == 1 and
  (map(.viewer_approval_subject) | unique | length) == 1
' "${REPORTS[@]}" >/dev/null

mkdir -p "$(dirname "${OUTPUT}")"
jq -S -s '{
  schema: "excalidraw-render/svg-font-subset-target-aggregate/v1",
  commit: .[0].commit,
  compatibility_revision: 1,
  corpus_sha256: .[0].corpus_sha256,
  subset_profile_sha256: .[0].subset_profile_sha256,
  viewer_approval_subject: .[0].viewer_approval_subject,
  targets: sort_by(.target),
  decision: "pass"
}' "${REPORTS[@]}" >"${OUTPUT}"
