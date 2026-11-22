#!/bin/sh
# Heavy jobs on this machine (Blender, the game, a large build, a benchmark) share a small pool of slots (D-113).
#   .work/tripo-round4/slots/s<N>/  one directory per running heavy job (mkdir is atomic), with an owner file
#   .work/tripo-round4/hold         an exclusive hold: while it exists no new heavy job starts
#   .work/tripo-round4/blender.lock the old single Blender lock, still counted as one busy slot while it exists
# The 3D lanes' gated.sh takes a slot from the same pool for each Blender step, so a 3D run never takes a slot itself.
# Usage:
#   scripts/heavy-slot.sh status
#   scripts/heavy-slot.sh take "<what runs, which session>" [max_wait_seconds]              one slot of the pool
#   scripts/heavy-slot.sh take --exclusive "<what runs, which session>" [max_wait_seconds]  every slot (a Windows game launch, a quiet benchmark)
#   scripts/heavy-slot.sh release "<same text or a unique part of it>"
# Take only when the job is ready to start, check the exit code directly (never through a pipe), and release in a trap on EXIT.
# Pool size: HEAVY_SLOTS, default 4. Every start also needs the machine lines: on Windows free RAM at least 6 GB and commit under
# 62 GB, on every machine at least 30 GB free disk. An exclusive take first writes the hold (so nothing new starts), then waits until
# every slot is free. A take prints TAKEN and exits 0. Exit codes: 0 done, 2 usage, 3 timed out waiting, 4 release refused.
set -u
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
dir="$root/.work/tripo-round4"
hold="$dir/hold"
legacy="$dir/blender.lock"
slots="$dir/slots"
# The pool size is read again on every loop: HEAVY_SLOTS if set, else the number in .work/tripo-round4/pool-size, else 4.
# A take that waits keeps up with a change of size this way.
pool_n() {
  if [ -n "${HEAVY_SLOTS:-}" ]; then echo "$HEAVY_SLOTS"
  elif [ -f "$dir/pool-size" ]; then tr -cd '0-9' < "$dir/pool-size"
  else echo 4; fi
}
n=$(pool_n)

machine_ok() {
  free_kb=$(df -k "$dir" | tail -1 | awk '{print $4}')
  if [ "$free_kb" -lt 31457280 ]; then echo "disk: under 30 GB free"; return 1; fi
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      line=$(powershell -NoProfile -Command '$os = Get-CimInstance Win32_OperatingSystem; $pf = Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory; "{0:N1} {1:N1}" -f ($os.FreePhysicalMemory/1MB), ($pf.CommittedBytes/1GB)' | tr -d '\r' | tr ',' '.')
      free=${line% *}; commit=${line#* }
      if awk -v f="$free" -v c="$commit" 'BEGIN { exit !((f + 0) < 6 || (c + 0) >= 62) }'; then
        echo "memory: free RAM $free GB, commit $commit GB (need 6 GB free and commit under 62 GB)"; return 1
      fi ;;
  esac
  return 0
}

used() {
  count=0
  [ -d "$legacy" ] && count=$((count + 1))
  i=1
  while [ "$i" -le "$n" ]; do
    [ -d "$slots/s$i" ] && count=$((count + 1))
    i=$((i + 1))
  done
  echo "$count"
}

list() {
  [ -f "$hold" ] && echo "EXCLUSIVE hold: $(head -1 "$hold" 2>/dev/null)"
  [ -d "$legacy" ] && echo "slot (old blender.lock): $(cat "$legacy/owner" 2>/dev/null)"
  i=1
  while [ "$i" -le "$n" ]; do
    [ -d "$slots/s$i" ] && echo "slot s$i: $(cat "$slots/s$i/owner" 2>/dev/null)"
    i=$((i + 1))
  done
  return 0
}

stamp() { printf '%s %s (pid %s)\n' "$(date '+%Y-%m-%d %H:%M')" "$1" "$PPID"; }

cmd=${1:-}
case "$cmd" in
  status)
    u=$(used)
    if [ -f "$hold" ]; then echo "BUSY exclusive, $u of $n slots used"; list; exit 0; fi
    if [ "$u" -ge "$n" ]; then echo "BUSY $u of $n slots used"; list; exit 0; fi
    if r=$(machine_ok); then echo "FREE $u of $n slots used"; else echo "FREE $u of $n slots used, but WAIT $r"; fi
    list; exit 0 ;;
  take)
    shift
    excl=0
    if [ "${1:-}" = "--exclusive" ]; then excl=1; shift; fi
    what=${1:-}
    [ -n "$what" ] || { echo "usage: heavy-slot.sh take [--exclusive] \"<what runs, which session>\" [max_wait_seconds]" >&2; exit 2; }
    max=${2:-3600}; waited=0
    mkdir -p "$slots"
    mine=""
    while :; do
      n=$(pool_n)
      if [ "$excl" -eq 1 ]; then
        if [ -z "$mine" ]; then
          if [ -f "$hold" ]; then reason="exclusive hold: $(head -1 "$hold" 2>/dev/null)"
          elif ( set -C; stamp "$what" > "$hold" ) 2>/dev/null; then mine=1; reason="waiting for the running slots to finish"
          else reason="another exclusive hold was written first"; fi
        fi
        if [ -n "$mine" ]; then
          if [ "$(used)" -eq 0 ]; then
            if r=$(machine_ok); then echo "TAKEN (exclusive): $(cat "$hold")"; exit 0; else reason=$r; fi
          else reason="waiting for $(used) running slot(s) to finish"; fi
        fi
      else
        if [ -f "$hold" ]; then reason="exclusive hold: $(head -1 "$hold" 2>/dev/null)"
        elif [ "$(used)" -ge "$n" ]; then reason="all $n slots used"
        elif ! reason=$(machine_ok); then :
        else
          i=1; got=""
          while [ "$i" -le "$n" ]; do
            if mkdir "$slots/s$i" 2>/dev/null; then got=$i; break; fi
            i=$((i + 1))
          done
          if [ -n "$got" ]; then
            stamp "$what" > "$slots/s$got/owner"
            sleep 2
            if [ -f "$hold" ] || [ "$(used)" -gt "$n" ]; then
              rm -rf "$slots/s$got"; reason="an exclusive hold or another taker appeared at the same moment, backing off"
            else
              echo "TAKEN: slot s$got: $(cat "$slots/s$got/owner")"; exit 0
            fi
          else reason="all $n slots used"; fi
        fi
      fi
      if [ "$waited" -ge "$max" ]; then
        if [ -n "$mine" ]; then rm -f "$hold"; fi
        echo "TIMEOUT after ${max}s: $reason"; exit 3
      fi
      [ $((waited % 300)) -eq 0 ] && echo "waiting: $reason"
      sleep 15; waited=$((waited + 15))
    done ;;
  release)
    what=${2:-}
    [ -n "$what" ] || { echo "usage: heavy-slot.sh release \"<text from your take>\"" >&2; exit 2; }
    if [ -f "$hold" ] && grep -qF -- "$what" "$hold"; then rm -f "$hold"; echo "RELEASED exclusive"; exit 0; fi
    i=1
    while [ "$i" -le "$n" ]; do
      if [ -f "$slots/s$i/owner" ] && grep -qF -- "$what" "$slots/s$i/owner"; then rm -rf "$slots/s$i"; echo "RELEASED slot s$i"; exit 0; fi
      i=$((i + 1))
    done
    if [ ! -f "$hold" ] && [ "$(used)" -eq 0 ]; then echo "nothing held, nothing to release"; exit 0; fi
    echo "REFUSED: no hold or slot contains that text"; list; exit 4 ;;
  *)
    echo "usage: heavy-slot.sh status | take [--exclusive] \"<what>\" [max_wait_seconds] | release \"<what>\"" >&2; exit 2 ;;
esac
