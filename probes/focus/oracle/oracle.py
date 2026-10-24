"""Independent EXP-02 oracle for focus rewrite results.

Run with: py -3.13 -I probes/focus/oracle/oracle.py <cases.json> <run-dir> <out.json>

This oracle does not import or reuse any hoi4-agent-tools code. It has its own
small tokenizer for the synthetic fixture syntax only. It is a test oracle, not
a general Clausewitz parser.

Two independent judgements are made for every case:
1. Byte oracle. Approved change regions are computed from the ORIGINAL bytes
   with this module's own tokenizer and the case's declared intent. Every byte
   outside those regions must survive, in order. Insert regions allow only a
   pure insertion inside a point range.
2. Semantic oracle. The output is re-parsed here and the focus relationships
   (ids, prerequisite groups, mutual exclusions, positions, relative anchors,
   costs, shared focus references and the raw bytes of every untouched entry)
   are compared with the original model plus the declared intent.
"""

from __future__ import annotations

import hashlib
import itertools
import json
import pathlib
import sys
from dataclasses import dataclass, field

BOM = b"\xef\xbb\xbf"


# ----------------------------------------------------------------- tokenizer

class SyntaxProblem(Exception):
    pass


@dataclass
class Tok:
    kind: str  # word, string, op, open, close
    start: int
    end: int
    text: bytes


def tokenize(data: bytes) -> tuple[list[Tok], list[tuple[int, int]]]:
    toks: list[Tok] = []
    comments: list[tuple[int, int]] = []
    i = 3 if data.startswith(BOM) else 0
    n = len(data)
    while i < n:
        c = data[i]
        if c in b" \t\r\n":
            i += 1
        elif c == 0x23:  # '#'
            j = i
            while j < n and data[j] not in b"\r\n":
                j += 1
            comments.append((i, j))
            i = j
        elif c == 0x7B:
            toks.append(Tok("open", i, i + 1, b"{"))
            i += 1
        elif c == 0x7D:
            toks.append(Tok("close", i, i + 1, b"}"))
            i += 1
        elif c == 0x22:  # quoted string, no newline allowed
            j = i + 1
            while j < n and data[j] != 0x22:
                if data[j] in b"\r\n":
                    raise SyntaxProblem(f"unterminated string at byte {i}")
                if data[j] == 0x5C:
                    j += 1
                j += 1
            if j >= n:
                raise SyntaxProblem(f"unterminated string at byte {i}")
            toks.append(Tok("string", i, j + 1, data[i : j + 1]))
            i = j + 1
        elif c in b"=<>!":
            j = i + 1
            if j < n and data[j] == 0x3D:
                j += 1
            toks.append(Tok("op", i, j, data[i:j]))
            i = j
        else:
            j = i
            while j < n and data[j] not in b" \t\r\n{}=<>!#\"":
                j += 1
            toks.append(Tok("word", i, j, data[i:j]))
            i = j
    return toks, comments


# -------------------------------------------------------------------- parser

@dataclass
class Node:
    kind: str  # scalar, block, assign
    start: int
    end: int
    key: bytes | None = None
    op: bytes | None = None
    value: "Node | None" = None
    text: bytes | None = None  # scalar text
    entries: list["Node"] = field(default_factory=list)
    open_at: int | None = None
    close_at: int | None = None


def parse(data: bytes) -> tuple[Node, list[tuple[int, int]]]:
    toks, comments = tokenize(data)
    pos = 0

    def value() -> Node:
        nonlocal pos
        t = toks[pos]
        if t.kind == "open":
            return block()
        if t.kind in ("word", "string"):
            pos += 1
            return Node("scalar", t.start, t.end, text=t.text)
        raise SyntaxProblem(f"unexpected {t.kind} at byte {t.start}")

    def block() -> Node:
        nonlocal pos
        open_tok = toks[pos]
        pos += 1
        node = Node("block", open_tok.start, open_tok.end, open_at=open_tok.start)
        while True:
            if pos >= len(toks):
                raise SyntaxProblem(f"unclosed block opened at byte {open_tok.start}")
            t = toks[pos]
            if t.kind == "close":
                node.close_at = t.start
                node.end = t.end
                pos += 1
                return node
            node.entries.append(entry())

    def entry() -> Node:
        nonlocal pos
        t = toks[pos]
        if t.kind in ("word", "string") and pos + 1 < len(toks) and toks[pos + 1].kind == "op":
            op = toks[pos + 1]
            pos += 2
            if pos >= len(toks):
                raise SyntaxProblem("assignment without value at end of file")
            v = value()
            return Node("assign", t.start, v.end, key=t.text, op=op.text, value=v)
        if t.kind == "close":
            raise SyntaxProblem(f"unexpected close at byte {t.start}")
        return value()

    root = Node("block", 0, len(data))
    while pos < len(toks):
        if toks[pos].kind == "close":
            raise SyntaxProblem(f"stray closing brace at byte {toks[pos].start}")
        root.entries.append(entry())
    return root, comments


def assigns(block: Node, key: bytes) -> list[Node]:
    return [e for e in block.entries if e.kind == "assign" and e.key == key]


def scalar(block: Node, key: bytes) -> Node | None:
    found = assigns(block, key)
    if found and found[0].value.kind == "scalar":
        return found[0].value
    return None


def block_id(block: Node) -> str | None:
    s = scalar(block, b"id")
    return None if s is None else s.text.decode("utf-8", "replace").strip('"')


# ------------------------------------------------------------ semantic model

TRACKED = {b"id", b"x", b"y", b"cost", b"relative_position_id", b"prerequisite", b"mutually_exclusive"}


def focus_model(data: bytes, block: Node) -> dict:
    def s(key: bytes) -> str | None:
        v = scalar(block, key)
        return None if v is None else v.text.decode("utf-8", "replace")

    prereq = []
    for a in assigns(block, b"prerequisite"):
        if a.value.kind == "block":
            prereq.append(sorted(
                e.value.text.decode() for e in a.value.entries
                if e.kind == "assign" and e.key == b"focus" and e.value.kind == "scalar"
            ))
    mex = sorted(
        e.value.text.decode()
        for a in assigns(block, b"mutually_exclusive") if a.value.kind == "block"
        for e in a.value.entries if e.kind == "assign" and e.key == b"focus"
    )
    other = [
        [e.key.decode("utf-8", "replace") if e.kind == "assign" else e.kind,
         hashlib.sha256(data[e.start:e.end]).hexdigest()]
        for e in block.entries
        if not (e.kind == "assign" and e.key in TRACKED)
    ]
    return {
        "id": s(b"id"), "x": s(b"x"), "y": s(b"y"), "cost": s(b"cost"),
        "relative_position_id": s(b"relative_position_id"),
        "prerequisite_groups": prereq, "mutually_exclusive": mex, "other_entries": other,
    }


def model(data: bytes) -> dict:
    root, comments = parse(data)
    trees = {}
    for a in assigns(root, b"focus_tree"):
        tree = a.value
        tid = block_id(tree)
        trees[tid] = {
            "focuses": [focus_model(data, f.value) for f in assigns(tree, b"focus") if f.value.kind == "block"],
            "shared_focus_refs": [e.value.text.decode() for e in assigns(tree, b"shared_focus") if e.value.kind == "scalar"],
            "tree_other_entries": [
                [e.key.decode() if e.kind == "assign" else e.kind, hashlib.sha256(data[e.start:e.end]).hexdigest()]
                for e in tree.entries if not (e.kind == "assign" and e.key in (b"focus", b"shared_focus"))
            ],
        }
    shared = [focus_model(data, a.value) for a in assigns(root, b"shared_focus") if a.value.kind == "block"]
    top_other = [
        [e.key.decode() if e.kind == "assign" else e.kind, hashlib.sha256(data[e.start:e.end]).hexdigest()]
        for e in root.entries if not (e.kind == "assign" and e.key in (b"focus_tree", b"shared_focus"))
    ]
    return {
        "trees": trees, "shared_focuses": shared, "top_other_entries": top_other,
        "comments": [hashlib.sha256(data[s:e]).hexdigest() for s, e in comments],
    }


# ------------------------------------------------------------ approved spans

def find_tree(root: Node, tree_id: str) -> Node:
    for a in assigns(root, b"focus_tree"):
        if block_id(a.value) == tree_id:
            return a.value
    raise KeyError(tree_id)


def find_focus(tree: Node, focus_id: str, occurrence: int = 0) -> Node:
    hits = [a.value for a in assigns(tree, b"focus") if a.value.kind == "block" and block_id(a.value) == focus_id]
    return hits[occurrence]


def approved_regions(data: bytes, case: dict) -> list[tuple[str, int, int]]:
    op = case["op"]
    kind = op["type"]
    if kind in ("inspect_only", "noop"):
        return []
    root, _ = parse(data)
    tree = find_tree(root, case["tree"])
    if kind == "create":
        focuses = assigns(tree, b"focus")
        lo = (focuses[-1].end if focuses else tree.open_at + 1)
        return [("insert", lo, tree.close_at)]
    focus = find_focus(tree, op["focusId"], op.get("occurrence", 0))
    regions: list[tuple[str, int, int]] = []

    def value_or_insert(key: bytes) -> None:
        v = scalar(focus, key)
        if v is None:
            regions.append(("insert", focus.open_at + 1, focus.close_at))
        else:
            regions.append(("replace", v.start, v.end))

    if kind == "cost":
        value_or_insert(b"cost")
    elif kind == "move":
        value_or_insert(b"x")
        value_or_insert(b"y")
        if op.get("toFixed"):
            for a in assigns(focus, b"relative_position_id"):
                regions.append(("replace", line_start(data, a.start), line_end(data, a.end)))
    elif kind == "link":
        regions.append(("insert", focus.open_at + 1, focus.close_at))
    elif kind == "raw_edit":
        # The intent touches an unmodelled entry. Approve that entry's bytes.
        for a in assigns(focus, op["key"].encode()):
            regions.append(("replace", a.start, a.end))
    else:
        raise ValueError(kind)
    # Merge duplicate insert regions (move on a block missing both x and y).
    uniq = []
    for r in regions:
        if r not in uniq:
            uniq.append(r)
    return uniq


def line_start(data: bytes, i: int) -> int:
    j = i
    while j > 0 and data[j - 1] in b" \t":
        j -= 1
    if j > 0 and data[j - 1] == 0x0A:
        j -= 1
        if j > 0 and data[j - 1] == 0x0D:
            j -= 1
    return j


def line_end(data: bytes, i: int) -> int:
    return i


def outside_preserved(a: bytes, b: bytes, regions: list[tuple[str, int, int]]) -> tuple[bool, list[bytes]]:
    """True when b = O0 X1 O1 ... Xk Ok, where Oi are the bytes of a outside the
    regions in order. Insert regions are tried at every point of their range and
    replaced by a zero-length replace region there. Returns inserted texts."""
    inserts = [r for r in regions if r[0] == "insert"]
    replaces = [(r[1], r[2]) for r in regions if r[0] == "replace"]
    ranges = [range(lo, hi + 1) for _, lo, hi in inserts]
    for points in itertools.product(*ranges) if ranges else [()]:
        spans = sorted(replaces + [(p, p) for p in points])
        if any(spans[i][1] > spans[i + 1][0] for i in range(len(spans) - 1)):
            continue
        pieces = []
        cursor = 0
        for s, e in spans:
            pieces.append(a[cursor:s])
            cursor = e
        pieces.append(a[cursor:])
        if not b.startswith(pieces[0]):
            continue
        if not b.endswith(pieces[-1]) or len(pieces[0]) + len(pieces[-1]) > len(b):
            continue
        cur = len(pieces[0])
        limit = len(b) - len(pieces[-1])
        xs = []
        ok = True
        for piece in pieces[1:-1]:
            idx = b.find(piece, cur, limit) if piece else cur
            if idx < 0:
                ok = False
                break
            xs.append(b[cur:idx])
            cur = idx + len(piece)
        if not ok or cur > limit:
            continue
        xs.append(b[cur:limit])
        return True, xs
    return False, []


# -------------------------------------------------------------- expectations

def expected_model(before: dict, case: dict) -> dict:
    exp = json.loads(json.dumps(before))
    op = case["op"]
    kind = op["type"]
    if kind in ("inspect_only", "noop"):
        return exp
    tree = exp["trees"][case["tree"]]
    if kind == "create":
        nf = op["focus"]
        tree["focuses"].append({"__created__": True, **nf})
        return exp
    hits = [f for f in tree["focuses"] if f["id"] == op["focusId"]]
    f = hits[op.get("occurrence", 0)]
    if kind == "cost":
        f["cost"] = str(op["value"])
    elif kind == "move":
        f["x"], f["y"] = str(op["x"]), str(op["y"])
        if op.get("toFixed"):
            f["relative_position_id"] = None
    elif kind == "link":
        f["prerequisite_groups"].append(sorted(op["prerequisites"]))
    elif kind == "raw_edit":
        f["__raw_edit__"] = op["key"]
    return exp


def compare_models(after: dict, expected: dict, case: dict) -> list[str]:
    problems = []
    if after["comments"] != expected["comments"]:
        problems.append("comment set changed")
    if after["shared_focuses"] != expected["shared_focuses"]:
        problems.append("shared focus definitions changed")
    if after["top_other_entries"] != expected["top_other_entries"]:
        problems.append("top-level unknown entries changed")
    for tid, et in expected["trees"].items():
        at = after["trees"].get(tid)
        if at is None:
            problems.append(f"tree {tid} missing")
            continue
        if at["shared_focus_refs"] != et["shared_focus_refs"]:
            problems.append(f"tree {tid} shared_focus references changed")
        if at["tree_other_entries"] != et["tree_other_entries"]:
            problems.append(f"tree {tid} unknown or other entries changed")
        if len(at["focuses"]) != len(et["focuses"]):
            problems.append(f"tree {tid} focus count {len(at['focuses'])} != expected {len(et['focuses'])}")
            continue
        for af, ef in zip(at["focuses"], et["focuses"]):
            if ef.get("__created__"):
                for key in ("id", "x", "y", "cost", "relative_position_id"):
                    if key in ef and str(ef[key]) != str(af.get(key)):
                        problems.append(f"created focus {ef['id']} {key}={af.get(key)} expected {ef[key]}")
                if "prerequisites" in ef:
                    want = [sorted(g) for g in ef["prerequisites"]]
                    if af["prerequisite_groups"] != want:
                        problems.append(f"created focus {ef['id']} prerequisites {af['prerequisite_groups']} expected {want}")
                continue
            if ef.get("__raw_edit__"):
                key = ef.pop("__raw_edit__")
                af = dict(af)
                af["other_entries"] = [e for e in af["other_entries"] if e[0] != key]
                ef = dict(ef)
                ef["other_entries"] = [e for e in ef["other_entries"] if e[0] != key]
            for key in ef:
                if af.get(key) != ef[key]:
                    problems.append(f"focus {ef.get('id')} {key}: {af.get(key)} expected {ef[key]}")
    return problems


def newline_kind(data: bytes) -> str:
    crlf = data.count(b"\r\n")
    lf = data.count(b"\n") - crlf
    if crlf and lf:
        return "mixed"
    return "crlf" if crlf else "lf"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


# ------------------------------------------------------------------- judging

def judge(case: dict, run: dict | None, before: bytes, after: bytes | None) -> dict:
    expect = case["expect"]  # "identical", "edit", "refuse"
    out = {
        "caseId": case["id"], "fixture": case["fixture"], "op": case["op"]["type"],
        "expect": expect, "inputSha256": sha(before),
        "outputSha256": None if after is None else sha(after),
    }
    if run is None or after is None:
        out.update(verdict="unavailable", reasons=["no run output"])
        return out
    tool = run.get("rewrite") or {}
    out["toolExecution"] = tool.get("execution")
    out["toolErrorCode"] = tool.get("errorCode")
    out["extraFilesChanged"] = run.get("extraFilesChanged", [])
    reasons: list[str] = []
    identical = before == after
    out["byteIdentical"] = identical
    mutated = (not identical) or bool(run.get("extraFilesChanged"))
    out["mutated"] = mutated
    refused = tool.get("errorCode") is not None or tool.get("execution") == "blocked"
    # Cases where both a correct edit and a clean refusal are safe outcomes.
    if expect == "refuse" and case.get("acceptEdit") and not identical:
        expect = "edit"
        out["judgedAs"] = "edit (refusal also acceptable)"
    if expect == "edit" and case.get("acceptRefusal") and refused:
        expect = "refuse"
        out["judgedAs"] = "refuse (edit also acceptable)"

    out["workspaceUnchanged"] = not run.get("extraFilesChanged")
    if expect == "identical":
        # Source byte identity is the T-VIS-01 no-op criterion. Files written
        # beside the source are reported separately as side effects.
        if not identical:
            reasons.append("source bytes changed")
        out["sideEffects"] = run.get("extraFilesChanged", [])
        out["verdict"] = "pass" if not reasons else "fail"
        out["reasons"] = reasons
        return out
    if expect == "refuse":
        if not identical:
            reasons.append("source bytes changed")
        if run.get("extraFilesChanged"):
            reasons.append("other workspace files changed: " + ", ".join(run["extraFilesChanged"]))
        if not refused:
            reasons.append("tool did not refuse")
        out["verdict"] = "pass" if not reasons else "fail"
        out["outcome"] = "refused_without_mutation" if not reasons else "unsafe"
        out["reasons"] = reasons
        return out

    # expect == "edit"
    if identical:
        if refused and not run.get("extraFilesChanged"):
            out.update(verdict="refused_safely", outcome="refused_without_mutation",
                       reasons=[f"tool refused: {tool.get('errorCode') or 'execution blocked'}"])
        else:
            out.update(verdict="fail", reasons=["no change was made and no refusal was reported"])
        return out
    regions = approved_regions(before, case)
    ok, inserted = outside_preserved(before, after, regions)
    out["approvedRegions"] = [list(r) for r in regions]
    out["byteOracle"] = "pass" if ok else "fail"
    if not ok:
        reasons.append("bytes outside the approved regions changed")
    else:
        out["changedTexts"] = [x.decode("utf-8", "replace") for x in inserted]
        nl = newline_kind(before)
        for x in inserted:
            if nl == "crlf" and (x.count(b"\n") != x.count(b"\r\n")):
                reasons.append("inserted text uses bare LF in a CRLF file")
            if nl == "lf" and b"\r" in x:
                reasons.append("inserted text uses CR in an LF file")
    if before.startswith(BOM) != after.startswith(BOM):
        reasons.append("BOM presence changed")
    def is_utf8(data: bytes) -> bool:
        try:
            (data[3:] if data.startswith(BOM) else data).decode("utf-8")
            return True
        except UnicodeDecodeError:
            return False

    # Encoding must not change class: a UTF-8 input must stay valid UTF-8.
    # A non-UTF-8 input is judged by the byte oracle alone.
    if is_utf8(before) and not is_utf8(after):
        reasons.append("output is not valid UTF-8")
    try:
        m_before = model(before)
        m_after = model(after)
        problems = compare_models(m_after, expected_model(m_before, case), case)
        out["semanticOracle"] = "pass" if not problems else "fail"
        reasons.extend(problems)
    except SyntaxProblem as error:
        out["semanticOracle"] = "fail"
        reasons.append(f"oracle parse failed: {error}")
    if run.get("extraFilesChanged"):
        out["note"] = "rewrite also wrote: " + ", ".join(run["extraFilesChanged"])
    out["verdict"] = "pass" if not reasons else "fail"
    out["outcome"] = "preserving_edit" if not reasons else "unsafe_or_wrong_edit"
    out["reasons"] = reasons
    return out


def main(argv: list[str]) -> int:
    cases = json.loads(pathlib.Path(argv[1]).read_text(encoding="utf-8"))["cases"]
    run_dir = pathlib.Path(argv[2])
    fixtures = pathlib.Path(argv[1]).resolve().parent / "fixtures"
    results = []
    for case in cases:
        before = (fixtures / case["fixture"]).read_bytes()
        case_dir = run_dir / case["id"]
        run = None
        after = None
        if (case_dir / "run.json").exists():
            run = json.loads((case_dir / "run.json").read_text(encoding="utf-8"))
            if (case_dir / "after.bin").exists():
                after = (case_dir / "after.bin").read_bytes()
        fixture_sha = sha(before)
        if (case_dir / "between.bin").exists():
            # External edit landed after inspection: judge against that baseline.
            before = (case_dir / "between.bin").read_bytes()
        result = judge(case, run, before, after)
        result["fixtureSha256"] = fixture_sha
        results.append(result)
    pathlib.Path(argv[3]).write_text(json.dumps(results, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    summary = {}
    for r in results:
        summary[r["verdict"]] = summary.get(r["verdict"], 0) + 1
    print(json.dumps(summary))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
