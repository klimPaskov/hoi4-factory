#!/bin/sh
# One session at a time drives Tripo in the maintainer's Chrome (D-114). A session takes the Tripo turn before its first Tripo
# step and gives it back right after its last one (usually the export), so several 3D workers can share the one browser.
#   .work/tripo-browser.lock/   the turn (mkdir is atomic), with an owner file
#   .work/tripo-turn-queue/     one ticket file per waiting take, named by its start time, so the turn goes first come, first served
# Usage:
#   scripts/tripo-turn.sh status
#   scripts/tripo-turn.sh take "<what, which session>" [max_wait_seconds]   (default 3600)
#   scripts/tripo-turn.sh release "<same text or a unique part of it>"
# Take the turn only when your Tripo steps are ready, check the exit code directly, and release as soon as the browser is free.
# A waiting take touches its ticket every 15 s. A ticket untouched for 2 minutes belongs to a take that died, and is removed,
# so a killed waiter never blocks the queue. Exit codes: 0 done, 2 usage, 3 timed out waiting, 4 release refused.
# On Windows the take also waits until free RAM is at least TRIPO_MIN_FREE_GB (default 3). A Tripo page opened with about
# 1 GB free froze Chrome, and a frozen Tripo tab hangs every later Tripo tab, because Chrome renders one site in one process.
set -u
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
lock="$root/.work/tripo-browser.lock"
queue="$root/.work/tripo-turn-queue"
cmd=${1:-}
memnote=""
mem_ok() {
  memnote=""
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) ;;
    *) return 0 ;;
  esac
  need=${TRIPO_MIN_FREE_GB:-3}
  free=$(powershell -NoProfile -Command '"{0:N1}" -f ((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1MB)' 2>/dev/null | tr -d '\r' | tr ',' '.')
  case "$free" in ''|*[!0-9.]*) return 0 ;; esac
  if awk -v f="$free" -v n="$need" 'BEGIN { exit !((f + 0) < (n + 0)) }'; then
    memnote="free RAM $free GB, need $need GB before a Tripo page opens"; return 1
  fi
  return 0
}
case "$cmd" in
  status)
    if [ -d "$lock" ]; then echo "BUSY: $(cat "$lock/owner" 2>/dev/null)"; else echo "FREE"; fi
    if [ -d "$queue" ]; then
      for t in $(ls "$queue" 2>/dev/null | sort); do echo "queued: $(cat "$queue/$t" 2>/dev/null)"; done
    fi
    exit 0 ;;
  take)
    what=${2:-}
    [ -n "$what" ] || { echo "usage: tripo-turn.sh take \"<what, which session>\" [max_wait_seconds]" >&2; exit 2; }
    max=${3:-3600}; waited=0
    mkdir -p "$queue"
    ticket="$queue/$(date +%s)-$$"
    printf '%s\n' "$what" > "$ticket"
    trap 'rm -f "$ticket"' EXIT INT TERM
    me=$(basename "$ticket")
    while :; do
      touch "$ticket"
      find "$queue" -type f -mmin +2 -exec rm -f {} + 2>/dev/null
      first=$(ls "$queue" 2>/dev/null | sort | head -1)
      if [ "$first" = "$me" ] && [ ! -d "$lock" ] && ! mem_ok; then
        [ $((waited % 300)) -eq 0 ] && echo "waiting: $memnote"
      elif [ "$first" = "$me" ] && mkdir "$lock" 2>/dev/null; then
        printf '%s %s (pid %s)\n' "$(date '+%Y-%m-%d %H:%M')" "$what" "$PPID" > "$lock/owner"
        echo "TAKEN: $(cat "$lock/owner")"; exit 0
      fi
      if [ "$waited" -ge "$max" ]; then
        if [ -n "$memnote" ]; then echo "TIMEOUT after ${max}s: $memnote"; else echo "TIMEOUT after ${max}s: turn held by $(cat "$lock/owner" 2>/dev/null)"; fi
        exit 3
      fi
      [ $((waited % 300)) -eq 0 ] && echo "waiting: turn held by $(cat "$lock/owner" 2>/dev/null), first in queue: $(cat "$queue/$first" 2>/dev/null)"
      sleep 15; waited=$((waited + 15))
    done ;;
  release)
    what=${2:-}
    [ -n "$what" ] || { echo "usage: tripo-turn.sh release \"<text from your take>\"" >&2; exit 2; }
    [ -d "$lock" ] || { echo "no turn held, nothing to release"; exit 0; }
    if grep -qF -- "$what" "$lock/owner" 2>/dev/null; then rm -rf "$lock"; echo "RELEASED"; exit 0; fi
    echo "REFUSED: the turn belongs to $(cat "$lock/owner" 2>/dev/null)"; exit 4 ;;
  *)
    echo "usage: tripo-turn.sh status | take \"<what>\" [max_wait_seconds] | release \"<what>\"" >&2; exit 2 ;;
esac
