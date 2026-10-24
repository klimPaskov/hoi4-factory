# SPEC-005: Git collaboration and review

Status: **Proposed**. Primary requirements: REQ-021, REQ-022, REQ-023, REQ-024, REQ-035 and REQ-047. Shared requirement: REQ-046. S02 establishes isolation and the GitHub connection, S04 delivers review, restore and pull requests for the focus slice, and S06 adds contributor handoff and invited collaborators. GitHub scope follows [ADR-014](../02-architecture/maintainer-decisions-2026-10-24.md#adr-014-native-github-integration).

Read first: [workspace setup](001-onboarding-and-mod-setup.md), [contracts](../02-architecture/contracts-and-interfaces.md), [recovery](003-orchestration-and-recovery.md), [visual synchronization](007-visual-state-synchronization.md), [permissions](011-extensibility-sandbox-and-permissions.md) and [acceptance matrix](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md).

## Purpose and user workflow

A contributor should be able to accept a useful agent change without trusting a chat summary or losing existing work. The application presents the task intent, semantic change, source diff, validation evidence and known limitations together. A novice can review the visible outcome. A scripter can inspect exact source. Both approve the same immutable proposal.

Every project's repository lives in a server-side workspace. Task work uses Git worktrees on the server volume and reviewed changes. A project may be connected to a GitHub repository, which becomes the shared remote for push and pull requests. The application remains useful when no remote is configured. An uploaded mod keeps its server-side Git history until the user publishes it.

A project owner can invite collaborators into the same workspace. Each collaborator works on owned branches or worktrees under the same rules as an agent. Real-time co-editing of one file, automatic merge fleets and a separate collaboration service are deferred.

## Evidence and reusable boundaries

[SRC-R02, Entire checkpoint interfaces](https://github.com/entireio/cli/blob/ee57ff55142e5078b625dd963fc398d49a8ddda1/api/checkpoint/interfaces.go) separate checkpoints, sessions and task records. Its [reference update implementation](https://github.com/entireio/cli/blob/ee57ff55142e5078b625dd963fc398d49a8ddda1/cmd/entire/cli/checkpoint/persistent_ref_update.go) distinguishes stale-reference conflicts from retryable lock contention. These are useful provenance and concurrency patterns without requiring the Entire CLI.

[SRC-B07, Fusion workspace leases](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/task-store/workspace-leases.ts) bind ownership to task, run and incarnation with an increasing fence token. Its [handoff test](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/__tests__/postgres/handoff-to-review-atomicity.pg.test.ts) checks that review state and audit do not split on rollback. The product should adapt these invariants to its own PostgreSQL store. Source inspection does not establish that these tests passed here.

## Workspace baseline and isolation

C-01 records repository identity, selected base reference, worktree location, exact source snapshot and dirty-state inventory. On import, inspect tracked changes, untracked files, nested repositories, submodules, large files, Git LFS pointers and existing worktrees. Existing content remains owned by the user. A GitHub clone records the remote URL, resolved commit and default branch. Submodules pointing at other repositories are recorded and fetched only when the GitHub connection grants them.

A task worktree begins from an identified Git base. If a person has uncommitted edits in their own browser-backed worktree, the host stages a selected overlay into the task worktree and records each included path and hash. It must not silently omit relevant unsaved work, sweep unrelated files into a commit, discard user edits or clean the original tree. Ambiguous scope produces an actionable baseline decision.

Read-only roots may include the server game reference of SPEC-006 and dependency mods. Writable roots contain the task's permitted mod content. Shared Git metadata needs its own controlled operation surface. A path ownership declaration is a scheduling aid, not proof that writing that path is authorized.

The source inventory and selected setup receipt must agree before isolation starts. A failed or unresolved setup transaction remains under C-09 recovery. Git isolation cannot make a partial setup appear complete.

## ChangeProposal and review record

C-02 contains the proposal ID, workspace identity, base revision and per-path hashes, operations, owned paths, semantic intent, source spans, binary hashes, requirements, policy and grant references, evidence and rollback receipt. Its hash identifies exactly what was reviewed.

A review record adds reviewer account and project role, decision, time, proposal hash, evidence set and rationale. Review can accept, request changes or reject. A standing task grant may authorize bounded apply or commit without another modal prompt. The host records which rule authorized the decision. A changed proposal invalidates a proposal-specific approval, while a broader still-valid grant is evaluated against the new final operations.

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
| Open pull request | Pushed owned branch, target branch, title and body return pull request receipt | Unknown outcome is reconciled by listing pull requests for that head before retry |
| Create repository | Account or organization, name, visibility and authorized scope return repository receipt | Unknown outcome is reconciled by looking up the name before retry |
| Download | Accepted snapshot returns a ZIP of native files | Uncommitted or unaccepted work is excluded unless explicitly selected |

Server-side commit, branch integration and remote publication are distinct operations. Approval to edit files does not silently permit push, pull request creation, repository creation or release. Existing user authorization may cover a named publication scope, in which case the product proceeds within it and records the receipt.

## GitHub integration

A user connects GitHub in two layers. Signing in with GitHub establishes identity under REQ-046. Granting repository access is separate and selective. Prefer a GitHub App installed on chosen repositories or organizations, which issues short-lived installation tokens scoped to those repositories. An OAuth app or a personal token is a fallback only where the App cannot express a needed permission, and the reason is recorded. Request the narrowest permissions the enabled features need, starting with contents and pull requests.

Tokens live in the server secret store, are minted for one operation or a short window and never appear in workspaces, Git config, mod history, evidence, logs or the browser. Git operations receive credentials through a controlled helper that the host supplies per operation. Revoking the App installation or disconnecting GitHub stops new remote operations immediately and leaves server workspaces intact.

| GitHub operation | Authority needed | Reconciliation after an uncertain outcome |
|---|---|---|
| List and select repositories | Connection only | Read-only, retry within policy |
| Import repository | Connection plus project creation | A partial clone is discarded and repeated from the recorded ref |
| Fetch and pull | Project write role | Read the remote ref again, integrate only through a reviewed proposal |
| Push owned branch | Publication grant naming remote and branch | Query the remote ref and compare with the intended hash |
| Open pull request | Publication grant naming head and base | List pull requests for that head before creating another |
| Create repository | Publication grant naming owner, name and visibility | Look up the repository by name before creating another |
| Push to a shared default branch | Separate explicit authorization every time unless a standing rule names it | Same as push, plus branch protection results are reported |

Fetching from GitHub never updates an accepted workspace snapshot silently. Remote changes arrive as a new base, and dependent proposals become stale under the normal rules. A pull request opened by the factory carries the proposal hash, evidence summary and limitations in its body. Review comments on that pull request are visible in the project. Their mapping to factory tasks is decided in S06. GitHub branch protection, required checks and repository rules are reported and never bypassed.

GitHub rate limits, outages and permission errors are distinct states. Rate limiting delays remote operations without failing local review. A permission error names the missing grant. Other forges are a backlog item.

## Project members and roles

A project has one owner and optional invited members. Proposed roles are owner, maintainer, contributor and viewer. Owners manage members, GitHub connection, budgets and publication rules. Maintainers can approve and integrate proposals and publish within granted scope. Contributors can create tasks and proposals on owned branches. Viewers can read source, previews and evidence. A role never exceeds the project's grants, and an agent run started by a member carries that member's role as an upper bound.

Removing a member stops their new operations and revokes their agent runs at the next boundary. Their accepted contributions and attribution remain. Every review, integration and publication records the acting account.

## Concurrent edits and integration

Before apply, the host re-reads per-path hashes and target references. Filesystem notifications accelerate invalidation, but are not the authority. Non-overlapping changes may be rebased and revalidated automatically within policy. A rebase creates a new proposal and evidence identity.

Text conflicts can offer a three-way comparison with source-aware diagnostics. The agent may propose a resolution, but it cannot silently choose between competing human intent. Binary conflicts require choosing an artifact or generating a separately reviewed replacement. Unknown script constructs remain preserved, as required by the structured editors.

An integration transaction acquires ownership, verifies current grant and expected ref, persists intent, applies the accepted change, validates final state and records the resulting commit or snapshot. Reference advancement is conditional on the observed old ref. Lock contention may retry within a limit. A changed ref requires a fresh plan.

If a crash occurs after creating a commit but before recording its receipt, recovery inspects the intended tree, operation metadata and target ref before creating another commit. An orphan local object is not proof that integration completed. If a remote accepted a push but the response was lost, query the named remote ref and compare the intended hash. Network retry must not invent a second publication.

## Restore and attribution

Restore is a new reviewed operation against current state. For accepted Git history, use a reversible change preserving provenance. For a task's unaccepted staging area, the host may restore its owned files from the operation journal after confirming no outside edits. A blanket destructive reset is not the recovery mechanism.

A run links task, acting account, model profile, visible user intent, tool receipts, evidence and commits. Attribute each human's edits and review decisions separately from generated changes. Commits record the acting member as author and the factory as committer or co-author, so attribution survives on GitHub. Record uncertainty when imported history lacks attribution. Do not infer who wrote code from a filename or a model claim.

Keep runtime transcripts and secrets outside normal mod history. A portable evidence export includes selected visible messages, tool/version information, hashes and validation coverage. The user can redact sensitive material before sharing. Redaction creates a new export manifest without rewriting the internal fact that an operation occurred.

Repository automation is part of the execution boundary. Imported hooks, filters, external diff tools and credential helpers must not gain execution simply because Git is used. The host needs a controlled Git environment and explicit opt-in for project automation. On a shared server this also prevents one project's repository configuration from affecting another account. Stage S01 should test the chosen Git adapter against the actual supported repository features.

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
| GIT-12 | Import a GitHub repository with history, LFS pointers and a dirty-looking tree (T-GH-01) | Workspace matches the remote ref byte for byte and Setup changes appear only on an owned branch |
| GIT-13 | Lose the response to push, pull request creation and repository creation (T-GH-02) | Each is reconciled against GitHub before any retry, with no duplicate branch, pull request or repository |
| GIT-14 | Disconnect GitHub or revoke the App during a task | New remote operations stop, local work and proposals remain |
| GIT-15 | A token appears in a worktree, Git config, log or evidence export | Fixture scan finds none |
| GIT-16 | A contributor attempts to integrate or publish, and a removed member's run reaches its next boundary | Contributor is refused, removed member's run is revoked |

AC-021 requires isolation demonstrated on a dirty fixture. AC-022 requires external edit and concurrent integration cases. AC-023 requires exact proposal attribution and evidence. AC-024 requires restore without loss of later changes. AC-035 requires a second contributor to reproduce the patch and checks from a handoff, and an invited member to work in the same project without losing either contribution. AC-047 requires the GitHub import, publication and reconciliation cases above. Store fixture repositories, before/after manifests, operation logs and review records. Actual game behavior remains a separate evidence level.

S02 delivers baseline capture, task worktrees, the GitHub connection, import and upload. S04 adds review, accepted source, inverse proposals, push and pull requests for the focus slice. S06 adds dependency-aware contributor handoff, invited members, roles and conflicts. S08 validates the selected Git implementation, path behavior, large binary handling, GitHub rate-limit behavior and the deployed images.

