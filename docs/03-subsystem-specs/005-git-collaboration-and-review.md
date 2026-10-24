# SPEC-005: Git collaboration and review

Status: **Proposed**. Primary requirements: REQ-021, REQ-022, REQ-023, REQ-024 and REQ-035. S02 establishes isolation, S04 delivers review and restore, and S06 adds contributor handoff.

Read first: [workspace setup](001-onboarding-and-mod-setup.md), [contracts](../02-architecture/contracts-and-interfaces.md), [recovery](003-orchestration-and-recovery.md), [visual synchronization](007-visual-state-synchronization.md), [permissions](011-extensibility-sandbox-and-permissions.md) and [acceptance matrix](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md).

## Purpose and user workflow

A contributor should be able to accept a useful agent change without trusting a chat summary or losing existing work. The application presents the task intent, semantic change, source diff, validation evidence and known limitations together. A novice can review the visible outcome. A scripter can inspect exact source. Both approve the same immutable proposal.

The first workflow uses local Git worktrees and reviewed changes. Hosted multiplayer editing, automatic merge fleets and a central collaboration service are deferred. The application remains useful when no remote is configured.

## Evidence and reusable boundaries

[SRC-R02, Entire checkpoint interfaces](https://github.com/entireio/cli/blob/ee57ff55142e5078b625dd963fc398d49a8ddda1/api/checkpoint/interfaces.go) separate checkpoints, sessions and task records. Its [reference update implementation](https://github.com/entireio/cli/blob/ee57ff55142e5078b625dd963fc398d49a8ddda1/cmd/entire/cli/checkpoint/persistent_ref_update.go) distinguishes stale-reference conflicts from retryable lock contention. These are useful provenance and concurrency patterns without requiring the Entire CLI.

[SRC-B07, Fusion workspace leases](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/task-store/workspace-leases.ts) bind ownership to task, run and incarnation with an increasing fence token. Its [handoff test](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/__tests__/postgres/handoff-to-review-atomicity.pg.test.ts) checks that review state and audit do not split on rollback. The product should adapt these invariants to its own local store. Source inspection does not establish that these tests passed here.

## Workspace baseline and isolation

C-01 records repository identity, selected base reference, worktree location, exact source snapshot and dirty-state inventory. On import, inspect tracked changes, untracked files, nested repositories, submodules, large files and existing worktrees. Existing content remains owned by the user.

A task worktree begins from an identified Git base. If the user is working with dirty source, the host stages a selected overlay into the task worktree and records each included path and hash. It must not silently omit relevant unsaved work, sweep unrelated files into a commit, discard user edits or clean the original tree. Ambiguous scope produces an actionable baseline decision.

Read-only roots may include game data and dependency mods. Writable roots contain the task's permitted mod content. Shared Git metadata needs its own controlled operation surface. A path ownership declaration is a scheduling aid, not proof that writing that path is authorized.

The source inventory and selected setup receipt must agree before isolation starts. A failed or unresolved setup transaction remains under C-09 recovery. Git isolation cannot make a partial setup appear complete.

## ChangeProposal and review record

C-02 contains the proposal ID, workspace identity, base revision and per-path hashes, operations, owned paths, semantic intent, source spans, binary hashes, requirements, policy and grant references, evidence and rollback receipt. Its hash identifies exactly what was reviewed.

A review record adds reviewer, decision, time, proposal hash, evidence set and rationale. Review can accept, request changes or reject. A standing task grant may authorize bounded apply or commit without another modal prompt. The host records which rule authorized the decision. A changed proposal invalidates a proposal-specific approval, while a broader still-valid grant is evaluated against the new final operations.

Show text and binary changes differently. Text review includes source locations and preservation diagnostics. Binary review includes before/after hashes, dimensions or format metadata where relevant, and a usable preview. A binary file with no supported preview remains inspectable and explicitly limited. Validation applies to the final bytes, including localization and asset references.

Do not use a screenshot, old test log or successful parser run from another revision as evidence for the proposal. C-05 must identify the exact source snapshot and tool version.

## Operations and failure outcomes

| Operation | Input and output | Failure behavior |
|---|---|---|
| Create isolated task workspace | C-01, base ref and selected dirty overlay return task worktree receipt | Path collision, unsupported repository state or missing baseline returns no partial accepted workspace |
| Build proposal | Task worktree and intent return immutable C-02 | Unowned or unexpected paths are reported before review |
| Review | Proposal hash and evidence set return attributed decision | Stale hash or changed evidence rejects the decision |
| Apply or integrate | Accepted proposal, expected target ref and C-08 return C-04 and updated snapshot | Concurrent source/ref change returns conflict without overwriting new work |
| Commit | Exact staged path set and review receipt return commit hash | Failure remains reconcilable by operation ID and observed Git state |
| Restore | Prior accepted snapshot and current hashes return inverse proposal | Later human changes create a conflict instead of being discarded |
| Export handoff | Task, patch, evidence manifest and attribution return portable bundle | Missing evidence is listed. Private prompts and credentials are excluded |
| Publish | Explicit remote, ref and authorized publication scope return remote receipt | Unknown network outcome is reconciled before retry |

Local commit, branch integration and remote publication are distinct operations. Approval to edit local files does not silently permit push or release. Existing user authorization may cover a named publication scope, in which case the product proceeds within it and records the receipt.

## Concurrent edits and integration

Before apply, the host re-reads per-path hashes and target references. Filesystem notifications accelerate invalidation, but are not the authority. Non-overlapping changes may be rebased and revalidated automatically within policy. A rebase creates a new proposal and evidence identity.

Text conflicts can offer a three-way comparison with source-aware diagnostics. The agent may propose a resolution, but it cannot silently choose between competing human intent. Binary conflicts require choosing an artifact or generating a separately reviewed replacement. Unknown script constructs remain preserved, as required by the structured editors.

An integration transaction acquires ownership, verifies current grant and expected ref, persists intent, applies the accepted change, validates final state and records the resulting commit or snapshot. Reference advancement is conditional on the observed old ref. Lock contention may retry within a limit. A changed ref requires a fresh plan.

If a crash occurs after creating a commit but before recording its receipt, recovery inspects the intended tree, operation metadata and target ref before creating another commit. An orphan local object is not proof that integration completed. If a remote accepted a push but the response was lost, query the named remote ref and compare the intended hash. Network retry must not invent a second publication.

## Restore and attribution

Restore is a new reviewed operation against current state. For accepted Git history, use a reversible change preserving provenance. For a task's unaccepted staging area, the host may restore its owned files from the operation journal after confirming no outside edits. A blanket destructive reset is not the recovery mechanism.

A run links task, model profile, visible user intent, tool receipts, evidence and commits. Attribute human edits and review decisions separately from generated changes. Record uncertainty when imported history lacks attribution. Do not infer who wrote code from a filename or a model claim.

Keep runtime transcripts and secrets outside normal mod history. A portable evidence export includes selected visible messages, tool/version information, hashes and validation coverage. The user can redact sensitive material before sharing. Redaction creates a new export manifest without rewriting the internal fact that an operation occurred.

Repository automation is part of the execution boundary. Imported hooks, filters, external diff tools and credential helpers must not gain execution simply because Git is used. The host needs a controlled Git environment and explicit opt-in for project automation. Stage S01 should test the chosen Git adapter against the actual supported repository features.

## Verification and acceptance

| Test | Scenario | Observable acceptance |
|---|---|---|
| GIT-01 | Import repository with tracked and untracked dirty work | Original tree is unchanged and selected overlay has exact manifest |
| GIT-02 | Agent edits outside owned scope | Proposal is rejected before apply and unexpected path is visible |
| GIT-03 | Human changes a file after review | Apply returns stale-base conflict, preserving human bytes |
| GIT-04 | Two tasks integrate at the same target ref | One conditional advancement wins, the other replans |
| GIT-05 | Binary artifact changes between preview and apply | Hash mismatch invalidates approval and evidence |
| GIT-06 | Crash after commit creation, before receipt | Recovery identifies observed state and does not blindly duplicate commit |
| GIT-07 | Roll back a prior change after later human edits | Inverse proposal preserves or explicitly conflicts with later work |
| GIT-08 | Review handoff database transaction fails | No in-review state appears without matching proposal and audit records |
| GIT-09 | Push response lost after remote success | Reconciliation reports actual ref state before retry |
| GIT-10 | Imported repository contains executable hook or filter | Controlled Git operations do not execute it without authorization |
| GIT-11 | Export contributor handoff | Patch, base, evidence and limitations are complete, secrets absent |

AC-021 requires isolation demonstrated on a dirty fixture. AC-022 requires external edit and concurrent integration cases. AC-023 requires exact proposal attribution and evidence. AC-024 requires restore without loss of later changes. AC-035 requires a second contributor to reproduce the patch and checks from a handoff. Store fixture repositories, before/after manifests, operation logs and review records. Actual game behavior remains a separate evidence level.

S02 delivers baseline capture and task worktrees. S04 adds review, accepted source and inverse proposals for the focus slice. S06 adds dependency-aware contributor handoff and conflicts. S08 validates the selected Git implementation, path behavior, large binary handling and packaging on target platforms.

