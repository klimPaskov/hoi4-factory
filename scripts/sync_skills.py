#!/usr/bin/env python3
"""Generate or check this repository's four owned skill projections.

Default mode is read-only. No global installation or network access occurs.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
from urllib.parse import quote, unquote, urlsplit, urlunsplit

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / "docs/07-developer-workflow"
MANIFEST = WORKFLOW / "skill-hashes.json"
EXPECTED_NAMES = {
    "start-implementation-stage",
    "validate-stage",
    "capture-engineering-lessons",
    "audit-hoi4-integrations",
}
CLIENTS = (".claude", ".codex", ".agents")
# ADR-023: these workflows stay invocable only by the maintainer. The others run automatically.
MANUAL_ONLY = {"start-implementation-stage", "audit-hoi4-integrations"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def assert_owned(path: Path) -> None:
    """Refuse an escaped or symlinked generated destination."""
    relative = path.relative_to(ROOT)
    current = ROOT
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"Refusing symlinked managed path: {relative}")
    path.resolve().relative_to(ROOT)


def rebase_links(body: str, source: Path, destination: Path) -> str:
    """Keep local Markdown links correct in every generated client copy."""
    def replace(match: re.Match[str]) -> str:
        target = match.group(2)
        parsed = urlsplit(target)
        if parsed.scheme or target.startswith("//") or not parsed.path:
            return match.group(0)
        resolved = (source.parent / unquote(parsed.path)).resolve()
        assert_owned(resolved)
        relative = Path(os.path.relpath(resolved, destination.parent)).as_posix()
        rebased = urlunsplit(("", "", quote(relative, safe="/.-_~"), parsed.query, parsed.fragment))
        return match.group(1) + rebased + match.group(3)

    return re.sub(r"(\[[^\]]*\]\()([^\s)]+)(\))", replace, body)


def expected_files() -> tuple[dict[Path, bytes], dict]:
    metadata_path = WORKFLOW / "skills.json"
    assert_owned(metadata_path)
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    if metadata.get("invocation") != "explicit":
        raise ValueError("Starter invocation policy must remain explicit")
    skills = metadata.get("skills", [])
    names = [entry.get("name") for entry in skills]
    if len(names) != len(set(names)) or set(names) != EXPECTED_NAMES:
        raise ValueError("Metadata must define exactly the four owned skill names")
    outputs: dict[Path, bytes] = {}
    records = []
    for entry in sorted(skills, key=lambda item: item["name"]):
        name = entry["name"]
        description = entry.get("description", "")
        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name) or len(name) > 64:
            raise ValueError(f"Invalid skill name: {name}")
        if not isinstance(description, str) or not 1 <= len(description) <= 1024:
            raise ValueError(f"Invalid description: {name}")
        if "\n" in description or "\r" in description:
            raise ValueError(f"Description must be one scalar line: {name}")
        source = WORKFLOW / "skill-workflows" / name / "BODY.md"
        assert_owned(source)
        body = source.read_text(encoding="utf-8")
        if not body.startswith("# ") or not body.endswith("\n"):
            raise ValueError(f"Body needs a heading and final newline: {name}")
        for required in ("## Trigger and inputs", "## Procedure", "## Outputs and evidence", "## Stop and error paths"):
            if required not in body:
                raise ValueError(f"Missing {required} in {name}")
        generated = []
        for client in CLIENTS:
            header = "---\n" + f"name: {name}\n" + f"description: {json.dumps(description, ensure_ascii=False)}\n"
            if client == ".claude" and name in MANUAL_ONLY:
                header += "disable-model-invocation: true\n"
            header += "---\n\n"
            path = ROOT / client / "skills" / name / "SKILL.md"
            assert_owned(path)
            content = (header + rebase_links(body, source, path)).encode("utf-8")
            outputs[path] = content
            generated.append({"path": path.relative_to(ROOT).as_posix(), "sha256": digest(content)})
            if client in (".codex", ".agents"):
                sidecar = path.parent / "agents" / "openai.yaml"
                assert_owned(sidecar)
                implicit = "false" if name in MANUAL_ONLY else "true"
                sidecar_content = f"policy:\n  allow_implicit_invocation: {implicit}\n".encode()
                outputs[sidecar] = sidecar_content
                generated.append({"path": sidecar.relative_to(ROOT).as_posix(), "sha256": digest(sidecar_content)})
        records.append({
            "name": name,
            "source": source.relative_to(ROOT).as_posix(),
            "body_sha256": digest(body.encode("utf-8")),
            "generated": generated,
        })
    manifest = {
        "schema_version": 1,
        "version": metadata["version"],
        "generator": "scripts/sync_skills.py",
        "metadata_sha256": digest(metadata_path.read_bytes()),
        "scope": "Only these repository projections. No personal installation.",
        "skills": records,
    }
    assert_owned(MANIFEST)
    outputs[MANIFEST] = (json.dumps(manifest, indent=2) + "\n").encode("utf-8")
    return outputs, manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="Read-only check, also the default")
    mode.add_argument("--write", action="store_true", help="Regenerate owned copies after reviewed source edits")
    args = parser.parse_args()
    try:
        outputs, manifest = expected_files()
        different = [p for p, content in outputs.items() if not p.is_file() or p.read_bytes() != content]
        if args.write:
            for path in different:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(outputs[path])
            print(json.dumps({"mode": "write", "changed": [p.relative_to(ROOT).as_posix() for p in different], "skills": len(manifest["skills"])}))
            return 0
        result = {"mode": "check", "status": "pass" if not different else "fail", "skills": len(manifest["skills"]), "generated_files": len(outputs), "drift": [p.relative_to(ROOT).as_posix() for p in different], "client_trials": "not performed by this checker"}
        print(json.dumps(result, indent=2))
        return 1 if different else 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(json.dumps({"status": "error", "error": str(error)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
