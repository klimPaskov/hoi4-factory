"""Negative and positive controls for the EXP-02 oracle.

Run with: py -3.13 -I probes/focus/oracle/selftest.py
Each control hand-crafts an output from a fixture and checks that the oracle
returns the expected verdict. A failing control means the oracle is not fit
to judge the tool.
"""

from __future__ import annotations

import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.dont_write_bytecode = True  # keep the probe tree free of __pycache__
sys.path.insert(0, str(HERE))
import oracle  # noqa: E402  (local module, isolated mode keeps sys.path explicit)

FIX = HERE.parent / "fixtures"


def run(case: dict, before: bytes, after: bytes, extra=None, error=None, execution="applied") -> dict:
    tool = {"errorCode": error, "execution": None if error else execution}
    return oracle.judge(case, {"rewrite": tool, "extraFilesChanged": extra or []}, before, after)


def main() -> int:
    crlf = (FIX / "basic_crlf_bom.txt").read_bytes()
    lf = (FIX / "basic_lf_nobom.txt").read_bytes()
    cost = {"id": "c", "fixture": "basic_crlf_bom.txt", "tree": "exp_basic_tree", "expect": "edit",
            "op": {"type": "cost", "focusId": "exp_left", "value": 9}}
    link = {"id": "l", "fixture": "basic_crlf_bom.txt", "tree": "exp_basic_tree", "expect": "edit",
            "op": {"type": "link", "focusId": "exp_join", "prerequisites": ["exp_root"]}}
    noop = {"id": "n", "fixture": "basic_crlf_bom.txt", "tree": "exp_basic_tree", "expect": "identical",
            "op": {"type": "noop"}}
    refuse = {"id": "r", "fixture": "basic_crlf_bom.txt", "tree": "exp_basic_tree", "expect": "refuse",
              "op": {"type": "cost", "focusId": "exp_left", "value": 9}}

    left = crlf.index(b"id = exp_left")
    at = crlf.index(b"cost = 5", left) + len(b"cost = ")
    good_cost = crlf[:at] + b"9" + crlf[at + 1:]
    join = crlf.index(b"id = exp_join")
    close = crlf.index(b"\r\n\t}", join) + 2
    good_link = crlf[:close] + b"\t\tprerequisite = { focus = exp_root }\r\n" + crlf[close:]

    controls = [
        ("correct cost edit passes", run(cost, crlf, good_cost), "pass"),
        ("cost edit on the wrong focus fails", run(cost, crlf, crlf.replace(b"cost = 7", b"cost = 9")), "fail"),
        ("cost edit plus comment change fails", run(cost, crlf, good_cost.replace(b"# ten weeks", b"# 10 weeks")), "fail"),
        ("cost edit that drops the BOM fails", run(cost, crlf, good_cost[3:]), "fail"),
        ("cost edit that converts CRLF to LF fails", run(cost, crlf, good_cost.replace(b"\r\n", b"\n")), "fail"),
        ("correct link insertion passes", run(link, crlf, good_link), "pass"),
        ("link with bare LF in a CRLF file fails", run(link, crlf, good_link.replace(b"exp_root }\r\n", b"exp_root }\n")), "fail"),
        ("link that also reorders an existing prerequisite fails",
         run(link, crlf, good_link.replace(b"focus = exp_left focus = exp_right", b"focus = exp_right focus = exp_left")), "fail"),
        ("link to the wrong target fails", run(link, crlf, good_link.replace(b"focus = exp_root }\r\n\t}", b"focus = exp_left }\r\n\t}", 1)), "fail"),
        ("identical no-op passes", run(noop, crlf, crlf), "pass"),
        ("no-op that rewrites whitespace fails", run(noop, crlf, crlf.replace(b"\t", b"    ", 1)), "fail"),
        ("refusal without mutation passes", run(refuse, crlf, crlf, error="X"), "pass"),
        ("refusal that still wrote a sidecar fails", run(refuse, crlf, crlf, extra=["a.focus-plan.json"], error="X"), "fail"),
        ("silent no-change on an edit request fails", run(cost, crlf, crlf), "fail"),
        ("blocked edit without mutation is a safe refusal", run(cost, crlf, crlf, execution="blocked"), "refused_safely"),
        ("LF file: correct cost edit passes", run({**cost, "fixture": "basic_lf_nobom.txt"}, lf,
            lf.replace(b"cost = 5\n\t\tai_will_do = { factor = 2 }", b"cost = 9\n\t\tai_will_do = { factor = 2 }")), "pass"),
    ]
    failures = 0
    report = []
    for name, result, want in controls:
        ok = result["verdict"] == want
        failures += 0 if ok else 1
        report.append({"control": name, "want": want, "got": result["verdict"], "ok": ok,
                       "reasons": result.get("reasons", [])})
    print(json.dumps({"controls": len(controls), "failures": failures, "results": report}, indent=2))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
