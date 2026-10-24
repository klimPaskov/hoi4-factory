#!/usr/bin/env bash
# S01 EXP-03 driver. Runs from the repository root. Makes no provider calls
# unless RUNTIME_PROBE_LIVE=1, which adds one live Anthropic turn through the
# worker process (needs ANTHROPIC_API_KEY in the environment). Recordings are
# captured separately by `runtime-probe record-anthropic`.
set -euo pipefail
cd "$(dirname "$0")/../.."
COMPOSE=(docker compose -f compose.yaml -f probes/runtime/compose.db-port.yaml)
BIN=target/release/runtime-probe
LOG=evidence/S01/exp03/runs/commands.log
mkdir -p evidence/S01/exp03/runs
: > "$LOG"; : > "$LOG.stderr"
wait_db() { for i in $(seq 1 60); do if docker exec hoi4-factory-db-1 pg_isready -U factory -q 2>/dev/null; then echo "db ready after ${i}s" | tee -a "$LOG"; return 0; fi; sleep 1; done; echo "db not ready" | tee -a "$LOG"; return 1; }
run() { echo "+ $*" | tee -a "$LOG"; set +e; "$@" 2>>"$LOG.stderr"; rc=$?; set -e; echo "exit=$rc" | tee -a "$LOG"; return 0; }

run cargo build --release --locked -p factory-runtime-probe
run "${COMPOSE[@]}" up -d --no-deps db
wait_db
run "$BIN" journal --phase reset
run "$BIN" broker
run "$BIN" faults
run "$BIN" journal --phase a
run "${COMPOSE[@]}" restart db
wait_db
run "$BIN" journal --phase b
if [ "${RUNTIME_PROBE_LIVE:-0}" = 1 ]; then run "$BIN" worker --live; else run "$BIN" worker; fi
