set -u
apt-get update -qq >/dev/null; apt-get install -y -qq --no-install-recommends strace libdbus-1-3 >/dev/null
HBIN=/target/debug/deps/hoi4_mod_setup-b5b13ec949d466c9
useradd -u 10001 -M -s /usr/sbin/nologin exp01
for MODE in 0711 0755; do
  rm -rf /srv/t && mkdir -p /srv/t/ws/acct /srv/t/state
  sh /probe/build_fixture.sh /probe/fixtures/import /srv/t/ws/acct/project >/dev/null 2>&1
  chown -R exp01:exp01 /srv/t/ws/acct/project /srv/t/state; chmod $MODE /srv/t/ws/acct
  runuser -u exp01 -- env EXP01_TRIAL_KILL=none EXP01_SKIP_RECOVERY=1 EXP01_PROJECT=/srv/t/ws/acct/project EXP01_APP=/srv/t/state/app EXP01_REPORT=/srv/t/state/r.json EXP01_OPS=3 EXP01_SIZE=100 \
    strace -f -o /tmp/st-$MODE.txt "$HBIN" --exact transaction::tests::exp01_repro::exp01_trial --nocapture >/dev/null 2>&1
  echo "MODE=$MODE worker_result=$(python3 -c 'import json;print(json.load(open("/srv/t/state/r.json"))["worker_result"])')"
  grep EACCES /tmp/st-$MODE.txt | grep -v "/proc\|/sys\|ld.so\|\.so" | head -8
done
