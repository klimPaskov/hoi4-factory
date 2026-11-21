#!/bin/sh
# One heavy job at a time on this machine (Blender, the game, a large build, a benchmark).
# The slot has two signals that every session must respect together:
#   .work/tripo-round4/hold          a one-line file written by whoever holds the slot
#   .work/tripo-round4/blender.lock  the folder the 3D lane's Blender gate (gated.sh) takes
# Usage:
#   scripts/heavy-slot.sh status
#   scripts/heavy-slot.sh take "<what runs, which session>" [max_wait_seconds]   (default 3600)
#   scripts/heavy-slot.sh release "<same text or a unique part of it>"
# take waits until neither signal is present and the machine lines hold (Windows: free RAM at least
# 6 GB and commit under 44 GB; both machines: at least 30 GB free disk), then creates the hold file
# atomically and checks again after a pause, so a Blender start in the same second is not missed.
# release deletes the hold only when it contains the given text, so nobody removes another job's hold.
# Exit codes: 0 done, 2 usage, 3 timed out waiting, 4 release refused.
set -u
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
dir="$root/.work/tripo-round4"
hold="$dir/hold"
lock="$dir/blender.lock"

machine_ok() {
  free_kb=$(df -k "$dir" | tail -1 | awk '{print $4}')
  if [ "$free_kb" -lt 31457280 ]; then echo "disk: under 30 GB free"; return 1; fi
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      line=$(powershell -NoProfile -Command '$os = Get-CimInstance Win32_OperatingSystem; $pf = Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory; "{0:N1} {1:N1}" -f ($os.FreePhysicalMemory/1MB), ($pf.CommittedBytes/1GB)' | tr -d '\r' | tr ',' '.')
      free=${line% *}; commit=${line#* }
      if awk -v f="$free" -v c="$commit" 'BEGIN { exit !((f + 0) < 6 || (c + 0) >= 44) }'; then
        echo "memory: free RAM $free GB, commit $commit GB (need 6 GB free and commit under 44 GB)"; return 1
      fi ;;
  esac
  return 0
}

busy() {
  if [ -f "$hold" ]; then echo "hold: $(head -1 "$hold" 2>/dev/null)"; return 0; fi
  if [ -d "$lock" ]; then echo "blender.lock: $(cat "$lock/owner" 2>/dev/null)"; return 0; fi
  return 1
}

cmd=${1:-}
case "$cmd" in
  status)
    if r=$(busy); then echo "BUSY $r"; exit 0; fi
    if r=$(machine_ok); then echo "FREE"; else echo "FREE but WAIT $r"; fi
    exit 0 ;;
  take)
    what=${2:-}
    [ -n "$what" ] || { echo "usage: heavy-slot.sh take \"<what runs, which session>\" [max_wait_seconds]" >&2; exit 2; }
    max=${3:-3600}; waited=0
    mkdir -p "$dir"
    while :; do
      if r=$(busy); then reason=$r
      elif ! reason=$(machine_ok); then :
      elif ( set -C; printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M')" "$what" > "$hold" ) 2>/dev/null; then
        sleep 5
        if [ -d "$lock" ]; then
          rm -f "$hold"; reason="blender.lock appeared at the same moment, backing off"
        else
          echo "TAKEN: $(cat "$hold")"; exit 0
        fi
      else reason="hold: another session took it first"
      fi
      if [ "$waited" -ge "$max" ]; then echo "TIMEOUT after ${max}s: $reason"; exit 3; fi
      [ $((waited % 300)) -eq 0 ] && echo "waiting: $reason"
      sleep 15; waited=$((waited + 15))
    done ;;
  release)
    what=${2:-}
    [ -n "$what" ] || { echo "usage: heavy-slot.sh release \"<text from your take>\"" >&2; exit 2; }
    [ -f "$hold" ] || { echo "no hold file, nothing to release"; exit 0; }
    if grep -qF -- "$what" "$hold"; then rm -f "$hold"; echo "RELEASED"; exit 0; fi
    echo "REFUSED: the hold belongs to someone else: $(head -1 "$hold")"; exit 4 ;;
  *)
    echo "usage: heavy-slot.sh status | take \"<what>\" [max_wait_seconds] | release \"<what>\"" >&2; exit 2 ;;
esac
