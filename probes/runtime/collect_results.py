"""Build evidence/S01/exp03/results.json and evidence/S01/exp04/results.json
from the probe run artifacts. Usage, from the repository root:
    python -I probes/runtime/collect_results.py
"""
import hashlib
import json
import pathlib

ROOT = pathlib.Path.cwd()
BASE_REV = "290af7c48ce160d4f7461b90aedc92528295330b"


def sha(p):
    return hashlib.sha256((ROOT / p).read_bytes()).hexdigest()


def load(p):
    return json.loads((ROOT / p).read_text(encoding="utf-8"))


def source_manifest():
    files = []
    for d in ["crates/agent", "probes/runtime/src", "probes/runtime/compare/thin/src", "probes/runtime/compare/async-openai/src", "probes/runtime/recordings"]:
        for f in sorted((ROOT / d).rglob("*")):
            if f.is_file() and "target" not in f.parts:
                files.append(f.relative_to(ROOT).as_posix())
    files += ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "compose.yaml", "probes/runtime/Cargo.toml",
              "probes/runtime/run-exp03.sh", "probes/runtime/run-exp04.sh", "probes/runtime/compose.db-port.yaml",
              "probes/runtime/licenses.py", "probes/runtime/scan_image_listing.py", "probes/runtime/collect_results.py",
              "probes/runtime/compare/body.json", "probes/runtime/compare/measure.sh", "probes/runtime/compare/fault-compare.sh",
              "probes/runtime/compare/async-openai/Cargo.toml", "probes/runtime/compare/async-openai/Cargo.lock",
              "probes/runtime/compare/thin/Cargo.toml", "docker/agent-probe.Dockerfile"]
    rows = {f: sha(f) for f in files if (ROOT / f).exists()}
    agg = hashlib.sha256("\n".join(f"{k} {v}" for k, v in sorted(rows.items())).encode()).hexdigest()
    return rows, agg


def main():
    rows, agg = source_manifest()
    rev = {"base_revision": BASE_REV, "working_tree": "uncommitted S01 changes on main", "source_manifest_sha256": agg}

    def check(id_, req, command, kind, exit_code, artifact, verdict, limitation, detail=None):
        c = {"id": id_, "maps_to": req, "command": command, "revision_inputs": rev, "evidence_kind": kind, "exit_code": exit_code,
             "artifact": {"path": artifact, "sha256": sha(artifact)} if artifact else None, "verdict": verdict, "limitation": limitation}
        if detail is not None:
            c["detail"] = detail
        return c

    KEY_A = 'ANTHROPIC_API_KEY="$(powershell.exe ... GetEnvironmentVariable(\'ANTHROPIC_API_KEY\',\'User\'))" '
    checks = []
    # OpenAI adapter, live refusals only.
    oa = load("evidence/S01/exp03/runs/live-record-quota-refusal.json")
    checks.append(check("EXP03-OPENAI-MODELS-LIST", ["RT-02"], "curl GET https://api.openai.com/v1/models (key from env, not printed)", "live", 0,
                        "evidence/S01/exp03/runs/models-list.json", "pass", "Proves authentication and model availability only. Not inference."))
    checks.append(check("EXP03-OPENAI-AUTH-401", ["RT-06"], "./target/debug/runtime-probe record (auth_refusal case, deliberately invalid key)", "live", 0,
                        "evidence/S01/exp03/runs/live-record-quota-refusal.json",
                        "pass" if oa["auth_refusal"]["error"]["class"] == "auth" and not oa["auth_refusal"]["auto_retry_allowed"] else "fail",
                        "Invalid key at request start. Credential loss mid-task not exercised."))
    checks.append(check("EXP03-OPENAI-QUOTA-429", ["RT-06"], "./target/debug/runtime-probe record (3 model requests, gpt-5.4-nano-2026-03-17)", "live", 0,
                        "evidence/S01/exp03/runs/live-record-quota-refusal.json", "defect found and fixed, live recheck not run",
                        "All three requests returned HTTP 429 type insufficient_quota code credit_balance_exhausted. The first classifier labelled this rate_limited with automatic retry allowed (visible in the artifact). Fixed in crates/agent/src/openai.rs, covered by unit test live_credit_exhausted_is_quota_not_retryable with the provider's own status, type and code. No live re-run under the no-further-OpenAI-calls instruction.",
                        {"observed": oa["tool_turn"]["error"], "spend_usd": 0.0}))
    for id_, req in [("EXP03-OPENAI-STREAM-FAULTS", ["RT-03", "RT-07"]), ("EXP03-OPENAI-LIVE-TOOL-TURN", ["RT-02"]), ("EXP03-OPENAI-LIVE-CANCELLATION", ["RT-10"])]:
        checks.append(check(id_, req, "not run", "live", None, None, "unavailable",
                            "OpenAI returned 429 credit_balance_exhausted for both keys on 2026-10-24 and the maintainer chose not to buy credits. No OpenAI stream exists to record or replay. Synthetic streams were not substituted."))

    # Anthropic adapter, live.
    an = load("evidence/S01/exp03/runs/live-record-anthropic.json")
    rec_cmd = KEY_A + "target/release/runtime-probe record-anthropic"
    tt = an["tool_turn"]["outcome"]
    checks.append(check("EXP03-ANTHROPIC-LIVE-TOOL-TURN", ["RT-02", "T-RUN-05"], rec_cmd, "live", 0, "evidence/S01/exp03/runs/live-record-anthropic.json",
                        "pass" if tt["ok"] and tt["turn"]["finish_reason"] == "tool_use" and tt["turn"]["tool_calls"] else "fail",
                        "One fixture, strict tool, tool_choice auto, thinking disabled. Parallel calls and long contexts not probed.",
                        {"model_reported": an["tool_turn"]["model_reported"], "recording": "probes/runtime/recordings/anthropic-tool-turn.sse"}))
    ft = an["followup_turn"]["outcome"]
    checks.append(check("EXP03-ANTHROPIC-LIVE-FOLLOWUP-TURN", ["RT-02"], rec_cmd, "live", 0, "evidence/S01/exp03/runs/live-record-anthropic.json",
                        "pass" if ft["ok"] and ft["turn"]["finish_reason"] == "end_turn" else "fail",
                        "The tool result sent back is the host broker's real execution receipt in a scratch workspace."))
    ct = an["cancelled_turn"]
    checks.append(check("EXP03-ANTHROPIC-LIVE-CANCELLATION", ["RT-10"], rec_cmd, "live", 0, "evidence/S01/exp03/runs/live-record-anthropic.json",
                        "pass" if ct["outcome"]["class"] == "cancelled_after_acceptance" and ct["outcome"]["auto_retry_allowed"] is False else "fail",
                        "Cancellation drops the connection after 9 events. Provider-side billing after cancel is not observable from the client, so cost is the max_tokens upper bound. The no-retry rule is the classifier's decision. No retry was attempted.",
                        {"cancel_at_ms": ct["cancel_at_ms"], "events_at_cancel": ct["events_at_cancel"], "spend": ct["spend"]}))
    checks.append(check("EXP03-ANTHROPIC-AUTH-401", ["RT-06"], rec_cmd + " (auth_refusal case, deliberately invalid key)", "live", 0,
                        "evidence/S01/exp03/runs/live-record-anthropic.json",
                        "pass" if an["auth_refusal"]["error"]["class"] == "auth" and not an["auth_refusal"]["auto_retry_allowed"] else "fail",
                        "Invalid key at request start. Quota exhaustion on Anthropic was not observed."))

    sf = load("evidence/S01/exp03/runs/stream-faults.json")
    for c in sf["cases"]:
        req = [r for r in ["RT-03", "RT-07", "T-RUN-02"] if r in c["case"]] or ["F-RUNTIME-FAULT positive control"]
        checks.append(check("EXP03-FAULT-" + c["case"].split(" ")[0], req, "target/release/runtime-probe faults", "recorded-real-replay", 0,
                            "evidence/S01/exp03/runs/stream-faults.json", c["verdict"],
                            "Faults injected into recorded real Anthropic streams (claude-haiku-5-5). Replay uses the recorded chunk boundaries. A live provider truncation was not observed.",
                            {"loop_result": c["loop_result"], "effect_observed": c["effect_observed"], "persisted": c["persisted_attempt"]}))

    br = load("evidence/S01/exp03/runs/broker.json")
    for c in br["cases"]:
        req = ["RT-04"] if "RT-04" in c["case"] else (["T-RUN-02"] if "T-RUN-02" in c["case"] else ["RT-04", "T-RUN-02"])
        checks.append(check("EXP03-BROKER-" + c["case"].split(" ")[0], req, "target/release/runtime-probe broker", "recorded-real-replay", 0,
                            "evidence/S01/exp03/runs/broker.json", c["verdict"],
                            c.get("limitation", "Initial arguments are the recorded real model tool call. Hooks, final validation, root policy and filesystem effects are real. Windows host filesystem only.")))

    for phase in ["a", "b"]:
        jr = load(f"evidence/S01/exp03/runs/journal-phase-{phase}.json")
        for c in jr["checks"]:
            req = [r for r in ["T-RUN-03", "T-RUN-04", "ORCH-09"] if r in c["id"]] or ["T-RUN-04"]
            cmd = f"target/release/runtime-probe journal --phase {phase}" + (" (after docker compose restart db)" if phase == "b" else "")
            checks.append(check("EXP03-JOURNAL-" + c["id"].split(" ")[0], req, cmd, "local-execution", 0, f"evidence/S01/exp03/runs/journal-phase-{phase}.json",
                                c["verdict"], "PostgreSQL 17.6 in the Compose stack, real child process aborts, recorded real tool arguments. Single host, no replica clock skew."))

    wk = load("evidence/S01/exp03/runs/worker.json")
    for c in wk["checks"]:
        live = c.get("evidence_kind") == "live"
        checks.append(check("EXP03-WORKER-" + c["id"].split(" ")[0], ["RT-08"] if "RT-08" in c["id"] else ["SPEC-002 worker protocol"],
                            (KEY_A if live else "") + "target/release/runtime-probe worker --live", "live" if live else "recorded-real-replay", 0,
                            "evidence/S01/exp03/runs/worker.json", c["verdict"],
                            "Worker is the same binary as the host, on the Windows host. Container and gVisor placement not measured."))

    cmp = load("evidence/S01/exp03/compare/comparison.json")
    checks.append(check("EXP03-COMPARE-PROVIDER-CRATE", ["ADR-011 open question 1"], "bash probes/runtime/compare/measure.sh <scratch dir> (3 runs, alternating order)", "static", 0,
                        "evidence/S01/exp03/compare/comparison.json", "measured",
                        "Clean build times vary 107 to 343 s between identical runs on this host, more than the gap between crates. async-openai fault behaviour is from source inspection because no OpenAI stream could be recorded.",
                        cmp["results"]))
    checks.append(check("EXP03-COMPARE-PLACEMENT", ["ADR-011 open question 2", "RT-08"], KEY_A + "target/release/runtime-probe worker --live", "recorded-real-replay", 0,
                        "evidence/S01/exp03/runs/worker.json", "measured", "Release build, Windows host, 200 iterations, one recorded real Anthropic tool-call stream.", wk["overhead"]))
    lic = load("evidence/S01/exp03/licenses.json")
    checks.append(check("EXP03-LICENSES", ["S01-W6 licenses", "ADR-017"], "python -I probes/runtime/licenses.py . factory-agent factory-runtime-probe", "static", 0,
                        "evidence/S01/exp03/licenses.json", "pass" if all(not x["flagged"] for t in lic["targets"].values() for x in t.values()) else "fail",
                        "Allow-list check over Cargo SPDX metadata. Not legal advice. NOTICE retention still needed at S08.",
                        {t: {r: x["count"] for r, x in v.items()} for t, v in lic["targets"].items()}))
    ut = load("evidence/S01/exp03/runs/unit-tests.json")
    checks.append(check("EXP03-AGENT-UNIT-TESTS", ["T-RUN-02", "RT-06"], "cargo test -p factory-agent", "static", ut["exit_code"], "evidence/S01/exp03/runs/unit-tests.json",
                        "pass" if ut["exit_code"] == 0 else "fail", "Framing, codec, request shapes and HTTP classes. No model output in tests."))

    spend = {
        "openai_usd": 0.0,
        "anthropic_calls": [s for s in an["spend"]] + wk["spend"] + [load("evidence/S01/exp04/runs/live-once.json")["result"]["spend"]],
    }
    spend["anthropic_usd_estimated"] = round(sum(s["estimate"]["usd"] for s in spend["anthropic_calls"]), 7)
    spend["note"] = "Estimates from provider-reported usage and the cited price records. The cancelled turn is counted at its max_tokens upper bound. The coordinator's own key check call is not included."

    out = {"experiment": "EXP-03", "stage": "S01", "packet": "S01-W5", "recorded": "2026-10-24",
           "models": {"openai": {"requested": "gpt-5.4-nano-2026-03-17", "inference_completed": False},
                      "anthropic": {"requested": "claude-haiku-5-5", "reported": an["tool_turn"]["model_reported"], "anthropic_version": "2026-10-21"}},
           "spend": spend,
           "evidence_kinds": {"live": "real provider request", "recorded-real-replay": "replay of bytes captured byte-for-byte from a real provider stream",
                              "local-execution": "real PostgreSQL, processes, pipes and filesystem with recorded real tool arguments",
                              "static": "source inspection, build metrics, unit tests"},
           "source_manifest": rows, "checks": checks}
    (ROOT / "evidence/S01/exp03/results.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")

    # EXP-04
    e4 = "evidence/S01/exp04/runs/"
    insp = load(e4 + "inspect.out")[0]
    scan = load(e4 + "filesystem-scan.json")
    sec = load(e4 + "secret-scan.json")
    lo = load(e4 + "live-once.json")
    nk = load(e4 + "run-no-key.json")
    image = {"tag": "hoi4-factory-agent-probe:s01", "image_id": insp["Id"], "repo_digests": insp["RepoDigests"], "size_bytes": insp["Size"],
             "os_arch": f'{insp["Os"]}/{insp["Architecture"]}', "user": insp["Config"]["User"], "entrypoint": insp["Config"]["Entrypoint"],
             "env": insp["Config"]["Env"],
             "base_images": {"build": "rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3",
                             "runtime": "debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8"},
             "added_packages": ["ca-certificates 20250419~deb12u2"]}
    c4 = [
        check("EXP04-IMAGE-BUILD", ["RT-01", "ADR-013"], "docker build --progress=plain -f docker/agent-probe.Dockerfile -t hoi4-factory-agent-probe:s01 .", "static", 0,
              e4 + "build.err", "pass", "Local Docker Desktop 28.5.2 build. No registry push, so the digest is the local image ID.", {"image_id": insp["Id"]}),
        check("EXP04-FS-NO-CODING-CLI-OR-RUNTIME", ["RT-01", "T-RUN-01"], "docker export $(docker create IMAGE) | tar -t, then python -I probes/runtime/scan_image_listing.py", "static", 0,
              e4 + "filesystem-scan.json", "pass" if scan["clean"] else "fail",
              "Name-pattern scan over all 4747 entries. Two hits were reviewed as false positives (CA bundle symlink, libstdc++ gdb scripts). Base OS shells and perl-base remain, they are not coding CLIs.",
              {"hits": scan["hits"], "reviewed_false_positives": scan["reviewed_false_positives"], "base_os_interpreters_present": scan["base_os_interpreters_present"]}),
        check("EXP04-NO-CREDENTIAL-IN-IMAGE", ["ADR-013", "SPEC-002"], "docker export | python -I in-memory comparison against both key values, plus key-prefix pattern scan", "static", 0,
              e4 + "secret-scan.json", "pass" if not any(sec["key_values_present"].values()) and sec["key_prefix_pattern_review"]["other_hits"] == 0 else "fail",
              "Only the deliberately invalid probe key literal matched the prefix pattern. Image Env holds only PATH.", sec),
        check("EXP04-NO-GAME-FILES", ["ADR-015"], "same scan as EXP04-FS-NO-CODING-CLI-OR-RUNTIME", "static", 0, e4 + "filesystem-scan.json",
              "pass" if not scan["hits"]["game_file"] else "fail", "Pattern scan for HOI4 paths and asset types. No game copy exists on this host to compare against."),
        check("EXP04-NO-KEY-REFUSES-BEFORE-SEND", ["SPEC-002 credentials"], "env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY docker run --rm hoi4-factory-agent-probe:s01 live-once", "static", 0,
              e4 + "run-no-key.json", "pass" if nk.get("request_sent") is False else "fail", "Local check, no network request."),
        check("EXP04-LIVE-NATIVE-REQUEST", ["RT-01", "T-RUN-01", "S01 gate 4"], KEY_A + "docker run --rm -e ANTHROPIC_API_KEY hoi4-factory-agent-probe:s01 live-once", "live", 0,
              e4 + "live-once.json",
              "pass" if lo["result"].get("status") == 200 and lo["result"].get("turn", {}).get("finish_reason") == "tool_use" else "fail",
              "One request to Anthropic Messages (claude-haiku-5-5) from the container. OpenAI from the container was not attempted because both OpenAI keys return 429.",
              {"status": lo["result"].get("status"), "model": (lo["result"].get("turn") or {}).get("model"), "finish_reason": (lo["result"].get("turn") or {}).get("finish_reason"),
               "spend": lo["result"].get("spend")}),
    ]
    (ROOT / "evidence/S01/exp04/results.json").write_text(json.dumps({"experiment": "EXP-04", "stage": "S01", "packet": "S01-W5", "recorded": "2026-10-24",
                                                                       "image": image, "checks": c4}, indent=1) + "\n", encoding="utf-8")
    print("exp03 checks", len(checks), "exp04 checks", len(c4), "anthropic usd", spend["anthropic_usd_estimated"])


if __name__ == "__main__":
    main()
