#!/usr/bin/env bash
# Replay the recorded real tool-call stream through a loopback HTTP server with
# each fault, and record how the thin adapter and async-openai surface it.
# A dummy key is used. The real key is never sent to the local server.
set -uo pipefail
cd "$(dirname "$0")/../../.."
OUT=evidence/S01/exp03/compare/fault-surfacing.jsonl
PORT=18431
: > "$OUT"
(cd probes/runtime/compare/thin && cargo build -q)
(cd probes/runtime/compare/async-openai && cargo build -q)
target/release/runtime-probe serve-replay --port $PORT 2> evidence/S01/exp03/compare/replay-server.log &
SRV=$!
sleep 2
for f in none truncate-mid-args drop-finish-done malformed-args disconnect-after-tool-call; do
  for c in thin async-openai; do
    exe=probes/runtime/compare/$c/target/debug/compare-$c.exe
    res=$(OPENAI_API_KEY=replay-no-key timeout 30 "$exe" "http://127.0.0.1:$PORT/$f/v1"); rc=$?
    echo "{\"fault\": \"$f\", \"client\": \"$c\", \"exit\": $rc, \"output\": ${res:-null}}" >> "$OUT"
  done
done
kill $SRV
cat "$OUT"
