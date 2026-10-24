# Product requirements

Status: **Proposed product baseline for architecture review**. Requirements baseline: v01, retained in the v02 project layout and revised by the [2026-10-24 maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md), which reworded ten requirements and added REQ-046 through REQ-048. The planning assignment is complete only when its archive and quality review are delivered. Implementation requires approval of one stage. The original brief and supplied-source coverage are recorded in the [research log](../00-research/04-evidence-and-research-log.md).

## Product outcome

Build a public hosted HOI4 modding workspace, used in a browser, where a contributor can describe a change, inspect or edit a preview, let a native agent produce a bounded proposal, review evidence, and retain an ordinary editable mod repository that can be downloaded or pushed to GitHub at any time. Claude Code is the primary future development tool. It is not an end-user runtime dependency.

The existing Mod Setup application is the onboarding foundation. Its source-derived plan, validation and transaction semantics must be reused through an adapter. The first valuable outcome is a small focus-tree change on a newly created or imported project, available through visual, hybrid and autonomous interaction, with scoped authorization, source review, a branch commit, a GitHub pull request and restore.

## Priorities

P0 covers source integrity, onboarding, accounts and tenant isolation, GitHub connection, one native agent, exact evidence, safe review and recoverability. P1 covers persistent context, invited collaborators, broader content workflows and bounded map work. P2 covers full-world map generation, many concurrent workers, real-time co-editing of one file, credit purchases and broad visual coverage. A P2 specification can be substantive without becoming a prerequisite for the first useful release.

## Stable requirement register

Each requirement has an acceptance ID. The [acceptance matrix](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md) expands the observable result and owns coverage status. “Planned” means the test has not yet run.

| ID | Requirement | Primary specification | Delivery owner |
|---|---|---|---|
| REQ-001 | **Independent native product.** End users use the hosted factory in a browser without Claude Code, Codex or another coding CLI, and the service invokes none. | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md) | S03 |
| REQ-002 | **Provider capability negotiation.** API profiles and any server-reachable OpenAI-compatible endpoint declare and probe streaming, tools, structured output, images, context and cancellation. | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md) | S03 |
| REQ-003 | **Authentication, cost and fallback.** User API keys and GitHub tokens stay in the server secret store and out of artifacts, subscription access is not assumed, per-user budgets and the operator credit allowance stop new work, and fallback respects data destinations. | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md) | S03 |
| REQ-004 | **Create through Mod Setup.** Reuse actual new-mod planning, validation, generated files and apply flow. | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md) | S02 |
| REQ-005 | **Import through Mod Setup.** Reuse inspection and preparation of an existing mod, including valid and partial setups. | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md) | S02 |
| REQ-006 | **Preserve existing content.** Keep source, binary assets, Git history and existing instruction files of uploaded and GitHub-imported mods unless an exact reviewed change authorizes replacement. | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md) | S02 |
| REQ-007 | **Validated workspace handoff.** Pass resolved server workspace roots, version closure, game reference build, capabilities, setup receipt and unresolved checks into the editor and native agent. | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md) | S02 |
| REQ-008 | **Intent and approved canon.** Capture lore and constraints with ownership, approval and links to resulting tasks and changes. | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) | S05 |
| REQ-009 | **Optional visual workflow.** Visual, hybrid and autonomous paths operate on the same source and proposal contracts. | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) | S04 |
| REQ-010 | **Progressive interface.** Expose intent, preview, status and meaningful review first, with source and advanced controls available on demand. | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) | S04 |
| REQ-011 | **Source and preview integrity.** Index exact source snapshots and preserve unsupported syntax, comments, ordering and encodings outside approved spans. | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) | S04 |
| REQ-012 | **Useful focus preview.** Render nodes, prerequisites, exclusion, identifiers and diagnostics with explicit fidelity and source revision. | [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md) | S04 |
| REQ-013 | **Bounded focus editing.** Support source-safe node moves and supported property edits, with explicit refusal for ambiguous or unsupported constructs. | [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md) | S04 |
| REQ-014 | **Bounded autonomous work.** Run an approved task without canvas supervision, within tool, path, time, cost and change limits. | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md) | S04 |
| REQ-015 | **Human intervention and approval.** Pause, revoke or narrow a grant, inspect pending changes and resume safely at a valid boundary. | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | S03 |
| REQ-016 | **Durable recovery.** Restore tasks, runs, attempts, approvals, evidence and interrupted outcomes after process death. | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md) | S03 |
| REQ-017 | **Task dependencies and retries.** Track dependency readiness, ownership, bounded attempts, blocked states and side-effect reconciliation before adding workers. | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md) | S06 |
| REQ-018 | **Context and model handoff.** Rebuild a bounded context bundle with provenance and unresolved work when a session or model changes. | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) | S05 |
| REQ-019 | **Memory freshness.** Separate approved canon, accepted project rules, inferred memories and caches, invalidating affected evidence after source or version changes. | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) | S05 |
| REQ-020 | **Reviewable runtime learning.** Capture candidate lessons with evidence and applicability, allowing approval, rejection and supersession without self-granted authority. | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) | S05 |
| REQ-021 | **Git isolation.** Give write tasks owned worktrees or branches in the server workspace and preserve collaborators' uncommitted work. | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) | S02 |
| REQ-022 | **Concurrent edit reconciliation.** Detect textual, semantic identifier and binary asset collisions between humans and agents before merge. | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) | S06 |
| REQ-023 | **Review and attribution.** Offer semantic summary, raw diff, provenance, validation and preview tied to the exact proposed commit. | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) | S04 |
| REQ-024 | **Restore and rollback.** Restore a prior accepted state through an owned reversible operation without discarding unrelated edits. | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) | S04 |
| REQ-025 | **Layered HOI4 validation.** Distinguish syntax, semantics, source render, simulation and actual game observations, with unresolved checks visible. | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md) | S04 |
| REQ-026 | **HOI4 integration adapters.** Use versioned tool schemas, health checks, timeouts, cancellation and limited retries for available HOI4 tools. | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md) | S03 |
| REQ-027 | **Game and DLC compatibility.** Record the server game reference build, DLC set and ordered content roots, warn when a mod targets another game version, and keep unknown compatibility and stale caches visible. | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md) | S02 |
| REQ-028 | **Assets and localization.** Track original assets, rights, transformations, localization keys, consumer wiring and hash-specific review. | [SPEC-010](../03-subsystem-specs/010-assets-localization-and-content.md) | S06 |
| REQ-029 | **Existing map inspection.** Inspect layered map data, source relationships and validation without destructive conversion. | [SPEC-008](../03-subsystem-specs/008-maps.md) | S07 |
| REQ-030 | **Structured map import.** Plan and validate an exact structured map import with dependency closure, ID preservation or explicit remapping. | [SPEC-008](../03-subsystem-specs/008-maps.md) | S07 |
| REQ-031 | **Reference image handling.** Keep a map image as attributed design reference until a human reviews a separately validated proposed topology. | [SPEC-008](../03-subsystem-specs/008-maps.md) | S07 |
| REQ-032 | **Map creation and editing roadmap.** Specify bounded painting and later creation or generation with topology, performance, source fidelity and game validation gates. | [SPEC-008](../03-subsystem-specs/008-maps.md) | S07 |
| REQ-033 | **Security and extensions.** Enforce file, process and network authority outside model prose, validate final tool arguments, and treat plugins as untrusted code. | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | S03 |
| REQ-034 | **Licensing and distribution.** Separate the AGPL-3.0 code license, game rights, provider terms, private data and redistribution decisions, with reproducible pinned Docker images. | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | S08 |
| REQ-035 | **Contributor collaboration.** Support attributable lore, art, maps and scripting contributions through invited project collaborators and GitHub pull requests, with Git review first. | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) | S06 |
| REQ-036 | **Continuous engineering learning.** Require evidence-based learning reviews after meaningful development, with the smallest useful versioned change or a justified no-change result. | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md) | S01 |
| REQ-037 | **Claude-first portable guidance.** Ship AGENTS, importing CLAUDE, real Claude and Codex starter skills, verified discovery paths and no permission escalation by instructions. | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md) | S01 |
| REQ-038 | **Source-audited configuration migration.** Map every nonsecret field of the actual Agentic config to verified safe client settings or explicit intentional omission. | [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md) | S01 |
| REQ-039 | **One-stage handoffs.** Provide self-contained entry, work packets, tests, gates and continuation prompts that stop before the next stage. | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md) | S01 |
| REQ-040 | **Research evidence.** Record requested sources, commits, source paths, licenses, inspection scope, factual status and unknowns. | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md) | S01 |
| REQ-041 | **Evaluation matrix.** Plan all required scenarios across two API providers and one self-hosted OpenAI-compatible endpoint, measuring accepted output, rework, failures, time and cost. | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md) | S08 |
| REQ-042 | **Interoperable extension surfaces.** Use MCP and skills appropriately, consider ACP as optional editor interoperability, and keep the native runtime replaceable. | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | S05 |
| REQ-043 | **Versioned maintenance.** Pin dependency closures and schemas, test upgrades, track source compatibility and retire stale guidance. | [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md) | S08 |
| REQ-044 | **Editable export and escape hatch.** Allow the mod to be downloaded, cloned or pushed to GitHub and continued outside the factory, with valid native files and no proprietary source lock-in. | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) | S04 |
| REQ-045 | **Complete planning archive.** Deliver substantive specs, stages, evidence, starter files, traceability and actual package validation in the versioned ZIP. | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md) | S01 |
| REQ-046 | **Accounts and tenant isolation.** Public sign-up through GitHub or email, per-account and per-project isolation of workspaces, agent runs, secrets and budgets, project roles for invited collaborators, and abuse and rate limits. | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | S02 |
| REQ-047 | **GitHub integration.** Connect selected repositories, import a mod from GitHub, upload a mod without a repository, push owned branches, open pull requests and create repositories, each separately authorized and reconciled. | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) | S02 |
| REQ-048 | **Server game reference.** A read-only full game copy, latest build with all DLC, used for validation and previews, never exported or baked into images, with the rights review completed before public exposure. | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md) | S02 |

## First useful vertical slice

A maintainer signs in, then creates a small mod, uploads one or imports a fixture repository from GitHub through the reused setup flow. The app opens the resulting workspace and records its source and tool profile. The user asks for a three-to-five-node focus branch with a small supported effect and localization, or makes an equivalent supported visual edit. A native agent can complete the bounded task while the canvas is closed.

The result includes native files, semantic and raw diffs, a useful graph preview and named static checks. The user sees any missing game observation. The host validates the exact proposal under its grant, commits only owned changes to a branch, can push it and open a pull request under separate authorization, and can restore the previous accepted snapshot without deleting unrelated work. Interrupting the server at a recorded mutation boundary must not leave an unclassified outcome.

This slice proves the core relationship between onboarding, models, tools, source, visuals and evidence. It does not need a large agent organization or a full map generator.

## Nonfunctional requirements

Source writes use explicit ownership and preconditions. Critical operations are recoverable and idempotent where possible, with uncertainty represented where exactly-once effects cannot be guaranteed. The UI must remain useful with optional integrations absent. Provider limits and data-sharing restrictions remain visible. The app must not silently treat a custom endpoint as equivalent to every hosted provider. One account's workspaces, runs, secrets and budgets are never visible to another account.

Performance targets are proposed budgets attached to reference fixtures and hardware. Security boundaries are implemented in the server, its containers and the operating system, never only in instructions. Distribution must account for the AGPL-3.0 license, upstream licenses and game rights, and the rights review completes before public exposure. Support claims name the tested deployment, game build, DLC profile, tool closure and model configuration.

## Change control

The requirement register is versioned. An architecture review may narrow a release while preserving deferred requirements and their owners. A test failure does not authorize weakening the requirement. A contradictory source or unsupported runtime feature creates a decision record with evidence and options. Accepted changes update the register, traceability, stage packet, tests and handoffs together.

