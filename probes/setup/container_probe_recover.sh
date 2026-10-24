#!/bin/bash
# EXP-01 probe, part 2: interrupt a real apply with the harness, then recover
# through the embedded library (probe binary) with no provider configured.
# Reuses binaries built by container_probe.sh in the same target volume.
# The account directory is 0755 here: Setup's RootedDir opens every ancestor
# with O_RDONLY|O_DIRECTORY, so a traverse-only (0711) parent fails apply.
set -u
OUT=/out/probe-recover; mkdir -p "$OUT"
apt-get update -qq >/dev/null; apt-get install -y -qq --no-install-recommends libdbus-1-3 >/dev/null
PROBE_BIN=/target/release/exp01-setup-probe
HBIN=$(ls -t /target/debug/deps/hoi4_mod_setup-* | grep -v '\.d$' | head -n1)
sha256sum "$PROBE_BIN" "$HBIN" | tee "$OUT/bins.sha256"
useradd -u 10001 -M -s /usr/sbin/nologin exp01 2>/dev/null || true
WS=/srv/hoi4f/workspaces; ST=/srv/hoi4f/state/acct-a
rm -rf /srv/hoi4f && mkdir -p $WS/acct-a $ST
sh /probe/build_fixture.sh /probe/fixtures/import $WS/acct-a/project2 > /dev/null 2>&1
chown -R exp01:exp01 $WS/acct-a/project2 $ST; chmod 0755 $WS/acct-a
for attempt in 1 2 3; do
  rm -rf $ST/setup-app-interrupted
  python3 -I /probe/inventory.py $WS/acct-a/project2 "$OUT/project2-before-$attempt.json"
  runuser -u exp01 -- env EXP01_TRIAL_KILL=apply EXP01_SKIP_RECOVERY=1 EXP01_PROJECT=$WS/acct-a/project2 EXP01_APP=$ST/setup-app-interrupted \
    EXP01_REPORT=$ST/interrupt-report-$attempt.json EXP01_OPS=200 EXP01_SIZE=262144 \
    "$HBIN" --exact transaction::tests::exp01_repro::exp01_trial --nocapture > "$OUT/interrupt-harness-$attempt.log" 2>&1
  cp $ST/interrupt-report-$attempt.json "$OUT/"
  python3 -I /probe/inventory.py $WS/acct-a/project2 "$OUT/project2-after-kill-$attempt.json"
  runuser -u exp01 -- env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY "$PROBE_BIN" $WS acct-a/project2 $ST setup-app-interrupted > "$OUT/probe-run-recover-$attempt.json" 2> "$OUT/probe-run-recover-$attempt.err"
  echo "attempt=$attempt probe_exit=$?"
  python3 -I /probe/inventory.py $WS/acct-a/project2 "$OUT/project2-after-recover-$attempt.json"
  python3 -I /probe/compare_inventory.py "$OUT/project2-before-$attempt.json" "$OUT/project2-after-recover-$attempt.json" "$OUT/project2-compare-recover-$attempt.json"
  # Remove any leftover so the next attempt starts from the same baseline bytes.
  find $WS/acct-a/project2 -name '.hoi4ms-*' -print -delete
  rm -rf $WS/acct-a/project2/exp01_setup/managed
done
echo done
