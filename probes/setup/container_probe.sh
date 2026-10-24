#!/bin/bash
# EXP-01 binding probe run inside the pinned Linux Rust image.
# Mounts: /probe (probes/setup, ro), /setup-src (Setup clone with the harness, ro), /out (evidence staging).
set -u
OUT=/out/probe; mkdir -p "$OUT"
log() { echo "== $*"; }
rustc -V; cargo -V
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends libdbus-1-dev pkg-config >/dev/null; echo "apt_exit=$?"
dpkg-query -W -f='${Package} ${Version}\n' libdbus-1-dev pkg-config

log "build probe crate with git dependencies pinned to 4488a418"
rm -rf /work && mkdir -p /work && cp -a /probe /work/probe && cd /work/probe
rm -rf target
cp /setup-src/Cargo.lock ./Cargo.lock   # seed with upstream's resolved versions
cargo build --release > "$OUT/probe-build.log" 2>&1; echo "probe_build_exit=$?"
tail -n 3 "$OUT/probe-build.log"
sha256sum Cargo.lock | tee "$OUT/probe-cargo-lock.sha256"
cp Cargo.lock "$OUT/probe-Cargo.lock"
cargo tree -e normal > "$OUT/probe-tree-normal.txt" 2>&1
cargo tree -e build > "$OUT/probe-tree-build.txt" 2>&1
echo "normal_tauri_crates=$(grep -c ' tauri' "$OUT/probe-tree-normal.txt")"
echo "normal_webkit_gtk=$(grep -Ec 'webkit|gtk|wry|tao ' "$OUT/probe-tree-normal.txt")"
echo "build_tauri_build=$(grep -c 'tauri-build' "$OUT/probe-tree-build.txt")"
grep -E 'libdbus-sys|keyring|dbus-secret' "$OUT/probe-tree-normal.txt" | sort -u | head
PROBE_BIN=${CARGO_TARGET_DIR:-/work/probe/target}/release/exp01-setup-probe
sha256sum "$PROBE_BIN" | tee "$OUT/probe-bin.sha256"
ldd "$PROBE_BIN" | tee "$OUT/probe-ldd.txt"

log "build harness from the Setup clone (upstream plus test harness only)"
rm -rf /build && cp -a /setup-src /build && cd /build && git config --global --add safe.directory /build
echo "setup_rev=$(git rev-parse HEAD)"; git diff --stat
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib --no-run --message-format=json > /tmp/h.json 2>/tmp/h.err; echo "harness_build_exit=$?"
HBIN=$(python3 -I -c 'import json
for l in open("/tmp/h.json"):
    try: m=json.loads(l)
    except Exception: continue
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["target"]["name"]=="hoi4_mod_setup": print(m["executable"])' | tail -n1)
sha256sum "$HBIN" | tee "$OUT/harness-bin.sha256"

log "server workspace layout"
useradd -u 10001 -M -s /usr/sbin/nologin exp01 2>/dev/null || true
rm -rf /srv/hoi4f && mkdir -p /srv/hoi4f/workspaces/acct-a /srv/hoi4f/state/acct-a /srv/hoi4f/outside
echo "outside canary, must never be read or changed" > /srv/hoi4f/outside/canary.txt
WS=/srv/hoi4f/workspaces
sh /probe/build_fixture.sh /probe/fixtures/import $WS/acct-a/project > "$OUT/fixture-git-status.txt" 2>&1
sh /probe/build_fixture.sh /probe/fixtures/import $WS/acct-a/project2 > /dev/null 2>&1
mkdir -p $WS/acct-a/new-empty
ln -s /etc $WS/acct-a/escape-link
echo 'name="sibling launcher canary"' > $WS/acct-a/project.mod
chown -R exp01:exp01 $WS/acct-a/project $WS/acct-a/project2 $WS/acct-a/new-empty /srv/hoi4f/state/acct-a
chmod 0711 $WS/acct-a          # the probe user may traverse but not list the account directory
chmod 0600 $WS/acct-a/project.mod
ls -la $WS/acct-a > "$OUT/workspace-layout.txt"

log "inventories before"
for p in project project2 new-empty; do python3 -I /probe/inventory.py $WS/acct-a/$p "$OUT/$p-before.json"; done
python3 -I /probe/inventory.py /srv/hoi4f/outside "$OUT/outside-before.json"

log "probe run 1: confinement, scan, plan gate, apply gate, fresh recovery (as uid 10001)"
runuser -u exp01 -- "$PROBE_BIN" $WS acct-a/project /srv/hoi4f/state/acct-a > "$OUT/probe-run1.json" 2> "$OUT/probe-run1.err"; echo "probe_run1_exit=$?"
python3 -I /probe/inventory.py $WS/acct-a/project "$OUT/project-after-run1.json"
python3 -I /probe/compare_inventory.py "$OUT/project-before.json" "$OUT/project-after-run1.json" "$OUT/project-compare-run1.json"

log "probe run 2: new-empty root, scan and gate"
runuser -u exp01 -- "$PROBE_BIN" $WS acct-a/new-empty /srv/hoi4f/state/acct-a > "$OUT/probe-run-new.json" 2> "$OUT/probe-run-new.err"; echo "probe_run_new_exit=$?"
python3 -I /probe/inventory.py $WS/acct-a/new-empty "$OUT/new-empty-after.json"
python3 -I /probe/compare_inventory.py "$OUT/new-empty-before.json" "$OUT/new-empty-after.json" "$OUT/new-empty-compare.json"

log "interrupt a real apply on project2 with the harness, recovery left to the probe"
INT_APP=/srv/hoi4f/state/acct-a/setup-app-interrupted
runuser -u exp01 -- env EXP01_TRIAL_KILL=apply EXP01_SKIP_RECOVERY=1 EXP01_PROJECT=$WS/acct-a/project2 EXP01_APP=$INT_APP \
  EXP01_REPORT=/srv/hoi4f/state/acct-a/interrupt-report.json EXP01_OPS=200 EXP01_SIZE=262144 \
  "$HBIN" --exact transaction::tests::exp01_repro::exp01_trial --nocapture > "$OUT/interrupt-harness.log" 2>&1; echo "interrupt_exit=$?"
cp /srv/hoi4f/state/acct-a/interrupt-report.json "$OUT/interrupt-report.json"
python3 -I /probe/inventory.py $WS/acct-a/project2 "$OUT/project2-after-kill.json"
log "probe run 3: embedded recovery with no provider configured"
runuser -u exp01 -- env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY "$PROBE_BIN" $WS acct-a/project2 /srv/hoi4f/state/acct-a setup-app-interrupted > "$OUT/probe-run-recover.json" 2> "$OUT/probe-run-recover.err"; echo "probe_run_recover_exit=$?"
python3 -I /probe/inventory.py $WS/acct-a/project2 "$OUT/project2-after-recover.json"
python3 -I /probe/compare_inventory.py "$OUT/project2-before.json" "$OUT/project2-after-recover.json" "$OUT/project2-compare-recover.json"
python3 -I /probe/inventory.py /srv/hoi4f/outside "$OUT/outside-after.json"
python3 -I /probe/compare_inventory.py "$OUT/outside-before.json" "$OUT/outside-after.json" "$OUT/outside-compare.json"
ls -la $WS/acct-a > "$OUT/workspace-layout-after.txt" 2>&1
echo done
