# S06: Invited collaborators, task dependencies and content workflows

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md). This stage is now required for the first public release.

## Objective and non-goals

Extend the single-agent workflow to invited human collaborators and a small task DAG. A project owner invites other accounts into the same server workspace with explicit project roles. Add controlled localization and asset pipelines, then selected event or decision authoring only where adapters support it.

The agent-tools work behind this stage's MCP items (Q-1, M-1 to M-14, the engine moves and the coverage gaps) is planned in the [MCP improvement plan](../08-expansion-research/12-mcp-improvement-plan.md).

Event, decision, character and idea editors follow the S04 two-way pattern (FR-5): each one shows the matching inspect and render tools to people, and people and agents write through the same rewrite operation. The writers they need are built in hoi4-agent-tools on the engine. A localisation tool covers every language, GFX assets are checked, `hoi4-imagegen-mcp` joins the tool profile, and Hagane Studio export ZIPs import with a preservation report. With the focus editor from S04, these editors and the import meet the editor and import conditions for archiving Hagane Studio.

First prove one owner, one invited collaborator and one agent collaborating through owned branches and reviewed integration. A second agent worker is an experiment with disjoint scopes. Many workers and real-time co-editing of one file remain deferred. External contributors can still work through GitHub pull requests.

## Entry conditions

S05 accepted. Source, context and ownership contracts are stable enough to test independent contributor work.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S06. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-010](../03-subsystem-specs/010-assets-localization-and-content.md), [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Engine, editors and tool quality

The cross-stage plan is in [engine, editors and tools across stages](ROADMAP.md#engine-editors-and-tools-across-stages).

| Topic | In S06 |
|---|---|
| Shared Rust engine | Event, decision, character and idea routes run on the engine. Target: the TypeScript index retires by the end of this stage |
| Event editor | `hoi4.event_inspect`, `hoi4.event_render`, `hoi4.probability_inspect`, `hoi4.probability_analyze`, `hoi4.impact_inspect` and `hoi4.scenario_test`, plus a new event writer |
| Decision editor | `hoi4.decision_inspect`, `hoi4.impact_inspect` and `hoi4.mechanic_test`, plus a new decision writer |
| Character and idea editors | New inspect, render and rewrite tools for each |
| Localisation and GFX | A localisation inspect and rewrite tool on the engine's reader, every language indexed, sprite and `.gfx` registration checks, DDS and profile checks through `hoi4-imagegen-mcp`. The sprite index and the content-to-sprite trace live in the shared engine (hoi4-agent-tools PR 73), and `hoi4.image_audit` and `hoi4.image_validate` use them with the 40 measured asset families. The hosted image adapter uses documented provider APIs only. The ChatGPT-login provider that is the default on a local install (D-12a) is never enabled in hosted HOI4 Factory. hoi4-imagegen-mcp already refuses it in `RunMode::Hosted` (its `plugin_gates.rs` test `hosted_mode_can_never_select_it_even_signed_in_and_asked_for`). S06-W3 adds the Factory-side test that the hosted tool profile starts the server with `--mode hosted` and cannot select the login route |
| Hagane harvest | Hagane export ZIP import with a preservation report. The draw.io sketch importer creates a new focus branch as a proposal |
| Q-1 | No regression on T-BENCH-01. New writers and the localisation tool join the benchmark |
| Coding-agent integration | The new tools are in the one tool profile, so local coding clients get them in the same release |

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S06-W1 | Workflow engineer | Task DAG | Implement dependency readiness, blocked reasons, owner leases, bounded attempts and review handoff. Reject cycles and stale claim release. Task metadata is not proof of completed effects. Show the tasks on the simple jobs board, with GitHub issues authoritative for linked jobs. |
| S06-W2 | Git engineer | Conflict and integration | Check textual overlap, symbol/ID collisions, localization keys and asset destinations. Create fresh merged proposals and evidence. Preserve unrelated index entries. |
| S06-W3 | Content engineer | Localization, assets and imports | Track rights, original hashes, conversion recipes, language coverage and exact consumer references. Preserve source assets and distinguish decode, import, wiring and visible runtime evidence. Use the new localisation tool for every language and the GFX checks. Add `hoi4-imagegen-mcp` to the tool profile behind the allowance. Import Hagane export ZIPs with a preservation report, and draw.io sketches as focus proposals. Split this packet if it cannot be reviewed in one session. |
| S06-W4 | Domain engineer | Event, decision, character and idea editors | Add narrow event, decision, character and idea operations with existing source lookup and validation, each built as an agent-tools writer on the engine and shown in a two-way editor through the S04 pattern. Keep read-only tool capabilities separate from newly implemented write support. Each editor screen goes through the design track. |
| S06-W5 | Collaboration designer | Invitations, roles and review surfaces | Implement invitations, acceptance, role changes and removal for project members. Roles bound what each member and their agents may grant. Make lore, art, map and scripting contributions attributable through intent, review and Git. Provide actionable conflict explanations and a source escape hatch. Bring GitHub issues, pull requests, reviews, comments, labels, checks and merges fully into the project view under ADR-018, mirrored by verified webhooks, with a one-click action that turns an issue or comment into an approval-gated task. Verify with T-GH-03. |
| S06-W6 | Reviewer | Controlled concurrency study | Run one-human/one-agent then two-disjoint-worker fixtures. Compare accepted output, conflict rate and rework to single writer before expanding. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S06-W1 | S05 | Large worker swarms |
| S06-W2 | S06-W1 contract | Text-only merge acceptance |
| S06-W3 | S05 | Bundling unlicensed game or generated assets, adopting Hagane platform conventions on import |
| S06-W4 | S06-W2, S06-W3 | Assuming read-only upstream tools already support writes, preview cards copied from Hagane Studio |
| S06-W5 | S06-W1, S06-W2, S06-W3 | Real-time co-editing of one file |
| S06-W6 | S06-W1, S06-W2, S06-W3, S06-W4, S06-W5 | Increasing worker count without measured benefit |

## Verification plan

Use F-COLLAB, F-ASSET and F-LOCALIZATION. Have a human and worker change the same focus ID, different text spans with coupled semantics, an existing binary path and a shared localization key. Simulate lease loss before commit and a late worker result after ownership changes. Remove a member while their agent task is running and confirm no further effect uses their authority. Confirm a viewer role cannot approve or publish.

Run T-EDIT-01 for every supported operation of the four new editors, including a person and an agent editing the same event at once. Index a fixture with three languages and check missing keys per language. Run T-IMPORT-01 on a synthetic Hagane export ZIP. Run T-BENCH-01 and compare with the stored baseline.

Exercise missing rights metadata, unsupported conversion, missing consumer registration, failed audio/image decode and partial output cleanup. The production pipeline must return explicit evidence limits instead of accepting a file because it exists.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. Dependency cycles, stale claims and lease-loss publication are rejected without losing work.
2. Conflict detection catches the required semantic and binary collisions, not only Git text conflicts.
3. Each accepted asset has provenance, transformation receipt, output hash and consumer wiring evidence.
4. Invited collaborators act only within their project role, can review owned intent and inspect the final source without needing every advanced control, and lose authority immediately on removal.
5. A two-worker experiment either demonstrates useful accepted throughput within risk budgets or records a decision to retain one writer.
6. Merge/review handoff is transactional at its defined store boundary and idempotent under duplicate requests.
7. The event, decision, character and idea editors pass parity across the gesture, native agent and coding-client paths on their supported operations, and localisation and GFX checks cover every language and sprite in the fixtures.
8. A Hagane Studio export ZIP imports with a preservation report for every path, and a draw.io sketch imports as a reviewable focus proposal.
9. T-BENCH-01 shows no regression from the stored baseline, with the new tools added to it.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Stop new claims, preserve branches and queued reviews, then revert owned integration changes. Do not close tasks as completed to clear the queue. Retain original assets and restore affected source through reviewed proposals.

## Engineering learning checkpoint

Record conflict patterns and repeated content-wiring errors as bounded tests and references. Do not turn role names into mandatory agent proliferation.

Record the result in `evidence/S06/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Review the tool profile with the new writers and `hoi4-imagegen-mcp`, whether any route still reads the TypeScript index, concurrency caps, worker scopes, member roles, optional asset-service credentials, network destinations and recurring costs. No inherited domain agent registry is enabled wholesale.

Record `evidence/S06/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

Provide supported content operations, editor parity results, the import reports, member role and authority map, merge receipts, DAG test results and the measured concurrency decision. S07 may run independently from S05 if file ownership is disjoint.

Produce `evidence/S06/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S06. **Do not start the next stage automatically.**

