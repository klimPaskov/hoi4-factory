#!/usr/bin/env sh
# Run an npm command for web/ inside the digest-pinned Node image, with web/ mounted.
# Usage: web/scripts/docker-npm.sh install | run check | run dev ...
# Do not run npm on the host: dependencies and builds must come from this image.
set -eu
IMAGE="node:22-bookworm-slim@sha256:de170b39439eb0e5a3444ad7d60b015614a381e10fecae2e30ec9e735122f38d"
HERE="$(cd "$(dirname "$0")/.." && pwd)"
if command -v cygpath >/dev/null 2>&1; then HERE="$(cygpath -m "$HERE")"; fi
PORTS=""
if [ "${1:-}" = "run" ] && [ "${2:-}" = "dev" ]; then PORTS="-p 5173:5173"; fi
# shellcheck disable=SC2086
MSYS_NO_PATHCONV=1 exec docker run --rm $PORTS \
  -v "$HERE:/app" -w /app \
  -e npm_config_update_notifier=false -e npm_config_fund=false -e npm_config_audit=false \
  "$IMAGE" npm "$@"
