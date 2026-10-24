"""License inventory for the agent crate and the runtime probe (S01 EXP-03).

Usage: python -I licenses.py <manifest_dir> <root_package> [<root_package> ...]
Prints JSON: every normal (non-dev, non-build) transitive dependency reachable
from each root for the Linux container target and the Windows host target, its
SPDX expression and an AGPL-3.0 compatibility flag. The allow list is a
planning heuristic, not legal advice.
"""
import json
import re
import subprocess
import sys

ALLOWED = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Zlib", "Unicode-3.0", "Unicode-DFS-2016", "MPL-2.0", "CDLA-Permissive-2.0",
    "0BSD", "BSL-1.0", "CC0-1.0", "Unlicense", "AGPL-3.0-only", "AGPL-3.0-or-later",
}


def tokens(expr):
    expr = expr.replace("/", " OR ")
    return re.findall(r"\(|\)|[A-Za-z0-9.\-+]+(?: WITH [A-Za-z0-9.\-+]+)?", expr)


def parse(ts, i=0):
    """Return (ok, i). OR = any operand ok, AND = all operands ok."""
    def atom(i):
        if ts[i] == "(":
            ok, i = expr(i + 1)
            return ok, i + 1
        return ts[i] in ALLOWED, i + 1

    def conj(i):
        ok, i = atom(i)
        while i < len(ts) and ts[i] == "AND":
            o2, i = atom(i + 1)
            ok = ok and o2
        return ok, i

    def expr(i):
        ok, i = conj(i)
        while i < len(ts) and ts[i] == "OR":
            o2, i = conj(i + 1)
            ok = ok or o2
        return ok, i

    return expr(i)


def compatible(lic):
    if not lic:
        return False
    try:
        return parse(tokens(lic))[0]
    except IndexError:
        return False


def closure(meta, root_name):
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(i for i, p in pkgs.items() if p["name"] == root_name)
    seen, stack = set(), [root]
    while stack:
        cur = stack.pop()
        if cur in seen:
            continue
        seen.add(cur)
        for d in nodes[cur]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):
                stack.append(d["pkg"])
    return [pkgs[i] for i in seen if i != root]


def main():
    mdir, roots = sys.argv[1], sys.argv[2:]
    out = {"allow_list": sorted(ALLOWED), "targets": {}}
    for target in ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]:
        meta = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
            cwd=mdir))
        per = {}
        for r in roots:
            deps = sorted(closure(meta, r), key=lambda p: (p["name"], p["version"]))
            rows = [{"name": p["name"], "version": p["version"], "license": p.get("license"),
                     "agpl3_compatible": compatible(p.get("license"))} for p in deps]
            per[r] = {"count": len(rows), "flagged": [x for x in rows if not x["agpl3_compatible"]], "crates": rows}
        out["targets"][target] = per
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
