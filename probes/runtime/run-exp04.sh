#!/usr/bin/env bash
# S01 EXP-04 driver: build the agent probe image, prove its filesystem holds no
# coding CLI, interpreter runtime, credential or game file, then run it.
# The key is passed by name only (docker run -e ANTHROPIC_API_KEY), never baked in.
# One live inference attempt, only with RUNTIME_PROBE_LIVE=1.
set -uo pipefail
cd "$(dirname "$0")/../.."
OUT=evidence/S01/exp04/runs
IMG=hoi4-factory-agent-probe:s01
mkdir -p "$OUT"
LOG="$OUT/commands.log"; [ "${APPEND_LOG:-0}" = 1 ] || : > "$LOG"
step() { local name=$1; shift; echo "+ $*" >> "$LOG"; "$@" > "$OUT/$name.out" 2> "$OUT/$name.err"; local rc=$?; echo "exit=$rc" >> "$LOG"; echo "$name $rc"; }

s=$(date +%s)
step build docker build --progress=plain -f docker/agent-probe.Dockerfile -t "$IMG" .
echo "build_seconds=$(( $(date +%s) - s ))" >> "$LOG"
step inspect docker image inspect "$IMG"
step history docker history --no-trunc --format '{{.CreatedBy}}' "$IMG"
cid=$(docker create "$IMG")
echo "+ docker export $cid | tar -t" >> "$LOG"
docker export "$cid" | tar -t > "$OUT/filesystem-listing.txt"; echo "exit=$?" >> "$LOG"
docker rm "$cid" > /dev/null
python -I probes/runtime/scan_image_listing.py "$OUT/filesystem-listing.txt" > "$OUT/filesystem-scan.json"; echo "scan exit=$?" >> "$LOG"
# No key: the probe must refuse before sending anything.
step run-no-key env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY docker run --rm "$IMG" live-once
# The single live native inference attempt, key passed by name at run time.
# Runs only with RUNTIME_PROBE_LIVE=1 so a rebuild never spends by accident.
if [ "${RUNTIME_PROBE_LIVE:-0}" = 1 ]; then
  step run-live-once docker run --rm -e ANTHROPIC_API_KEY "$IMG" live-once
fi
