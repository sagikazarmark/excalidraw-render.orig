#!/usr/bin/env bash
set -euo pipefail

git diff --exit-code HEAD -- .
if [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
  echo "target evidence requires a committed source tree (untracked files present)" >&2
  exit 1
fi
