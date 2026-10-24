# Agent workflow and review rules

Status: Proposed engineering process. Claude Code is primary. Other clients use the same shared rules and evidence boundary. Read [AGENTS.md](../../AGENTS.md), [instruction map](../07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md), [knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md) and the one approved stage packet.

## Task packets and execution

A task packet identifies the stage, requirements, expected outputs, owned paths, dependencies, relevant source/spec links, actual tool availability, acceptance criteria and stopping conditions. It cites the existing user authorization. The packet must be small enough that its completion can be reviewed from artifacts and evidence, not just the agent's narrative.

Use start-implementation-stage before substantive stage work. Open `hoi4-factory/` as the project root, inspect its guidance and prior evidence, and preserve repository conventions and user edits. Its client settings and skills are already in place. If integrating into a different repository, do not overwrite AGENTS, CLAUDE or the team's skills. If instructions conflict, perform unaffected useful work and report the precise conflict to the appropriate owner.

An existing stage authorization covers routine reversible work within its bounds. Do not ask for it again at every edit or test. New stages, destructive actions, broadened external effects or costs require their own applicable authority. Never rewrite a skill or config to manufacture permission for a blocked action.

## Subagents and parallel ownership

Delegate independent work when it reduces uncertainty or time without creating competing ownership. Assign each worker explicit inputs, allowed paths, expected outputs, evidence and a clear merge boundary. Send relevant context, not the entire history by default. A reviewer can inspect the parent work read-only while implementation proceeds elsewhere.

Use separate worktrees or disjoint owned paths for simultaneous writes. Record actual base revisions. Worktrees share repository history and do not create security isolation. Review child diffs and evidence before integrating. Do not treat a role title, planned delegation or agent configuration as proof that a native custom agent ran.

Claude subagents have their own context and may need explicit skill preloading. Tool restrictions must account for shell and MCP effects. The starter does not preinstall the upstream's 23 specialized mod roles or assume their tools exist. Add a role only after scope, dependency and permission review. [Claude subagents](https://code.claude.com/docs/en/sub-agents)

## Git, branches and review

Inspect status before changes. Keep branch naming and commit conventions consistent with the existing repository. A planning starter does not authorize pushing, publishing, merging or opening a person-directed message. Prepare reviewable diffs and use the user's existing publication authority when present. Avoid force pushes, broad resets, cleaning unrelated files or deleting another task's branch.

Review changes for semantic intent, preservation of user files, requirements coverage, error/recovery behavior, secrets, license impact and unnecessary scope. Generated outputs must trace to canonical sources. For binary or generated assets, retain source lineage and hashes. Do not trust a count of files as evidence of complete registration or consumption.

## Validation and evidence

Use validate-stage with the exact acceptance rows. Test required normal, failure and recovery paths and the material risks introduced by the change. Record exact commands, versions, input revisions, artifacts and limitations. Stop optional repetition when the stage's gates are adequately resolved.

Run `python scripts/validate_package.py --root . --working-tree` and `python scripts/sync_skills.py --check` from the project root for evolving documentation and guidance. Default validator mode checks the untouched distributed baseline against its manifest. Neither mode validates application behavior or grants a stage pass.

Source parse, semantic checks, visual previews and real game execution remain separate evidence layers. The application may provide useful approximate previews, but no label should imply exact in-game fidelity without the corresponding observation. A missing game, client or credential creates an unavailable check, not a fabricated pass.

For uncertain effects, inspect receipts and actual state before retry. Persist a factual checkpoint when interrupted. Resume the correct branch/worktree and re-read scope and handoff before continuing. Client sessions are convenient context. The repository evidence and artifact state determine what completed. [Claude sessions](https://code.claude.com/docs/en/sessions)

## Learning and configuration at every handoff

Use capture-engineering-lessons to consider reusable improvements. Record candidates with reproduction, evidence, applicability, counterexample and owner. Accepted changes follow the knowledge policy and remain reviewable in Git. Update canonical workflow bodies, all client projections and relevant fixtures together. Keep a specific “no reusable change” result when appropriate.

Complete a configuration checkpoint whenever a task touches providers, MCP, permissions, context, discovery or tool installation. Record no-change reviews too. Keep secrets and machine-specific values local. Do not modify user or system configuration automatically. A proposed lesson cannot silently expand paid operations or alter game design canon.

## Completion and next owner

Deliver scope.md, environment.json, checks.json, config-review.md, learning-review.md and handoff.md under the stage evidence directory. Use [the handoff template](STAGE-HANDOFF-TEMPLATE.md). Lead the report with the actual outcome, then its evidence and limits. State blockers and the exact next decision.

When the stage finishes, stop implementation. The roadmap identifies dependencies but does not grant permission for the next stage. A fresh agent should be able to resume by reading the handoff, checking current state and seeing a concrete proposed next scope without access to the original chat.
