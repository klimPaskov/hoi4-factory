#!/usr/bin/env python3
"""Validate project planning and guidance without invoking product tools.

Default mode checks the clean v02 distribution, including its frozen manifest.
Use --working-tree during development to validate the documentation and owned
starter assets without comparing frozen hashes or inspecting unrelated product
files. Archive comparison always uses strict distribution checks.
"""
from __future__ import annotations
import argparse
import ast
import csv
import hashlib
import json
import re
import subprocess
import sys
import tomllib
import zipfile
from pathlib import Path
from urllib.parse import unquote, urlsplit

REQUIRED = [
    "README.md", "docs/README.md", "scripts/validate_package.py", "docs/MANIFEST.md", "docs/MASTER_GOAL.md", "docs/PACKAGE_STATUS.md", "AGENTS.md", "CLAUDE.md",
    "docs/00-research/00-executive-recommendation.md", "docs/00-research/01-research-sources.md",
    "docs/00-research/02-existing-assets-audit.md", "docs/00-research/03-candidate-comparison.md",
    "docs/00-research/04-evidence-and-research-log.md", "docs/00-research/source-registry.json",
    "docs/00-research/supplied-source-coverage.json",
    "docs/01-product/product-requirements.md", "docs/01-product/users-and-workflows.md",
    "docs/01-product/visual-and-autonomy-model.md", "docs/01-product/requirements-traceability.md",
    "docs/01-product/requirements.json",
    "docs/02-architecture/options-and-decisions.md", "docs/02-architecture/recommended-architecture.md",
    "docs/02-architecture/data-context-and-state.md", "docs/02-architecture/contracts-and-interfaces.md",
    "docs/02-architecture/deployment-and-security.md",
    "docs/04-implementation-stages/ROADMAP.md", "docs/04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md",
    "docs/05-verification/TEST-STRATEGY.md", "docs/05-verification/EVALUATION-MATRIX.md",
    "docs/05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md",
    "docs/05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md",
    "docs/05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md",
    "docs/05-verification/test-catalog.json", "docs/05-verification/acceptance-matrix.json",
    "docs/05-verification/starter-config-and-skills-checks.json",
    "docs/06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md", "docs/06-agent-handoffs/START-STAGE-01.md",
    "docs/06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md",
    "docs/06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md",
    "docs/06-agent-handoffs/AGENT-WORKFLOW-AND-REVIEW-RULES.md",
    "docs/07-developer-workflow/CONFIG-MIGRATION-REPORT.md",
    "docs/07-developer-workflow/CONFIG-FIELD-MAP.csv",
    "docs/07-developer-workflow/CONFIG-SOURCE-INVENTORY.json",
    "docs/07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md",
    "docs/07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md",
    "docs/07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md",
    "docs/07-developer-workflow/KNOWLEDGE-CHANGELOG.md",
    "scripts/sync_skills.py",
    ".codex/config.toml", ".claude/settings.json", ".mcp.json",
]
SKILLS = {
    "start-implementation-stage", "validate-stage",
    "capture-engineering-lessons", "audit-hoi4-integrations",
    "production-readiness-review",
    "delegate-parallel-work",
}
STAGE_SECTIONS = [
    "Objective and non-goals", "Entry conditions", "Read-first sequence and reuse",
    "Implementation-sized work packets", "Packet dependencies and exclusions",
    "Verification plan", "Measurable exit gates", "Risk and rollback",
    "Engineering learning checkpoint", "Configuration checkpoint",
    "Standalone completion and continuation handoff",
]
EXCLUDED_HASH_FILES = {"docs/MANIFEST.json", "docs/05-verification/verification-run.json"}

def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def without_fences(text: str) -> str:
    return re.sub(r"(?ms)^\x60\x60\x60[^\n]*\n.*?^\x60\x60\x60\s*$", "", text)

def anchors(text: str) -> set[str]:
    out, seen = set(), {}
    for heading in re.findall(r"(?m)^#{1,6}\s+(.+?)\s*#*\s*$", without_fences(text)):
        slug = re.sub(r"[^\w\s-]", "", heading.lower())
        slug = re.sub(r"\s", "-", slug)
        n = seen.get(slug, 0)
        seen[slug] = n + 1
        out.add(slug if n == 0 else f"{slug}-{n}")
    out.update(re.findall(r'<a\s+id=["\x27]([^"\x27]+)', text))
    return out

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--archive", type=Path, help="Compare a ZIP with the clean distribution, including its single parent folder")
    parser.add_argument("--working-tree", action="store_true",
        help="Check docs and owned starter assets without frozen manifest hashes or unrelated product files")
    parser.add_argument("--write-report", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    if args.working_tree and args.archive:
        parser.error("--working-tree cannot be combined with --archive. Archive verification requires the clean distribution.")
    if args.archive and args.write_report:
        report_target = args.write_report if args.write_report.is_absolute() else root / args.write_report
        if report_target.resolve().is_relative_to(root):
            parser.error("Archive comparison must be read-only for the package. Write its report outside the package or omit --write-report.")
    checks: list[dict] = []
    if args.working_tree:
        scoped = {root / name for name in (
            "README.md", "AGENTS.md", "CLAUDE.md", ".mcp.json",
            ".claude/settings.json", ".codex/config.toml",
            "scripts/validate_package.py", "scripts/sync_skills.py",
        )}
        owned_directories = [root / "docs"] + [
            root / client / "skills" / name
            for client in (".claude", ".codex", ".agents") for name in SKILLS
        ]
        for directory in owned_directories:
            if directory.exists() or directory.is_symlink():
                scoped.add(directory)
                if not directory.is_symlink():
                    scoped.update(directory.rglob("*"))
        all_paths = sorted(p for p in scoped if p.exists() or p.is_symlink())
    else:
        all_paths = sorted(root.rglob("*"))
    symlinks = [p for p in all_paths if p.is_symlink()]
    files = [p for p in all_paths if p.is_file() and not p.is_symlink()]
    by_name = {p.relative_to(root).as_posix(): p for p in files}
    def add(name: str, errors: list[str], **details) -> None:
        checks.append({"check": name, "passed": not errors, "errors": errors, **details})
    def read_json(name: str):
        return json.loads((root / name).read_text(encoding="utf-8"))
    missing = [p for p in REQUIRED if p not in by_name]
    add("required_content", missing, required_paths=len(REQUIRED))
    parse_errors, parsed_json, parsed_toml = [], {}, {}
    for name, path in by_name.items():
        try:
            if path.suffix == ".json":
                parsed_json[name] = json.loads(path.read_text(encoding="utf-8"))
            elif path.suffix == ".toml":
                parsed_toml[name] = tomllib.loads(path.read_text(encoding="utf-8"))
            elif path.suffix == ".py":
                ast.parse(path.read_text(encoding="utf-8"), filename=name)
        except (ValueError, SyntaxError) as exc:
            parse_errors.append(f"{name}: {exc}")
    add("json_toml_python_syntax", parse_errors, json_files=len(parsed_json), toml_files=len(parsed_toml),
        python_files=sum(p.suffix == ".py" for p in files))
    link_errors, style_errors, link_count, mermaid_count = [], [], 0, 0
    md_texts = {n: p.read_text(encoding="utf-8") for n, p in by_name.items() if p.suffix == ".md"}
    for name, text in md_texts.items():
        plain = without_fences(text)
        if "\u2014" in plain:
            style_errors.append(f"{name}: em dash in prose")
        if ";" in plain:
            style_errors.append(f"{name}: semicolon in prose")
        if len(re.findall(r"(?m)^\x60\x60\x60", text)) % 2:
            style_errors.append(f"{name}: unbalanced code fences")
        mermaid_count += len(re.findall(r"(?m)^\x60\x60\x60mermaid\s*$", text))
        for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", plain):
            target = target.strip().split(' "')[0]
            parsed = urlsplit(target)
            if parsed.scheme or target.startswith("//"):
                continue
            link_count += 1
            dest = (by_name[name].parent / unquote(parsed.path)).resolve() if parsed.path else by_name[name]
            try:
                rel = dest.relative_to(root).as_posix()
            except ValueError:
                link_errors.append(f"{name}: link escapes package: {target}")
                continue
            if not dest.exists():
                link_errors.append(f"{name}: missing {target}")
            elif parsed.fragment and dest.suffix == ".md":
                if unquote(parsed.fragment) not in anchors(md_texts.get(rel, dest.read_text(encoding="utf-8"))):
                    link_errors.append(f"{name}: missing heading in {target}")
    add("relative_markdown_links", link_errors, checked_links=link_count)
    add("prose_and_fence_checks", style_errors, markdown_files=len(md_texts), mermaid_blocks=mermaid_count,
        diagram_limit="Fences and human structural review only, no Mermaid renderer executed")
    structural = []
    specs = sorted(root.glob("docs/03-subsystem-specs/[0-9][0-9][0-9]-*.md"))
    stages = sorted(root.glob("docs/04-implementation-stages/stage-[0-9][0-9]-*.md"))
    if len(specs) != 13: structural.append(f"Expected 13 specs, got {len(specs)}")
    if len(stages) != 8: structural.append(f"Expected 8 stages, got {len(stages)}")
    packet_ids, gate_counts = set(), {}
    for p in specs:
        if len(p.read_text().split()) < 600: structural.append(f"{p.name}: insufficient substantive content")
    for p in stages:
        t = p.read_text()
        for h in STAGE_SECTIONS:
            if f"## {h}" not in t: structural.append(f"{p.name}: missing {h}")
        sid = "S" + p.name.split("-")[1]
        ids = set(re.findall(rf"\b{sid}-W[1-6]\b", t))
        packet_ids.update(ids)
        if len(ids) != 6: structural.append(f"{sid}: expected six packet IDs")
        part = t.split("## Measurable exit gates", 1)[-1].split("\n## ", 1)[0]
        gate_counts[sid] = len(re.findall(r"(?m)^\d+\. ", part))
        if "Do not start the next stage automatically" not in t:
            structural.append(f"{sid}: missing continuation stop")
        for f in ("learning-review.md", "config-review.md", "handoff.md"):
            if f not in t: structural.append(f"{sid}: missing {f} evidence")
    if len(packet_ids) != 48: structural.append(f"Expected 48 packet IDs, got {len(packet_ids)}")
    if sum(gate_counts.values()) != 47: structural.append("Expected 47 numbered stage gates")
    add("specification_and_stage_structure", structural, specifications=len(specs), stages=len(stages),
        unique_packets=len(packet_ids), stage_exit_gates=sum(gate_counts.values()))
    matrix_errors = []
    try:
        reqs = read_json("docs/01-product/requirements.json")
        matrix = read_json("docs/05-verification/acceptance-matrix.json")
        tests = read_json("docs/05-verification/test-catalog.json")["tests"]
        req_ids = {r["id"] for r in reqs}
        test_ids = {t["id"] for t in tests}
        expected = {f"REQ-{i:03d}" for i in range(1,51)}
        if req_ids != expected or len(reqs) != 50: matrix_errors.append("Requirement inventory is not exactly REQ-001..050")
        if len(test_ids) != len(tests): matrix_errors.append("Duplicate test IDs")
        rows = matrix["requirements"]
        if {m["requirement"] for m in rows} != expected or len(rows) != 50:
            matrix_errors.append("Acceptance does not cover each requirement exactly once")
        source = {r["id"]: r for r in reqs}
        for m in rows:
            r = source[m["requirement"]]
            for k in ("spec", "stage", "acceptance", "spec_file", "stage_file"):
                if m[k] != r[k]: matrix_errors.append(f'{r["id"]}: mismatched {k}')
            if not m["tests"] or not set(m["tests"]).issubset(test_ids):
                matrix_errors.append(f'{r["id"]}: missing or undefined tests')
            if not m["oracle"].strip(): matrix_errors.append(f'{r["id"]}: empty acceptance oracle')
        expected_gates = {f"{s}-G{i}" for s,n in gate_counts.items() for i in range(1,n+1)}
        rows = matrix["stage_gates"]
        if {g["id"] for g in rows} != expected_gates or len(rows) != len(expected_gates):
            matrix_errors.append("Stage gate mapping differs from packet exit lists")
        for g in rows:
            if not g["tests"] or not set(g["tests"]).issubset(test_ids):
                matrix_errors.append(f'{g["id"]}: missing or undefined tests')
        add("requirements_tests_and_gates", matrix_errors, requirements=len(reqs), tests=len(tests), gates=len(rows))
    except (OSError, ValueError, KeyError) as exc:
        add("requirements_tests_and_gates", [str(exc)])
    skill_errors = []
    for client in (".claude", ".codex", ".agents"):
        paths = list((root / client / "skills").glob("*/SKILL.md"))
        discovered = {p.parent.name for p in paths}
        skill_set_ok = SKILLS.issubset(discovered) if args.working_tree else discovered == SKILLS
        if not skill_set_ok:
            skill_errors.append(f"{client}: owned skill set mismatch")
        if args.working_tree:
            paths = [p for p in paths if p.parent.name in SKILLS]
        for p in paths:
            t = p.read_text()
            if not t.startswith("---\n") or "\n---\n" not in t[4:]:
                skill_errors.append(f"{p.relative_to(root)}: missing frontmatter")
                continue
            header = t.split("---\n", 2)[1]
            values = dict(re.findall(r"(?m)^([a-z-]+):\s*(.+)$", header))
            name = values.get("name", "")
            desc = values.get("description", "").strip('"')
            if name != p.parent.name or not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*",name):
                skill_errors.append(f"{p.relative_to(root)}: invalid name")
            if not 30 <= len(desc) <= 1024: skill_errors.append(f"{name}: unusable description")
            # ADR-023: stage start and integration audits stay maintainer-invoked. Validation and lesson
            # capture run automatically at handoff.
            manual = name in {"start-implementation-stage", "audit-hoi4-integrations"}
            if client == ".claude" and (values.get("disable-model-invocation") == "true") != manual:
                skill_errors.append(f"{name}: Claude invocation mode differs from ADR-023")
    # ADR-024: CLAUDE.md is canonical and AGENTS.md must be its byte-identical copy.
    if (root / "CLAUDE.md").read_bytes() != (root / "AGENTS.md").read_bytes():
        skill_errors.append("AGENTS.md differs from canonical CLAUDE.md, run sync_skills.py --write")
    try:
        run = subprocess.run([sys.executable, str(root / "scripts/sync_skills.py"), "--check"],
            cwd=root, capture_output=True, text=True, timeout=30)
        if run.returncode: skill_errors.append("Skill mirror checker failed: " + run.stdout.strip() + run.stderr.strip())
    except (OSError, subprocess.TimeoutExpired) as exc:
        skill_errors.append(str(exc))
    add("skills_frontmatter_import_and_drift", skill_errors, real_skill_files=12)
    config_errors = []
    try:
        c = parsed_toml[".codex/config.toml"]
        allowed = {"approval_policy","sandbox_mode","project_doc_max_bytes","agents","mcp_servers"}
        if set(c) - allowed: config_errors.append("Unexpected active Codex root keys")
        if c["approval_policy"] != "on-request" or c["sandbox_mode"] != "workspace-write":
            config_errors.append("Unexpected Codex starter permission defaults")
        if c["agents"] != {"max_concurrent_threads_per_session":4}:
            config_errors.append("Unexpected Codex concurrency shape")
        expected_servers = {"hoi4_agent_tools", "comfy_cloud_portraits"}
        if set(c["mcp_servers"]) != expected_servers:
            config_errors.append("Unexpected Codex MCP server names")
        for name, server in c["mcp_servers"].items():
            if server.get("enabled") is not False: config_errors.append(f"{name}: must remain disabled")
            server_keys = ({"enabled","command","args","cwd","startup_timeout_sec","tool_timeout_sec","default_tools_approval_mode","env"}
                if name == "hoi4_agent_tools" else {"enabled","url"})
            if set(server) - server_keys: config_errors.append(f"{name}: unsupported active Codex MCP key")
        tool = c["mcp_servers"]["hoi4_agent_tools"]
        if tool.get("env") != {"HOI4_AGENT_STDIO_IDLE_EXIT_MINUTES":"30"}:
            config_errors.append("Unexpected Codex tool environment")
        if tool.get("default_tools_approval_mode") != "prompt" or tool.get("cwd") != ".":
            config_errors.append("Unexpected Codex tool approval or working-directory shape")
        claude = parsed_json[".claude/settings.json"]
        if set(claude) != {"$schema","permissions","enableAllProjectMcpServers","disabledMcpjsonServers"}:
            config_errors.append("Unexpected active Claude root key")
        if set(claude.get("permissions",{})) != {"defaultMode","deny"}:
            config_errors.append("Unexpected active Claude permission key")
        if claude["permissions"].get("deny") != ["Read(./.env)","Read(./.env.*)"]:
            config_errors.append("Unexpected Claude secret-file deny example")
        mcp_root = parsed_json[".mcp.json"]
        if set(mcp_root) != {"mcpServers"}: config_errors.append("Unexpected Claude MCP root key")
        mcp = mcp_root["mcpServers"]
        for name,server in mcp.items():
            expected_keys = {"type","command","args","env","timeout"} if name == "hoi4_agent_tools" else {"type","url"}
            if set(server) != expected_keys: config_errors.append(f"{name}: unsupported Claude MCP key")
            if server.get("type") != ("stdio" if name == "hoi4_agent_tools" else "http"):
                config_errors.append(f"{name}: wrong Claude MCP transport")
        if mcp["hoi4_agent_tools"].get("env") != {"HOI4_AGENT_STDIO_IDLE_EXIT_MINUTES":"30"}:
            config_errors.append("Unexpected Claude tool environment")
        if mcp["hoi4_agent_tools"].get("args") != []:
            config_errors.append("Unexpected Claude tool arguments")
        if set(mcp) != set(c["mcp_servers"]): config_errors.append("MCP client relationship sets differ")
        if set(claude["disabledMcpjsonServers"]) != set(mcp): config_errors.append("Claude MCP is not fully disabled")
        if claude["enableAllProjectMcpServers"] is not False: config_errors.append("Blanket MCP approval enabled")
        if claude["permissions"]["defaultMode"] != "default": config_errors.append("Unexpected Claude permission mode")
        if mcp["hoi4_agent_tools"]["timeout"] != c["mcp_servers"]["hoi4_agent_tools"]["tool_timeout_sec"] * 1000:
            config_errors.append("MCP timeout unit translation differs")
        with (root / "docs/07-developer-workflow/CONFIG-FIELD-MAP.csv").open(newline="") as f:
            rows = list(csv.DictReader(f))
        keys = {(r["source_path"],r["source_key"]) for r in rows}
        if len(rows) != 196 or len(keys) != 196: config_errors.append("Expected 196 unique source-field mappings")
        inventory = parsed_json["docs/07-developer-workflow/CONFIG-SOURCE-INVENTORY.json"]
        expected_keys = {(".codex/config.toml",k) for k in inventory["root_field_keys"]}
        for role in inventory["roles"]:
            expected_keys.update((role["path"],k) for k in role["fields"])
        if keys != expected_keys or len(expected_keys) != 196:
            config_errors.append("Mapping identities differ from actual pinned source inventory")
        if any(row["source_revision"] != inventory["source_revision"] for row in rows):
            config_errors.append("Mapping source revision differs from pinned inventory")
        for row in rows:
            if any(not str(v).strip() for v in row.values()): config_errors.append("Incomplete source-field mapping")
        add("starter_config_and_field_mapping", config_errors, mapped_fields=len(rows),
            validation_scope="Parse and explicit documented used-key checks. No exhaustive JSON Schema engine or live client invocation.")
    except (OSError, ValueError, KeyError) as exc:
        add("starter_config_and_field_mapping", [str(exc)])
    registry_errors = []
    try:
        registry = read_json("docs/00-research/source-registry.json")["records"]
        ids = {r["id"] for r in registry}
        if len(ids) != len(registry): registry_errors.append("Duplicate source IDs")
        for r in registry:
            if r["status"] not in {"Verified","Documented only","Proposed","Unknown"}:
                registry_errors.append(f'{r["id"]}: invalid evidence status')
            if not r.get("url") or not r.get("limitations"): registry_errors.append(f'{r["id"]}: missing source or limits')
        for name,t in md_texts.items():
            for sid in set(re.findall(r"\bSRC-[ABCR]\d{2}\b",t)):
                if sid not in ids: registry_errors.append(f"{name}: undefined {sid}")
        coverage = read_json("docs/00-research/supplied-source-coverage.json")
        sources = coverage["distinct_texts"]
        if len(sources) != 152 or len({s["sha256"] for s in sources}) != 152:
            registry_errors.append("Supplied distinct-source count differs from source gate")
        if sum(s["bytes"] for s in sources) != 3286648: registry_errors.append("Source byte accounting differs")
        chunks = [c["id"] for s in sources for c in s["chunks"]]
        if sorted(chunks) != list(range(1,251)): registry_errors.append("Source chunk coverage is incomplete or duplicated")
        add("research_registry_and_source_coverage", registry_errors, records=len(registry),
            distinct_supplied_texts=len(sources), full_read_chunks=len(chunks))
    except (OSError, ValueError, KeyError) as exc:
        add("research_registry_and_source_coverage", [str(exc)])
    hygiene = [f"{p.relative_to(root)}: symlink" for p in symlinks]
    secret_patterns = [
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
        r"\bsk-[A-Za-z0-9_-]{24,}\b",
        r"\bgh[pousr]_[A-Za-z0-9]{30,}\b",
    ]
    allowed_extensions = {".md",".json",".csv",".toml",".yaml",".yml",".py"}
    for name,p in by_name.items():
        if p.is_symlink(): hygiene.append(f"{name}: symlink")
        if not p.stat().st_size: hygiene.append(f"{name}: empty file")
        if p.suffix not in allowed_extensions and not args.working_tree:
            hygiene.append(f"{name}: unexpected artifact type")
        if any(part in {".git","node_modules","__pycache__","source-b","cache-a","project_sources","upload"} for part in p.parts):
            hygiene.append(f"{name}: source cache or dependency directory")
        if p.suffix in allowed_extensions - {".py"}:
            text = p.read_text(encoding="utf-8")
            if re.search(r"/workspace/scratch/|/home/[A-Za-z0-9_-]+/|/Users/[A-Za-z0-9_-]+/|[A-Za-z]:\\Users\\",text):
                hygiene.append(f"{name}: machine-specific path")
            if any(re.search(pattern,text) for pattern in secret_patterns):
                hygiene.append(f"{name}: possible credential material")
    add("artifact_hygiene", hygiene, file_count=len(files),
        limitation="Pattern-based credential scan, not a guarantee that arbitrary encoded secrets are detectable")
    manifest_errors = []
    manifest_path = root / "docs/MANIFEST.json"
    if args.working_tree:
        add("manifest_hashes", [], skipped=True,
            reason="Working-tree mode checks current planning and guidance. Frozen distribution hashes are not compared.")
    elif manifest_path.exists():
        try:
            manifest = read_json("docs/MANIFEST.json")
            entries = manifest["files"]
            expected_names = set(by_name) - EXCLUDED_HASH_FILES
            if {e["path"] for e in entries} != expected_names: manifest_errors.append("Manifest file inventory mismatch")
            if len(entries) != len(expected_names) or len({e["path"] for e in entries}) != len(entries):
                manifest_errors.append("Manifest has duplicate entries or incorrect count")
            for e in entries:
                entry_path = e["path"]
                relative = Path(entry_path)
                if relative.is_absolute() or ".." in relative.parts or "\\" in entry_path or re.match(r"^[A-Za-z]:",entry_path) or relative.as_posix() != entry_path:
                    manifest_errors.append(f"{entry_path}: unsafe manifest path")
                    continue
                p = root / relative
                if not p.resolve().is_relative_to(root):
                    manifest_errors.append(f"{entry_path}: manifest target escapes package")
                    continue
                if not p.is_file() or len(p.read_bytes()) != e["bytes"] or digest(p.read_bytes()) != e["sha256"]:
                    manifest_errors.append(f'{e["path"]}: manifest hash or length mismatch')
            if set(manifest["self_audit_exclusions"]) != EXCLUDED_HASH_FILES:
                manifest_errors.append("Unexpected manifest exclusions")
            add("manifest_hashes", manifest_errors, hashed_files=len(entries))
        except (OSError, ValueError, KeyError) as exc:
            add("manifest_hashes", [str(exc)])
    else:
        add("manifest_hashes", ["docs/MANIFEST.json is missing"])
    archive_checked = False
    if args.archive:
        archive_errors = []
        try:
            with zipfile.ZipFile(args.archive) as z:
                all_members = z.infolist()
                members = [i.filename for i in all_members if not i.is_dir()]
                expected_members = {root.name + "/" + n for n in by_name}
                if len(all_members) != len({i.filename for i in all_members}): archive_errors.append("Duplicate archive members")
                if set(members) != expected_members: archive_errors.append("Archive inventory differs from disk")
                if z.testzip() is not None: archive_errors.append("ZIP CRC failure")
                for info in all_members:
                    n = info.filename
                    pp = Path(n)
                    if pp.is_absolute() or ".." in pp.parts or "\\" in n or re.match(r"^[A-Za-z]:",n) or not pp.parts or pp.parts[0] != root.name:
                        archive_errors.append(f"Unsafe member {n}")
                        continue
                    kind = (info.external_attr >> 16) & 0o170000
                    if kind not in {0,0o100000,0o040000}:
                        archive_errors.append(f"Unsupported archive entry type {n}")
                    if info.is_dir():
                        continue
                    rel = pp.relative_to(root.name).as_posix()
                    if rel in by_name and z.read(n) != by_name[rel].read_bytes():
                        archive_errors.append(f"{rel}: archive bytes differ")
                if not any("/.claude/" in n for n in members) or not any("/.codex/" in n for n in members):
                    archive_errors.append("Hidden starter files missing")
                archive_checked = True
                add("zip_inventory_crc_and_bytes", archive_errors, members=len(members))
        except (OSError, ValueError, zipfile.BadZipFile) as exc:
            add("zip_inventory_crc_and_bytes", [str(exc)])
    report = {
        "schema_version":1, "package":"hoi4-factory", "version":"v02", "date":"2026-10-23",
        "python_version":sys.version.split()[0], "scope":"Planning artifacts only",
        "mode":"working-tree" if args.working_tree else "distribution",
        "status":"pass" if all(c["passed"] for c in checks) else "fail",
        "archive_checked":archive_checked, "checks":checks,
        "not_executed":["Product runtime tests","Live Claude/Codex loading or invocation","MCP connectivity",
            "Paid model/API trials","HOI4 game tests","OS sandbox enforcement","Mermaid renderer",
            "Exhaustive JSON Schema validation"],
    }
    if args.write_report:
        path = args.write_report if args.write_report.is_absolute() else root / args.write_report
        path.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"status":report["status"],"checks":len(checks),
        "failed":[c for c in checks if not c["passed"]],"archive_checked":archive_checked},indent=2))
    return 0 if report["status"] == "pass" else 1

if __name__ == "__main__":
    raise SystemExit(main())

