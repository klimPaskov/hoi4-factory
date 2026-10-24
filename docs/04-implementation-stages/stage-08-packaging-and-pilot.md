# S08: Deployment hardening, compatibility and evaluated pilot

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership.

## Objective and non-goals

Package and evaluate the selected product surface. The first useful slice already exists. This stage makes its support, recovery, upgrade, licensing and usability claims reviewable.

Do not add hosted collaboration, many workers or full-world generation as release polish. A deferred feature is absent or clearly disabled, never silently marked accepted.

## Entry conditions

S06 and S07 accepted for the full selected scope, or architecture review explicitly defers one and excludes its feature from this release. S04 and S05 are mandatory.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S08. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [SPEC-008](../03-subsystem-specs/008-maps.md), [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md), [SPEC-010](../03-subsystem-specs/010-assets-localization-and-content.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S08-W1 | Release engineer | Reproducible package | Pin runtime/tool dependencies, build platform artifacts, record checksums and notices, and verify no external coding CLI is required. Sign or otherwise document artifact authenticity according to the distribution plan. |
| S08-W2 | Security engineer | Boundary validation | Exercise renderer IPC, path and symlink escapes, malicious source instructions, plugin authority, credential redaction, destination fallback and stale worker fences. |
| S08-W3 | Reliability engineer | EXP-08 backup and upgrade | Test consistent backup, restore, schema migration, interrupted update, uninstall and rollback while preserving native mod files and unresolved journals. |
| S08-W4 | Evaluation owner | Three-profile scenario matrix | Run all selected scenarios with M-API-A, M-API-B and M-LOCAL-C under the declared budgets. Record exact model/provider versions, skips, failures, intervention and rework. |
| S08-W5 | Product reviewer | Contributor pilot | Observe a non-scripter, technical contributor and maintainer using the selected surface. Record task completion and comprehension of evidence limits. Do not infer usability from screenshots alone. |
| S08-W6 | Maintainer | Release decision | Review known defects, actual support matrix, legal/asset constraints and evidence. Approve, narrow or defer release. Preserve the backlog and do not publish externally without the applicable authorization. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S08-W1 | Frozen capability scope from prerequisites | External publication |
| S08-W2 | S08-W1 candidate package | Claiming a same-user process is a sandbox |
| S08-W3 | S08-W1, S08-W2 contract | Deleting native projects on uninstall |
| S08-W4 | S08-W1, S08-W2, S08-W3 | Fabricated results for unavailable models |
| S08-W5 | S08-W1, S08-W2, S08-W3 | Equating screenshots with usability trials |
| S08-W6 | S08-W2, S08-W3, S08-W4, S08-W5 | Starting deferred features or publishing automatically |

## Verification plan

Run the acceptance matrix and scenario matrix on packaged artifacts, not only development servers. Include unsupported game build, missing local model, API refusal/rate limit, offline reopen, worker crash, disk-full simulation and corrupted/missing artifact recovery.

Repeat critical source-loss and authorization fixtures after update and restore. Test on each advertised platform. Actual game observations require a permitted installation and user-authorized scenario. A missing environment is a support limitation, not a passing test.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. Every advertised capability maps to passed acceptance evidence or an explicitly accepted limitation with narrow wording. Critical unauthorized-write and source-loss defects are absent on the supported scope.
2. Packaged end-user runs require no developer coding harness and preserve the native repository escape hatch.
3. All required scenarios and three distinct provider configurations have truthful results, with no hidden substitution for the local profile.
4. Backup, update, uninstall and restore preserve source, accepted canon and recoverable task state under declared conditions.
5. License inventory excludes restricted or unlicensed reuse and confirms game/asset distribution boundaries.
6. Pilot findings, cost/time/rework metrics and remaining risks support a concrete release decision. No automatic publication occurs.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Pause task dispatch, retain current source and evidence, roll back the application through a tested version path and reconcile schema state. If downgrade is unsafe, open read-only and restore a compatible backup rather than force a corrupt downgrade.

## Engineering learning checkpoint

Run the full engineering learning review on pilot failures and recurring fixes. Retire obsolete guidance and record accepted or rejected changes with evidence.

Record the result in `evidence/S08/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Revalidate current Claude/Codex guidance and runtime profiles at the release pins. Review optional MCP endpoints, secrets, permissions, concurrency, model capability changes and source manifest upgrades.

Record `evidence/S08/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

Deliver release evidence, support matrix, unresolved defects, backup/restore instructions and a prioritized next-stage proposal. Implementation ends at the approved release scope.

Produce `evidence/S08/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S08. **Do not start the next stage automatically.**

