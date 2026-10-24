# S02: Mod Setup integration and authoritative workspaces

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership.

## Objective and non-goals

Implement the reused new/import workflow and the authoritative workspace boundary. Reuse the portable planning core and extract or adapt existing desktop scanner and transaction services where their source boundary requires it. Preserve the established workflow rather than replacing it with a new scaffolder.

Deliver a local workspace that can be opened, inspected, prepared, re-run and restored. No autonomous model writes or focus canvas are required yet.

## Entry conditions

S01 accepted with a selected architecture, a supportable Setup pin and no unresolved preservation defect on the intended apply path.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S02. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S02-W1 | Onboarding engineer | C-09 adapter | Bind inspected functions for scan, provider analysis, plan and approved apply. Preserve core-owned plan identity and expected root. Expose actual recovery variants. Keep web ZIP export distinct from desktop in-place installation. |
| S02-W2 | Workspace engineer | C-01 and source snapshots | Register stable workspace IDs, root handles, content order, game/DLC unknowns and file hashes. Inventory dirty and untracked files. Detect metadata-prefix collisions and unknown schema versions. |
| S02-W3 | Onboarding engineer | New/import interface | Reuse identity, scan review, component, workflow, Git, dry-run and readiness controls where relevant. Separate optional external coding environments from required native factory runtime. Preserve existing instructions through explicit merge choices. |
| S02-W4 | Integrity engineer | Transaction and recovery bridge | Persist plan, stage and apply receipts. Follow rollback-required partial apply behavior. Verify finalization against success lock and rollback records. Avoid competing transaction owners between Setup and host. |
| S02-W5 | Git engineer | Owned workspace baseline | Create and reopen owned branches/worktrees, preserve real index and unrelated edits, record expected HEAD and restore points. Guard path changes and source preconditions. |
| S02-W6 | Reviewer | Compatibility closure and handoff | Record actual manifest5/component closure, including tools4.0.3 if selected. Treat newer tools4.3.0 as an explicit tested migration. Produce capabilities and unresolved checks without advertising unsupported clients. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S02-W1 | S01 | Inventing a public Setup HTTP service |
| S02-W2 | S02-W1 contract | Creating a proprietary replacement for native mod source |
| S02-W3 | S02-W1, S02-W2 | Mandatory coding CLI installation for product users |
| S02-W4 | S02-W1, S02-W2 | Resuming a partial native apply contrary to Setup's contract |
| S02-W5 | S02-W2, S02-W4 | Committing unrelated dirty files |
| S02-W6 | S02-W3, S02-W4, S02-W5 | Implicit tool-version upgrades |

## Verification plan

Run new mod, existing clean mod, dirty Git mod, non-Git import, partial setup, modified managed instruction, custom asset, missing optional tool, Unicode path and interrupted apply fixtures. A non-Git import requires explicit consent before initializing Git and must preserve the original folder.

Kill at staging, first apply, post-install and finalization boundaries. Test source change after dry-run, path identity replacement and a pre-existing launcher descriptor outside the selected root. Default-deny outside-root use until the reviewed plan authorizes it. Repeat setup twice and compare changed paths.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. New and import paths produce a C-01 descriptor from real reused setup logic, with no redundant importer.
2. Protected native files, custom assets, existing instructions and unrelated Git state remain byte-identical unless an exact reviewed change covers them.
3. Re-running a completed setup produces no unexpected changes. Modified generated files become explicit conflicts or preserved files.
4. Every injected interruption has a classified recoverable state, including rollback-required partial apply. Restoration inventory matches its expected baseline.
5. The exact installed manifest/tool closure and missing capabilities are visible. Current audited HEAD behavior is not attributed to an older release.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Use the recorded Setup recovery operation and host-owned baseline. If user files changed after apply, preserve them and produce a conflict instead of blindly restoring. Revert owned integration commits without deleting the imported mod.

## Engineering learning checkpoint

Capture discovered setup and preservation edge cases with minimal fixture regressions. Keep project-specific onboarding choices out of global instructions.

Record the result in `evidence/S02/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Verify project-scoped working directory behavior, wrapper paths, disabled optional dependencies and generated-file ownership. Any manifest upgrade must update the compatibility receipt and migration report.

Record `evidence/S02/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

Provide the exact Setup pin, extracted service boundaries, C-01 fixture, failed/recovered operation evidence and supported platforms. S03 may assume validated workspace registration, not universal game compatibility.

Produce `evidence/S02/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S02. **Do not start the next stage automatically.**

