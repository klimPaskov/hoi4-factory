#!/bin/bash
# Drive EXP-01 interruption trials against the upstream transaction service.
# Usage: run_trials.sh <test_binary> <probe_dir> <runs_dir> <label> <plan>
# <plan> is a space separated list of mode:count, for example "apply:20 none:2".
# The Rust harness only plans, kills and recovers. Inventories come from
# inventory.py and compare_inventory.py, which share no code with Setup.
set -u
BIN="$1"; PROBE="$2"; RUNS="$3"; LABEL="$4"; PLAN="$5"
PY="${PYTHON:-python3}"
OPS="${EXP01_OPS:-200}"; SIZE="${EXP01_SIZE:-262144}"
mkdir -p "$RUNS"
SUMMARY="$RUNS/$LABEL-trials.jsonl"
: > "$SUMMARY"
for item in $PLAN; do
  mode="${item%%:*}"; count="${item##*:}"
  for n in $(seq 1 "$count"); do
    D="$RUNS/$LABEL/$mode-$n"
    rm -rf "$D"; mkdir -p "$D/workspaces/acct-a" "$D/state"
    P="$D/workspaces/acct-a/project"
    sh "$PROBE/build_fixture.sh" "$PROBE/fixtures/import" "$P" > "$D/fixture-git-status.txt" 2>&1
    "$PY" -I "$PROBE/inventory.py" "$P" "$D/before.json" > /dev/null
    EXP01_TRIAL_KILL="$mode" EXP01_PROJECT="$P" EXP01_APP="$D/state/setup-app" EXP01_REPORT="$D/report.json" \
      EXP01_OPS="$OPS" EXP01_SIZE="$SIZE" \
      "$BIN" --exact transaction::tests::exp01_repro::exp01_trial --nocapture > "$D/harness.log" 2>&1
    hexit=$?
    "$PY" -I "$PROBE/inventory.py" "$P" "$D/after.json" > /dev/null
    cmp=$("$PY" -I "$PROBE/compare_inventory.py" "$D/before.json" "$D/after.json" "$D/compare.json" \
      exp01_setup/managed exp01_setup/replace-me.txt .hoi4-mod-setup)
    "$PY" -I - "$D" "$mode" "$n" "$hexit" "$cmp" >> "$SUMMARY" <<'EOF'
import json, sys
d, mode, n, hexit, cmp = sys.argv[1:]
rep = json.load(open(d + "/report.json")) if __import__("os").path.exists(d + "/report.json") else {}
c = json.load(open(d + "/compare.json"))
rec = rep.get("recovery", {})
print(json.dumps({
  "trial": f"{mode}-{n}", "harness_exit": int(hexit), "killed_at": rep.get("killed_at"),
  "worker_result": rep.get("worker_result"), "worker_exit_code": rep.get("worker_exit_code"),
  "recommended_action": rec.get("recommended_action"), "state_before": rec.get("state_before"),
  "recovery_outcome": rec.get("outcome"), "leftovers_after_kill": rep.get("hoi4ms_leftovers_after_kill"),
  "leftovers_after_recovery": rep.get("hoi4ms_leftovers_after_recovery"),
  "unapproved_bytes_untouched": c["unapproved_bytes_untouched"], "exact_files": c["exact_files"],
  "added_files": c["all"]["added_files"], "added_dirs": c["all"]["added_dirs"],
  "removed": c["all"]["removed"], "content_changed": c["all"]["content_changed"],
  "unapproved_metadata_only": c["unapproved"]["metadata_only"],
  "git_index_before": c["git_index_sha256"]["before"], "git_index_after": c["git_index_sha256"]["after"],
  "git_status_identical": c["git_status_identical"], "git_ls_files_identical": c["git_ls_files_identical"],
}))
EOF
    tail -n 1 "$SUMMARY" | cut -c1-400
  done
done
