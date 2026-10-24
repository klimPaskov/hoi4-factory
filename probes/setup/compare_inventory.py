#!/usr/bin/env python3
"""Compare two EXP-01 inventories. Written for HOI4 Factory, shares no code with Setup.

Usage: python3 -I compare_inventory.py <before.json> <after.json> <out.json> [approved_prefix ...]

A path is approved when it equals or lies under one of the approved prefixes.
Every other path is unapproved and must keep its type, permission bits, size,
SHA-256 and symlink target. Added and removed unapproved paths are failures.
mtime and inode changes are reported as metadata, never hidden.
"""
import json
import sys


def approved(path, prefixes):
    return any(path == p or path.startswith(p.rstrip("/") + "/") for p in prefixes)


def key(entry):
    return (entry["type"], entry["mode"], entry.get("size"), entry.get("sha256"), entry.get("target"))


def main():
    before_path, after_path, out_path, *prefixes = sys.argv[1:]
    before = json.load(open(before_path, encoding="utf-8"))
    after = json.load(open(after_path, encoding="utf-8"))
    b, a = before["entries"], after["entries"]
    result = {
        "before": before_path, "after": after_path, "approved_prefixes": prefixes,
        "unapproved": {"changed": [], "mode_changed": [], "added": [], "removed": [], "metadata_only": []},
        "approved": {"changed": [], "added": [], "removed": []},
        "all": {"added_files": [], "added_dirs": [], "removed": [], "content_changed": []},
    }
    for path in sorted(set(b) | set(a)):
        side = "approved" if approved(path, prefixes) else "unapproved"
        eb, ea = b.get(path), a.get(path)
        if eb and not ea:
            result[side]["removed"].append(path)
            result["all"]["removed"].append(path)
        elif ea and not eb:
            result[side]["added"].append({"path": path, "type": ea["type"], "size": ea.get("size"), "sha256": ea.get("sha256")})
            result["all"]["added_dirs" if ea["type"] == "dir" else "added_files"].append(path)
        elif key(eb) != key(ea):
            if side == "unapproved" and eb["type"] == ea["type"] and eb.get("sha256") == ea.get("sha256") and eb["mode"] != ea["mode"]:
                result["unapproved"]["mode_changed"].append({"path": path, "before": eb["mode"], "after": ea["mode"]})
            else:
                result[side]["changed"].append({"path": path, "before": key(eb), "after": key(ea)})
            result["all"]["content_changed"].append(path)
        elif side == "unapproved" and eb["type"] != "dir" and (eb["mtime_ns"] != ea["mtime_ns"] or eb["ino"] != ea["ino"]):
            result["unapproved"]["metadata_only"].append(path)
    u = result["unapproved"]
    result["unapproved_bytes_untouched"] = not (u["changed"] or u["mode_changed"] or u["added"] or u["removed"])
    result["exact_files"] = not (result["all"]["added_files"] or result["all"]["removed"] or result["all"]["content_changed"])
    result["exact_files_and_dirs"] = result["exact_files"] and not result["all"]["added_dirs"]
    result["git_index_sha256"] = {
        "before": (before.get("git") or {}).get("index_sha256"),
        "after": (after.get("git") or {}).get("index_sha256"),
    }
    gb, ga = before.get("git") or {}, after.get("git") or {}
    result["git_status_identical"] = gb.get("status_porcelain_v2") == ga.get("status_porcelain_v2")
    result["git_ls_files_identical"] = gb.get("ls_files_stage") == ga.get("ls_files_stage")
    with open(out_path, "w", encoding="utf-8") as handle:
        json.dump(result, handle, indent=1)
    print(json.dumps({"unapproved_bytes_untouched": result["unapproved_bytes_untouched"],
                      "exact_files": result["exact_files"], "exact_files_and_dirs": result["exact_files_and_dirs"],
                      "added_files": result["all"]["added_files"][:10], "git_status_identical": result["git_status_identical"],
                      "metadata_only_unapproved": len(u["metadata_only"])}))


if __name__ == "__main__":
    main()
