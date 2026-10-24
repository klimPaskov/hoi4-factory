"""Write probes/focus/cases.json, the declared intent for every EXP-02 check.

Run with: py -3.13 -I probes/focus/tools/make_cases.py
Both the in-container harness and the host oracle read the same intent. The
oracle derives approved byte regions from the original fixture independently.
expect: identical (no byte may change), edit (only approved regions change and
semantics match), refuse (tool must fail and nothing may change).
"""

from __future__ import annotations

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent

cases: list[dict] = []


def add(cid: str, fixture: str, tree: str, op: dict, expect: str, note: str = "") -> None:
    cases.append({"id": cid, "fixture": fixture, "tree": tree, "op": op, "expect": expect, "note": note})


BASIC_VARIANTS = [
    "basic_lf_nobom.txt",
    "basic_lf_bom.txt",
    "basic_crlf_nobom.txt",
    "basic_crlf_bom.txt",
    "basic_mixed_bom.txt",
    "nonascii_crlf_bom.txt",
    "invalid_utf8_lf.txt",
    "windows1252_lf.txt",
]

for fx in BASIC_VARIANTS:
    stem = fx.removesuffix(".txt")
    t = "exp_basic_tree"
    add(f"{stem}__inspect", fx, t, {"type": "inspect_only"}, "identical")
    add(f"{stem}__noop", fx, t, {"type": "noop"}, "identical")
    add(f"{stem}__cost", fx, t, {"type": "cost", "focusId": "exp_left", "value": 9}, "edit")
    add(f"{stem}__cost_commented_line", fx, t, {"type": "cost", "focusId": "exp_root", "value": 12}, "edit",
        "cost line carries a trailing comment")
    add(f"{stem}__move", fx, t, {"type": "move", "focusId": "exp_join", "x": 5, "y": 3}, "edit")
    add(f"{stem}__link", fx, t, {"type": "link", "focusId": "exp_join", "prerequisites": ["exp_root"]}, "edit")
    add(f"{stem}__create", fx, t, {"type": "create", "focus": {
        "id": "exp_new", "x": 4, "y": 3, "cost": 5, "prerequisites": [["exp_join"]]}}, "edit")

# The invalid UTF-8 file cannot be preserved when re-encoded: a refusal is the safe answer.
for c in cases:
    if c["fixture"] == "invalid_utf8_lf.txt" and c["expect"] == "edit":
        c["expect"] = "refuse"
        c["note"] = "file is not valid UTF-8; the only safe outcomes are an exact edit or a refusal"
        c["acceptEdit"] = True

U = "unknown_crlf_bom.txt"
ut = "exp_unknown_tree"
add("unknown__inspect", U, ut, {"type": "inspect_only"}, "identical")
add("unknown__noop", U, ut, {"type": "noop"}, "identical")
add("unknown__cost_beside_unknown_key", U, ut, {"type": "cost", "focusId": "exp_u_root", "value": 11}, "edit",
    "focus has an unmodelled block and an unmodelled scalar key")
add("unknown__cost_scripted_constant", U, ut, {"type": "cost", "focusId": "exp_u_const", "value": 9}, "edit",
    "cost is a scripted constant reference @exp_cost")
add("unknown__move", U, ut, {"type": "move", "focusId": "exp_u_plain", "x": 6, "y": 3}, "edit",
    "focus has an available block with a comparison operator")
add("unknown__link", U, ut, {"type": "link", "focusId": "exp_u_plain", "prerequisites": ["exp_u_const"]}, "edit")
add("unknown__create", U, ut, {"type": "create", "focus": {
    "id": "exp_u_new", "x": 3, "y": 2, "cost": 5, "prerequisites": [["exp_u_const"]]}}, "edit")
add("unknown__raw_edit", U, ut, {"type": "raw_edit", "focusId": "exp_u_root", "key": "exp_custom_block",
    "text": "exp_custom_block = { foo = baz }"}, "refuse", "editing an unmodelled entry through the plan")

D = "duplicate_lf.txt"
dt = "exp_dup_tree"
add("duplicate__inspect", D, dt, {"type": "inspect_only"}, "identical")
add("duplicate__noop", D, dt, {"type": "noop"}, "identical")
add("duplicate__cost_first", D, dt, {"type": "cost", "focusId": "exp_d_twin", "occurrence": 0, "value": 4}, "refuse",
    "ambiguous target: two focus blocks share the id")
add("duplicate__cost_second", D, dt, {"type": "cost", "focusId": "exp_d_twin", "occurrence": 1, "value": 9}, "refuse",
    "ambiguous target: two focus blocks share the id")
add("duplicate__move_first", D, dt, {"type": "move", "focusId": "exp_d_twin", "occurrence": 0, "x": 0, "y": 2}, "refuse")
add("duplicate__cost_unique", D, dt, {"type": "cost", "focusId": "exp_d_root", "value": 12}, "edit",
    "edit of a unique focus in a file that also has duplicates")

S = "shared_lf.txt"
st = "exp_shared_tree"
add("shared__inspect", S, st, {"type": "inspect_only"}, "identical")
add("shared__noop", S, st, {"type": "noop"}, "identical")
add("shared__cost_tree_focus", S, st, {"type": "cost", "focusId": "exp_s_after", "value": 8}, "edit")
add("shared__move_tree_focus", S, st, {"type": "move", "focusId": "exp_s_after", "x": 3, "y": 2}, "edit")
add("shared__link_to_shared", S, st, {"type": "link", "focusId": "exp_s_root", "prerequisites": ["exp_s_shared_a"]}, "edit",
    "prerequisite pointing to a shared focus defined outside the tree")
add("shared__create_after_shared", S, st, {"type": "create", "focus": {
    "id": "exp_s_new", "x": 2, "y": 2, "cost": 5, "prerequisites": [["exp_s_after"]]}}, "edit")
add("shared__cost_shared_focus", S, st, {"type": "cost", "focusId": "exp_s_shared_a", "value": 9}, "refuse",
    "the shared focus lives outside focus_tree; expect no structured edit")

R = "relative_crlf.txt"
rt = "exp_rel_tree"
add("relative__inspect", R, rt, {"type": "inspect_only"}, "identical")
add("relative__noop", R, rt, {"type": "noop"}, "identical")
add("relative__move_offset", R, rt, {"type": "move", "focusId": "exp_r_child", "x": 0, "y": 1}, "edit",
    "changes the relative offset, keeps the anchor")
add("relative__move_to_fixed", R, rt, {"type": "move", "focusId": "exp_r_child", "x": 6, "y": 2, "toFixed": True}, "edit",
    "explicit conversion from relative to absolute")
add("relative__cost_child", R, rt, {"type": "cost", "focusId": "exp_r_child", "value": 4}, "edit",
    "scalar edit on a relatively positioned focus")
add("relative__link", R, rt, {"type": "link", "focusId": "exp_r_grandchild", "prerequisites": ["exp_r_anchor"]}, "edit")
add("relative__create_relative", R, rt, {"type": "create", "focus": {
    "id": "exp_r_new", "x": 0, "y": 1, "cost": 5, "relative_position_id": "exp_r_grandchild",
    "prerequisites": [["exp_r_grandchild"]]}}, "edit")

O = "relative_orphan_lf.txt"
ot = "exp_orphan_tree"
add("orphan__noop", O, ot, {"type": "noop"}, "identical")
add("orphan__cost_root", O, ot, {"type": "cost", "focusId": "exp_o_root", "value": 8}, "edit",
    "edit of a valid focus in a tree that also holds a missing relative anchor")
add("orphan__cost_orphan", O, ot, {"type": "cost", "focusId": "exp_o_orphan", "value": 4}, "edit",
    "edit of the focus whose relative anchor is missing")

SS = "shared_standalone_crlf_bom.txt"
sst = "exp_ss_tree"
add("sharedalone__noop", SS, sst, {"type": "noop"}, "identical")
add("sharedalone__cost", SS, sst, {"type": "cost", "focusId": "exp_ss_next", "value": 8}, "edit")
add("sharedalone__move", SS, sst, {"type": "move", "focusId": "exp_ss_next", "x": 3, "y": 1}, "edit")
add("sharedalone__link", SS, sst, {"type": "link", "focusId": "exp_ss_next", "prerequisites": ["exp_ss_shared"]}, "edit",
    "new prerequisite on a shared focus defined outside the tree")
add("sharedalone__create", SS, sst, {"type": "create", "focus": {
    "id": "exp_ss_new", "x": 2, "y": 2, "cost": 5, "prerequisites": [["exp_ss_next"]]}}, "edit")

# External edits between inspection and rewrite (stale plan base).
add("stale__comment_then_cost", "basic_crlf_bom.txt", "exp_basic_tree",
    {"type": "cost", "focusId": "exp_left", "value": 9,
     "external": {"find": "# leading comment before the root focus", "replace": "# leading comment edited externally"}},
    "edit", "external formatting-only change after inspect; preserve it or refuse")
add("stale__semantic_then_cost", "basic_crlf_bom.txt", "exp_basic_tree",
    {"type": "cost", "focusId": "exp_left", "value": 9,
     "external": {"find": "cost = 7", "replace": "cost = 3"}},
    "edit", "external semantic change to another focus after inspect; preserve it or refuse")

C = "commented_prereq_lf.txt"
ct = "exp_cmt_tree"
add("commented__noop", C, ct, {"type": "noop"}, "identical")
add("commented__link", C, ct, {"type": "link", "focusId": "exp_c_next", "prerequisites": ["exp_c_side"]}, "edit",
    "the existing prerequisite block holds a comment; refusal is also acceptable")
cases[-1]["acceptRefusal"] = True
add("commented__cost", C, ct, {"type": "cost", "focusId": "exp_c_next", "value": 6}, "edit")

for fx, tree in (("malformed_unclosed_lf.txt", "exp_bad_tree"), ("malformed_stray_crlf.txt", "exp_stray_tree")):
    stem = fx.removesuffix(".txt")
    focus = "exp_b_root" if "unclosed" in fx else "exp_t_root"
    add(f"{stem}__inspect", fx, tree, {"type": "inspect_only"}, "identical")
    add(f"{stem}__noop", fx, tree, {"type": "noop"}, "identical")
    add(f"{stem}__cost", fx, tree, {"type": "cost", "focusId": focus, "value": 7}, "refuse")
    add(f"{stem}__create", fx, tree, {"type": "create", "focus": {
        "id": "exp_bad_new", "x": 1, "y": 2, "cost": 5, "prerequisites": [[focus]]}}, "refuse")

for c in cases:
    if c["id"].startswith("stale__"):
        c["acceptRefusal"] = True

(ROOT / "cases.json").write_text(json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps({"cases": len(cases)}))
