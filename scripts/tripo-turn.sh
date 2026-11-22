#!/bin/sh
# One session at a time drives Tripo in the maintainer's Chrome (D-114). A session takes the Tripo turn before its first Tripo
# step and gives it back right after its last one (usually the export), so several 3D workers can share the one browser.
#   .work/tripo-browser.lock/  the turn (mkdir is atomic), with an owner file
# Usage:
#   scripts/tripo-turn.sh status
#   scripts/tripo-turn.sh take "<what, which session>" [max_wait_seconds]   (default 3600)
#   scripts/tripo-turn.sh release "<same text or a unique part of it>"
# Take the turn only when your Tripo steps are ready, check the exit code directly, and release as soon as the browser is free.
# Exit codes: 0 done, 2 usage, 3 timed out waiting, 4 release refused.
set -u
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
lock="$root/.work/tripo-browser.lock"
cmd=${1:-}
case "$cmd" in
  status)
    if [ -d "$lock" ]; then echo "BUSY: $(cat "$lock/owner" 2>/dev/null)"; else echo "FREE"; fi
    exit 0 ;;
  take)
    what=${2:-}
    [ -n "$what" ] || { echo "usage: tripo-turn.sh take \"<what, which session>\" [max_wait_seconds]" >&2; exit 2; }
    max=${3:-3600}; waited=0
    mkdir -p "$root/.work"
    while :; do
      if mkdir "$lock" 2>/dev/null; then
        printf '%s %s (pid %s)\n' "$(date '+%Y-%m-%d %H:%M')" "$what" "$PPID" > "$lock/owner"
        echo "TAKEN: $(cat "$lock/owner")"; exit 0
      fi
      if [ "$waited" -ge "$max" ]; then echo "TIMEOUT after ${max}s: turn held by $(cat "$lock/owner" 2>/dev/null)"; exit 3; fi
      [ $((waited % 300)) -eq 0 ] && echo "waiting: turn held by $(cat "$lock/owner" 2>/dev/null)"
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
