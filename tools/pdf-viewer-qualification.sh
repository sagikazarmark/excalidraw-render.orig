#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT_REAL="$(realpath "${REPO_ROOT}")"
CASE_ROOT="${REPO_ROOT}/tests/conformance/pdf-viewer-qualification/rectangle-outline"
STAGING_ROOT="${REPO_ROOT}/target/pdf-viewer-qualification/rectangle-outline"
REFERENCE="${REPO_ROOT}/tests/conformance/svg-viewer-qualification/rectangle-outline/evidence/chromium-reference.rgba"
EXPECTATION="${REPO_ROOT}/tests/conformance/pdf-viewer-qualification/rectangle-outline/expected-structure.json"
VISUAL_FAIL_EXPECTATION="${REPO_ROOT}/tests/conformance/pdf-viewer-qualification/rectangle-outline/expected-visual-fail-structure.json"
COVERAGE_EXPECTATION="${REPO_ROOT}/tests/conformance/pdf-viewer-qualification/rectangle-outline/expected-coverage-structure.json"

if [[ ! -x "${REPO_ROOT}/target/pdf-harness/qpdf/bin/qpdf" || ! -d "${REPO_ROOT}/target/pdf-harness/pdfium" || ! -d "${REPO_ROOT}/tools/pdf-harness/pdfjs/node_modules" ]]; then
  echo "in the devenv shell, install the pinned development tools with: bash tools/pdf-harness/install.sh" >&2
  exit 1
fi

# Independent translucent vectors and image soft masks qualify the bitmap's
# straight-alpha representation; opaque calibration alone cannot distinguish it.
cargo test --quiet --locked --release \
  --manifest-path "${REPO_ROOT}/tools/pdf-harness/pdfium-rust/Cargo.toml" \
  --target-dir "${REPO_ROOT}/target/pdf-harness/pdfium-rust"

case "${1:-}" in
  generate)
    if [[ -e "${CASE_ROOT}/manifest.json" ]]; then
      echo "generation refuses to replace manifested PDF qualification evidence" >&2
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
    if [[ -L "${REPO_ROOT}/target/pdf-viewer-qualification" ]]; then
      echo "PDF qualification staging parent must not be a symlink" >&2
      exit 1
    fi
    mkdir -p "${REPO_ROOT}/target/pdf-viewer-qualification"
    if [[ "$(realpath "${REPO_ROOT}/target/pdf-viewer-qualification")" != "${REPO_ROOT_REAL}/target/pdf-viewer-qualification" ]]; then
      echo "PDF qualification staging parent resolves outside the repository" >&2
      exit 1
    fi
    if [[ -L "${OUTPUT_ROOT}" ]]; then
      echo "PDF qualification staging path must not be a symlink" >&2
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
bash "${REPO_ROOT}/tools/pdf-harness/run-xtask.sh" generate-fixtures "${OUTPUT_ROOT}" > /dev/null

render() {
  local viewer="$1"
  local role="$2"
  local fixture="$3"
  bash "${REPO_ROOT}/tools/pdf-harness/run-${viewer}.sh" render \
    "${OUTPUT_ROOT}/${fixture}" "${OUTPUT_ROOT}/evidence/${viewer}-${role}.rgba" 200 120 \
    > "${OUTPUT_ROOT}/evidence/${viewer}-${role}.runtime.json"
}

for viewer in pdfium pdfjs; do
  render "${viewer}" valid candidate-control.pdf
  render "${viewer}" valid-repeat candidate-control.pdf
  render "${viewer}" visual-fail visual-fail-structure-pass.pdf
  render "${viewer}" visual-fail-repeat visual-fail-structure-pass.pdf
  render "${viewer}" structure-fail visual-pass-structure-fail.pdf
  render "${viewer}" structure-fail-repeat visual-pass-structure-fail.pdf
  for role in valid visual-fail structure-fail; do
    cmp "${OUTPUT_ROOT}/evidence/${viewer}-${role}.rgba" \
      "${OUTPUT_ROOT}/evidence/${viewer}-${role}-repeat.rgba"
  done
done

compare() {
  local viewer="$1"
  local role="$2"
  local expected="$3"
  cargo run --quiet --locked -p excalidraw-render-compare --example compare_pdf_rgba -- \
    "${REFERENCE}" "${OUTPUT_ROOT}/evidence/${viewer}-${role}.rgba" \
    "${viewer}" "${expected}" "${OUTPUT_ROOT}/evidence/${viewer}-${role}-comparison.json"
}

for viewer in pdfium pdfjs; do
  compare "${viewer}" valid pass
  compare "${viewer}" visual-fail fail
  compare "${viewer}" structure-fail pass
done

inspect() {
  local role="$1"
  local fixture="$2"
  local expectation="$3"
  local expected="$4"
  bash "${REPO_ROOT}/tools/pdf-harness/run-structure.sh" inspect \
    "${OUTPUT_ROOT}/${fixture}" "${expectation}" "${expected}" \
    "${OUTPUT_ROOT}/evidence/${role}-structure.json"
  bash "${REPO_ROOT}/tools/pdf-harness/run-structure.sh" inspect \
    "${OUTPUT_ROOT}/${fixture}" "${expectation}" "${expected}" \
    "${OUTPUT_ROOT}/evidence/${role}-structure-repeat.json"
  cmp "${OUTPUT_ROOT}/evidence/${role}-structure.json" \
    "${OUTPUT_ROOT}/evidence/${role}-structure-repeat.json"
}

inspect valid candidate-control.pdf "${EXPECTATION}" pass
inspect visual-fail visual-fail-structure-pass.pdf "${VISUAL_FAIL_EXPECTATION}" pass
inspect structure-fail visual-pass-structure-fail.pdf "${EXPECTATION}" fail
inspect parser-warning parser-warning.pdf "${EXPECTATION}" fail
inspect coverage structure-coverage.pdf "${COVERAGE_EXPECTATION}" pass
inspect active-action active-action.pdf "${EXPECTATION}" fail

rm "${OUTPUT_ROOT}/evidence/"*-repeat.rgba
rm "${OUTPUT_ROOT}/evidence/"*-structure-repeat.json
rm "${OUTPUT_ROOT}/evidence/"*.runtime.json

if [[ "${1}" == "reproduce" ]]; then
  for name in candidate-control.pdf visual-fail-structure-pass.pdf visual-pass-structure-fail.pdf parser-warning.pdf structure-coverage.pdf active-action.pdf; do
    cmp "${OUTPUT_ROOT}/${name}" "${CASE_ROOT}/${name}"
  done
  for name in \
    pdfium-valid.rgba pdfium-valid-comparison.json \
    pdfium-visual-fail.rgba pdfium-visual-fail-comparison.json \
    pdfium-structure-fail.rgba pdfium-structure-fail-comparison.json \
    pdfjs-valid.rgba pdfjs-valid-comparison.json \
    pdfjs-visual-fail.rgba pdfjs-visual-fail-comparison.json \
    pdfjs-structure-fail.rgba pdfjs-structure-fail-comparison.json \
    valid-structure.json visual-fail-structure.json \
    structure-fail-structure.json parser-warning-structure.json \
    coverage-structure.json active-action-structure.json; do
    cmp "${OUTPUT_ROOT}/evidence/${name}" "${CASE_ROOT}/evidence/${name}"
  done
fi

echo "${1}d reproducible PDFium, PDF.js, and structural qualification evidence"
