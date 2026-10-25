# HOI4 Factory documentation

This directory contains the complete research and implementation-planning baseline for a hosted, model-agnostic Hearts of Iron IV modding workspace. **No product implementation has begun.** The [2026-10-24 maintainer decisions](02-architecture/maintainer-decisions-2026-10-24.md) select a public web application with a Rust server, a native Rust agent loop, a React browser client, Docker deployment and GitHub integration. The earlier Tauri/Pi and Electron/OpenCode candidates remain in the research record.

Project-folder edition: **v02**. Use the parent `hoi4-factory/` directory as your working root. [The root README](../README.md) explains how to open the project and run its checks. The root client guidance is already in place.

## Start here

| Reader | Reading order |
|---|---|
| Product/architecture reviewer | [Maintainer decisions](02-architecture/maintainer-decisions-2026-10-24.md), [master goal](MASTER_GOAL.md), [executive recommendation](00-research/00-executive-recommendation.md), [architecture options](02-architecture/options-and-decisions.md), [roadmap](04-implementation-stages/ROADMAP.md), [risks](05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) |
| Research reviewer | [Source registry](00-research/01-research-sources.md), [asset audit](00-research/02-existing-assets-audit.md), [candidate comparison](00-research/03-candidate-comparison.md), [research log](00-research/04-evidence-and-research-log.md) |
| Future implementation agent | Current target-repository instructions, [AGENTS](../AGENTS.md), [CLAUDE](../CLAUDE.md), [implementation master goal](06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), then the one authorized stage |
| Verification reviewer | [Traceability](01-product/requirements-traceability.md), [test strategy](05-verification/TEST-STRATEGY.md), [evaluation](05-verification/EVALUATION-MATRIX.md), [acceptance matrix](05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md), [actual package QA](05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) |

Implementation is deferred by the maintainer. The next implementation action, when it comes, is explicit authorization of **S01 only**. The copyable [Claude Code S01 prompt](06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md) is ready for that future step. Do not infer permission to initialize or implement the product from opening this archive.

## Package map

| Area | Contents |
|---|---|
| docs root | Vision, current status, documentation index and inventory |
| Project root | Working-folder README, shared developer instructions and native client configuration |
| scripts | Planning validator and skill synchronization utilities |
| docs/00-research | Primary-source audits, exact pins, license/maintenance observations, source coverage and evidence ledgers |
| docs/01-product | 50 requirements, users, required scenarios, optional visuals/autonomy and traceability |
| docs/02-architecture | Maintainer decisions, the selected direction and two research candidates, ADRs, state model, ten contracts, deployment and security |
| docs/03-subsystem-specs | 13 detailed subsystem specifications with functional, failure, recovery and acceptance behavior |
| docs/04-implementation-stages | Eight stages, 48 work packets, 47 exit gates and prioritized deferred scope |
| docs/05-verification | Fixtures, test catalog, three-profile evaluation, requirement/gate mappings, risks and executable package QA |
| docs/06-agent-handoffs | One-stage implementation goal, Claude-first start prompt, portable start prompt, workflow guide and handoff template |
| docs/07-developer-workflow | Exhaustive 196-field config migration, learning policy, skill source and deterministic synchronization |
| Hidden starter directories | Four real skills each under .claude, .codex and currently supported .agents discovery paths, plus safe native configs |

[MANIFEST](MANIFEST.md) maps the requested content contract to actual filenames. The numbered spec prefixes differ from the suggested names only for readability. Every required content area is present. Hidden starter directories are included in the ZIP even if a file manager initially hides them.

## Evidence labels

**Verified** means inspected implementation or a documented reproducible check within its stated scope. **Documented only** means a source describes behavior without this assignment executing or independently proving it. **Proposed** means a design or future check. **Unknown** means missing access, execution or a decision. A verified code path does not imply a verified product.

[PACKAGE_STATUS](PACKAGE_STATUS.md) distinguishes completed planning checks from unexecuted runtime, client, model and game tests. No claimed passing product tests appear in this archive.

## Starter guidance and safe adaptation

Shared rules live in AGENTS. CLAUDE imports them and adds concise Claude-specific behavior. Canonical workflow bodies generate client copies with a drift check. The requested .codex/skills files are retained for portability, while .agents/skills supplies the currently documented Codex discovery path.

Codex and Claude settings are separate native formats. Both inherited MCP relationships are present but disabled. No secret or personal absolute path is shipped. The 196-field mapping preserves provenance and explicitly adapts unsafe inherited defaults. Use these files directly when working in the extracted project folder. If integrating into another existing repository, merge its guidance under S01 without overwriting the team's instructions.

## Reproduce package checks

From the `hoi4-factory/` project root, with Python 3.11 or newer, check the freshly extracted distribution:

```bash
scripts/py.sh scripts/sync_skills.py --check
scripts/py.sh scripts/validate_package.py --root .
```

After implementation work adds files or changes the distributed baseline, run `scripts/py.sh scripts/validate_package.py --root . --working-tree` instead. This mode checks maintained planning and guidance without enforcing the original distribution hashes.

The validator checks the authored package. It does not connect MCP, invoke a model, install dependencies or run HOI4. Its recorded result is [verification-run.json](05-verification/verification-run.json). See the quality-review document for exact coverage and limitations.

## Glossary

| Term | Meaning here |
|---|---|
| Canon | Human-approved design facts and constraints with an owner |
| Proposal | Immutable intended source change tied to a base snapshot |
| Grant | Bounded authority over operations, paths, destinations, budget and time |
| Receipt | Persisted intent/outcome evidence for a specific effect |
| Fence | Increasing ownership identity that prevents stale workers publishing |
| Projection | Rebuildable visual or search view of authoritative source |
| Unknown effect | An operation may have happened, requiring reconciliation before retry |
| Source closure | Exact tool, component, game/profile and input versions used |
| Learning review | Evidence-based decision to improve guidance or record justified no change |
| Vertical slice | One useful end-to-end workflow crossing setup, model, tools, UI, validation and history |

