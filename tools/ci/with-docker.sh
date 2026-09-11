#!/usr/bin/env bash
set -euo pipefail

# A private daemon in this Dagger exec: pinned harness scripts bind this same
# filesystem and retain their --network none boundaries. Never use a host socket.
export DOCKER_HOST=unix:///var/run/docker.sock
dockerd --storage-driver=vfs > /tmp/dockerd.log 2>&1 &
daemon=$!
trap 'kill "$daemon" 2>/dev/null || true; wait "$daemon" 2>/dev/null || true' EXIT
for attempt in {1..60}; do
  if docker info >/dev/null 2>&1; then
    "$@"
    exit
  fi
  if ! kill -0 "$daemon" 2>/dev/null; then
    cat /tmp/dockerd.log >&2
    exit 1
  fi
  sleep 1
done
cat /tmp/dockerd.log >&2
exit 1
