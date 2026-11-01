#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT_REAL="$(realpath "${REPO_ROOT}")"
CASE_ROOT="${REPO_ROOT}/tests/conformance/svg-viewer-qualification/rectangle-outline"
STAGING_ROOT="${REPO_ROOT}/target/svg-viewer-qualification/rectangle-outline"
OFFICIAL="tests/conformance/cases/focused/rectangle-outline/evidence/official.svg"
CANDIDATE="tests/conformance/svg-viewer-qualification/rectangle-outline/candidate-control.svg"
ASSETS="tests/conformance/svg-viewer-qualification/rectangle-outline/assets.none.json"
RUST_TARGET="${REPO_ROOT}/target/svg-viewer-rust"

# The pure-Rust viewer builds for this host. The approved profile records the
# x86_64-unknown-linux-gnu qualification; other hosts reproduce its bytes.
# The devenv shell provides the pinned toolchain as rustc and cargo (ADR-0024);
# the exact build is still asserted here.
if [[ "$(rustc --version)" != "rustc 1.97.1 (8bab26f4f 2026-07-14)" ]]; then
  echo "pure-Rust SVG viewer qualification requires rustc 1.97.1 (8bab26f4f 2026-07-14)" >&2
  exit 1
fi
for variable in RUSTC RUSTC_WRAPPER RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_TARGET; do
  if [[ -n "${!variable:-}" ]]; then
    echo "pure-Rust SVG viewer qualification rejects ambient ${variable}" >&2
    exit 1
  fi
done

case "${1:-}" in
  generate)
    if [[ -e "${CASE_ROOT}/manifest.json" ]]; then
      echo "generation refuses to replace manifested SVG viewer qualification evidence" >&2
      exit 1
    fi
    OUTPUT_ROOT="${CASE_ROOT}"
    ;;
  reproduce)
    OUTPUT_ROOT="${STAGING_ROOT}"
    if [[ -L "${REPO_ROOT}/target" ]]; then
      echo "repository target directory must not be a symlink" >&2
      exit 1
    fi
    mkdir -p "${REPO_ROOT}/target"
    if [[ "$(realpath "${REPO_ROOT}/target")" != "${REPO_ROOT_REAL}/target" ]]; then
      echo "repository target directory resolves outside the repository" >&2
      exit 1
    fi
    if [[ -L "${OUTPUT_ROOT}" ]]; then
      echo "SVG viewer qualification staging path must not be a symlink" >&2
      exit 1
    fi
    rm -rf "${OUTPUT_ROOT}"
    ;;
  *)
    echo "usage: $0 <generate|reproduce>" >&2
    exit 2
    ;;
esac

mkdir -p "${OUTPUT_ROOT}/evidence"
OUTPUT_RELATIVE="${OUTPUT_ROOT#"${REPO_ROOT}/"}"
cargo run --quiet --locked -p xtask -- conformance verify-svg-viewers > /dev/null

render_chromium() {
  local name="$1"
  local input="$2"
  bash "${REPO_ROOT}/tools/reference-oracle/run-svg-viewer.sh" render \
    "${input}" "${OUTPUT_RELATIVE}/evidence/${name}.rgba" "${ASSETS}" 0 \
    > "${OUTPUT_ROOT}/evidence/${name}.json"
}

render_chromium chromium-reference "${OFFICIAL}"
render_chromium chromium-candidate "${CANDIDATE}"
render_chromium chromium-reference-repeat "${OFFICIAL}"
render_chromium chromium-candidate-repeat "${CANDIDATE}"

HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
CARGO_TARGET_DIR="${RUST_TARGET}" cargo build --frozen --release \
  --target "${HOST_TARGET}" \
  --manifest-path "${REPO_ROOT}/tools/svg-viewer-rust/Cargo.toml"
RUST_VIEWER="${RUST_TARGET}/${HOST_TARGET}/release/excalidraw-render-svg-viewer"

render_rust() {
  local name="$1"
  local input="$2"
  "${RUST_VIEWER}" render "${REPO_ROOT}/${input}" \
    "${OUTPUT_ROOT}/evidence/${name}.rgba" "${REPO_ROOT}/${ASSETS}" \
    > "${OUTPUT_ROOT}/evidence/${name}.json"
}

render_rust pure-rust-reference "${OFFICIAL}"
render_rust pure-rust-candidate "${CANDIDATE}"
render_rust pure-rust-reference-repeat "${OFFICIAL}"
render_rust pure-rust-candidate-repeat "${CANDIDATE}"

for viewer in chromium pure-rust; do
  cmp "${OUTPUT_ROOT}/evidence/${viewer}-reference.rgba" \
    "${OUTPUT_ROOT}/evidence/${viewer}-reference-repeat.rgba"
  cmp "${OUTPUT_ROOT}/evidence/${viewer}-candidate.rgba" \
    "${OUTPUT_ROOT}/evidence/${viewer}-candidate-repeat.rgba"
done

for viewer in chromium pure-rust; do
  cargo run --quiet --locked -p excalidraw-render-compare --example compare_rgba -- \
    "${OUTPUT_ROOT}/evidence/${viewer}-reference.rgba" \
    "${OUTPUT_ROOT}/evidence/${viewer}-candidate.rgba" \
    "${viewer}" "${OUTPUT_ROOT}/evidence/${viewer}-comparison.json"
done

rm "${OUTPUT_ROOT}/evidence/"*-repeat.rgba
rm "${OUTPUT_ROOT}/evidence/"{chromium,pure-rust}-{reference,candidate,reference-repeat,candidate-repeat}.json

if [[ "${1}" == "reproduce" ]]; then
  for name in \
    chromium-reference.rgba chromium-candidate.rgba \
    pure-rust-reference.rgba pure-rust-candidate.rgba \
    chromium-comparison.json pure-rust-comparison.json; do
    cmp "${OUTPUT_ROOT}/evidence/${name}" "${CASE_ROOT}/evidence/${name}"
  done
fi

echo "${1}d reproducible Chromium and pure-Rust SVG viewer qualification buffers"
