"""Generate the original synthetic EXP-02 focus fixtures.

Every fixture is invented for this probe. No text is copied from the game or a
real mod. Run with: py -3.13 -I probes/focus/tools/make_fixtures.py
The script writes probes/focus/fixtures/*.txt and fixtures/manifest.json.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent / "fixtures"

BOM = b"\xef\xbb\xbf"

# A small national tree with comments in several positions.
BASIC = """\
# EXP-02 synthetic fixture: basic tree (original text, not game content)
focus_tree = {
\tid = exp_basic_tree # trailing comment on the tree id
\tcountry = {
\t\tfactor = 0
\t\tmodifier = { add = 10 tag = XAA }
\t}
\tdefault = no

\t# leading comment before the root focus
\tfocus = {
\t\tid = exp_root
\t\ticon = GFX_exp_icon_root
\t\tx = 4
\t\ty = 0
\t\tcost = 10 # ten weeks
\t\tsearch_filters = { FOCUS_FILTER_POLITICAL }
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = {
\t\t\tadd_political_power = 25 # small reward
\t\t}
\t}

\tfocus = {
\t\tid = exp_left
\t\ticon = GFX_exp_icon_left
\t\tprerequisite = { focus = exp_root }
\t\tmutually_exclusive = { focus = exp_right }
\t\tx = 2
\t\ty = 1
\t\tcost = 5
\t\tai_will_do = { factor = 2 }
\t\tcompletion_reward = { add_stability = 0.05 }
\t}

\tfocus = {
\t\tid = exp_right
\t\ticon = GFX_exp_icon_right
\t\tprerequisite = { focus = exp_root }
\t\tmutually_exclusive = { focus = exp_left }
\t\tx = 6
\t\ty = 1
\t\tcost = 5
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_war_support = 0.05 }
\t}

\tfocus = {
\t\tid = exp_join
\t\ticon = GFX_exp_icon_join
\t\tprerequisite = { focus = exp_left focus = exp_right }
\t\tx = 4
\t\ty = 2
\t\tcost = 7
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 50 }
\t}
}
"""

# Unknown and unsupported syntax: unmodelled keys, scripted constants, a
# comparison operator, a bare value list and a tree-level unknown block.
UNKNOWN = """\
# EXP-02 synthetic fixture: unknown and unsupported syntax
@exp_cost = 6
focus_tree = {
\tid = exp_unknown_tree
\tdefault = no
\treset_on_civilwar = no
\texp_tree_custom = { alpha = beta gamma = { 1 2 3 } }

\tfocus = {
\t\tid = exp_u_root
\t\ticon = GFX_exp_icon_u_root
\t\tx = 3
\t\ty = 0
\t\tcost = 10
\t\texp_custom_block = { foo = bar } # not a modelled focus key
\t\tcancel_if_invalid = no
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_u_const
\t\ticon = GFX_exp_icon_u_const
\t\tprerequisite = { focus = exp_u_root }
\t\tx = 3
\t\ty = 1
\t\tcost = @exp_cost
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_stability = 0.01 }
\t}

\tfocus = {
\t\tid = exp_u_plain
\t\ticon = GFX_exp_icon_u_plain
\t\tprerequisite = { focus = exp_u_root }
\t\tx = 5
\t\ty = 2
\t\tcost = 4
\t\tavailable = { exp_weird_trigger < 3 }
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_war_support = 0.01 }
\t}
}
"""

# Duplicate focus IDs in one tree.
DUPLICATE = """\
# EXP-02 synthetic fixture: duplicate focus ids
focus_tree = {
\tid = exp_dup_tree
\tdefault = no

\tfocus = {
\t\tid = exp_d_root
\t\ticon = GFX_exp_icon_d_root
\t\tx = 2
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_d_twin
\t\ticon = GFX_exp_icon_d_twin_a
\t\tprerequisite = { focus = exp_d_root }
\t\tx = 1
\t\ty = 1
\t\tcost = 3
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_stability = 0.01 }
\t}

\tfocus = {
\t\tid = exp_d_twin
\t\ticon = GFX_exp_icon_d_twin_b
\t\tprerequisite = { focus = exp_d_root }
\t\tx = 3
\t\ty = 1
\t\tcost = 8
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_war_support = 0.01 }
\t}
}
"""

# Shared focus definitions outside the tree, referenced from the tree.
SHARED = """\
# EXP-02 synthetic fixture: shared focuses
shared_focus = {
\tid = exp_s_shared_a
\ticon = GFX_exp_icon_s_a
\tx = 10
\ty = 0
\tcost = 5
\tai_will_do = { factor = 1 }
\tcompletion_reward = { add_political_power = 5 }
}

shared_focus = {
\tid = exp_s_shared_b
\ticon = GFX_exp_icon_s_b
\tprerequisite = { focus = exp_s_shared_a }
\trelative_position_id = exp_s_shared_a
\tx = 0
\ty = 1
\tcost = 5
\tai_will_do = { factor = 1 }
\tcompletion_reward = { add_stability = 0.02 }
}

focus_tree = {
\tid = exp_shared_tree
\tdefault = no
\tshared_focus = exp_s_shared_a # pulls in the shared branch

\tfocus = {
\t\tid = exp_s_root
\t\ticon = GFX_exp_icon_s_root
\t\tx = 2
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_s_after
\t\ticon = GFX_exp_icon_s_after
\t\tprerequisite = { focus = exp_s_root focus = exp_s_shared_b }
\t\tx = 2
\t\ty = 1
\t\tcost = 6
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_war_support = 0.02 }
\t}
}
"""

# relative_position_id chains, including one to a missing anchor.
RELATIVE = """\
# EXP-02 synthetic fixture: relative positions
focus_tree = {
\tid = exp_rel_tree
\tdefault = no

\tfocus = {
\t\tid = exp_r_anchor
\t\ticon = GFX_exp_icon_r_anchor
\t\tx = 5
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_r_child
\t\ticon = GFX_exp_icon_r_child
\t\tprerequisite = { focus = exp_r_anchor }
\t\trelative_position_id = exp_r_anchor
\t\tx = -1
\t\ty = 1
\t\tcost = 5
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_stability = 0.01 }
\t}

\tfocus = {
\t\tid = exp_r_grandchild
\t\ticon = GFX_exp_icon_r_grandchild
\t\tprerequisite = { focus = exp_r_child }
\t\trelative_position_id = exp_r_child
\t\tx = 0
\t\ty = 1
\t\tcost = 5
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_war_support = 0.01 }
\t}
}
"""

# A focus anchored to a relative_position_id that does not exist.
RELATIVE_ORPHAN = """\
# EXP-02 synthetic fixture: relative position with a missing anchor
focus_tree = {
\tid = exp_orphan_tree
\tdefault = no

\tfocus = {
\t\tid = exp_o_root
\t\ticon = GFX_exp_icon_o_root
\t\tx = 1
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_o_orphan
\t\ticon = GFX_exp_icon_o_orphan
\t\trelative_position_id = exp_o_missing_anchor
\t\tx = 2
\t\ty = 0
\t\tcost = 3
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 1 }
\t}
}
"""

# Shared focus pulled into a tree with no tree focus depending on it.
SHARED_STANDALONE = """\
# EXP-02 synthetic fixture: standalone shared focus branch
shared_focus = {
\tid = exp_ss_shared
\ticon = GFX_exp_icon_ss_shared
\tx = 8
\ty = 0
\tcost = 5
\tai_will_do = { factor = 1 }
\tcompletion_reward = { add_political_power = 5 }
}

focus_tree = {
\tid = exp_ss_tree
\tdefault = no
\tshared_focus = exp_ss_shared

\tfocus = {
\t\tid = exp_ss_root
\t\ticon = GFX_exp_icon_ss_root
\t\tx = 2
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_ss_next
\t\ticon = GFX_exp_icon_ss_next
\t\tprerequisite = { focus = exp_ss_root }
\t\tx = 2
\t\ty = 1
\t\tcost = 6
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_war_support = 0.02 }
\t}
}
"""

# A comment inside a prerequisite block that a link edit must touch.
COMMENTED_PREREQ = """\
# EXP-02 synthetic fixture: comment inside a prerequisite block
focus_tree = {
\tid = exp_cmt_tree
\tdefault = no

\tfocus = {
\t\tid = exp_c_root
\t\ticon = GFX_exp_icon_c_root
\t\tx = 2
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_c_side
\t\ticon = GFX_exp_icon_c_side
\t\tx = 4
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}

\tfocus = {
\t\tid = exp_c_next
\t\ticon = GFX_exp_icon_c_next
\t\tprerequisite = {
\t\t\tfocus = exp_c_root # keep this note
\t\t}
\t\tx = 2
\t\ty = 1
\t\tcost = 5
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_stability = 0.01 }
\t}
}
"""

MALFORMED_UNCLOSED = """\
# EXP-02 synthetic fixture: malformed, the tree block is never closed
focus_tree = {
\tid = exp_bad_tree
\tdefault = no

\tfocus = {
\t\tid = exp_b_root
\t\ticon = GFX_exp_icon_b_root
\t\tx = 1
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}
"""

MALFORMED_STRAY = """\
# EXP-02 synthetic fixture: malformed, a stray closing brace and an unterminated string
focus_tree = {
\tid = exp_stray_tree
\tdefault = no

\tfocus = {
\t\tid = exp_t_root
\t\ticon = GFX_exp_icon_t_root
\t\tx = 1
\t\ty = 0
\t\tcost = 10
\t\tai_will_do = { factor = 1 }
\t\tcompletion_reward = { add_political_power = 10 }
\t}
\t}
\tfocus = {
\t\tid = exp_t_next
\t\ticon = "GFX_exp_icon_t_next
\t\tx = 1
\t\ty = 1
\t}
}
"""


def crlf(text: str) -> str:
    return text.replace("\n", "\r\n")


def mixed(text: str) -> str:
    """CRLF everywhere except every third line, which keeps a bare LF."""
    lines = text.split("\n")
    out = []
    for index, line in enumerate(lines[:-1]):
        out.append(line + ("\n" if index % 3 == 2 else "\r\n"))
    out.append(lines[-1])
    return "".join(out)


def enc(text: str, bom: bool) -> bytes:
    data = text.encode("utf-8")
    return BOM + data if bom else data


def fixtures() -> dict[str, tuple[bytes, dict]]:
    out: dict[str, tuple[bytes, dict]] = {}

    def add(name: str, data: bytes, **meta) -> None:
        out[name] = (data, meta)

    add("basic_lf_nobom.txt", enc(BASIC, False), tree="exp_basic_tree", bom=False, newline="lf", conditions=["no_bom", "lf", "comments"])
    add("basic_lf_bom.txt", enc(BASIC, True), tree="exp_basic_tree", bom=True, newline="lf", conditions=["bom", "lf", "comments"])
    add("basic_crlf_nobom.txt", enc(crlf(BASIC), False), tree="exp_basic_tree", bom=False, newline="crlf", conditions=["no_bom", "crlf", "comments"])
    add("basic_crlf_bom.txt", enc(crlf(BASIC), True), tree="exp_basic_tree", bom=True, newline="crlf", conditions=["bom", "crlf", "comments"])
    add("basic_mixed_bom.txt", enc(mixed(BASIC), True), tree="exp_basic_tree", bom=True, newline="mixed", conditions=["bom", "mixed_newlines", "comments"])
    # Non-ASCII UTF-8 in a comment and a quoted value: must stay UTF-8.
    nonascii = BASIC.replace(
        "# leading comment before the root focus",
        "# leading comment before the root focus: Zürich, Łódź, Ἀθῆναι",
    ).replace("icon = GFX_exp_icon_join", 'icon = "GFX_exp_icon_join" # naïve café')
    add("nonascii_crlf_bom.txt", enc(crlf(nonascii), True), tree="exp_basic_tree", bom=True, newline="crlf", conditions=["bom", "crlf", "comments", "non_ascii_utf8"])
    # Not valid UTF-8: one 0x81 byte inside a comment (undefined in Windows-1252).
    bad = enc(BASIC, False).replace(
        b"# small reward", b"# small reward \x81 stray byte"
    )
    add("invalid_utf8_lf.txt", bad, tree="exp_basic_tree", bom=False, newline="lf", conditions=["invalid_utf8", "lf", "comments"])
    # Valid Windows-1252 (legacy encoding): 0xE9 is e-acute in a comment.
    cp = enc(BASIC, False).replace(b"# small reward", b"# small reward caf\xe9")
    add("windows1252_lf.txt", cp, tree="exp_basic_tree", bom=False, newline="lf", conditions=["windows_1252", "lf", "comments"])
    add("relative_orphan_lf.txt", enc(RELATIVE_ORPHAN, False), tree="exp_orphan_tree", bom=False, newline="lf", conditions=["relative_position", "missing_anchor", "lf"])
    add("shared_standalone_crlf_bom.txt", enc(crlf(SHARED_STANDALONE), True), tree="exp_ss_tree", bom=True, newline="crlf", conditions=["shared_focus", "bom", "crlf"])
    add("unknown_crlf_bom.txt", enc(crlf(UNKNOWN), True), tree="exp_unknown_tree", bom=True, newline="crlf", conditions=["bom", "crlf", "comments", "unknown_syntax", "scripted_constant"])
    add("duplicate_lf.txt", enc(DUPLICATE, False), tree="exp_dup_tree", bom=False, newline="lf", conditions=["duplicate_ids", "lf"])
    add("shared_lf.txt", enc(SHARED, False), tree="exp_shared_tree", bom=False, newline="lf", conditions=["shared_focus", "relative_position", "comments", "lf"])
    add("relative_crlf.txt", enc(crlf(RELATIVE), False), tree="exp_rel_tree", bom=False, newline="crlf", conditions=["relative_position", "crlf"])
    add("commented_prereq_lf.txt", enc(COMMENTED_PREREQ, False), tree="exp_cmt_tree", bom=False, newline="lf", conditions=["comment_in_edited_block", "lf"])
    add("malformed_unclosed_lf.txt", enc(MALFORMED_UNCLOSED, False), tree="exp_bad_tree", bom=False, newline="lf", conditions=["malformed_unclosed"])
    add("malformed_stray_crlf.txt", enc(crlf(MALFORMED_STRAY), False), tree="exp_stray_tree", bom=False, newline="crlf", conditions=["malformed_stray_brace", "unterminated_string"])
    return out


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for name, (data, meta) in fixtures().items():
        (OUT / name).write_bytes(data)
        manifest[name] = {**meta, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    # Support files copied into every probe workspace so that reference lint
    # (sprite lookup) does not block the writer under test. Original text.
    import re
    names = set()
    for data, _ in fixtures().values():
        names.update(m.decode() for m in re.findall(rb"GFX_exp_icon_[a-z_]+", data))
    names.add("GFX_exp_icon_created")
    support = HERE.parent / "fixture-support" / "interface"
    support.mkdir(parents=True, exist_ok=True)
    nl, tab = "\n", "\t"
    body = "spriteTypes = {" + nl + "".join(
        f'{tab}spriteType = {{{nl}{tab}{tab}name = "{n}"{nl}{tab}{tab}texturefile = "gfx/interface/goals/exp02_placeholder.dds"{nl}{tab}}}{nl}'
        for n in sorted(names)
    ) + "}" + nl
    (support / "exp02_icons.gfx").write_bytes(body.encode("utf-8"))
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"written": len(manifest), "dir": str(OUT)}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
