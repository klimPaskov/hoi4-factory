#!/bin/sh
# Time Hagane Studio's focus editor round trip on F-FOCUS-SMALL (S04 gate 10).
# Runs inside hf-hagane-measure:1 with the fixture mounted at /fixture (read-only).
set -eu
export HOME=/tmp/home XDG_CONFIG_HOME=/tmp/home/.config
mkdir -p "$HOME" /tmp/out /tmp/projects "$XDG_CONFIG_HOME/Hagane Works"
printf '{"projects_dir":"/tmp/projects","game_dir":"/game"}' > "$XDG_CONFIG_HOME/Hagane Works/settings.json"
haganeworks --no-window --port 37330 >/tmp/hw.log 2>&1 &
B=http://127.0.0.1:37330
ms() { date +%s%3N; }
p() { printf '%s
' "$1"; }
i=0
until curl -fs -c /tmp/jar "$B/" >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 300 ] && { cat /tmp/hw.log; exit 2; }; sleep 0.2; done
H="Origin: $B"
P=$(curl -fsS -b /tmp/jar -H "$H" -H 'Content-Type: application/json' -d '{"name":"FocusSmall","country_tag":"XAA","country_name":"Test"}' "$B/api/go/projects")
PID=$(p "$P" | jq -r '.id // .project.id')
echo "project $PID"
IMP=$(curl -sS -b /tmp/jar -H "$H" -F "file=@/fixture/common/national_focus/exp_basic.txt" "$B/api/go/projects/$PID/import/focus-tree")
echo "import $(p "$IMP" | head -c 900)"
TID=$(curl -fsS -b /tmp/jar -H "$H" "$B/api/go/projects/$PID/focus-trees" | jq -r ".[0].id")
NODES=$(curl -fsS -b /tmp/jar -H "$H" "$B/api/go/projects/$PID/focus-trees/$TID/nodes")
NID=$(p "$NODES" | jq -r '.[] | select(.focus_id == "exp_right") | .id')
echo "tree $TID node exp_right=$NID nodes=$(p "$NODES" | jq length)"
round=""
k=0
while [ $k -lt 5 ]; do
  x=$((7 + k % 2))
  t0=$(ms)
  out=$(curl -sS -w '\n%{http_code}' -b /tmp/jar -H "$H" -H 'Content-Type: application/json' -X PUT \
    -d "{\"positions\":[{\"id\":$NID,\"x\":$x,\"y\":1}]}" "$B/api/go/projects/$PID/focus-trees/$TID/node-positions")
  code=$(p "$out" | tail -1)
  got=$(curl -fsS -b /tmp/jar -H "$H" "$B/api/go/projects/$PID/focus-trees/$TID/nodes" | jq -r ".[] | select(.id == $NID) | .x")
  t1=$(ms)
  [ "$code" = 200 ] && [ "$got" = "$x" ] || { echo "move $k failed: $code x=$got $(p "$out" | head -c 400)"; exit 3; }
  round="$round $((t1 - t0))"
  k=$((k+1))
done
echo "move_round_trip_ms$round"
TOKEN=$(cat "$XDG_CONFIG_HOME/Hagane Works/tools-token" 2>/dev/null || find "$HOME" -name tools-token -exec cat {} \;)
exp=""
k=0
while [ $k -lt 5 ]; do
  x=$((7 + k % 2))
  rm -rf /tmp/out/* /tmp/unz
  t0=$(ms)
  curl -sS -o /dev/null -b /tmp/jar -H "$H" -H 'Content-Type: application/json' -X PUT \
    -d "{\"positions\":[{\"id\":$NID,\"x\":$x,\"y\":1}]}" "$B/api/go/projects/$PID/focus-trees/$TID/node-positions"
  r=$(curl -sS -w '\n%{http_code}' -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
    -d "{\"project_id\":$PID,\"save_to\":\"/tmp/out\"}" "$B/api/tools/export_project")
  t1=$(ms)
  [ "$(p "$r" | tail -1)" = 200 ] || { echo "export $k failed: $(p "$r" | head -c 600)"; exit 4; }
  z=$(ls /tmp/out/*.zip | head -1)
  mkdir -p /tmp/unz && (cd /tmp/unz && unzip -q -o "$z")
  exp="$exp $((t1 - t0))"
  k=$((k+1))
done
echo "move_and_export_ms$exp"
f=$(grep -rl "exp_right" /tmp/unz 2>/dev/null | grep national_focus | head -1 || true)
if [ -n "$f" ]; then
  echo "exported focus file: ${f#/tmp/unz/}"
  awk '/id = exp_right/{p=1} p&&/x = /{print "exp_right " $0; exit}' "$f"
  echo "--- exported tree"
  cat "$f"; mkdir -p /out && cp "$f" /out/hagane-export-exp_basic.txt
fi
