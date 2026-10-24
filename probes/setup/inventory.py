#!/usr/bin/env python3
"""Independent EXP-01 inventory. Written for HOI4 Factory, shares no code with Setup.

Usage: python3 -I inventory.py <root> <out.json>

Records every entry under <root>, including .git, with type, permission bits,
size, SHA-256 of regular-file bytes, symlink target, owner and timestamps.
It never follows symlinks and never writes inside <root>. Git state is read
from .git/index bytes directly. Optional git porcelain output is taken with
--no-optional-locks so the index is not refreshed.
"""
import hashlib
import json
import os
import stat
import subprocess
import sys


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def walk(root):
    entries = {}
    for current, dirs, files in os.walk(root, topdown=True, followlinks=False):
        dirs.sort()
        for name in sorted(dirs + files):
            path = os.path.join(current, name)
            rel = os.path.relpath(path, root).replace(os.sep, "/")
            info = os.lstat(path)
            mode = info.st_mode
            entry = {
                "mode": oct(stat.S_IMODE(mode)),
                "uid": info.st_uid,
                "gid": info.st_gid,
                "mtime_ns": info.st_mtime_ns,
                "ino": info.st_ino,
            }
            if stat.S_ISLNK(mode):
                entry["type"] = "symlink"
                entry["target"] = os.readlink(path)
            elif stat.S_ISDIR(mode):
                entry["type"] = "dir"
            elif stat.S_ISREG(mode):
                entry["type"] = "file"
                entry["size"] = info.st_size
                entry["sha256"] = sha256_file(path)
            else:
                entry["type"] = "other"
            entries[rel] = entry
    return entries


def git_state(root):
    if not os.path.isdir(os.path.join(root, ".git")):
        return None
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "GIT_CONFIG_NOSYSTEM": "1",
           "HOME": "/nonexistent", "GIT_OPTIONAL_LOCKS": "0"}
    def run(*args):
        result = subprocess.run(["git", "--no-optional-locks", "-c", "safe.directory=*", "-C", root, *args],
                                capture_output=True, env=env)
        return {"exit": result.returncode, "stdout": result.stdout.decode("utf-8", "replace")}
    index = os.path.join(root, ".git", "index")
    return {
        "index_sha256": sha256_file(index) if os.path.exists(index) else None,
        "head": run("rev-parse", "HEAD"),
        "ls_files_stage": run("ls-files", "-s"),
        "status_porcelain_v2": run("status", "--porcelain=v2", "--untracked-files=all", "--ignored=no"),
    }


def main():
    root, out = sys.argv[1], sys.argv[2]
    root = os.path.abspath(root)
    before_index = None
    index = os.path.join(root, ".git", "index")
    if os.path.exists(index):
        before_index = sha256_file(index)
    entries = walk(root)
    git = git_state(root)
    if git is not None and before_index is not None and git["index_sha256"] != before_index:
        raise SystemExit("inventory tool changed the git index, refusing to report")
    files = {k: v for k, v in entries.items() if v["type"] == "file"}
    content_digest = hashlib.sha256(json.dumps(
        sorted((k, v["type"], v["mode"], v.get("size"), v.get("sha256"), v.get("target")) for k, v in entries.items()),
        separators=(",", ":")).encode()).hexdigest()
    report = {
        "tool": "probes/setup/inventory.py",
        "root": root,
        "entry_count": len(entries),
        "file_count": len(files),
        "byte_total": sum(v["size"] for v in files.values()),
        "content_digest": content_digest,
        "entries": entries,
        "git": git,
    }
    with open(out, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=1, sort_keys=True)
    print(json.dumps({"out": out, "entries": len(entries), "files": len(files), "content_digest": content_digest}))


if __name__ == "__main__":
    main()
