#!/bin/sh
# T-ENG-01: the server image runs HOI4 Agent Tools with its bundled hoi4-engine, and an image
# built without the binary (ENGINE_BINARY=omit) answers "unavailable" instead of falling back.
#
#   scripts/t-eng-01.sh [image]       default image: hoi4-factory-server:latest (build it first
#                                     with scripts/compose.sh build server)
#
# For each image it runs `factory-server engine-status` and an engine-backed `hoi4.focus_rewrite`
# (layoutMode "engine") on the F-FOCUS-SMALL fixture, offline (--network none) in a throwaway
# container. Results go to .work/s04/t-eng-01/. The no-engine image is removed afterwards.
set -eu
cd "$(dirname "$0")/.."
IMAGE="${1:-hoi4-factory-server:latest}"
OMIT=hoi4f-t-eng-01:omit
OUT=.work/s04/t-eng-01
FIXTURE=crates/server/tests/fixtures/focus-small
rm -rf "$OUT"
mkdir -p "$OUT/probe"
cp scripts/mcp-call.mjs "$OUT/probe/"
cat >"$OUT/probe/calls.json" <<'EOF'
[{"name":"hoi4.focus_rewrite","arguments":{"relativePath":"common/national_focus/exp_basic.txt","treeId":"exp_basic_tree","layoutMode":"engine"}}]
EOF

hostpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
export MSYS_NO_PATHCONV=1
FIX_MOUNT="$(hostpath "$PWD/$FIXTURE")"
PROBE_MOUNT="$(hostpath "$PWD/$OUT/probe")"

docker build -q -f docker/server.Dockerfile --build-arg ENGINE_BINARY=omit -t "$OMIT" . >/dev/null

status=0
for img in "$IMAGE" "$OMIT"; do
  tag=$(printf '%s' "$img" | tr ':/' '__')
  set +e
  docker run --rm --network none --entrypoint /usr/local/bin/factory-server "$img" engine-status \
    >"$OUT/$tag.engine-status.json"
  echo "exit=$?" >>"$OUT/$tag.engine-status.json"
  docker run --rm --network none --tmpfs /tmp:exec \
    -v "$FIX_MOUNT:/fixture:ro" -v "$PROBE_MOUNT:/probe:ro" --entrypoint /bin/sh "$img" -c '
      cp -r /fixture /tmp/mod && cd /tmp/mod &&
      before=$(sha256sum common/national_focus/exp_basic.txt) &&
      HOME=/tmp/home XDG_STATE_HOME=/tmp/state /opt/node22/bin/node /probe/mcp-call.mjs \
        /opt/hoi4-agent-tools/dist/bin/stdio.js /probe/calls.json &&
      after=$(sha256sum common/national_focus/exp_basic.txt) &&
      echo "{\"source_unchanged\": $([ "$before" = "$after" ] && echo true || echo false), \"files\": \"$(find . -type f | sort | tr "\n" " ")\"}"' \
    >"$OUT/$tag.rewrite.jsonl" 2>"$OUT/$tag.rewrite.stderr"
  echo "exit=$?" >>"$OUT/$tag.rewrite.jsonl"
  set -e
done

docker image rm "$OMIT" >/dev/null

with=$(printf '%s' "$IMAGE" | tr ':/' '__')
without=$(printf '%s' "$OMIT" | tr ':/' '__')
check() { if grep -q "$2" "$OUT/$1"; then echo "ok: $1 has $2"; else echo "FAIL: $1 lacks $2"; status=1; fi; }
check "$with.engine-status.json" '"state":"available"'
check "$with.engine-status.json" 'exit=0'
check "$with.rewrite.jsonl" '"code":"FOCUS_CHANGES_APPLIED"'
check "$without.engine-status.json" '"reason":"binary_not_found"'
check "$without.engine-status.json" 'exit=1'
check "$without.rewrite.jsonl" 'FOCUS_ENGINE_UNAVAILABLE'
check "$without.rewrite.jsonl" '"source_unchanged": true'
if grep -q '"files": .*focus-plan.json' "$OUT/$without.rewrite.jsonl"; then
  echo "FAIL: the no-engine rewrite wrote a plan file"; status=1
else
  echo "ok: the no-engine rewrite wrote nothing"
fi
exit $status
