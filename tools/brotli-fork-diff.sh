#!/usr/bin/env bash
# Mechanical diff of the vendored `crates/brotli-deterministic` fork against an
# upstream rust-brotli revision.
#
# The fork exists only because released rust-brotli lets the platform choose
# floating-point evaluation order, which breaks byte-identical WOFF2 output
# across the five supported targets. Upstream PR #259 fixes that in tree behind
# a `portable-float` feature, so this script is the evidence that the fork has
# no other divergence worth keeping: everything it reports should be either the
# renaming of the crate, the removal of surface the renderer does not use, or
# the float-order patch that `portable-float` replaces.
#
# Usage:
#   tools/brotli-fork-diff.sh [<upstream-ref>]
#
# `<upstream-ref>` defaults to the released `8.0.4` tag. Pass the released tag
# that carries PR #259 when re-running this at stage C.
set -euo pipefail

REF="${1:-8.0.4}"
REPO="https://github.com/dropbox/rust-brotli"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
FORK="${REPO_ROOT}/crates/brotli-deterministic"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

git -c advice.detachedHead=false clone --quiet --depth 1 --branch "${REF}" "${REPO}" "${WORK}/upstream"
UPSTREAM_COMMIT="$(git -C "${WORK}/upstream" rev-parse HEAD)"
rm -rf "${WORK}/upstream/.git"

echo "fork:     ${FORK}"
echo "upstream: ${REPO} @ ${REF} (${UPSTREAM_COMMIT})"
echo

# The vendored fork is a published crate: it carries `src/` and nothing else,
# while the upstream repository also carries the C, Go, Python, and CI trees
# that never reach the renderer. Comparing `src/` is therefore the whole of the
# compression path, and the only place a divergence could change WOFF2 bytes.
if diff --recursive --unified=3 --new-file "${WORK}/upstream/src" "${FORK}/src"; then
  echo "no source divergence against ${REF}"
fi

echo
echo "--- fork files outside src/ (not compared; upstream ships more) ---"
(cd "${FORK}" && find . -type f -not -path './src/*' -not -path './target/*' | LC_ALL=C sort)
