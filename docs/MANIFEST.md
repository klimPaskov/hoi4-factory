# Project file manifest

Project folder: **hoi4-factory/**. Distribution version: **v02**. Date: 2026-10-23.

This project-folder edition retains the completed planning baseline. The root is ready to open as the working folder, with all planning documentation under `docs/`. Product implementation has not started.

## Inventory and verification

[MANIFEST.json](MANIFEST.json) records project-root-relative file paths, byte lengths and SHA-256 hashes. Its two self-audit exclusions are `docs/MANIFEST.json` and `docs/05-verification/verification-run.json`. Both files remain present in the ZIP and this readable inventory. These exclusions prevent recursive hashing.

The manifest describes the frozen distributed snapshot. During development, use the validator's explicit `--working-tree` mode for current planning and guidance. The strict default mode and archive comparison continue to verify the unchanged distribution. See the [quality review](05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) and [project layout review](05-verification/PROJECT-LAYOUT-REVIEW.md).

Distributed file count: **111**. Every v01 planning file is retained at its mapped location. The root README and project layout review are additions. No symlinks, application scaffold, source repositories, Git history, game assets or dependencies are bundled.

| Area | Files | Purpose |
|---|---:|---|
| Root | 4 | Working-folder entry point, shared instructions and MCP definitions |
| .agents | 8 | Current Codex discovery copies of the four owned workflows |
| .claude | 5 | Claude project settings and four reusable skills |
| .codex | 9 | Codex project settings and portable workflow copies |
| docs root | 5 | Documentation index, master goal, status and integrity manifests |
| docs/00-research | 15 | Primary-source evidence, coverage, audits and synthesis |
| docs/01-product | 5 | Requirements, contributor scenarios, visuals and traceability |
| docs/02-architecture | 6 | Alternatives, provisional and maintainer decisions, contracts and security |
| docs/03-subsystem-specs | 13 | Thirteen detailed subsystem specifications |
| docs/04-implementation-stages | 10 | Eight stages, roadmap and deferred scope |
| docs/05-verification | 11 | Test and evaluation plans, risks, checks and layout review |
| docs/06-agent-handoffs | 5 | Claude-first and portable one-stage handoffs |
| docs/07-developer-workflow | 14 | Configuration migration, canonical workflows and learning policy |
| scripts | 2 | Planning validation and skill synchronization utilities |

## Content contract

| Required content | Project location |
|---|---|
| Working-folder entry point | Root README.md |
| Vision, status and documentation index | docs/MASTER_GOAL.md, docs/PACKAGE_STATUS.md and docs/README.md |
| Research and source audits | docs/00-research/ |
| Requirements and user workflows | docs/01-product/ |
| Architecture alternatives and contracts | docs/02-architecture/ |
| SPEC-001 through SPEC-013 | docs/03-subsystem-specs/ |
| Roadmap, stage packets and backlog | docs/04-implementation-stages/ |
| Tests, evaluation, acceptance, risks and artifact QA | docs/05-verification/ |
| Implementation master goal and one-stage prompts | docs/06-agent-handoffs/ |
| Configuration migration and engineering learning | docs/07-developer-workflow/ |
| Shared and Claude-specific root instructions | AGENTS.md and CLAUDE.md |
| Four skills per client | .claude/skills/, .codex/skills/ and .agents/skills/ |
| Native client settings | .claude/settings.json, .codex/config.toml and .mcp.json |
| Maintenance utilities | scripts/validate_package.py and scripts/sync_skills.py |

## Evidence and distribution boundaries

Research labels distinguish inspected source, documented behavior, proposed design and unknown behavior. The retained v01 starter and negative-test receipts keep their original checker identities. The v02 verification receipt records checks of the reorganized distribution. The final ZIP is checked separately after all content has been finalized.

The archive contains authored planning and guidance, evidence metadata and primary-source links. It does not redistribute inspected implementation or private mod content. Upstream license observations do not grant additional rights to game assets, external code, art or model services.

## Complete file list

### Root

- [.mcp.json](../.mcp.json)
- [AGENTS.md](../AGENTS.md)
- [CLAUDE.md](../CLAUDE.md)
- [README.md](../README.md)

### .agents

- [.agents/skills/audit-hoi4-integrations/SKILL.md](../.agents/skills/audit-hoi4-integrations/SKILL.md)
- [.agents/skills/audit-hoi4-integrations/agents/openai.yaml](../.agents/skills/audit-hoi4-integrations/agents/openai.yaml)
- [.agents/skills/capture-engineering-lessons/SKILL.md](../.agents/skills/capture-engineering-lessons/SKILL.md)
- [.agents/skills/capture-engineering-lessons/agents/openai.yaml](../.agents/skills/capture-engineering-lessons/agents/openai.yaml)
- [.agents/skills/start-implementation-stage/SKILL.md](../.agents/skills/start-implementation-stage/SKILL.md)
- [.agents/skills/start-implementation-stage/agents/openai.yaml](../.agents/skills/start-implementation-stage/agents/openai.yaml)
- [.agents/skills/validate-stage/SKILL.md](../.agents/skills/validate-stage/SKILL.md)
- [.agents/skills/validate-stage/agents/openai.yaml](../.agents/skills/validate-stage/agents/openai.yaml)

### .claude

- [.claude/settings.json](../.claude/settings.json)
- [.claude/skills/audit-hoi4-integrations/SKILL.md](../.claude/skills/audit-hoi4-integrations/SKILL.md)
- [.claude/skills/capture-engineering-lessons/SKILL.md](../.claude/skills/capture-engineering-lessons/SKILL.md)
- [.claude/skills/start-implementation-stage/SKILL.md](../.claude/skills/start-implementation-stage/SKILL.md)
- [.claude/skills/validate-stage/SKILL.md](../.claude/skills/validate-stage/SKILL.md)

### .codex

- [.codex/config.toml](../.codex/config.toml)
- [.codex/skills/audit-hoi4-integrations/SKILL.md](../.codex/skills/audit-hoi4-integrations/SKILL.md)
- [.codex/skills/audit-hoi4-integrations/agents/openai.yaml](../.codex/skills/audit-hoi4-integrations/agents/openai.yaml)
- [.codex/skills/capture-engineering-lessons/SKILL.md](../.codex/skills/capture-engineering-lessons/SKILL.md)
- [.codex/skills/capture-engineering-lessons/agents/openai.yaml](../.codex/skills/capture-engineering-lessons/agents/openai.yaml)
- [.codex/skills/start-implementation-stage/SKILL.md](../.codex/skills/start-implementation-stage/SKILL.md)
- [.codex/skills/start-implementation-stage/agents/openai.yaml](../.codex/skills/start-implementation-stage/agents/openai.yaml)
- [.codex/skills/validate-stage/SKILL.md](../.codex/skills/validate-stage/SKILL.md)
- [.codex/skills/validate-stage/agents/openai.yaml](../.codex/skills/validate-stage/agents/openai.yaml)

### docs root

- [docs/MANIFEST.json](MANIFEST.json)
- [docs/MANIFEST.md](MANIFEST.md)
- [docs/MASTER_GOAL.md](MASTER_GOAL.md)
- [docs/PACKAGE_STATUS.md](PACKAGE_STATUS.md)
- [docs/README.md](README.md)

### docs/00-research

- [docs/00-research/00-executive-recommendation.md](00-research/00-executive-recommendation.md)
- [docs/00-research/01-research-sources.md](00-research/01-research-sources.md)
- [docs/00-research/02-existing-assets-audit.md](00-research/02-existing-assets-audit.md)
- [docs/00-research/03-candidate-comparison.md](00-research/03-candidate-comparison.md)
- [docs/00-research/04-evidence-and-research-log.md](00-research/04-evidence-and-research-log.md)
- [docs/00-research/05-developer-guidance-research.md](00-research/05-developer-guidance-research.md)
- [docs/00-research/06-context-protocols-and-evaluation-research.md](00-research/06-context-protocols-and-evaluation-research.md)
- [docs/00-research/agentic-projection-check.json](00-research/agentic-projection-check.json)
- [docs/00-research/context-guidance-audit.json](00-research/context-guidance-audit.json)
- [docs/00-research/evidence-a.json](00-research/evidence-a.json)
- [docs/00-research/evidence-b.json](00-research/evidence-b.json)
- [docs/00-research/evidence-c.json](00-research/evidence-c.json)
- [docs/00-research/evidence-root.json](00-research/evidence-root.json)
- [docs/00-research/source-registry.json](00-research/source-registry.json)
- [docs/00-research/supplied-source-coverage.json](00-research/supplied-source-coverage.json)

### docs/01-product

- [docs/01-product/product-requirements.md](01-product/product-requirements.md)
- [docs/01-product/requirements-traceability.md](01-product/requirements-traceability.md)
- [docs/01-product/requirements.json](01-product/requirements.json)
- [docs/01-product/users-and-workflows.md](01-product/users-and-workflows.md)
- [docs/01-product/visual-and-autonomy-model.md](01-product/visual-and-autonomy-model.md)

### docs/02-architecture

- [docs/02-architecture/contracts-and-interfaces.md](02-architecture/contracts-and-interfaces.md)
- [docs/02-architecture/data-context-and-state.md](02-architecture/data-context-and-state.md)
- [docs/02-architecture/deployment-and-security.md](02-architecture/deployment-and-security.md)
- [docs/02-architecture/maintainer-decisions-2026-10-24.md](02-architecture/maintainer-decisions-2026-10-24.md)
- [docs/02-architecture/options-and-decisions.md](02-architecture/options-and-decisions.md)
- [docs/02-architecture/recommended-architecture.md](02-architecture/recommended-architecture.md)

### docs/03-subsystem-specs

- [docs/03-subsystem-specs/001-onboarding-and-mod-setup.md](03-subsystem-specs/001-onboarding-and-mod-setup.md)
- [docs/03-subsystem-specs/002-native-runtime-and-models.md](03-subsystem-specs/002-native-runtime-and-models.md)
- [docs/03-subsystem-specs/003-orchestration-and-recovery.md](03-subsystem-specs/003-orchestration-and-recovery.md)
- [docs/03-subsystem-specs/004-context-memory-and-runtime-learning.md](03-subsystem-specs/004-context-memory-and-runtime-learning.md)
- [docs/03-subsystem-specs/005-git-collaboration-and-review.md](03-subsystem-specs/005-git-collaboration-and-review.md)
- [docs/03-subsystem-specs/006-hoi4-tools-and-validation.md](03-subsystem-specs/006-hoi4-tools-and-validation.md)
- [docs/03-subsystem-specs/007-visual-state-synchronization.md](03-subsystem-specs/007-visual-state-synchronization.md)
- [docs/03-subsystem-specs/008-maps.md](03-subsystem-specs/008-maps.md)
- [docs/03-subsystem-specs/009-focus-and-other-editors.md](03-subsystem-specs/009-focus-and-other-editors.md)
- [docs/03-subsystem-specs/010-assets-localization-and-content.md](03-subsystem-specs/010-assets-localization-and-content.md)
- [docs/03-subsystem-specs/011-extensibility-sandbox-and-permissions.md](03-subsystem-specs/011-extensibility-sandbox-and-permissions.md)
- [docs/03-subsystem-specs/012-engineering-guidance-skills-and-learning.md](03-subsystem-specs/012-engineering-guidance-skills-and-learning.md)
- [docs/03-subsystem-specs/013-config-reuse-and-developer-environment.md](03-subsystem-specs/013-config-reuse-and-developer-environment.md)

### docs/04-implementation-stages

- [docs/04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md](04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md)
- [docs/04-implementation-stages/ROADMAP.md](04-implementation-stages/ROADMAP.md)
- [docs/04-implementation-stages/stage-01-bootstrap-and-probes.md](04-implementation-stages/stage-01-bootstrap-and-probes.md)
- [docs/04-implementation-stages/stage-02-onboarding-and-workspaces.md](04-implementation-stages/stage-02-onboarding-and-workspaces.md)
- [docs/04-implementation-stages/stage-03-native-agent-and-durability.md](04-implementation-stages/stage-03-native-agent-and-durability.md)
- [docs/04-implementation-stages/stage-04-focus-vertical-slice.md](04-implementation-stages/stage-04-focus-vertical-slice.md)
- [docs/04-implementation-stages/stage-05-context-and-recovery-hardening.md](04-implementation-stages/stage-05-context-and-recovery-hardening.md)
- [docs/04-implementation-stages/stage-06-collaboration-and-content.md](04-implementation-stages/stage-06-collaboration-and-content.md)
- [docs/04-implementation-stages/stage-07-maps-and-structured-import.md](04-implementation-stages/stage-07-maps-and-structured-import.md)
- [docs/04-implementation-stages/stage-08-packaging-and-pilot.md](04-implementation-stages/stage-08-packaging-and-pilot.md)

### docs/05-verification

- [docs/05-verification/EVALUATION-MATRIX.md](05-verification/EVALUATION-MATRIX.md)
- [docs/05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md](05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md)
- [docs/05-verification/PROJECT-LAYOUT-REVIEW.md](05-verification/PROJECT-LAYOUT-REVIEW.md)
- [docs/05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md](05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md)
- [docs/05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md](05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md)
- [docs/05-verification/TEST-STRATEGY.md](05-verification/TEST-STRATEGY.md)
- [docs/05-verification/acceptance-matrix.json](05-verification/acceptance-matrix.json)
- [docs/05-verification/starter-config-and-skills-checks.json](05-verification/starter-config-and-skills-checks.json)
- [docs/05-verification/test-catalog.json](05-verification/test-catalog.json)
- [docs/05-verification/validator-negative-fixtures.json](05-verification/validator-negative-fixtures.json)
- [docs/05-verification/verification-run.json](05-verification/verification-run.json)

### docs/06-agent-handoffs

- [docs/06-agent-handoffs/AGENT-WORKFLOW-AND-REVIEW-RULES.md](06-agent-handoffs/AGENT-WORKFLOW-AND-REVIEW-RULES.md)
- [docs/06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md](06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md)
- [docs/06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md](06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md)
- [docs/06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md](06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md)
- [docs/06-agent-handoffs/START-STAGE-01.md](06-agent-handoffs/START-STAGE-01.md)

### docs/07-developer-workflow

- [docs/07-developer-workflow/CONFIG-FIELD-MAP.csv](07-developer-workflow/CONFIG-FIELD-MAP.csv)
- [docs/07-developer-workflow/CONFIG-MIGRATION-REPORT.md](07-developer-workflow/CONFIG-MIGRATION-REPORT.md)
- [docs/07-developer-workflow/CONFIG-SOURCE-INVENTORY.json](07-developer-workflow/CONFIG-SOURCE-INVENTORY.json)
- [docs/07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md](07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md)
- [docs/07-developer-workflow/KNOWLEDGE-CHANGELOG.md](07-developer-workflow/KNOWLEDGE-CHANGELOG.md)
- [docs/07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md](07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md)
- [docs/07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md](07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md)
- [docs/07-developer-workflow/skill-hashes.json](07-developer-workflow/skill-hashes.json)
- [docs/07-developer-workflow/skill-trigger-fixtures.json](07-developer-workflow/skill-trigger-fixtures.json)
- [docs/07-developer-workflow/skill-workflows/audit-hoi4-integrations/BODY.md](07-developer-workflow/skill-workflows/audit-hoi4-integrations/BODY.md)
- [docs/07-developer-workflow/skill-workflows/capture-engineering-lessons/BODY.md](07-developer-workflow/skill-workflows/capture-engineering-lessons/BODY.md)
- [docs/07-developer-workflow/skill-workflows/start-implementation-stage/BODY.md](07-developer-workflow/skill-workflows/start-implementation-stage/BODY.md)
- [docs/07-developer-workflow/skill-workflows/validate-stage/BODY.md](07-developer-workflow/skill-workflows/validate-stage/BODY.md)
- [docs/07-developer-workflow/skills.json](07-developer-workflow/skills.json)

### scripts

- [scripts/sync_skills.py](../scripts/sync_skills.py)
- [scripts/validate_package.py](../scripts/validate_package.py)

