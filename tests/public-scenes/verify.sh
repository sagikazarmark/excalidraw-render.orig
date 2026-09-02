#!/usr/bin/env bash
# Re-verify the public scene pool: upstream bytes still match the pinned
# commit, and the renderer still behaves as recorded in README.md.
set -uo pipefail
cd "$(dirname "$0")"
BIN=${BIN:-../../target/release/excalidraw-render}
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0

for dir in */; do
  name=${dir%/}
  prov="$name/PROVENANCE.json"
  [ -f "$prov" ] || { echo "MISSING  $name: no PROVENANCE.json"; fail=1; continue; }

  want=$(jq -r .scene.sha256 "$prov")
  have=$(sha256sum "$name/scene.excalidraw" | cut -d' ' -f1)
  if [ "$want" != "$have" ]; then
    echo "LOCAL    $name: scene.excalidraw does not match its recorded digest"
    fail=1; continue
  fi

  status=upstream-ok
  if [ "${OFFLINE:-0}" != "1" ]; then
    raw=$(jq -r .upstream.raw "$prov")
    up=$(curl -sfL "$raw" | sha256sum | cut -d' ' -f1)
    if [ "$up" != "$have" ]; then
      echo "UPSTREAM $name: pinned commit no longer serves these bytes"
      fail=1; continue
    fi
  else
    status=upstream-skipped
  fi

  if [ -x "$BIN" ]; then
    diag=$("$BIN" "$name/scene.excalidraw" --output "$tmp/$name.svg" 2>&1 >/dev/null)
    code=$?
    diag=$(printf '%s' "$diag" | cut -d: -f1 | sort -u | paste -sd,)
    printf 'OK       %-32s %s exit=%d %s\n' "$name" "$status" "$code" "$diag"
  else
    printf 'OK       %-32s %s (renderer not built)\n' "$name" "$status"
  fi
done

[ $fail -eq 0 ] && echo "All scenes verified." || echo "Verification failed."
exit $fail
