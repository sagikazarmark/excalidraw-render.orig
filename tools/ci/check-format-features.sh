#!/usr/bin/env bash
# Checks ADR-0023's promises for builds of the renderer that leave out the
# `png` or `pdf` feature:
#
#   1. the dependency graph drops the libraries only those formats need;
#   2. a request for a left-out format fails with `request.format-unavailable`
#      (the `format_features` tests, compiled only in such a build);
#   3. every format the build keeps renders the same artifact and report bytes
#      as the default build, for the public scene corpus.
#
# Run from the repository root. `lint:features` covers the warnings.
set -euo pipefail

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

scenes=(tests/public-scenes/*/scene.excalidraw)

# Each reduced build and the formats it keeps. CJK stays on so the corpus
# renders as in the default build; `optional_cjk` covers its absence.
builds=("cjk:svg" "cjk,png:svg png" "cjk,pdf:svg pdf")

graph() {
  cargo tree --locked -p excalidraw-render --no-default-features --features "$1" \
    --edges normal --prefix none --format '{p}'
}

refuse() {
  local features="$1"
  shift
  local graph
  graph="$(graph "${features}")"
  for package in "$@"; do
    if grep -q "^${package} " <<< "${graph}"; then
      echo "features '${features}' unexpectedly depend on ${package}" >&2
      exit 1
    fi
  done
}

refuse "" krilla resvg tiny-skia
refuse cjk krilla resvg tiny-skia
refuse png krilla
echo "verified that the reduced dependency graphs leave out their formats' libraries"

digests() {
  cargo run --locked -q -p excalidraw-render --no-default-features --features "$1" \
    --example format_digests -- "${scenes[@]}"
}

cargo run --locked -q -p excalidraw-render --example format_digests -- "${scenes[@]}" \
  > "${work}/default.tsv"

for build in "${builds[@]}"; do
  features="${build%%:*}"
  formats="${build#*:}"

  cargo test --locked -p excalidraw-render --no-default-features --features "${features}" \
    --test format_features

  digests "${features}" > "${work}/reduced.tsv"
  # Column 3 is the format; keep the default build's lines for this build's.
  awk -F '\t' -v formats=" ${formats} " 'index(formats, " " $3 " ")' \
    "${work}/default.tsv" > "${work}/expected.tsv"
  if [[ ! -s "${work}/expected.tsv" ]]; then
    echo "no default-build digests for features '${features}'" >&2
    exit 1
  fi
  if ! diff -u "${work}/expected.tsv" "${work}/reduced.tsv"; then
    echo "features '${features}' changed the bytes of a format it keeps" >&2
    exit 1
  fi
  echo "features '${features}': $(wc -l < "${work}/reduced.tsv") renders match the default build"
done
