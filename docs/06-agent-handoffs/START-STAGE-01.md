# Portable first-stage handoff

The primary implementation path is [the complete Claude Code prompt](CLAUDE-CODE-START-STAGE-01.md). This companion gives another capable client the same scope without requiring Claude-specific commands. It also serves as a concise copyable entry point.

## Copyable prompt

Open the extracted `hoi4-factory/` folder as the project root and implement **S01 only** after reading applicable repository instructions and root AGENTS.md. The client files are already in their working locations, and documentation lives under `docs/`. Treat this request as authorization for that bounded stage, not later stages. Preserve existing files and user edits. Use the [S01 packet](../04-implementation-stages/stage-01-bootstrap-and-probes.md) as the exact scope, experiment and acceptance authority.

Read [package status](../PACKAGE_STATUS.md), [master goal](../MASTER_GOAL.md), [implementation master goal](IMPLEMENTATION_MASTER_GOAL.md), [architecture options](../02-architecture/options-and-decisions.md), [contracts](../02-architecture/contracts-and-interfaces.md), [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md), [config migration](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md), [bootstrap guide](../07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md), and relevant acceptance/tests before editing. Follow the stage's additional spec references.

Inspect Git initialization, branch and worktree when present, dirty paths, dependency versions and available tools. Record missing Git metadata honestly. Do not expose secrets or edit global configuration. Write `evidence/S01/scope.md` relative to the project root with the exact approved outputs, excluded later-stage work, required probes and evidence. Verify and adapt the existing starter files in place. Use a reviewed merge only when integrating into a different existing repository. Verify actual instruction and skill discovery in this client. Run `python scripts/validate_package.py --root . --working-tree` and `python scripts/sync_skills.py --check` from the project root. The validator's default mode is for the untouched distribution and its frozen manifest.

Run the stage's bounded bootstrap and EXP-01 through EXP-04. Resolve the Setup recovery uncertainty through artifact evidence, test source preservation, failure/replay/revocation semantics and the native provider-loop packaging boundary. Use disposable fixtures and relevant dependencies. The stage packet defines the precise pass criteria. Do not implement production onboarding, editors, maps, collaboration or asset pipelines beyond these probes.

Keep dormant MCP connections disabled until their specific activation is reviewed and necessary. Record absent tools or credentials as unavailable. Do not substitute broad permissions, fabricated paths, guessed config keys or invented results. Continue useful independent approved work while preserving the failed gate.

Validate requirements against actual artifacts and check results. Review reusable engineering lessons and configuration impact. Produce environment.json, scope.md, checks.json, config-review.md, learning-review.md and handoff.md under `evidence/S01/`. Report exact revisions, changed files, commands, versions, evidence kinds, hashes, limitations, architecture decisions and rollback. Stop at the S01 handoff. Propose S02 only after the stage review.

## Codex-specific adaptation

Current Codex repository skill discovery uses `.agents/skills`. The package's required `.codex/skills` copies share the same workflow bodies but are not asserted to load automatically. Confirm the discovery mirror before using `$start-implementation-stage`, `$validate-stage`, `$capture-engineering-lessons` or `$audit-hoi4-integrations`. The sidecar disables implicit invocation. No new personal installation is required. [Current skill documentation](https://developers.openai.com/codex/skills)

Project `.codex/config.toml` loads under the client's trust and configuration precedence. Verify that on-request, workspace-write and the disabled server flags are effective. Use the documented canonical concurrency key and the installed backend's behavior. Claude JSON settings do not configure Codex, and Codex TOML does not configure Claude. [Basic configuration](https://developers.openai.com/codex/config-basic)

If a local or alternative client lacks native skills, read the canonical workflow bodies and record this as a manual guidance fallback. It does not establish native skill compatibility. Keep the same evidence requirements and authority boundaries. No chosen developer client becomes a mandatory dependency of the finished product.
