# S02: Accounts, server workspaces, Mod Setup and GitHub import

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md).

## Objective and non-goals

Implement the reused new/import workflow inside authoritative server workspaces that belong to accounts. Reuse the portable planning core and the desktop scanner and transaction services adapted to server paths, as established by EXP-01. Preserve the established Setup workflow rather than replacing it with a new scaffolder.

Deliver basic accounts with GitHub and email sign-in, per-account and per-project workspace isolation, three ways to bring a mod in (create new, upload a ZIP or folder, import from a connected GitHub repository), and a read-only game reference mount. A workspace can be opened, inspected, prepared, re-run and restored. No autonomous model writes, focus canvas, collaborator invitations or public deployment are required yet.

## Entry conditions

S01 accepted with a confirmed Rust runtime boundary, a supportable Setup pin on server paths, a container packaging path and no unresolved preservation defect on the intended apply path.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S02. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S02-W1 | Onboarding engineer | C-09 adapter on server paths | Bind inspected functions for scan, provider analysis, plan and approved apply against a server workspace directory. Preserve core-owned plan identity and expected root. Expose actual recovery variants. Keep ZIP export distinct from in-place apply. No launcher or local Documents integration. |
| S02-W2 | Workspace engineer | Accounts, C-01 and source snapshots | Implement GitHub and email sign-in, account and project records, and project-scoped storage roots. Register stable workspace IDs, root handles, content order, game reference build and DLC set, and file hashes. Inventory dirty and untracked files. Detect metadata-prefix collisions and unknown schema versions. One account can never resolve another account's workspace handle. |
| S02-W3 | Onboarding engineer | New, upload and GitHub import interface | Reuse identity, scan review, component, workflow, Git, dry-run and readiness controls where relevant. Accept a ZIP or folder upload with size and path limits and server-side Git history. Connect selected GitHub repositories through the GitHub App and clone the chosen branch into the workspace, recording the link as a C-10 RepositoryLink. Separate optional external coding environments from the required native factory runtime. Preserve existing instructions through explicit merge choices. |
| S02-W4 | Integrity engineer | Transaction and recovery bridge | Persist plan, stage and apply receipts in PostgreSQL. Follow rollback-required partial apply behavior. Verify finalization against success lock and rollback records. Avoid competing transaction owners between Setup and host. |
| S02-W5 | Git engineer | Owned workspace baseline | Create and reopen owned branches and worktrees on the server volume, preserve the real index and unrelated edits, record expected HEAD and restore points. Record the linked GitHub remote and its default branch without pushing. Guard path changes and source preconditions. |
| S02-W6 | Reviewer | Compatibility closure, game reference and handoff | Record actual manifest5/component closure, including tools4.0.3 if selected. Treat newer tools4.3.0 as an explicit tested migration. Mount the latest game build with all DLC read-only, record its build and DLC identifiers, and confirm it is absent from images and from every export path. Produce capabilities and unresolved checks without advertising unsupported clients. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S02-W1 | S01 | Inventing a public Setup HTTP service, reading a user's local install |
| S02-W2 | S02-W1 contract | Creating a proprietary replacement for native mod source, collaborator invitations |
| S02-W3 | S02-W1, S02-W2 | Mandatory coding CLI installation for product users, requesting access to unselected repositories |
| S02-W4 | S02-W1, S02-W2 | Resuming a partial native apply contrary to Setup's contract |
| S02-W5 | S02-W2, S02-W4 | Committing unrelated dirty files, pushing to GitHub |
| S02-W6 | S02-W3, S02-W4, S02-W5 | Implicit tool-version upgrades, serving raw game files to users |

## Verification plan

Run new mod, existing clean mod, dirty Git mod, non-Git upload, GitHub import of a disposable repository, partial setup, modified managed instruction, custom asset, missing optional tool, Unicode path and interrupted apply fixtures. A non-Git upload requires explicit consent before initializing Git and must preserve the original upload as received.

Kill at staging, first apply, post-install and finalization boundaries. Test source change after dry-run, path identity replacement, an upload containing traversal or symlink entries, an oversized upload and a revoked GitHub installation during import. Default-deny outside-root use until the reviewed plan authorizes it. Repeat setup twice and compare changed paths.

Create two accounts and confirm that neither can list, open or infer the other's projects, workspaces or uploads through the API. Confirm the game reference mount is read-only from every process that receives it.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. New, upload and GitHub import paths produce a C-01 descriptor from real reused setup logic in a per-account server workspace, with no redundant importer and no cross-account access.
2. Protected native files, custom assets, existing instructions and unrelated Git state remain byte-identical unless an exact reviewed change covers them.
3. Re-running a completed setup produces no unexpected changes. Modified generated files become explicit conflicts or preserved files.
4. Every injected interruption has a classified recoverable state, including rollback-required partial apply. Restoration inventory matches its expected baseline.
5. The exact installed manifest/tool closure, game reference build and DLC set, and missing capabilities are visible. Game files stay read-only and absent from images and exports. Current audited HEAD behavior is not attributed to an older release.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Use the recorded Setup recovery operation and host-owned baseline. If user files changed after apply, preserve them and produce a conflict instead of blindly restoring. Revert owned integration commits without deleting the imported mod or the user's GitHub repository. A failed import leaves the GitHub repository untouched because S02 never pushes.

## Engineering learning checkpoint

Capture discovered setup, upload and preservation edge cases with minimal fixture regressions. Keep project-specific onboarding choices out of global instructions.

Record the result in `evidence/S02/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Verify workspace root configuration, volume mounts, the read-only game reference mount, disabled optional dependencies, GitHub App permissions and generated-file ownership. Any manifest upgrade must update the compatibility receipt and migration report.

Record `evidence/S02/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

Provide the exact Setup pin, adapted service boundaries, C-01 fixture, failed and recovered operation evidence, account isolation results, GitHub App permission set and game reference identifiers. S03 may assume validated workspace registration per account, not universal game compatibility.

Produce `evidence/S02/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S02. **Do not start the next stage automatically.**
