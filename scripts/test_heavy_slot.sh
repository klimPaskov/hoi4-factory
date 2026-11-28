#!/bin/sh
# Regression tests for scripts/heavy-slot.sh. Every test runs in its own scratch pool (HEAVY_STATE_DIR), never in
# .work/tripo-round4, with the memory and disk lines faked and the clocks sped up. Run: scripts/test_heavy_slot.sh
# Set SCRIPT=<path> to test another copy of heavy-slot.sh (for example the old one, to see the tests fail).
set -u
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SCRIPT=${SCRIPT:-$here/heavy-slot.sh}
base=$(mktemp -d "${TMPDIR:-/tmp}/heavy-slot-test.XXXXXX") || exit 1
trap 'rm -rf "$base"' EXIT
fails=0; count=0

export HEAVY_MACHINE_FAKE=ok HEAVY_POLL_SECONDS=1 HEAVY_SETTLE_SECONDS=0 HEAVY_SLOTS=4
unset HEAVY_OWNER_PID HEAVY_RUST_RESERVE HEAVY_DEAD_AFTER_MIN

# run <seconds> <command...>: a hard timeout where the machine has one.
run() {
  t=$1; shift
  if command -v timeout >/dev/null 2>&1; then timeout "$t" "$@"
  elif command -v gtimeout >/dev/null 2>&1; then gtimeout "$t" "$@"
  else "$@"; fi
}
hs() { run 60 sh "$SCRIPT" "$@"; }

# new_pool <name>: a fresh scratch pool with the live reserves, prints nothing, sets HEAVY_STATE_DIR.
new_pool() {
  HEAVY_STATE_DIR="$base/$1"; export HEAVY_STATE_DIR
  mkdir -p "$HEAVY_STATE_DIR/slots"
  cat > "$HEAVY_STATE_DIR/reserves" <<'EOF'
# name count pattern
tutorial 1 ^tutorial-path:
zin 1 ^zin-lane
rust 2 rust|cargo
EOF
}
# old <file> <minutes>: backdate a file or folder, GNU touch first, BSD touch as the fallback.
old() { touch -d "$2 minutes ago" "$1" 2>/dev/null || touch -t "$(date -v-"$2"M +%Y%m%d%H%M)" "$1"; }
# slot <n> <text> <pid> [minutes old]
slot() {
  mkdir -p "$HEAVY_STATE_DIR/slots/s$1"
  printf '2026-11-28 01:00 %s (pid %s)\n' "$2" "$3" > "$HEAVY_STATE_DIR/slots/s$1/owner"
  [ -n "${4:-}" ] && old "$HEAVY_STATE_DIR/slots/s$1/owner" "$4"
  return 0
}
# ticket <group> <name> <start epoch>: a waiting take of a group, as if its take loop ran right now.
ticket() { mkdir -p "$HEAVY_STATE_DIR/$1-wait"; echo "$3" > "$HEAVY_STATE_DIR/$1-wait/$2"; }
dead_pid() { sh -c 'exit 0' & dp=$!; wait "$dp"; echo "$dp"; }

ok() { count=$((count + 1)); echo "ok   $1"; }
bad() { count=$((count + 1)); fails=$((fails + 1)); echo "FAIL $1: $2"; }
expect_rc() { # <name> <wanted> <got> <output>
  if [ "$3" -eq "$2" ]; then ok "$1"; else bad "$1" "exit $3, wanted $2: $4"; fi
}
now=$(date +%s)
live=$$   # a pid that is alive for the whole run

# ---- basics -------------------------------------------------------------------------------------------------------
new_pool basic
out=$(hs take "plain job" 5); expect_rc "plain take starts" 0 $? "$out"
out=$(hs release "plain job"); expect_rc "release frees it" 0 $? "$out"
[ -z "$(ls "$HEAVY_STATE_DIR/slots")" ] && ok "slot folder is gone" || bad "slot folder is gone" "$(ls "$HEAVY_STATE_DIR/slots")"

# ---- reserve deadlock (2026-11-28 01:45) ---------------------------------------------------------------------------
# One free slot. ZIN reserve 0 of 1 with a take waiting, Rust reserve 1 of 2 with a take waiting.
deadlock_pool() {
  new_pool "$1"
  slot 1 "cargo build rust packet" "$live"      # Rust holds 1 of its 2
  slot 2 "gated step: blender" "$live"
  slot 3 "some other job" "$live"
}
deadlock_pool dl1
ticket zin zin-old "$((now - 100))"
out=$(hs take "cargo test rust packet 7" 6); expect_rc "rust take starts although a ZIN take waits" 0 $? "$out"

deadlock_pool dl2
ticket rust rust-old "$((now - 100))"
out=$(hs take "zin-lane: apply" 0); expect_rc "zin take waits while the higher Rust reserve is unfilled" 3 $? "$out"
case "$out" in *"kept for waiting group"*rust*) ok "the wait reason names rust" ;; *) bad "the wait reason names rust" "$out" ;; esac

deadlock_pool dl3
ticket tutorial tut-old "$((now - 5))"
ticket rust rust-old "$((now - 100))"
out=$(hs take "cargo test rust packet 8" 0); expect_rc "rust take waits for a waiting tutorial take" 3 $? "$out"
out=$(hs take "tutorial-path: game_migrate" 6); expect_rc "tutorial take starts before rust and zin" 0 $? "$out"

# Equal priority: the oldest waiting take goes first.
new_pool tie
printf 'alpha 1 ^alpha\nbeta 1 ^beta\n' > "$HEAVY_STATE_DIR/reserves"
slot 1 "j1" "$live"; slot 2 "j2" "$live"; slot 3 "j3" "$live"
ticket alpha alpha-old "$((now - 200))"
out=$(hs take "beta job" 0); expect_rc "a younger waiting take of the same level waits" 3 $? "$out"
ticket beta beta-old "$((now - 400))"; rm -f "$HEAVY_STATE_DIR/alpha-wait/alpha-old"; ticket alpha alpha-new "$((now - 10))"
out=$(hs take "alpha job" 0); expect_rc "the younger group waits when the other is older" 3 $? "$out"
rm -f "$HEAVY_STATE_DIR/beta-wait/beta-old"; ticket alpha alpha-old "$((now - 400))"; rm -f "$HEAVY_STATE_DIR/alpha-wait/alpha-new"
ticket beta beta-new "$((now - 5))"
out=$(hs take "alpha job" 6); expect_rc "the older group starts" 0 $? "$out"

# An explicit priority file overrides the defaults, and an old pool without one still works (every test above).
deadlock_pool prio
printf 'zin 1\nrust 5\n' > "$HEAVY_STATE_DIR/reserve-priority"
ticket rust rust-old "$((now - 100))"
out=$(hs take "zin-lane: apply" 6); expect_rc "reserve-priority puts zin before rust" 0 $? "$out"

# A take with no group still leaves slots for waiting groups, as before.
deadlock_pool nogroup
ticket zin zin-old "$((now - 100))"
out=$(hs take "unrelated job" 0); expect_rc "an ungrouped take leaves the last slot to a waiting group" 3 $? "$out"

# `kept` for a text answers with the groups that go first.
deadlock_pool kept
ticket zin zin-old "$((now - 100))"; ticket rust rust-old "$((now - 100))"
out=$(hs kept "cargo build"); [ "${out%% *}" = 0 ] && ok "kept: rust has nothing ahead of it" || bad "kept: rust has nothing ahead of it" "$out"
out=$(hs kept "zin-lane: x"); [ "${out%% *}" = 1 ] && ok "kept: zin leaves rust's free slot" || bad "kept: zin leaves rust's free slot" "$out"

# ---- dead plain takes (2026-11-28 06:1x) -----------------------------------------------------------------------------
dead=$(dead_pid)
new_pool dead1
export HEAVY_SLOTS=1
slot 1 "dead job" "$dead" 10
out=$(hs take "next job" 6); expect_rc "a slot whose owner is gone and old is cleared and reused" 0 $? "$out"
grep -q "CLEARED dead slot s1" "$HEAVY_STATE_DIR/cleared.log" && ok "the cleared slot is logged" || bad "the cleared slot is logged" "no log line"
hs release "next job" >/dev/null

new_pool dead2
slot 1 "young dead job" "$dead"
out=$(hs take "next job" 0); expect_rc "a dead owner younger than the limit keeps its slot" 3 $? "$out"

new_pool dead3
slot 1 "live old job" "$live" 30
out=$(hs take "next job" 0); expect_rc "an old slot with a live owner keeps its slot" 3 $? "$out"

new_pool dead4
mkdir -p "$HEAVY_STATE_DIR/slots/s1"; printf '2026-11-28 01:00 no pid here\n' > "$HEAVY_STATE_DIR/slots/s1/owner"; old "$HEAVY_STATE_DIR/slots/s1/owner" 30
out=$(hs take "next job" 0); expect_rc "a slot without a recorded pid is never cleared" 3 $? "$out"

new_pool dead5
mkdir -p "$HEAVY_STATE_DIR/slots/s1"; old "$HEAVY_STATE_DIR/slots/s1" 30
out=$(hs take "next job" 6); expect_rc "an old slot folder with no owner file is cleared" 0 $? "$out"
hs release "next job" >/dev/null

new_pool dead6
slot 1 "dead job" "$dead" 10
out=$(hs status 2>&1); case "$out" in *"CLEARED dead slot s1"*FREE*|*FREE*"CLEARED dead slot s1"*) ok "status clears and reports it" ;; *) bad "status clears and reports it" "$out" ;; esac

new_pool owner
out=$(HEAVY_OWNER_PID=$live hs take "owned job" 5); expect_rc "HEAVY_OWNER_PID is accepted" 0 $? "$out"
grep -q "(pid $live)" "$HEAVY_STATE_DIR/slots/s1/owner" && ok "HEAVY_OWNER_PID is recorded" || bad "HEAVY_OWNER_PID is recorded" "$(cat "$HEAVY_STATE_DIR/slots/s1/owner")"

echo "$count checks, $fails failed"
[ "$fails" -eq 0 ]
