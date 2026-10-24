# S05: Context, model handoff and recovery hardening

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md).

## Objective and non-goals

Make repeated work reliable across sessions and provider changes. Implement approved canon, provenance-aware context, freshness, reviewable runtime lessons and stronger recovery. Keep engineering learning separate from product memory.

Do not add vector retrieval or a separate memory service by default. Canon and memory stay scoped to their project and account. Do not convert inferred conversation statements into accepted lore.

## Entry conditions

S04 accepted with a complete small vertical slice and clear fidelity limits.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S05. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S05-W1 | Context engineer | C-06 builder | Assemble approved requirements, source pointers, task state and receipt references under the actual model budget. Record inclusion reasons, omissions and immutable manifest hashes. |
| S05-W2 | Domain engineer | Canon and ownership | Implement named canon owners, explicit design proposals and versioned approvals. Show conflicts between accepted design and inferred suggestions. |
| S05-W3 | Index engineer | Freshness and rebuilding | Invalidate changed dependency closures, tool/game profiles and source locators. Publish complete derived-index generations atomically and reconcile cancelled jobs. |
| S05-W4 | Learning engineer | C-07 runtime review | Implement candidate review with expected versions, rejection and supersession. Preserve evidence and keep runtime lesson approval separate from canon or engineering policy. |
| S05-W5 | Runtime engineer | Cross-model handoff | Drain or reconcile in-flight effects, rebuild context for the next provider and recheck destinations/capabilities. Test smaller context, missing image support and revoked authority. |
| S05-W6 | Reviewer | EXP-05 and recovery study | Run exact/full-text retrieval baseline and stale-answer fixtures. Add a new retrieval component only if the measured gap persists. Audit recovery state after long sequences. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S05-W1 | S04 | Copying private provider reasoning between models |
| S05-W2 | S05-W1 contract | Silently promoting suggestions to approved canon |
| S05-W3 | S05-W1, S05-W2 contracts | Adding a vector database before baseline measurement |
| S05-W4 | S05-W1, S05-W2, S05-W3 | Changing engineering policy from runtime feedback |
| S05-W5 | S05-W1, S05-W2, S05-W3, S05-W4 | Provider fallback to an unapproved destination |
| S05-W6 | S05-W1, S05-W2, S05-W3, S05-W4, S05-W5 | Indefinite recovery loops |

## Verification plan

Run CTX-01 through CTX-10 from SPEC-004. Include a source rename, a game reference update to a new build, removed permission, duplicate lesson generation, competing reviewers and interrupted index publication. Exercise model A to B, B to the self-hosted endpoint and that endpoint back to A while preserving data-destination restrictions. Confirm one project's canon and lessons never appear in another project's context.

Use at least ten related tasks on the same fixture to expose accumulated stale context. Compare accepted output and human correction with the S04 baseline. Do not compare only prompt length or summary fluency.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. Approved canon, inferred memory, accepted engineering rules and caches remain distinct in storage, context and UI.
2. Changed source, tool profile or game reference build makes affected evidence stale before a new apply can rely on it.
3. Cross-model resume preserves task and receipt identity, strips incompatible private provider artifacts and repeats no unsafe effect.
4. Candidate approval/rejection races have one winner, and superseded memories disappear from active recall.
5. Retrieval benchmark records correct source selection, missed facts and stale answers. New infrastructure has an evidence-based decision.
6. Long-sequence reopen and restore tests maintain source and policy invariants without indefinite audit loops.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Restore prior index generation or rebuild derived caches from source and the current game reference. Revert a reviewed lesson through supersession, preserving its history. Roll back store migration only through its tested path.

## Engineering learning checkpoint

Review what the long-sequence test actually taught about development. Promote the smallest tested engineering fix separately from runtime candidate data.

Record the result in `evidence/S05/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Recheck context budgets per exact model, remote-data destination rules, tool schema versions and mirror consistency of changed workflow guidance.

Record `evidence/S05/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

Provide canon and memory schemas, invalidation rules, retrieval results, model-switch traces and backup assumptions. S06 and S07 can branch from this stage with independent owned files.

Produce `evidence/S05/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S05. **Do not start the next stage automatically.**

