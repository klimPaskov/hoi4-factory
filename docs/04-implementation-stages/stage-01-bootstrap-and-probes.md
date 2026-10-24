# S01: Claude Code bootstrap and architectural probes

Status: **Executed 2026-10-24, review requested**. The result and decision receipt are in [the S01 handoff](../../evidence/S01/handoff.md). Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md) ADR-011 through ADR-017. The maintainer authorized this stage on 2026-10-24 with the amendments recorded in [the S01 scope](../../evidence/S01/scope.md).

## Objective and non-goals

Use this repository as the implementation workspace after this stage is explicitly authorized. Root guidance, client settings and skills are already in place, with planning documents under `docs/`. Read AGENTS, relevant skills and applicable local instructions before adapting these files. Establish verified Claude-first loading, environment evidence, a minimal Rust Cargo workspace with a Docker Compose skeleton, and isolated feasibility probes for the hosted architecture.

The runtime direction is decided by ADR-011. The model loop is written in Rust. S01 does not compare Pi or OpenCode as dependencies. It measures the open Rust questions instead. Those are a vetted third-party provider crate versus thin project adapters, and running the loop inside the server process versus a supervised worker process or container.

The deliverable is a reviewable foundation and a decision receipt that confirms the Rust runtime boundary, the Setup binding on server workspace paths and the container packaging path, or records a bounded follow-up experiment. It may contain disposable probe code. Do not build production editors, accounts, a broad task platform, a public deployment or a payment flow.

## Entry conditions

Architecture and roadmap reviewed enough to authorize this bounded stage, including the 2026-10-24 decision record. No prior implementation is assumed.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S01. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Pi and OpenCode source remain behavioral references for hook mutation and restart replay, not dependencies. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S01-W1 | Implementation lead | Record environment and source pins | Inventory OS, Git, Rust toolchain, Docker engine and Compose, Node for the browser client build, Python for package validation, Claude and optional Codex versions without reading secrets. Record missing prerequisites, the actual Git revision and current repository instructions. Store environment.json and source closure under root-relative evidence/S01/. Do not install globally or replace user settings. |
| S01-W2 | Guidance owner | Verify guidance and create the workspace skeleton | Verify root AGENTS and importing CLAUDE, four workflow skills and safe project config already in place. Check all 196 inherited config leaves against the migration report. Confirm current client discovery, disabled optional MCP servers and no automatic next-stage behavior. Create a minimal Cargo workspace (server, agent, setup adapter crates) and a Docker Compose file with the server and PostgreSQL only. The skeleton carries no product features. |
| S01-W3 | Onboarding engineer | EXP-01 Setup binding on server paths and recovery | Prove the callable binding for the portable planning core plus the desktop scanner and transaction services adapted to operate on a server workspace directory inside a container. Compare direct crate embedding with an extracted facade. Launcher integration and the user's Documents folder are out of scope. Preserve provider analysis and confirmed scan evidence gates. Reproduce the audited recovery CI mismatch in a disposable fixture using the exact revision and log context. Compare before/after inventories including mode, bytes and relevant metadata. Identify cause, fix within approved investigation scope or choose a separately verified pin. Do not assume the last release is safer. |
| S01-W4 | Domain engineer | EXP-02 source fidelity | Exercise existing Agent Tools no-op and bounded focus rewrite on UTF-8 BOM, CRLF, unknown syntax, duplicate IDs, shared focuses and relative positions. Probe supported node creation, prerequisite linking, moves and scalar edits. Compare bytes and semantics independently. Determine the supported writable subset and parser limits, and whether the tools run correctly inside the Linux server image. |
| S01-W5 | Runtime engineer | EXP-03 and EXP-04 Rust runtime probes | Drive a minimal Rust agent loop with deterministic fake transports. Test malformed final arguments, a hook or repair step that mutates arguments, stream truncation, missing finish markers, cancellation, stale fence ownership, changed-root replay and persistent restart. Compare a third-party provider crate with thin adapters, and in-process with worker execution. Build a container image that performs one native request with no coding CLI present in the image. With credentials and a bounded budget, probe two API profiles and one reachable OpenAI-compatible endpoint. Probe a GitHub App connection against a disposable repository and record the actual granted scope. |
| S01-W6 | Reviewer | Decision and handoff | Compare measured probe evidence, integration complexity, licenses including AGPL-3.0 compatibility of chosen crates, and residual risk. Confirm the Rust runtime boundary or record a bounded follow-up experiment. Record ADR dispositions and exact dependency pins. Ask for review of the concrete S01 result, then stop. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S01-W1 | None | Global installs or secret inspection |
| S01-W2 | S01-W1 | Installing every inherited role or MCP automatically, product features in the skeleton |
| S01-W3 | S01-W1, S01-W2 | Replacing the importer, changing real mods or reading a user's local game install |
| S01-W4 | S01-W1, S01-W2 | Building a general-purpose parser |
| S01-W5 | S01-W1, S01-W2 | Production orchestration, open-ended model spending, writes to a non-disposable GitHub repository |
| S01-W6 | S01-W3, S01-W4, S01-W5 | Starting S02 |

## Verification plan

Use F-SETUP-NEW, F-SETUP-IMPORT, F-SETUP-PARTIAL, F-FOCUS-FIDELITY and F-RUNTIME-FAULT from the test strategy. Start with fake transports that invoke the real Rust adapter code. A fake model must be labelled as such. At least one final mutated argument is rejected by the host boundary, an unsafe interrupted effect is not replayed, and a changed root invalidates replay identity.

Use M-API-A, M-API-B and M-LOCAL-C, now a self-hosted OpenAI-compatible endpoint reachable from the server, for capability probes when their prerequisites are available. Record exact model IDs and versions at execution. No unavailable provider is marked passed. Proposed live probe budget is 10 USD total across APIs. A missing credential can defer a provider result but cannot justify claiming native-provider acceptance. The GitHub probe uses a disposable repository created for the test and records token type, lifetime and scope without storing the token.

Record the container image digest and confirm by inspecting the image filesystem that no coding CLI, game file or credential is present. Run Setup and Agent Tools probes inside the same Linux image where the stage expects them to run.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

For documentation and guidance during this stage, run `python scripts/validate_package.py --root . --working-tree` and `python scripts/sync_skills.py --check` from the project root. Default validator mode verifies the untouched distribution against its frozen manifest. Working-tree mode permits later Git metadata, evidence and product files while checking the authored planning and skill projections. These checks do not satisfy runtime or game gates.

## Measurable exit gates

1. Every starter JSON/TOML file parses, all generated skill projections match their canonical sources after local-link rebasing, and the actual client reads the intended project guidance.
2. All mandatory new/import preservation fixtures and the reproduced recovery case have exact before/after evidence on server workspace paths. An unresolved source-loss case blocks production apply reliance.
3. The Rust agent loop has recorded deterministic probe outcomes for every listed fault. It rejects invalid or mutated final tool arguments and stale authority, and the provider-crate and execution-placement comparisons have measured results.
4. One containerized native request works with no Claude Code, Codex or other coding CLI in the image, and the GitHub App probe records its actual granted scope on a disposable repository. Any unavailable provider or platform remains explicitly unsupported.
5. A decision receipt names selected pins, supported operations, remaining unknowns and the next stage's bounded scope. Architecture review is requested, S02 is not started.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Remove only the isolated probe workspace, disposable containers and the disposable GitHub repository, or revert owned bootstrap commits. Preserve existing user guidance and global configuration. If a probe argues against the Rust runtime decision, keep the probe evidence and the documented Candidate A and B reference designs rather than deleting inconvenient results, and return the question to the maintainer.

## Engineering learning checkpoint

Review every corrected assumption from source and probes, especially Setup recovery on Linux paths, model context versus instruction limits, Rust stream assembly and runtime replay. Promote only reproduced general lessons. Record a justified no-change result if the source guidance already covers the issue.

Record the result in `evidence/S01/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Record current client versions, exact accepted settings, all intentionally disabled integrations, local override locations and unsupported keys. Record the Rust toolchain, base image digests and Compose file. Re-run projection and parse checks after any config change. Do not copy machine paths, API values or GitHub tokens into evidence.

Record `evidence/S01/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

S01 ends with environment, probe evidence, ADR dispositions and a proposed S02 entry contract. The next agent must be told which Setup binding is feasible on server paths, whether apply is cleared, which focus operations preserve source inside the image, which provider adapter approach and execution placement were selected, what the GitHub probe granted and which model or platform probes remain missing.

Produce `evidence/S01/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S01. **Do not start the next stage automatically.**
