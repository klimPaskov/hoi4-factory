# S01: Claude Code bootstrap and architectural probes

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership.

## Objective and non-goals

Use the extracted `hoi4-factory/` folder as the implementation workspace after this stage is explicitly authorized. Root guidance, client settings and skills are already in place, with planning documents under `docs/`. Read AGENTS, relevant skills and applicable local instructions before adapting these files. Establish verified Claude-first loading, environment evidence and isolated feasibility probes. Do not build production editors, a broad task platform or a full deployment.

The deliverable is a reviewable foundation and a decision receipt choosing or rejecting the proposed architecture. It may contain disposable probe code in the future authorized repository. This planning assignment has created no such product code.

## Entry conditions

Architecture and roadmap reviewed enough to authorize this bounded stage. No prior implementation is assumed.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S01. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S01-W1 | Implementation lead | Record environment and source pins | Inventory OS, Git, Node, Rust, Python, Claude and optional Codex versions without reading secrets. Record missing prerequisites, whether Git metadata and an initial commit exist, and current repository instructions. Store environment.json and source closure under root-relative evidence/S01/. Do not install globally or replace user settings. |
| S01-W2 | Guidance owner | Verify and adapt starter guidance | Verify and adapt root AGENTS and importing CLAUDE, four workflow skills and safe project config already in place. Merge as a reviewed diff only when integrating into a different existing repository. Check all 196 inherited config leaves against the migration report. Confirm current client discovery, disabled optional MCP servers and no automatic next-stage behavior. |
| S01-W3 | Onboarding engineer | EXP-01 Setup boundary and recovery | Prove the callable binding for the portable planning core plus existing desktop scanner and transaction services, comparing direct embedding with an extracted facade. Preserve provider analysis and confirmed scan evidence gates. Reproduce the audited recovery CI mismatch in a disposable fixture using the exact revision and log context. Compare before/after inventories including mode, bytes and relevant metadata. Identify cause, fix within approved investigation scope or choose a separately verified pin. Do not assume the last release is safer. |
| S01-W4 | Domain engineer | EXP-02 source fidelity | Exercise existing Agent Tools no-op and bounded focus rewrite on UTF-8 BOM, CRLF, unknown syntax, duplicate IDs, shared focuses and relative positions. Probe supported node creation, prerequisite linking, moves and scalar edits. Compare bytes and semantics independently. Determine the supported writable subset and parser limits. |
| S01-W5 | Runtime engineer | EXP-03 and EXP-04 runtime probes | Run Pi and OpenCode with deterministic provider fixtures. Test hooks, malformed final arguments, stream truncation, cancellation, stale ownership, changed-root replay and persistent restart. Package a minimal native request without a coding CLI. With credentials and a bounded provider budget, probe two API profiles and one local endpoint. |
| S01-W6 | Reviewer | Decision and handoff | Compare measured probe evidence, integration complexity, licenses and residual risk. Select Candidate A, B or a bounded follow-up experiment. Record ADR dispositions and exact dependency pins. Ask for review of the concrete S01 result, then stop. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S01-W1 | None | Global installs or secret inspection |
| S01-W2 | S01-W1 | Installing every inherited role or MCP automatically |
| S01-W3 | S01-W1, S01-W2 | Replacing the importer or changing real mods |
| S01-W4 | S01-W1, S01-W2 | Building a general-purpose parser |
| S01-W5 | S01-W1, S01-W2 | Production orchestration or open-ended model spending |
| S01-W6 | S01-W3, S01-W4, S01-W5 | Starting S02 |

## Verification plan

Use F-SETUP-NEW, F-SETUP-IMPORT, F-SETUP-PARTIAL, F-FOCUS-FIDELITY and F-RUNTIME-FAULT from the test strategy. Start with fake transports that invoke the real adapter. A fake model must be labelled as such. At least one final-hook mutation is rejected by the host boundary, an unsafe interrupted effect is not replayed, and a changed root invalidates replay identity.

Use M-API-A, M-API-B and M-LOCAL-C for capability probes when their prerequisites are available. Record exact model IDs and versions at execution. No unavailable provider is marked passed. Proposed live probe budget is 10 USD total across APIs, with local time capped separately. A missing credential can defer a provider result but cannot justify claiming native-provider acceptance.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

For documentation and guidance during this stage, run `python scripts/validate_package.py --root . --working-tree` and `python scripts/sync_skills.py --check` from the project root. Default validator mode verifies the untouched distribution against its frozen manifest. Working-tree mode permits later Git metadata, evidence and product files while checking the authored planning and skill projections. These checks do not satisfy runtime or game gates.

## Measurable exit gates

1. Every starter JSON/TOML file parses, all generated skill projections match their canonical sources after local-link rebasing, and the actual client reads the intended project guidance.
2. All mandatory new/import preservation fixtures and the reproduced recovery case have exact before/after evidence. An unresolved source-loss case blocks production apply reliance.
3. Both runtime candidates have recorded deterministic probe outcomes. The selected candidate rejects invalid final tool arguments and stale authority.
4. One packaged native request works without Claude Code, Codex or an external coding CLI. Any unavailable platform remains explicitly unsupported.
5. A decision receipt names selected pins, supported operations, remaining unknowns and the next stage's bounded scope. Architecture review is requested, S02 is not started.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Remove only the isolated probe workspace or revert owned bootstrap commits. Preserve existing user guidance and global configuration. If architecture changes, keep the probe evidence and alternate adapter contract rather than deleting inconvenient results.

## Engineering learning checkpoint

Review every corrected assumption from source and probes, especially Setup recovery, model context versus instruction limits and runtime replay. Promote only reproduced general lessons. Record a justified no-change result if the source guidance already covers the issue.

Record the result in `evidence/S01/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Record current client versions, exact accepted settings, all intentionally disabled integrations, local override locations and unsupported keys. Re-run projection and parse checks after any config change. Do not copy machine paths or API values into evidence.

Record `evidence/S01/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

S01 ends with environment, probe evidence, ADR dispositions and a proposed S02 entry contract. The next agent must be told which Setup binding is feasible, whether apply is cleared, which focus operations preserve source, which runtime is selected and which model/platform probes remain missing.

Produce `evidence/S01/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S01. **Do not start the next stage automatically.**

