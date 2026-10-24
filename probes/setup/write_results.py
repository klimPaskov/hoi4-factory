#!/usr/bin/env python3
"""Write evidence/S01/exp01/results.json from reviewed check definitions.

Usage: python -I write_results.py <repo_root>
Every artifact listed is hashed at write time. A missing artifact is recorded
as missing, never silently dropped.
"""
import hashlib
import json
import os
import sys

root = sys.argv[1]
EV = "evidence/S01/exp01"
IMG = "rust@sha256:af306cfa71d987911a781c37b59d7d67d934f49684058f96cf72079c3626bfe0 (rust:1.88-bookworm)"
WIN = "Windows 11 host, rustup toolchain 1.88.0-x86_64-pc-windows-msvc (rustc 1.88.0 6b00bc388)"
REV = "HOI4-Mod-Setup 4488a418cad56a158273cb66c4ce7794696b7ed1"
REPRO = REV + " plus local test harness only (patch/harness-include.diff, patch/exp01_repro.rs)"
FIXED = REV + " plus harness plus confirming fix (patch/fix-placement-temp.diff)"
summary = json.load(open(os.path.join(root, EV, "trials", "summary.json"), encoding="utf-8"))


def art(path):
    full = os.path.join(root, path)
    if not os.path.exists(full):
        return {"path": path, "sha256": None, "missing": True}
    return {"path": path, "sha256": hashlib.sha256(open(full, "rb").read()).hexdigest()}


def tests_line(path):
    full = os.path.join(root, path)
    if not os.path.exists(full):
        return None
    lines = [l.strip() for l in open(full, encoding="utf-8", errors="replace") if l.startswith("test result")]
    return lines[-1] if lines else None


def trial_set(name):
    s = summary.get(name, {})
    return {k: {kk: vv for kk, vv in v.items() if kk != "added_files"} for k, v in s.get("modes", {}).items()} if s.get("available") else "unavailable"


checks = []
_off = os.path.join(root, EV, "logs", "linux-upstream-offline.log")
_off_text = open(_off, encoding="utf-8", errors="replace").read() if os.path.exists(_off) else ""
if "cargo_exit=" in _off_text:
    OFFLINE_EXIT = int(_off_text.split("cargo_exit=")[-1].split()[0])
    OFFLINE_RESULT = tests_line(EV + "/logs/linux-upstream-offline.log") or "no test result line"
    OFFLINE_LIMIT = ("build_plan failed before any write: Process('the trusted native Node runtime is unavailable'). "
                     "The rust image has no Node. Rerun with a pinned Node was not attempted to keep the work bounded, so this check is unavailable, not failed")
else:
    OFFLINE_EXIT = None
    OFFLINE_RESULT = "not run to completion"
    OFFLINE_LIMIT = "Stopped to keep the remaining work bounded. Optional check, recorded as not run"


def check(**kw):
    kw["artifacts"] = [art(p) for p in kw.pop("artifacts")]
    checks.append(kw)


check(id="EXP01-CI-01", title="Fetch failed recovery job log", command="gh run view 39281117181 --repo klimPaskov/HOI4-Mod-Setup --job 118160257466 --log",
      working_dir=EV + "/ci", toolchain="gh 2.87.3", input_revision="run head 4488a418cad56a158273cb66c4ce7794696b7ed1", exit_code=0,
      artifacts=[EV + "/ci/job-118160257466.log", EV + "/ci/run.json"], evidence_kind="remote CI observation",
      result="Job failed at scripts/run_released_e2e.mjs:652 with 'The project was not restored exactly' digest 7a0535da... versus 1641aef2...",
      limitation="GitHub log retention. The log has no per-file inventory")
check(id="EXP01-CI-02", title="Download recovery CI artifact", command="gh run download 39281117181 --repo klimPaskov/HOI4-Mod-Setup -n candidate-acceptance-recovery",
      working_dir=".work/exp01/ci-artifact-recovery", toolchain="gh 2.87.3", input_revision="artifact 12201302573 sha256:5d337672d3c46e90928b894a05058e4506609bad2dcde4553b8c844d294e6952", exit_code=0,
      artifacts=[EV + "/ci/candidate-acceptance-recovery-result.json", EV + "/ci/candidate-acceptance-recovery-artifact.sha256"], evidence_kind="remote CI artifact",
      result="Interruption observed in state 'applying' after a verified non-skip operation, rollback reached 'rolled_back', exact inventory digest differed",
      limitation="Artifact stores only screenshots and digests, so the extra path in CI cannot be named from artifacts")
check(id="EXP01-SRC-01", title="Static reading of assertOriginals and write_atomic_noreplace_from",
      command="read scripts/released_e2e_support.mjs:188-226, src-tauri/src/safe_fs.rs:583-660, src-tauri/src/transaction.rs:4392-4560", working_dir=".work/HOI4-Mod-Setup",
      toolchain="none", input_revision=REV, exit_code=None, artifacts=[], evidence_kind="source reading",
      result="assertOriginals passes per-file checks first, so a digest-only failure means extra files. Placement writes a random .hoi4ms-<uuid>.tmp beside the destination before an exclusive rename and nothing journals or sweeps it",
      limitation="Reading alone is not proof. Trials below supply execution evidence")
check(id="EXP01-TEST-WIN-UPSTREAM", title="Upstream transaction and lifecycle tests on Windows host",
      command="cargo +1.88.0 test --locked --manifest-path src-tauri/Cargo.toml --all-features --lib -- transaction:: commands::lifecycle_tests::", working_dir=".work/HOI4-Mod-Setup",
      toolchain=WIN, input_revision=REV, exit_code=0, artifacts=[EV + "/logs/win-host-upstream-tests.log"], evidence_kind="source test execution",
      result=tests_line(EV + "/logs/win-host-upstream-tests.log"),
      limitation="6 lifecycle tests are #[ignore] upstream and need an explicit package checkout or live installs")
check(id="EXP01-TEST-LINUX-NODEPS", title="Upstream lib tests in Linux container without system deps", command="cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- transaction::",
      working_dir="/build (copy of .work/HOI4-Mod-Setup)", toolchain=IMG, input_revision=REV, exit_code=101, artifacts=[EV + "/logs/linux-nodesktop-nodeps.log"],
      evidence_kind="build execution", result="Build fails: libdbus-sys needs dbus-1 via pkg-config (keyring sync-secret-service on Linux) even with the desktop feature off",
      limitation="None. This is the dependency finding itself")
check(id="EXP01-TEST-LINUX-UPSTREAM", title="Upstream transaction and safe_fs tests in Linux container, desktop feature off",
      command="apt-get install libdbus-1-dev pkg-config; cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- transaction:: safe_fs:: --test-threads=4",
      working_dir="/build (copy of .work/HOI4-Mod-Setup)", toolchain=IMG, input_revision=REV, exit_code=0, artifacts=[EV + "/logs/linux-upstream-nodesktop.log"],
      evidence_kind="source test execution", result=tests_line(EV + "/logs/linux-upstream-nodesktop.log"), limitation="Container-local apt packages. First attempt in the earlier session was interrupted by a Docker restart (logs/linux-nodesktop-tests-interrupted-session.log)")
check(id="EXP01-TEST-LINUX-DESKTOP", title="Upstream lifecycle tests in Linux container with Tauri and webkit deps",
      command="apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev ...; cargo test --locked --manifest-path src-tauri/Cargo.toml --all-features --lib -- commands::lifecycle_tests::",
      working_dir="/build (copy of .work/HOI4-Mod-Setup)", toolchain=IMG, input_revision=REV, exit_code=0, artifacts=[EV + "/logs/linux-upstream-desktop.log"],
      evidence_kind="source test execution", result=tests_line(EV + "/logs/linux-upstream-desktop.log"), limitation="Only 3 lifecycle tests run without an explicit package checkout. The desktop feature requires webkit2gtk and gtk in the build image")
check(id="EXP01-TEST-LINUX-OFFLINE", title="Ignored offline all-component install/import/repair/remove/rollback test",
      command="HOI4_MOD_SETUP_SOURCE_REPOSITORY=/agentic cargo test --all-features --lib -- --ignored --exact commands::lifecycle_tests::offline_lifecycle::offline_all_component_install_import_repair_remove_and_rollback",
      working_dir="/build", toolchain=IMG, input_revision=REV + " with Agentic-HOI4-Modding 856940f2a629e0e2f3e03fc484cb0c0309c96734",
      exit_code=OFFLINE_EXIT, artifacts=[EV + "/logs/linux-upstream-offline.log"], evidence_kind="source test execution", result=OFFLINE_RESULT, limitation=OFFLINE_LIMIT)
for host, toolchain, rev, sets, kind_note in [
    ("windows", WIN, REPRO, ["windows-unpatched", "windows-unpatched-fin"], "Process kill via Child::kill (TerminateProcess), finalization via upstream HOI4_MOD_SETUP_TEST_ABORT_AT=after_lock_write"),
    ("linux", IMG, REPRO, ["linux-unpatched"], "Process kill via Child::kill (SIGKILL), finalization via upstream abort checkpoint"),
]:
    check(id=f"EXP01-REPRO-{host.upper()}", title=f"Interruption trials on unpatched Setup, {host}",
          command="probes/setup/run_trials.sh <harness> probes/setup <runs> unpatched \"none:1 apply:20 staging:5 finalization:3\"",
          working_dir=".work/exp01/runs-win" if host == "windows" else "/runs in container, copied to .work/exp01/out-linux", toolchain=toolchain, input_revision=rev,
          exit_code=0, artifacts=[f"{EV}/trials/{s}.jsonl" for s in sets] + [EV + "/trials/summary.json"], evidence_kind="process-kill execution with independent inventory",
          result={s: trial_set(s) for s in sets}, limitation=kind_note + ". Synthetic 200x256 KiB plan, not the Kaiserreich CI fixture. Plain fixture descriptor lacks picture=, so its finalization trials end at the readiness block and roll back instead of reaching the abort checkpoint")
for host, toolchain, sets in [("windows", WIN, ["windows-fixed"]), ("linux", IMG, ["linux-fixed"])]:
    check(id=f"EXP01-FIX-{host.upper()}", title=f"Same trials with the confirming fix, {host}",
          command="probes/setup/run_trials.sh <fixed harness> probes/setup <runs> fixed \"none:1 apply:20 staging:5 finalization:3\"",
          working_dir=".work/exp01", toolchain=toolchain, input_revision=FIXED, exit_code=0,
          artifacts=[f"{EV}/trials/{s}.jsonl" for s in sets], evidence_kind="process-kill execution with independent inventory",
          result={s: trial_set(s) for s in sets} | {"apply_trials_with_bound_placement_temp_after_kill": sum(
              any(".hoi4ms-placing-" in x for x in (r.get("leftovers_after_kill") or []))
              for s in sets for r in (json.loads(l) for l in open(os.path.join(root, EV, "trials", s + ".jsonl"), encoding="utf-8"))
              if r["trial"].startswith("apply"))},
          limitation="Windows none-1 harness was terminated externally (exit 1, no panic) during its rollback, see EXP01-RETRY-WIN. Linux first fixed attempt is invalid (corrupt rlib in a target volume after the Docker restart, logs/linux-repro-fixed-corrupt-target.log)" if host == "windows" else "Rerun after removing the corrupted target volume")
check(id="EXP01-RETRY-WIN", title="Recover-only retry of an externally interrupted rollback (fixed, Windows)",
      command="EXP01_TRIAL_KILL=recover_only <fixed harness> --exact transaction::tests::exp01_repro::exp01_trial", working_dir=".work/exp01/runs-win/fixed/none-1",
      toolchain=WIN, input_revision=FIXED, exit_code=0,
      artifacts=[EV + "/trials/windows-fixed/none-1/report-retry.json", EV + "/trials/windows-fixed/none-1/compare-retry.json"],
      evidence_kind="recovery execution with independent inventory",
      result="Journal state rolling_back, recommended rollback, retry rolled_back, rollback quarantine leftover settled, exact files restored, only an empty created directory remains",
      limitation="Cause of the external termination is unknown")
for host, toolchain in [("windows", WIN), ("linux", IMG)]:
    sets = [f"{host}-control-ready", f"{host}-control-ready-fixed", f"{host}-final-ready", f"{host}-final-ready-fixed"]
    check(id=f"EXP01-CONTROL-{host.upper()}", title=f"Completed apply and finalization recovery with a readiness-valid descriptor, {host}",
          command="EXP01_DESCRIPTOR_PICTURE=1 run_trials.sh ... control-ready \"none:1 [apply:3]\" and final-ready \"finalization:3\"",
          working_dir=".work/exp01", toolchain=toolchain, input_revision=REPRO + " and " + FIXED, exit_code=0,
          artifacts=[f"{EV}/trials/{s}.jsonl" for s in sets], evidence_kind="process-kill execution with independent inventory",
          result={s: trial_set(s) for s in sets},
          limitation="Approved paths are exp01_setup/managed, exp01_setup/replace-me.txt and .hoi4-mod-setup. The descriptor variant adds one picture= line before the baseline commit")
check(id="EXP01-FIX-REGRESSION", title="Upstream tests on the fixed clone",
      command="cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- transaction:: safe_fs::", working_dir=".work/exp01/setup-fixed",
      toolchain=WIN + " and " + IMG, input_revision=FIXED, exit_code=0,
      artifacts=[EV + "/logs/win-fixed-upstream-tests.log", EV + "/logs/linux-fixed-nodesktop.log"], evidence_kind="source test execution",
      result={"windows": tests_line(EV + "/logs/win-fixed-upstream-tests.log"), "linux": tests_line(EV + "/logs/linux-fixed-nodesktop.log")},
      limitation="Counts include the two env-gated harness tests, which return immediately. The fix has no new upstream unit test")
check(id="EXP01-PROBE-BUILD", title="Probe crate embedding hoi4-setup-core and hoi4-mod-setup (desktop off) as git deps",
      command="cargo build --release (probes/setup, Cargo.lock seeded from upstream)", working_dir="/work/probe", toolchain=IMG,
      input_revision="git rev 4488a418cad56a158273cb66c4ce7794696b7ed1", exit_code=0,
      artifacts=[EV + "/probe-run/probe-build.log", EV + "/probe-run/probe-Cargo.lock", EV + "/probe-run/probe-tree-normal.txt", EV + "/probe-run/probe-ldd.txt", EV + "/probe-run/probe-bin.sha256"],
      evidence_kind="build execution", result="Builds on Linux. No tauri, wry, tao, webkit or gtk crate in the normal graph. tauri-build, tauri-utils and tauri-winres are compiled as build deps of hoi4-mod-setup. Runtime links libdbus-1.so.3 through keyring",
      limitation="Probe calls public scanner and transaction APIs only. commands.rs is not reachable")
check(id="EXP01-PROBE-IMPORT", title="Probe on F-SETUP-IMPORT: confinement, read-only scan, analysis gate, refused apply, fresh recovery",
      command="runuser -u exp01 -- exp01-setup-probe /srv/hoi4f/workspaces acct-a/project /srv/hoi4f/state/acct-a", working_dir="/srv/hoi4f", toolchain=IMG,
      input_revision="probe binary 3ceb0ac76d38192f6906baae24e18090634c25ee2f9d4b6004655330e63e4270", exit_code=0,
      artifacts=[EV + "/probe-run/probe-run1.json", EV + "/probe-run/project-before.json", EV + "/probe-run/project-after-run1.json", EV + "/probe-run/project-compare-run1.json", EV + "/probe-run/outside-compare.json", EV + "/probe-run/workspace-layout.txt"],
      evidence_kind="embedded service execution with independent inventory",
      result="5 of 5 outside paths refused by the probe gate. Scan ok, read_only, semantic_analysis required with status authentication_required. validate_plan and run_transaction refuse with 'a confirmed selected-provider analysis is required before apply', no app root created. Inventory byte-identical including .git/index",
      limitation="The confinement gate is probe code, Setup has none. No model was called by design")
check(id="EXP01-PROBE-NEW", title="Probe on F-SETUP-NEW empty owned directory", command="exp01-setup-probe /srv/hoi4f/workspaces acct-a/new-empty /srv/hoi4f/state/acct-a",
      working_dir="/srv/hoi4f", toolchain=IMG, input_revision="probe binary 3ceb0ac7...", exit_code=0,
      artifacts=[EV + "/probe-run/probe-run-new.json", EV + "/probe-run/new-empty-compare.json"], evidence_kind="embedded service execution with independent inventory",
      result="Scan ok on the empty root, apply refused by the analysis gate, directory still empty",
      limitation="The generated closure of a new project was not produced: the plan builder is desktop-only and analysis was not run")
check(id="EXP01-PROBE-RECOVER", title="Interrupted real apply recovered through the embedded library with no provider",
      command="harness EXP01_TRIAL_KILL=apply EXP01_SKIP_RECOVERY=1, then exp01-setup-probe ... setup-app-interrupted (x3)", working_dir="/srv/hoi4f",
      toolchain=IMG, input_revision=REPRO + " (kill) and upstream git dep (recovery)", exit_code=0,
      artifacts=[EV + "/probe-recover/probe-run-recover-1.json", EV + "/probe-recover/project2-compare-recover-1.json", EV + "/probe-recover/project2-compare-recover-2.json", EV + "/probe-recover/project2-compare-recover-3.json", EV + "/logs/linux-probe-recover.log"],
      evidence_kind="embedded service execution with independent inventory",
      result="3 of 3: find_incomplete_transaction found state applying, recommended rollback, rollback ok, and each left one .hoi4ms-<uuid>.tmp. Same defect through the embedded path",
      limitation="Unpatched upstream library")
check(id="EXP01-PROBE-ANCESTOR", title="Apply under a traverse-only account directory", command="strace -f harness with parent mode 0711 then 0755 (probes/setup/debug_ancestor_access.sh)",
      working_dir="/srv/t", toolchain=IMG, input_revision=REPRO, exit_code=0, artifacts=[EV + "/logs/linux-ancestor-access-strace.log", EV + "/probe-run/interrupt-report.json"],
      evidence_kind="system call trace", result="With 0711 the transaction fails with EACCES on openat(parent fd, 'acct', O_RDONLY|O_DIRECTORY). RootedDir opens every ancestor for reading. Scan works with 0711",
      limitation="Layout constraint for the server, not a security defect in Setup")
results = {
    "schema_version": 1, "experiment": "EXP-01", "stage": "S01", "recorded": "2026-10-24",
    "setup_revision": "4488a418cad56a158273cb66c4ce7794696b7ed1", "image": IMG,
    "factory_base_revision": "290af7c48ce160d4f7461b90aedc92528295330b",
    "fixtures": "probes/setup/fixtures (original synthetic), built by probes/setup/build_fixture.sh",
    "independent_inventory": ["probes/setup/inventory.py", "probes/setup/compare_inventory.py"],
    "checks": checks,
}
with open(os.path.join(root, EV, "results.json"), "w", encoding="utf-8", newline="\n") as handle:
    json.dump(results, handle, indent=1)
print(len(checks), "checks;", sum(1 for c in checks for a in c["artifacts"] if a.get("missing")), "missing artifacts")
