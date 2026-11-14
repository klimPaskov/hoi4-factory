#!/usr/bin/env bash
# Run the EXP-02 harness inside the pinned hoi4-agent-tools Linux image.
# Usage: probes/focus/run-probe.sh <run-name> [case-filter]
# Output: .work/exp02/<run-name>/ (git-ignored). No network, read-only rootfs,
# probe directory mounted read-only, scratch workspaces in a tmpfs.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/../.." && pwd -W 2>/dev/null || pwd)"
IMAGE="${EXP02_IMAGE:-hoi4f-exp02-agent-tools:25f5641}"
RUN="$1"
FILTER="${2:-}"
OUT="$REPO/.work/exp02/$RUN"
mkdir -p "$OUT"
MSYS_NO_PATHCONV=1 docker run --rm \
  --network none \
  --read-only \
  --tmpfs /work:rw,exec,size=256m,uid=1000,gid=1000 \
  --tmpfs /tmp:rw,size=64m \
  -v "$REPO/probes/focus:/probe:ro" \
  -v "$OUT:/out" \
  --entrypoint node \
  "$IMAGE" /probe/harness/run.mjs /probe/cases.json /out $FILTER
