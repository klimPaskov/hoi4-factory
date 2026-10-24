#!/usr/bin/env python3
"""Collect EXP-01 artifacts into evidence/S01/exp01 and summarize trial sets.

Usage: python -I collect_results.py <repo_root>
Copies trial summaries and logs from .work/exp01, writes trials/summary.json
and prints per-set counts. results.json is written separately by hand-reviewed
check definitions in results_checks.py so every claim names its artifact.
"""
import hashlib
import json
import os
import shutil
import sys

root = sys.argv[1]
work = os.path.join(root, ".work", "exp01")
ev = os.path.join(root, "evidence", "S01", "exp01")
os.makedirs(os.path.join(ev, "trials"), exist_ok=True)
os.makedirs(os.path.join(ev, "logs"), exist_ok=True)

sets = {
    "windows-unpatched": os.path.join(work, "runs-win", "unpatched-trials.jsonl"),
    "windows-unpatched-fin": os.path.join(work, "runs-win", "unpatched-fin-trials.jsonl"),
    "windows-fixed": os.path.join(work, "runs-win", "fixed-trials.jsonl"),
    "windows-control-ready": os.path.join(work, "runs-win", "control-ready-trials.jsonl"),
    "windows-control-ready-fixed": os.path.join(work, "runs-win", "control-ready-fixed-trials.jsonl"),
    "linux-unpatched": os.path.join(work, "out-linux", "unpatched-trials.jsonl"),
    "linux-fixed": os.path.join(work, "out-linux", "fixed-trials.jsonl"),
    "linux-control-ready": os.path.join(work, "out-linux", "control-ready-trials.jsonl"),
    "linux-control-ready-fixed": os.path.join(work, "out-linux", "control-ready-fixed-trials.jsonl"),
    "windows-final-ready": os.path.join(work, "runs-win", "final-ready-trials.jsonl"),
    "windows-final-ready-fixed": os.path.join(work, "runs-win", "final-ready-fixed-trials.jsonl"),
    "linux-final-ready": os.path.join(work, "out-linux", "final-ready-trials.jsonl"),
    "linux-final-ready-fixed": os.path.join(work, "out-linux", "final-ready-fixed-trials.jsonl"),
    "linux-fixed-invalid-corrupt-target": os.path.join(work, "out-linux", "fixed-trials-corrupt-target-invalid.jsonl"),
    "linux-unpatched-interrupted-session": os.path.join(work, "out-linux-partial", "unpatched-trials-interrupted-session.jsonl"),
}

summary = {}
for name, path in sets.items():
    if not os.path.exists(path):
        summary[name] = {"available": False}
        continue
    dest = os.path.join(ev, "trials", name + ".jsonl")
    shutil.copyfile(path, dest)
    rows = [json.loads(line) for line in open(path, encoding="utf-8") if line.strip()]
    by_mode = {}
    for row in rows:
        mode = row["trial"].rsplit("-", 1)[0]
        m = by_mode.setdefault(mode, {"trials": 0, "exact_files": 0, "unapproved_untouched": 0,
                                      "leftover_temp_after_recovery": 0, "actions": {}, "outcomes": {},
                                      "git_index_unchanged": 0, "added_files": []})
        m["trials"] += 1
        m["exact_files"] += bool(row["exact_files"])
        m["unapproved_untouched"] += bool(row["unapproved_bytes_untouched"])
        m["leftover_temp_after_recovery"] += bool(row.get("leftovers_after_recovery"))
        m["git_index_unchanged"] += row.get("git_index_before") == row.get("git_index_after")
        a = str(row.get("recommended_action"))
        m["actions"][a] = m["actions"].get(a, 0) + 1
        o = json.dumps(row.get("recovery_outcome"), sort_keys=True)[:160]
        m["outcomes"][o] = m["outcomes"].get(o, 0) + 1
        m["added_files"].extend(row.get("added_files") or [])
    summary[name] = {"available": True, "artifact": os.path.relpath(dest, root).replace(os.sep, "/"),
                     "sha256": hashlib.sha256(open(dest, "rb").read()).hexdigest(), "modes": by_mode}

for log in os.listdir(os.path.join(work, "logs")):
    if log.endswith((".log", ".err", ".sha256")):
        shutil.copyfile(os.path.join(work, "logs", log), os.path.join(ev, "logs", log))

with open(os.path.join(ev, "trials", "summary.json"), "w", encoding="utf-8") as handle:
    json.dump(summary, handle, indent=1, sort_keys=True)
for name, s in summary.items():
    if not s["available"]:
        print(name, "unavailable")
        continue
    for mode, m in s["modes"].items():
        print(name, mode, {k: v for k, v in m.items() if k != "added_files"}, "added:", len(m["added_files"]))

# Per-trial harness reports and inventory comparisons (small), plus full
# before/after inventories for the first apply trial of each main set.
dirs = {
    "windows-unpatched": ("runs-win", "unpatched"), "windows-unpatched-fin": ("runs-win", "unpatched-fin"),
    "windows-fixed": ("runs-win", "fixed"), "windows-control-ready": ("runs-win", "control-ready"),
    "windows-control-ready-fixed": ("runs-win", "control-ready-fixed"), "windows-final-ready": ("runs-win", "final-ready"),
    "windows-final-ready-fixed": ("runs-win", "final-ready-fixed"), "linux-unpatched": ("out-linux", "unpatched"),
    "linux-fixed": ("out-linux", "fixed"), "linux-control-ready": ("out-linux", "control-ready"),
    "linux-control-ready-fixed": ("out-linux", "control-ready-fixed"), "linux-final-ready": ("out-linux", "final-ready"),
    "linux-final-ready-fixed": ("out-linux", "final-ready-fixed"),
}
for name, (base, label) in dirs.items():
    src = os.path.join(work, base, label)
    if not os.path.isdir(src):
        continue
    for trial in sorted(os.listdir(src)):
        tdir = os.path.join(src, trial)
        out = os.path.join(ev, "trials", name, trial)
        os.makedirs(out, exist_ok=True)
        wanted = ["report.json", "compare.json", "fixture-git-status.txt", "harness.log",
                  "report-retry.json", "compare-retry.json", "harness-retry.log"]
        if trial in ("apply-1", "none-1"):
            wanted += ["before.json", "after.json", "before-retry.json", "after-retry.json"]
        for f in wanted:
            if os.path.exists(os.path.join(tdir, f)):
                shutil.copyfile(os.path.join(tdir, f), os.path.join(out, f))
print("trial dirs copied")
