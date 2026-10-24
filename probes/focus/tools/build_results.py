"""Assemble evidence/S01/exp02/results.json from a probe run and its oracle output.

Run with: py -3.13 -I probes/focus/tools/build_results.py <primary-run> <repeat-run>
Reads .work/exp02/<run>/ and .work/exp02/<run>-oracle.json. Writes only
evidence/S01/exp02/results.json. No secrets or host paths are recorded.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
PROBE = ROOT / "probes" / "focus"
WORK = ROOT / ".work" / "exp02"
OUT = ROOT / "evidence" / "S01" / "exp02" / "results.json"

IMAGE = {
    "tag": "hoi4f-exp02-agent-tools:25f5641",
    "imageId": "sha256:a30012e94c6cbfcf0f5620db7695dc99411710036890ba58ad3808cf942b7bc7",
    "baseImage": "docker.io/library/node:22-bookworm-slim@sha256:53ada149d435c38b14476cb57e4a7da73c15595aba79bd6971b547ceb6d018bf",
    "dockerfileFrontend": "docker.io/docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e",
    "source": "hoi4-agent-tools 25f5641bbc07c65a822ef576ac6e92bd12666880 (package 4.3.0), upstream Dockerfile, npm ci from npm-shrinkwrap.json",
    "platform": "linux/amd64, Debian 12.14, Node v22.23.1, npm 10.9.8, runs as uid 1000 (node)",
    "engine": "Docker 28.5.2 Linux engine (WSL2) on Windows 11 Pro 10.0.26200",
}

LIMITS = {
    "general": "Synthetic fixtures only. Source parsing and rewrite evidence, not game execution. The HOI4 game never loaded these files.",
    "oracle": "The oracle tokenizer covers the fixture syntax only. It is a test oracle, not a full Clausewitz parser.",
}


def sha_file(p: pathlib.Path) -> str | None:
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def main(argv: list[str]) -> int:
    primary, repeat = argv[1], argv[2]
    cases = {c["id"]: c for c in json.loads((PROBE / "cases.json").read_text(encoding="utf-8"))["cases"]}
    verdicts = json.loads((WORK / f"{primary}-oracle.json").read_text(encoding="utf-8"))
    repeat_verdicts = {r["caseId"]: r for r in json.loads((WORK / f"{repeat}-oracle.json").read_text(encoding="utf-8"))}
    checks = []
    for v in verdicts:
        case = cases[v["caseId"]]
        run = json.loads((WORK / primary / v["caseId"] / "run.json").read_text(encoding="utf-8"))
        tool = run.get("rewrite") or {}
        op = case["op"]
        if op["type"] == "inspect_only":
            command = "MCP stdio tools/call hoi4.focus_inspect {workspaceId, relativePath, treeId}"
        else:
            command = ("MCP stdio tools/call hoi4.focus_inspect, then hoi4.focus_rewrite "
                       "{workspaceId, relativePath, treeId, plan} with the imported plan edited as: " + json.dumps(op, sort_keys=True))
        rep = repeat_verdicts.get(v["caseId"], {})
        limitation = [LIMITS["general"]]
        if v.get("sideEffects") or (v.get("extraFilesChanged") and v["verdict"] in ("pass",)):
            limitation.append("Rewrite also wrote a planning sidecar beside the source: "
                              + ", ".join(v.get("sideEffects") or v.get("extraFilesChanged")))
        if v["verdict"] == "refused_safely":
            limitation.append("Operation not available on this input: the tool blocked it without mutation")
        if case.get("note"):
            limitation.append("Case note: " + case["note"])
        checks.append({
            "checkId": f"EXP02-{v['caseId']}",
            "fixture": case["fixture"],
            "operation": op["type"],
            "intent": op,
            "expected": case["expect"],
            "command": command,
            "harness": "probes/focus/run-probe.sh -> node /probe/harness/run.mjs (in container, --network none, read-only rootfs)",
            "imageId": IMAGE["imageId"],
            "inputFixtureSha256": v.get("fixtureSha256") or v["inputSha256"],
            "baselineSha256": v["inputSha256"],
            "externalEditBeforeRewrite": run.get("externalEdit"),
            "outputSha256": v["outputSha256"],
            "toolResultCode": (tool.get("structured") or {}).get("code") if tool else None,
            "toolExecution": v.get("toolExecution"),
            "toolErrorCode": v.get("toolErrorCode"),
            "toolDrift": ((tool.get("structured") or {}).get("data") or {}).get("drift") if tool else None,
            "sourceByteIdentical": v.get("byteIdentical"),
            "otherWorkspaceFilesChanged": v.get("extraFilesChanged", []),
            "byteOracle": v.get("byteOracle"),
            "semanticOracle": v.get("semanticOracle"),
            "oracleVerdict": v["verdict"],
            "oracleReasons": v.get("reasons", []),
            "repeatRunOutputSha256": rep.get("outputSha256"),
            "repeatRunVerdict": rep.get("verdict"),
            "repeatable": rep.get("outputSha256") == v["outputSha256"] and rep.get("verdict") == v["verdict"],
            "evidenceKind": "source rewrite executed in Linux container via MCP stdio, judged by independent byte and semantic oracle",
            "oracleCommand": f"py -3.13 -I probes/focus/oracle/oracle.py probes/focus/cases.json .work/exp02/{primary} .work/exp02/{primary}-oracle.json",
            "limitation": " ".join(limitation),
        })
    selftest = json.loads((WORK / "oracle-selftest.json").read_text(encoding="utf-8"))
    summary: dict[str, int] = {}
    for c in checks:
        summary[c["oracleVerdict"]] = summary.get(c["oracleVerdict"], 0) + 1
    doc = {
        "experiment": "EXP-02",
        "stage": "S01",
        "packet": "S01-W4",
        "generatedFrom": {"primaryRun": primary, "repeatRun": repeat},
        "image": IMAGE,
        "inputs": {
            "casesSha256": sha_file(PROBE / "cases.json"),
            "fixtureManifestSha256": sha_file(PROBE / "fixtures" / "manifest.json"),
            "fixtureSupportGfxSha256": sha_file(PROBE / "fixture-support" / "interface" / "exp02_icons.gfx"),
            "harnessSha256": sha_file(PROBE / "harness" / "run.mjs"),
            "mcpClientSha256": sha_file(PROBE / "harness" / "mcp-client.mjs"),
            "oracleSha256": sha_file(PROBE / "oracle" / "oracle.py"),
        },
        "oracleSelfTest": {
            "command": "py -3.13 -I probes/focus/oracle/selftest.py",
            "controls": selftest["controls"], "failures": selftest["failures"],
            "evidenceKind": "oracle negative and positive controls on hand-crafted outputs",
        },
        "summary": {"checks": len(checks), "byVerdict": summary,
                    "repeatableChecks": sum(1 for c in checks if c["repeatable"])},
        "limitations": [LIMITS["general"], LIMITS["oracle"],
                        "A Windows host comparison was not run: building on the host was out of scope by instruction. Cross-platform identity is unavailable.",
                        "The probe ran in the upstream hoi4-agent-tools image. The future HOI4 Factory server image does not yet package Agent Tools, so the probe could not run inside it."],
        "checks": checks,
    }
    extra_path = WORK / "upstream-tests.json"
    if extra_path.exists():
        doc["upstreamTests"] = json.loads(extra_path.read_text(encoding="utf-8"))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(doc["summary"]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
